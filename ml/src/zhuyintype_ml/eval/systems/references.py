"""The other input methods' public repositories, pinned, and how this machine gets them: clone, fetch their released
assets, build.  Nothing of theirs enters this repository; everything goes under the systems directory.

    zt systems setup                    # both, on the GPU host: zt --remote systems setup
    zt systems setup 0c --jobs 8
    zt systems status

The systems directory is $ZT_SYSTEMS_DIR, or data/build/systems (never tracked).  Under it, per system:

    <name>/src/        the repository at the pinned commit (git checkout, never edited)
    <name>/setup.json  what setup did: commit, toolchains, the SHA-256 of every built binary and model

0c (靈犀注音, AGPL-3.0): its own fetch script installs the dictionary and models of the release that matches the
commit; `go build` builds 0c-server; a separate virtual environment holds the Python packages of its model worker
(platforms/neural/requirements.txt).  Go comes from PATH, $ZT_GO, or the official release, checked by SHA-256.

善解 (shanjie, Apache-2.0): the model files are downloaded from its GitHub releases and checked against the SHA-256
files in its repository; `cargo build` builds shanjie-eval, and `cargo rustc --crate-type cdylib` builds its unmodified
core library with the documented C interface (core/include/shanjie.h) as a shared library, for typing key by key.  Its
evaluation sets are read from the checkout; the sets that have no readings get them from its own reading tool
(reference/proto/ime.py, run in its own process), so its sets carry its readings as before.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths


def systems_dir() -> Path:
    return Path(os.environ.get("ZT_SYSTEMS_DIR", paths.data("systems"))).resolve()


@dataclass(frozen=True)
class Reference:
    name: str
    title: str
    url: str
    commit: str
    license: str
    release: str = ""

    @property
    def root(self) -> Path:
        return systems_dir() / self.name

    @property
    def checkout(self) -> Path:
        return self.root / "src"

    @property
    def setup_file(self) -> Path:
        return self.root / "setup.json"

    def setup_record(self) -> dict[str, Any]:
        if not self.setup_file.exists():
            raise SystemExit(f"zt: {self.title} is not set up here; run `zt systems setup {self.name}`")
        record: dict[str, Any] = json.loads(self.setup_file.read_text(encoding="utf-8"))
        return record


ZEROC = Reference(
    "0c",
    "靈犀注音（0c）",
    "https://github.com/0c-biggo/0c-ime",
    "3fb091797d14f71e49a18ed3d58122bcb61f6ec5",
    "程式 AGPL-3.0，模型 MIT，詞庫 CC BY-SA 4.0",
    release="v0.2.6",
)
SHANJIE = Reference(
    "shanjie",
    "善解（shanjie）",
    "https://github.com/Nanako0129/shanjie",
    "bd3b6095e170012e5675b15b9443786a04103790",
    "程式 Apache-2.0，語言模型 CC BY-SA 4.0，評測資料多為 CC0／CC BY-SA（另有 CC BY-ND 檔案）",
)
REFERENCES = {r.name: r for r in (ZEROC, SHANJIE)}

# The official Go release that builds 0c (its go.mod asks for Go 1.26).
GO_VERSION = "go1.26.10"
GO_LISTING = "https://go.dev/dl/?mode=json&include=all"
# 善解's model files: release tag and file, as its repository lists them (data/*.sha256 hold their digests).
SHANJIE_MODELS = (
    ("model-v6", "bigram.sjlm"),
    ("classes-v5", "classes.sjc"),
    ("kn-v1", "kn.sjkn"),
    ("sj-sr-6-14m", "tinylm.sjtl"),
    ("sj-cx-1-7.7m", "trigram.sjt3"),
    ("sj-cl-1-68k", "colloquial.sjlm"),
    ("sj-cl-1-68k", "colloquial-attribution.tsv"),
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def run(command: list[str], cwd: Path | None = None, env: dict[str, str] | None = None) -> str:
    print("$ " + " ".join(command), flush=True)
    done = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"zt systems: command failed ({done.returncode}): {' '.join(command)}\n{done.stderr[-2000:]}")
    return done.stdout


def download(url: str, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    print(f"downloading {url}", flush=True)
    request = urllib.request.Request(url, headers={"User-Agent": "zhuyintype-zt"})
    with urllib.request.urlopen(request, timeout=120) as response, open(target.with_suffix(".part"), "wb") as out:
        shutil.copyfileobj(response, out)
    target.with_suffix(".part").replace(target)


def clone(reference: Reference) -> None:
    """The repository at the pinned commit, as a detached checkout; an existing checkout must be that commit, clean."""
    src = reference.checkout
    if not (src / ".git").exists():
        src.mkdir(parents=True, exist_ok=True)
        run(["git", "init", "--quiet"], src)
        run(["git", "remote", "add", "origin", reference.url], src)
        run(["git", "fetch", "--quiet", "--depth", "1", "origin", reference.commit], src)
        run(["git", "checkout", "--quiet", "--detach", "FETCH_HEAD"], src)
    head = run(["git", "rev-parse", "HEAD"], src).strip()
    if head != reference.commit:
        raise SystemExit(f"zt systems: {src} is at {head}, not the pinned {reference.commit}")
    changed = run(["git", "status", "--porcelain", "--untracked-files=no"], src).strip()
    if changed:
        raise SystemExit(f"zt systems: {src} has changes to tracked files; remove it and set up again")


def jobs(requested: int | None) -> int:
    return max(1, requested or int(os.environ.get("ZT_BUILD_JOBS", "0")) or min(8, os.cpu_count() or 1))


# MARK: - 0c


def go_toolchain() -> tuple[str, dict[str, Any]]:
    """A `go` executable: $ZT_GO, PATH, or the pinned official release (downloaded once, checked by SHA-256)."""
    found = os.environ.get("ZT_GO") or shutil.which("go")
    if found:
        return found, {"go": run([found, "version"]).strip(), "source": "PATH or ZT_GO"}
    machine = {"x86_64": "amd64", "aarch64": "arm64", "arm64": "arm64"}[platform.machine()]
    system = platform.system().lower()
    root = systems_dir() / "toolchains" / f"{GO_VERSION}.{system}-{machine}"
    binary = root / "go" / "bin" / "go"
    if not binary.exists():
        with urllib.request.urlopen(GO_LISTING, timeout=60) as response:
            releases = json.load(response)
        release = next(r for r in releases if r["version"] == GO_VERSION)
        asset = next(
            f for f in release["files"] if f["os"] == system and f["arch"] == machine and f["kind"] == "archive"
        )
        archive = systems_dir() / "toolchains" / asset["filename"]
        if not archive.exists() or sha256(archive) != asset["sha256"]:
            download("https://go.dev/dl/" + asset["filename"], archive)
        if sha256(archive) != asset["sha256"]:
            raise SystemExit(f"zt systems: {archive.name} does not match the SHA-256 go.dev lists")
        root.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive) as tar:
            tar.extractall(root, filter="data")
    return str(binary), {"go": run([str(binary), "version"]).strip(), "source": "go.dev " + GO_VERSION}


def setup_0c(build_jobs: int) -> dict[str, Any]:
    reference = ZEROC
    clone(reference)
    src = reference.checkout
    record: dict[str, Any] = {"url": reference.url, "commit": reference.commit, "release": reference.release}
    # Its own script: downloads the release's source and model tarballs, verifies them, installs assets/data and
    # models/{select,punct} in the checkout (ignored by its .gitignore).
    if (
        not (src / "assets" / "data" / "lexicon.bin").exists()
        or not (src / "models" / "select" / "model.onnx").exists()
    ):
        run(
            [
                sys.executable,
                "-E",
                "-s",
                "scripts/fetch_assets.py",
                "--release",
                reference.release,
                "--cache",
                str(reference.root / "cache"),
            ],
            src,
        )
    go, toolchain = go_toolchain()
    record["toolchain"] = toolchain
    cache = systems_dir() / "toolchains" / "go-cache"
    env = dict(
        os.environ,
        GOCACHE=str(cache / "build"),
        GOPATH=str(cache / "path"),
        GOMODCACHE=str(cache / "mod"),
        GOTOOLCHAIN="local",
        GOFLAGS="-mod=readonly",
        CGO_ENABLED="0",
    )
    binary = reference.root / "bin" / "0c-server"
    binary.parent.mkdir(parents=True, exist_ok=True)
    run([go, "build", "-p", str(build_jobs), "-trimpath", "-o", str(binary), "./cmd/0c-server"], src, env)
    run([str(binary), "-data", str(src / "assets" / "data"), "-check-data"], src)
    # The model worker's Python packages, pinned by 0c, in an environment of their own.
    venv = reference.root / "venv"
    uv = os.environ.get("ZT_UV") or shutil.which("uv") or "uv"
    if not (venv / "bin" / "python").exists():
        run([uv, "venv", "--quiet", "--python", "3.12", str(venv)])
    requirements = src / "platforms" / "neural" / "requirements.txt"
    run([uv, "pip", "install", "--quiet", "--python", str(venv / "bin" / "python"), "-r", str(requirements)])
    record["python_packages"] = requirements.read_text(encoding="utf-8").split()
    record["files"] = {
        "0c-server": sha256(binary),
        **{
            str(p.relative_to(src)): sha256(p)
            for p in sorted([*(src / "assets" / "data").glob("*.bin"), *(src / "models").glob("*/model.onnx")])
        },
    }
    return record


# MARK: - 善解


def shanjie_models() -> dict[str, str]:
    """Downloads 善解's model files into its data/lm and checks each against its data/<file>.sha256."""
    reference = SHANJIE
    src = reference.checkout
    found: dict[str, str] = {}
    for tag, name in SHANJIE_MODELS:
        listed = (src / "data" / f"{name}.sha256").read_text(encoding="utf-8").split()
        want, target = listed[0], src / listed[1]
        if not target.exists() or sha256(target) != want:
            download(f"{reference.url}/releases/download/{tag}/{name}", target)
        if sha256(target) != want:
            raise SystemExit(f"zt systems: 善解 {name} does not match data/{name}.sha256")
        found[f"data/lm/{name}"] = want
    return found


# Its reading tool, in a process of its own: the most likely reading of each word in its lexicon with its overlays.
READINGS = """
import json, sys
sys.path.insert(0, sys.argv[1])
import ime
lexicon = ime.Lexicon(sys.argv[2], overlay=ime.overlays())
for line in sys.stdin:
    syllables = lexicon.to_syllables(line.rstrip("\\n"))
    print(json.dumps(syllables, ensure_ascii=False))
"""


def shanjie_readings(texts: list[str]) -> list[list[str] | None]:
    src = SHANJIE.checkout
    command = [
        sys.executable,
        "-I",
        "-c",
        READINGS,
        str(src / "reference" / "proto"),
        str(src / "data" / "lexicon" / "mcbpmf-data.txt"),
    ]
    done = subprocess.run(command, input="".join(t + "\n" for t in texts), capture_output=True, text=True, cwd=src)
    if done.returncode:
        raise SystemExit(f"zt systems: 善解's reading tool failed: {done.stderr[-1000:]}")
    lines = done.stdout.splitlines()
    if len(lines) != len(texts):
        raise SystemExit("zt systems: 善解's reading tool returned the wrong number of lines")
    return [json.loads(line) for line in lines]


def shanjie_row_sets() -> dict[str, Any]:
    """Every row set of 善解's eval/ (the goldens excepted), as context|truth|syllables files under
    <systems>/shanjie/rows/, byte-identical sets reported once (aliases), plus its learning cases typed without
    learning.  Rows without readings get them from its reading tool; rows it cannot read are left out (counted)."""
    src = SHANJIE.checkout
    out = SHANJIE.root / "rows"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    sets: list[dict[str, Any]] = []
    aliases: dict[str, str] = {}
    by_digest: dict[str, str] = {}

    def add(name: str, source: str, rows: list[str], unread: list[int]) -> None:
        text = "\n".join(rows) + "\n"
        digest = hashlib.sha256(text.encode()).hexdigest()
        if digest in by_digest:
            aliases[source] = by_digest[digest]
            return
        by_digest[digest] = name
        (out / f"{name.replace('/', '--')}.txt").write_text(text, encoding="utf-8")
        sets.append({"name": name, "source": source, "rows": len(rows), "sha256": digest, "unread_lines": unread})

    for path in sorted((src / "eval").rglob("*.txt")):
        relative = path.relative_to(src)
        if "golden" in relative.parts:
            continue
        entries: list[tuple[int, list[str]]] = []
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not line.strip() or line.startswith("#"):
                continue
            fields = ["", line] if relative.as_posix() == "eval/sets/moedict.txt" else line.split("|")
            if len(fields) in (2, 3):
                entries.append((number, fields))
        if not entries:
            continue
        missing = [fields[1] for _, fields in entries if len(fields) == 2]
        readings = iter(shanjie_readings(missing)) if missing else iter([])
        rows, unread = [], []
        for number, fields in entries:
            if len(fields) == 2:
                syllables = next(readings)
                if not syllables:
                    unread.append(number)
                    continue
                fields = [*fields, " ".join(syllables)]
            rows.append("|".join(fields))
        name = "sj/" + relative.with_suffix("").as_posix().removeprefix("eval/")
        add(name, relative.as_posix(), rows, unread)
    cases = src / "eval" / "learn" / "cases.tsv"
    header, *lines = cases.read_text(encoding="utf-8").splitlines()
    columns = header.split("\t")
    learn = []
    for line in lines:
        row = dict(zip(columns, line.split("\t")))
        learn.append(f"|{row['sentence']}|{row['reading']}")
    add("sj/learn/cases", "eval/learn/cases.tsv", learn, [])
    manifest = {"commit": SHANJIE.commit, "sets": sets, "aliases": aliases}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return manifest


def setup_shanjie(build_jobs: int) -> dict[str, Any]:
    reference = SHANJIE
    clone(reference)
    src = reference.checkout
    record: dict[str, Any] = {"url": reference.url, "commit": reference.commit}
    models = shanjie_models()
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo" / "bin" / "cargo")
    record["toolchain"] = {"cargo": run([cargo, "--version"]).strip()}
    base = [cargo, "--quiet"]
    run([*base, "build", "--release", "--locked", "-j", str(build_jobs), "-p", "cli", "--bin", "shanjie-eval"], src)
    # The same core, unmodified, as a shared library: its C interface for typing key by key.
    run(
        [
            *base,
            "rustc",
            "--release",
            "--locked",
            "-j",
            str(build_jobs),
            "-p",
            "core",
            "--lib",
            "--crate-type",
            "cdylib",
        ],
        src,
    )
    record["files"] = {
        "shanjie-eval": sha256(src / "target" / "release" / "shanjie-eval"),
        "libcore.so": sha256(src / "target" / "release" / "libcore.so"),
        **models,
        "eval/variants.tsv": sha256(src / "eval" / "variants.tsv"),
    }
    manifest = shanjie_row_sets()
    record["row_sets"] = len(manifest["sets"])
    record["aliases"] = manifest["aliases"]
    return record


SETUPS = {"0c": setup_0c, "shanjie": setup_shanjie}


def setup(names: list[str], build_jobs: int | None = None) -> int:
    if not sys.platform.startswith("linux"):
        raise SystemExit("zt systems setup: run it on the Linux GPU host (zt --remote systems setup)")
    for name in names:
        reference = REFERENCES[name]
        started = time.time()
        record = SETUPS[name](jobs(build_jobs))
        record.update(
            {
                "title": reference.title,
                "license": reference.license,
                "date": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "seconds": round(time.time() - started, 1),
                "host": f"{platform.system()} {platform.machine()}",
            }
        )
        reference.root.mkdir(parents=True, exist_ok=True)
        reference.setup_file.write_text(json.dumps(record, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"{reference.title}: ready in {paths.relative(reference.root)}", flush=True)
    return 0


def status() -> int:
    print(f"systems directory: {paths.relative(systems_dir())}")
    for reference in REFERENCES.values():
        if reference.setup_file.exists():
            record = reference.setup_record()
            print(f"✓ {reference.name:8} {reference.commit[:12]}  set up {record['date']}")
        else:
            print(f"· {reference.name:8} {reference.commit[:12]}  not set up (zt systems setup {reference.name})")
    return 0


def temporary_dir(prefix: str) -> Path:
    """A short private directory for Unix sockets (paths over ~100 bytes do not work)."""
    return Path(tempfile.mkdtemp(prefix=prefix, dir="/tmp" if Path("/tmp").is_dir() else None))
