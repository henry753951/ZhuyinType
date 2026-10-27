"""The lexicon and language models as mmap binaries (ZTLX/1 lexicon.bin, ZTLM/1 lm/lm.bin), which the engine maps
instead of parsing TSV: it starts in about 0.1 s and many parallel CLI processes share the pages.  The reading
channel's tables (`zt train lexicon channel`: `channel/` or `lexicon_coverage/channel/` in the data directory) become
channel.bin next to the lexicon (ZTCB/1), which `zhuyin-cli --channel on` loads.

    zt data compile                   # next to the TSV files in the data directory (what evaluations load)
    zt data compile OUT               # into OUT (the app bundle's Resources, Makefile's data/build/compiled)
    zt data compile --quiet           # what `zt --remote` runs before evaluations: only what is out of date

A binary is out of date when its source TSV files no longer hash to the values in its .json, or the compiler changed
after it was written.  The app loads these binaries; an engine process given only the TSV files parses them (about 4 s
per process with the chat model), with the same results.

The compiler is ml/src/zhuyintype_ml/training/lm/compile_data.py (standard library only), next to the format it writes.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from zhuyintype_ml.core import paths

MODELS = ("lm", "lm_chat")
COMPILER = paths.ML / "src" / "zhuyintype_ml" / "training" / "lm" / "compile_data.py"


def current(binary: Path, sources: dict[str, Path]) -> bool:
    """Whether `binary` was compiled from exactly these sources (by name) with the present compiler."""
    metadata = binary.with_suffix(".json")
    if not binary.exists() or not metadata.exists() or binary.stat().st_mtime < COMPILER.stat().st_mtime:
        return False
    try:
        recorded = json.loads(metadata.read_text(encoding="utf-8")).get("source_hashes")
    except ValueError:
        return False
    return recorded == {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in sources.items()}


def channel_source(source: Path) -> Path | None:
    """The reading channel's tables in a data directory: `channel/`, else `lexicon_coverage/channel/`."""
    for directory in (source / "channel", source / "lexicon_coverage" / "channel"):
        if (directory / "channel.tsv").exists():
            return directory
    return None


def compile_all(source: Path | None = None, output: Path | None = None, quiet: bool = False) -> Path:
    """Compiles lexicon.tsv and each language model that has a bigram.tsv, unless already current; returns the output
    directory."""
    from zhuyintype_ml.training.lm.compile_data import CHANNEL_SOURCES, compile_channel, compile_lexicon, compile_lm

    source = source or paths.data_dir()
    output = output or source
    if not (source / "lexicon.tsv").exists():
        raise SystemExit(f"zt: no lexicon.tsv in {paths.relative(source)}; run `zt data fetch` or `zt data lexicon`")
    for model in MODELS:
        if (source / model / "bigram.tsv").exists():
            tsv = {name: source / model / name for name in ("unigram.tsv", "bigram.tsv", "backoff.tsv")}
            if not current(output / model / "lm.bin", tsv):
                compile_lm(source / model, output / model)
        elif not quiet:
            print(f"no {model}/bigram.tsv; {model} skipped")
    if not current(output / "lexicon.bin", {"lexicon.tsv": source / "lexicon.tsv"}):
        compile_lexicon(source / "lexicon.tsv", output)
    channel = channel_source(source)
    if channel is not None:
        tables = {name: channel / name for name in CHANNEL_SOURCES}
        if not current(output / "channel.bin", tables):
            compile_channel(channel, output)
    elif not quiet:
        print("no channel/channel.tsv; channel.bin skipped")
    if not quiet:
        print(f"compiled into {paths.relative(output)}")
    return output
