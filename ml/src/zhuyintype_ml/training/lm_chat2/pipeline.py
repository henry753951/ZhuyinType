"""zt train lm_chat2: the chat word bigram 0.4.0 ships as lm_chat/ (gazette, talk pages, Common Voice).

    zt train lm_chat2 sources                     # may run on the Mac: cached downloads with receipts
    zt --remote train lm_chat2                    # GPU host: test … ship
    zt --remote train lm_chat2 perplexity --force

Steps run in the order they were run.  Review data/build/lm_chat2/source_samples.json after `extract` and before
`split`.  `split`, `real` and `mixed` never redo finished work (their modules refuse to); `--force` on them only
reports that.  `real` is the shipped variant (no_generated/); `ship` copies it with its provenance into lm_chat/,
first archiving the 0.3.0 TSVs there as lm_chat_v1/.
"""

from __future__ import annotations

import shutil
from collections.abc import Callable

from zhuyintype_ml.training.lm.pipeline import run, tests
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

MODULE = "zhuyintype_ml.training.lm_chat2"


def sources(ctx: Context) -> None:
    """The three sources as cached for the experiment (the gazette API stopped at 506 of 800 documents)."""
    for source, *options in (["government", "--documents", "800"], ["wikipedia", "--batches", "20"], ["similar"]):
        ctx.python(f"{MODULE}.sources", source, *options)


def once(marker: str, step: Callable[[Context], None]) -> Callable[[Context], None]:
    """Runs `step` unless data/build/lm_chat2/<marker> exists: frozen splits and prepared corpora are never redone."""

    def guarded(ctx: Context) -> None:
        if (ctx.out / marker).exists():
            ctx.log(f"{marker} exists; kept as frozen")
            return
        step(ctx)

    return guarded


def freeze(ctx: Context) -> None:
    """The seed lexicon and the 0.3.0 chat tokenizer, copied into inputs/ unless already frozen there."""
    inputs = ctx.out / "inputs"
    tokenizer = inputs / "tokenizer"
    tokenizer.mkdir(parents=True, exist_ok=True)
    for source, target in [
        (ctx.data / "lm_chat" / "inputs" / "lexicon.tsv", inputs / "lexicon.tsv"),
        (ctx.data / "lm_chat" / "work" / "vocabulary.json", tokenizer / "vocabulary.json"),
        (ctx.data / "lm_chat" / "work" / "segmentation_scores.npy", tokenizer / "segmentation_scores.npy"),
    ]:
        if not target.exists():
            shutil.copy2(source, target)
            ctx.log(f"{source} → {target}")


def variant(name: str) -> Callable[[Context], None]:
    """Prepares (once) and trains one variant: real → no_generated/, mixed → lm_chat2/."""
    directory = "no_generated/work" if name == "real" else "work"
    prepare = once(
        f"{directory}/prepared.json",
        run(f"{MODULE}.prepare", "prepare", "--variant", name, "--workers", "24"),
    )
    train = run(f"{MODULE}.train", "--variant", name, "--workers", "24")

    def step(ctx: Context) -> None:
        prepare(ctx)
        train(ctx)

    return step


SHIPPED = ["unigram.tsv", "bigram.tsv", "backoff.tsv"]
PROVENANCE = ["NOTICE.md", "attribution.jsonl", "source-manifest.json"]


def ship(ctx: Context) -> None:
    """The real variant and its provenance into lm_chat/, as 0.4.0 ships it; the 0.3.0 model goes to lm_chat_v1/."""
    target = ctx.data / "lm_chat"
    archive = ctx.data / "lm_chat_v1"
    if not archive.exists():
        archive.mkdir()
        for name in SHIPPED + ["NOTICE.md"]:
            shutil.copy2(target / name, archive / name)
        ctx.log(f"0.3.0 chat model archived in {archive}")
    for name in SHIPPED:
        shutil.copy2(ctx.out / "no_generated" / name, target / name)
    for name in PROVENANCE:
        shutil.copy2(ctx.out / name, target / name)
    ctx.log(f"chat2 (no generated text) → {target}")


PIPELINE = Pipeline(
    name="lm_chat2",
    help="chat word bigram from real dialogue, shipped as lm_chat/ since 0.4.0",
    output="lm_chat2",
    steps=(
        Step(
            "test",
            "unit tests of turn parsing, context selection, leakage and the estimator",
            tests("ml/src/zhuyintype_ml/training/lm_chat2"),
        ),
        Step("sources", "cache gazette transcripts, talk pages and a data.gov.tw sample with receipts", sources),
        Step(
            "extract",
            "speaker turns and review samples (review source_samples.json next)",
            run(f"{MODULE}.prepare", "extract"),
        ),
        Step(
            "split",
            "1,000 dev and 1,000 test turn pairs, whole documents per split (once)",
            once("splits.json", run(f"{MODULE}.prepare", "split")),
        ),
        Step("freeze", "seed lexicon and 0.3.0 chat tokenizer into inputs/", freeze),
        Step("manifest", "source-manifest.json, attribution.jsonl, NOTICE.md", run(f"{MODULE}.manifest", group=None)),
        Step("real", "real dialogue and Common Voice model, no_generated/ (GPU host)", variant("real")),
        Step("mixed", "real plus the 0.3.0 colloquial corpus (GPU host)", variant("mixed")),
        Step(
            "perplexity",
            "dev word perplexity of four models on one tokenization (GPU host)",
            run(f"{MODULE}.evaluate", "--phase", "perplexity"),
        ),
        Step(
            "conversion",
            "dev conversion at chat priors 0.3/0.5/0.7; stopped at 7 of 24 rows (GPU host)",
            run(f"{MODULE}.evaluate", "--phase", "conversion"),
        ),
        Step("ship", "real variant and provenance into lm_chat/, the 0.3.0 model into lm_chat_v1/", ship),
    ),
)
