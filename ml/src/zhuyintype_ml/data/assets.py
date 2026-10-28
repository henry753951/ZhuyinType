"""Data and model assets that are not in git: packed into GitHub Release archives, pinned by SHA-256 in
resources/assets.json, and fetched into the data directory by builds, CI and the GPU host.

    zt data fetch                       # the app's components
    zt data fetch eval                  # what evaluations need (app + eval-sets + checkpoints)
    zt data status                      # which components are present and current
    zt data pack data-4 v3              # archives into dist/assets/, updates assets.json
    zt data publish data-4              # gh release upload of dist/assets/ (after pack)

Archives are deterministic (sorted, zero mtimes), so an unchanged component keeps its hash.  Downloads are cached in
$ZHUYIN_ASSET_CACHE (default .cache/assets) and checked against the pinned hash before they are unpacked.
"""

from __future__ import annotations

import gzip
import hashlib
import io
import json
import os
import shutil
import subprocess
import tarfile
import urllib.request
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

MANIFEST = paths.RESOURCES / "assets.json"
REPOSITORY = "henry753951/ZhuyinType"
DATA_NOTES = (
    "建置 ZhuyinType 所需的詞庫、語言模型、神經模型與評測集。使用的版本記錄在 resources/assets.json。"
    "授權見 resources/SOURCES.md 與各壓縮檔內的 NOTICE 檔。"
)

# Paths relative to the data directory.  "app" components are what the macOS app bundles; the rest serve evaluation.
COMPONENTS: dict[str, list[str]] = {
    "lexicon": ["lexicon.tsv"],
    "lm": ["lm/unigram.tsv", "lm/bigram.tsv", "lm/backoff.tsv", "lm/NOTICE.md"],
    "lm_chat": [
        "lm_chat/unigram.tsv",
        "lm_chat/bigram.tsv",
        "lm_chat/backoff.tsv",
        "lm_chat/NOTICE.md",
        "lm_chat/attribution.jsonl",
        "lm_chat/source-manifest.json",
    ],
    # The prefix-tree model only: the batched Reranker.mlmodelc holds the same weights and the app no longer loads
    # it when the tree is there (docs/engine/ARCHITECTURE.md §10).
    "reranker": [
        "reranker/RerankerTree.mlmodelc",
        "reranker/vocab.json",
        "reranker/config.json",
    ],
    "v3": [
        "v3/ContextEncoder.mlmodelc",
        "v3/SyllableEncoder.mlmodelc",
        "v3/chars.json",
        "v3/syllables.json",
        "v3/config.json",
        "v3/NOTICE.md",
        "v3/attribution.jsonl",
        "v3/source-manifest.json",
        "v3/Licenses",
    ],
    "eval-sets": [
        "corpus/dev_colloquial.txt",
        "corpus/dev_wikipedia.txt",
        "corpus/test_colloquial.txt",
        "lm/test_sentences.txt",
        "lm_chat/dev_chat_tt.txt",
        "lm_chat/test_chat_tt.txt",
        # The Taiwan Tongues sets are derived data: their terms travel with them.
        "lm_chat/Taiwan-Tongues-LICENSE",
        "lm_chat/Taiwan-Tongues-README.md",
    ],
    "checkpoints": [
        "reranker/charlm.pt",
        "v3/context.pt",
        "v3/syllable.pt",
        "v3/test_vectors.json",
        # The Wikipedia article list the v3 training text came from (its attribution).
        "v3/articles.tsv.gz",
    ],
}
# Named groups of components.
GROUPS: dict[str, list[str]] = {
    "app": ["lexicon", "lm", "lm_chat", "reranker", "v3"],
    "engine": ["lexicon", "lm"],
    "eval": ["lexicon", "lm", "lm_chat", "reranker", "v3", "eval-sets", "checkpoints"],
    "all": list(COMPONENTS),
}


@dataclass(frozen=True)
class Entry:
    """One component's pinned archive."""

    name: str
    release: str
    file: str
    sha256: str
    size: int

    def url(self, repository: str) -> str:
        return f"https://github.com/{repository}/releases/download/{self.release}/{self.file}"


def load_manifest() -> dict[str, Any]:
    if not MANIFEST.exists():
        return {"components": {}, "repository": REPOSITORY}
    data: dict[str, Any] = json.loads(MANIFEST.read_text(encoding="utf-8"))
    return data


def entries() -> dict[str, Entry]:
    return {name: Entry(name, **value) for name, value in load_manifest()["components"].items()}


def expand(names: list[str]) -> list[str]:
    """Component names for component and group names, in order, without repeats."""
    result: list[str] = []
    for name in names:
        if name in GROUPS:
            result += GROUPS[name]
        elif name in COMPONENTS:
            result.append(name)
        else:
            known = ", ".join(list(GROUPS) + list(COMPONENTS))
            raise SystemExit(f"zt: unknown component {name!r}; known: {known}")
    return list(dict.fromkeys(result))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def cache_dir() -> Path:
    return Path(os.environ.get("ZHUYIN_ASSET_CACHE", paths.ROOT / ".cache" / "assets"))


def marker(name: str) -> Path:
    """Records which archive a component was unpacked from."""
    return paths.data(".assets", name)


def files_of(name: str) -> Iterator[Path]:
    build = paths.data_dir()
    for relative in COMPONENTS[name]:
        path = build / relative
        if not path.exists():
            raise SystemExit(f"zt: missing {paths.relative(path)} for {name}")
        yield from sorted([path] if path.is_file() else (p for p in path.rglob("*") if p.is_file()))


# MARK: - Status


@dataclass(frozen=True)
class Status:
    name: str
    pinned: str | None  # release of the pinned archive
    present: bool  # every file of the component exists
    current: bool  # unpacked from the pinned archive


def status(names: list[str]) -> list[Status]:
    pinned = entries()
    result = []
    for name in names:
        entry = pinned.get(name)
        present = all(paths.data(relative).exists() for relative in COMPONENTS[name])
        recorded = marker(name).read_text().strip() if marker(name).exists() else ""
        current = entry is not None and present and recorded == entry.sha256
        result.append(Status(name, entry.release if entry else None, present, current))
    return result


# MARK: - Fetch


def download(url: str, target: Path) -> None:
    partial = target.with_name(target.name + ".part")
    with urllib.request.urlopen(url) as response, open(partial, "wb") as out:
        shutil.copyfileobj(response, out, 1 << 20)
    partial.replace(target)


def fetch(names: list[str], quiet: bool = False) -> None:
    """Downloads (or reuses from the cache) and unpacks each component unless it is already current."""
    manifest = load_manifest()
    pinned = entries()
    build = paths.data_dir()
    for name in names:
        entry = pinned.get(name)
        if entry is None:
            raise SystemExit(f"zt: {name} has no pinned archive in {paths.relative(MANIFEST)}")
        if marker(name).exists() and marker(name).read_text().strip() == entry.sha256:
            continue
        cache = cache_dir()
        cache.mkdir(parents=True, exist_ok=True)
        archive = cache / entry.file
        if not archive.exists() or sha256(archive) != entry.sha256:
            print(f"downloading {name} ({entry.size / 1e6:.0f} MB)", flush=True)
            download(entry.url(manifest.get("repository", REPOSITORY)), archive)
        if sha256(archive) != entry.sha256:
            raise SystemExit(f"zt: {name}: checksum mismatch for {archive}")
        build.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive, "r:gz") as tar:
            tar.extractall(build, filter="data")
        marker(name).parent.mkdir(parents=True, exist_ok=True)
        marker(name).write_text(entry.sha256 + "\n")
        if not quiet:
            print(f"{name}: ready ({entry.release})")


# MARK: - Pack and publish


def archive_bytes(name: str) -> bytes:
    build = paths.data_dir()
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", mtime=0, compresslevel=6) as zipped:
        with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as tar:
            for path in files_of(name):
                info = tar.gettarinfo(str(path), arcname=str(path.relative_to(build)))
                info.mtime, info.uid, info.gid, info.uname, info.gname, info.mode = 0, 0, 0, "", "", 0o644
                with open(path, "rb") as f:
                    tar.addfile(info, f)
    return buffer.getvalue()


def pack(release: str, names: list[str]) -> None:
    """Writes a deterministic archive per changed component to dist/assets/ and pins it in assets.json."""
    out = paths.DIST / "assets"
    out.mkdir(parents=True, exist_ok=True)
    manifest = load_manifest()
    for name in names:
        data = archive_bytes(name)
        digest = hashlib.sha256(data).hexdigest()
        previous = manifest["components"].get(name)
        if previous and previous["sha256"] == digest:
            print(f"{name}: unchanged ({previous['release']})")
            continue
        filename = f"zhuyintype-{name}-{digest[:12]}.tar.gz"
        (out / filename).write_bytes(data)
        manifest["components"][name] = {"release": release, "file": filename, "sha256": digest, "size": len(data)}
        print(f"{name}: {filename} {len(data) / 1e6:.1f} MB")
    manifest["repository"] = REPOSITORY
    MANIFEST.write_text(json.dumps(manifest, indent=2, ensure_ascii=False, sort_keys=True) + "\n")


def publish(release: str) -> None:
    """Uploads the archives pinned to `release` (creating the release when it does not exist)."""
    files = [
        paths.DIST / "assets" / e.file
        for e in entries().values()
        if e.release == release and (paths.DIST / "assets" / e.file).exists()
    ]
    if not files:
        raise SystemExit(f"zt: nothing in dist/assets/ for {release}; run `zt data pack {release}` first")
    if subprocess.run(["gh", "release", "view", release, "-R", REPOSITORY], capture_output=True).returncode:
        subprocess.run(
            [
                "gh",
                "release",
                "create",
                release,
                "-R",
                REPOSITORY,
                "--latest=false",
                "--title",
                f"資料與模型 {release}",
                "--notes",
                DATA_NOTES,
            ],
            check=True,
        )
    subprocess.run(["gh", "release", "upload", release, "-R", REPOSITORY, "--clobber", *map(str, files)], check=True)
