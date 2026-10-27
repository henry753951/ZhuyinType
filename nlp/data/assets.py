#!/usr/bin/env python3
"""Data and model assets that are not in git: packed into GitHub Release archives, pinned by SHA-256 in
nlp/data/assets.json, and fetched into data/build/ by builds, CI and evaluation hosts.

    python3 nlp/data/assets.py fetch [COMPONENT ...]     # default: the app's components
    python3 nlp/data/assets.py pack RELEASE [COMPONENT ...]  # archives into dist/assets/, updates assets.json
    python3 nlp/data/assets.py publish RELEASE            # gh release upload of dist/assets/ (after pack)

Standard library only.  Archives are deterministic (sorted, zero mtimes), so an unchanged component keeps its hash.
"""
import gzip
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "data" / "build"
MANIFEST = Path(__file__).with_name("assets.json")
REPOSITORY = "henry753951/ZhuyinType"

# Paths relative to data/build.  "app" components are what the macOS app bundles; the rest serve evaluation.
COMPONENTS = {
    "lexicon": ["lexicon.tsv"],
    "lm": ["lm/unigram.tsv", "lm/bigram.tsv", "lm/backoff.tsv", "lm/NOTICE.md"],
    "lm_chat": ["lm_chat/unigram.tsv", "lm_chat/bigram.tsv", "lm_chat/backoff.tsv", "lm_chat/NOTICE.md",
                "lm_chat/attribution.jsonl", "lm_chat/source-manifest.json"],
    "reranker": ["reranker/Reranker.mlmodelc", "reranker/RerankerTree.mlmodelc", "reranker/vocab.json",
                 "reranker/config.json"],
    "v3": ["v3/ContextEncoder.mlmodelc", "v3/SyllableEncoder.mlmodelc", "v3/chars.json", "v3/syllables.json",
           "v3/config.json", "v3/NOTICE.md", "v3/attribution.jsonl", "v3/source-manifest.json", "v3/Licenses"],
    "eval-sets": ["corpus/dev_colloquial.txt", "corpus/dev_wikipedia.txt", "corpus/test_colloquial.txt",
                  "lm/test_sentences.txt", "lm_chat/dev_chat_tt.txt", "lm_chat/test_chat_tt.txt",
                  # The Taiwan Tongues sets are derived data: their terms travel with them.
                  "lm_chat/Taiwan-Tongues-LICENSE", "lm_chat/Taiwan-Tongues-README.md"],
    "checkpoints": ["reranker/charlm.pt", "v3/context.pt", "v3/syllable.pt", "v3/test_vectors.json",
                    # The Wikipedia article list the v3 training text came from (its attribution).
                    "v3/articles.tsv.gz"],
}
APP = ["lexicon", "lm", "lm_chat", "reranker", "v3"]


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def files_of(name):
    for relative in COMPONENTS[name]:
        path = BUILD / relative
        if not path.exists():
            sys.exit(f"missing data/build/{relative} for {name}")
        yield from sorted([path] if path.is_file() else (p for p in path.rglob("*") if p.is_file()))


def pack(release, names):
    out = ROOT / "dist" / "assets"
    out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {"components": {}}
    for name in names:
        buffer = io.BytesIO()
        with gzip.GzipFile(fileobj=buffer, mode="wb", mtime=0, compresslevel=6) as zipped:
            with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as tar:
                for path in files_of(name):
                    info = tar.gettarinfo(str(path), arcname=str(path.relative_to(BUILD)))
                    info.mtime, info.uid, info.gid, info.uname, info.gname, info.mode = 0, 0, 0, "", "", 0o644
                    with open(path, "rb") as f:
                        tar.addfile(info, f)
        data = buffer.getvalue()
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


def publish(release):
    manifest = json.loads(MANIFEST.read_text())
    files = [ROOT / "dist" / "assets" / c["file"] for c in manifest["components"].values()
             if c["release"] == release and (ROOT / "dist" / "assets" / c["file"]).exists()]
    if subprocess.run(["gh", "release", "view", release, "-R", REPOSITORY], capture_output=True).returncode:
        subprocess.run(["gh", "release", "create", release, "-R", REPOSITORY, "--latest=false", "--title",
                        f"Data and models {release}", "--notes",
                        "Lexicon, language models, neural models and evaluation sets for ZhuyinType builds. "
                        "Pinned by nlp/data/assets.json; licenses: see nlp/data/SOURCES.md and the NOTICE files "
                        "inside each archive."], check=True)
    subprocess.run(["gh", "release", "upload", release, "-R", REPOSITORY, "--clobber", *map(str, files)], check=True)


def fetch(names):
    manifest = json.loads(MANIFEST.read_text())
    marker_dir = BUILD / ".assets"
    marker_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        entry = manifest["components"][name]
        marker = marker_dir / name
        if marker.exists() and marker.read_text().strip() == entry["sha256"]:
            continue
        url = f"https://github.com/{manifest['repository']}/releases/download/{entry['release']}/{entry['file']}"
        cache = Path(os.environ.get("ZHUYIN_ASSET_CACHE", ROOT / ".cache" / "assets"))
        cache.mkdir(parents=True, exist_ok=True)
        archive = cache / entry["file"]
        if not archive.exists() or sha256(archive) != entry["sha256"]:
            print(f"downloading {name} ({entry['size'] / 1e6:.0f} MB)", flush=True)
            urllib.request.urlretrieve(url, archive)
        if sha256(archive) != entry["sha256"]:
            sys.exit(f"{name}: checksum mismatch for {archive}")
        with tarfile.open(archive, "r:gz") as tar:
            tar.extractall(BUILD, filter="data")
        marker.write_text(entry["sha256"] + "\n")
        print(f"{name}: ready")


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("fetch", "pack", "publish"):
        sys.exit(__doc__)
    command, rest = sys.argv[1], sys.argv[2:]
    if command == "fetch":
        fetch(rest or APP)
    elif command == "pack":
        if not rest:
            sys.exit("pack needs a release name, e.g. data-1")
        pack(rest[0], rest[1:] or list(COMPONENTS))
    else:
        publish(rest[0])


if __name__ == "__main__":
    main()
