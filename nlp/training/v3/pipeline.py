"""The v3 pipeline: corpus, polyphone labels, encoded shards, the two encoders, dev quality, Core ML export.

    zt --remote train v3                        # GPU host: snapshot … quality, export (stops at compile off macOS)
    zt train v3 export compile validate         # Mac, with the checkpoints and vocabularies fetched to v3_030/
    zt train v3 syllable --force                # one step again
    zt train v3 --option workers=48             # Wikipedia extraction workers (default 24)

This is the 0.3.0 v3 (data directory `v3_030/`).  The 0.4.0 app model continues it: `zt train homophone`.
Steps run in the order they were run for 0.3.0.  Training steps need CUDA; compile and validate need macOS.
"""

from __future__ import annotations

import shutil
from collections.abc import Callable

from nlp.common import paths
from nlp.training.pipeline import Context, Pipeline, Step

from .common import NAME

MODULE = "nlp.training.v3"


def module(name: str, *args: str, group: str = "train", omp: int = 4) -> Callable[[Context], None]:
    def run(ctx: Context) -> None:
        ctx.python(f"{MODULE}.{name}", *args, group=group, threads=omp)

    return run


def snapshot(ctx: Context) -> None:
    """Freezes the inputs the later steps read from v3_030/: the lexicon and the two dev sets (kept when there)."""
    for source, name in [
        (paths.data("lexicon.tsv"), "lexicon.tsv"),
        (paths.data("corpus", "dev_wikipedia.txt"), "dev_wikipedia.txt"),
        (paths.data("corpus", "dev_colloquial.txt"), "dev_colloquial.txt"),
    ]:
        if (ctx.out / name).exists():
            ctx.log(f"kept {paths.relative(ctx.out / name)}")
            continue
        shutil.copy2(source, ctx.out / name)
        ctx.log(f"{paths.relative(source)} → {paths.relative(ctx.out / name)}")


def tests(ctx: Context) -> None:
    ctx.python("unittest", "discover", "-s", str(paths.ROOT / "nlp/training/v3"), "-t", str(paths.ROOT), group="train")


def wiki(ctx: Context) -> None:
    ctx.python(f"{MODULE}.prepare", "wiki", "--workers", str(ctx.options.get("workers", 24)), group="train", threads=1)


def syllable(ctx: Context) -> None:
    """Syllable training up to 60,000 steps, then 10,000-step continuations until the dev NLL plateaus."""
    import json

    train = (f"{MODULE}.train", "syllable", "--batch", "64")
    ctx.python(*train, "--steps", "60000", group="train", threads=4)
    done = json.loads((ctx.out / "syllable-done.json").read_text())
    while not done["plateau"]:
        previous = done["steps"]
        ctx.log(f"syllable: no plateau after {previous} steps; continuing 10,000 steps")
        ctx.python(*train, "--resume", "--steps", "10000", group="train", threads=4)
        done = json.loads((ctx.out / "syllable-done.json").read_text())
        if done["steps"] <= previous:
            raise RuntimeError("continuation did not advance training")


def compile_models(ctx: Context) -> None:
    from .coreml import compile_package

    for name in ["ContextEncoder", "SyllableEncoder"]:
        compile_package(ctx.out / f"{name}.mlpackage", ctx.out)


PIPELINE = Pipeline(
    name="v3",
    help="0.3.0 context and syllable encoders (v3_030/); the 0.4.0 model continues it in homophone",
    output=NAME,
    steps=(
        Step("test", "unit tests of readings, typing noise and the model interfaces", tests),
        Step("snapshot", "copy the lexicon and the dev sets into v3_030/", snapshot),
        Step("wiki", "ordered Wikipedia paragraphs without held-out text (GPU host)", wiki),
        Step("colloquial", "deduplicated colloquial lines", module("prepare", "colloquial", omp=1)),
        Step("vocab", "character and syllable vocabularies, provenance", module("prepare", "vocab")),
        Step("polyphone", "polyphone classifier for single-character readings (CUDA)", module("polyphone")),
        Step("encode", "mmap shards of ids, readings and examples", module("encode")),
        Step("audit", "held-out leakage and shard checks", module("verify_data")),
        Step("selfcheck", "invariants of labels, noise and model interfaces", module("selfcheck")),
        Step(
            "context", "context LM to plateau (CUDA)", module("train", "context", "--steps", "30000", "--batch", "64")
        ),
        Step("syllable", "syllable encoder to plateau, with continuations (CUDA)", syllable),
        Step("quality", "dev top-1 selection with and without context and noise (CUDA)", module("evaluate")),
        Step("export", "Core ML packages and config.json", module("export", group="export")),
        Step("compile", "compile the packages with coremlcompiler (macOS only)", compile_models),
        Step(
            "validate",
            "Core ML parity, golden test vectors, CPU latency (macOS only)",
            module("validate", group="export"),
        ),
    ),
)
