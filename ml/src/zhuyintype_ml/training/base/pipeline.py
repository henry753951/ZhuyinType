"""zt train base: statistical base A1, a McBopomofo-augmented lexicon and expected-count word bigrams (research).

    zt --remote train base                     # GPU host: test … fallback
    zt --remote train base fresh               # one step

Not adopted for 0.4.0 (REPORT.md).  Reads the frozen inputs in data/build/base/inputs and the Wikipedia, chat2 and
0.3.0 corpora; the first five models use the archived first lexicon, lexicon_mcbopomofo.tsv (data only).  Each
model directory is written once: steps skip models that exist.  GPU host only.
"""

from __future__ import annotations

from collections.abc import Callable

from zhuyintype_ml.training.lm.pipeline import run, tests
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

MODULE = "zhuyintype_ml.training.base"
# (name, domain, method, generated-text weight), as fitted with the archived first lexicon.
INCUMBENT = [
    ("chat_hard", "chat", "hard", "0"),
    ("chat_expected", "chat", "expected", "0"),
    ("chat_generated01", "chat", "expected", "0.1"),
    ("wiki_hard", "wiki", "hard", "0"),
    ("wiki_expected", "wiki", "expected", "0"),
]
# Chat controls with corpus-adapted segmentation (--fresh), fitted with the corrected lexicon.
FRESH = [
    ("chat_fresh_hard", "hard", "0"),
    ("chat_fresh_expected", "expected", "0"),
    ("chat_fresh_generated01", "expected", "0.1"),
]
# (new model, source model) of the unused-vocabulary ablation.
FALLBACK = [("chat_unused_fallback", "chat_fresh_expected"), ("wiki_unused_fallback", "wiki_expected_wide")]


def unless(path: str, step: Callable[[Context], None]) -> Callable[[Context], None]:
    """Runs `step` unless data/build/base/<path> exists (the modules refuse to overwrite it)."""

    def guarded(ctx: Context) -> None:
        if (ctx.out / path).exists():
            ctx.log(f"base/{path} exists; kept")
            return
        step(ctx)

    return guarded


def fit(name: str, *options: str, lexicon: str) -> Callable[[Context], None]:
    step = run(
        f"{MODULE}.train",
        "fit",
        "--name",
        name,
        *options,
        "--lexicon",
        lambda ctx: str(ctx.out / lexicon),
        "--workers",
        "8",
    )
    return unless(f"models/{name}", step)


def model(name: str) -> Callable[[Context], str]:
    return lambda ctx: str(ctx.out / "models" / name)


def each(steps: list[Callable[[Context], None]]) -> Callable[[Context], None]:
    def all_of(ctx: Context) -> None:
        for step in steps:
            step(ctx)

    return all_of


PIPELINE = Pipeline(
    name="base",
    help="statistical base A1: McBopomofo lexicon and expected-count bigrams (research, not adopted)",
    output="base",
    steps=(
        Step(
            "test",
            "unit tests of lattice posteriors, the lexicon merge, prior and fallback",
            tests("ml/src/zhuyintype_ml/training/base"),
        ),
        Step(
            "lexicon",
            "lexicon_source.tsv: seed rows plus calibrated McBopomofo rows",
            run(f"{MODULE}.train", "lexicon"),
        ),
        Step(
            "prepare",
            "Wikipedia, real-chat and generated shards rescanned against every dev/test file (once)",
            unless("corpus", run(f"{MODULE}.train", "prepare", "--workers", "8")),
        ),
        Step(
            "fit",
            "hard and expected-count chat and Wikipedia models, incumbent tokenizers (GPU host)",
            each(
                [
                    fit(
                        name,
                        "--domain",
                        domain,
                        "--method",
                        method,
                        "--generated-weight",
                        weight,
                        lexicon="lexicon_mcbopomofo.tsv",
                    )
                    for name, domain, method, weight in INCUMBENT
                ]
            ),
        ),
        Step(
            "wide",
            "wiki_expected refitted with up to six million bigrams",
            unless("models/wiki_expected_wide", run(f"{MODULE}.prune")),
        ),
        Step(
            "prior",
            "McBopomofo frequencies as a context-free chat prior (control)",
            unless("models/native_prior", run(f"{MODULE}.prior")),
        ),
        Step(
            "fresh",
            "chat models with three corpus E/M segmentation passes (GPU host)",
            each(
                [
                    fit(
                        name,
                        "--domain",
                        "chat",
                        "--method",
                        method,
                        "--generated-weight",
                        weight,
                        "--fresh",
                        lexicon="lexicon_source.tsv",
                    )
                    for name, method, weight in FRESH
                ]
            ),
        ),
        Step(
            "fallback",
            "unused lexicon words left to the engine fallback (ablation)",
            each(
                [
                    unless(
                        f"models/{name}",
                        run(f"{MODULE}.fallback", "--source", model(source), "--output", model(name)),
                    )
                    for name, source in FALLBACK
                ]
            ),
        ),
    ),
)
