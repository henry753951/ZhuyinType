"""The lexicon and language models as mmap binaries (ZTLX/1 lexicon.bin, ZTLM/1 lm/lm.bin), which the engine maps
instead of parsing TSV: it starts in about 0.1 s and many parallel CLI processes share the pages.

    zt data compile                   # next to the TSV files in the data directory (what evaluations load)
    zt data compile OUT               # into OUT (the app bundle's Resources, Makefile's data/build/compiled)

The compiler is nlp/training/lm/compile_data.py (standard library only), next to the format it writes.
"""

from __future__ import annotations

from pathlib import Path

from nlp.common import paths

MODELS = ("lm", "lm_chat")


def compile_all(source: Path | None = None, output: Path | None = None, quiet: bool = False) -> Path:
    """Compiles lexicon.tsv and each language model that has a bigram.tsv; returns the output directory."""
    from nlp.training.lm.compile_data import compile_lexicon, compile_lm

    source = source or paths.data_dir()
    output = output or source
    if not (source / "lexicon.tsv").exists():
        raise SystemExit(f"zt: no lexicon.tsv in {paths.relative(source)}; run `zt data fetch` or `zt data lexicon`")
    for model in MODELS:
        if (source / model / "bigram.tsv").exists():
            compile_lm(source / model, output / model)
        elif not quiet:
            print(f"no {model}/bigram.tsv; {model} skipped")
    compile_lexicon(source / "lexicon.tsv", output)
    if not quiet:
        print(f"compiled into {paths.relative(output)}")
    return output
