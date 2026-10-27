"""zt train lm: the Wikipedia word bigram the app ships as lm/ (unigram.tsv, bigram.tsv, backoff.tsv).

    zt --remote train lm                          # GPU host (CPU only): download … prefer-tai
    zt --remote train lm --option workers=48      # extraction, segmentation and verification processes (default 64)
    zt train lm prefer-tai --force                # one step again

Steps run in the order they were run for 0.3.0.  `train` and `verify` need the GPU host (64 workers, ~50 GB disk,
>=64 GB RAM); `prefer-tai` rewrites the TSVs once, after `verify`.  Intermediate files go to data/build/lm/work.
"""

from __future__ import annotations

from collections.abc import Callable

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.paths import raw
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

from .download import DUMP_URL


def run(module: str, *args: str | Callable[[Context], str], group: str | None = "train") -> Callable[[Context], None]:
    """A step that runs `python -m module args` (callable arguments are resolved against the context)."""

    def step(ctx: Context) -> None:
        ctx.python(module, *[a(ctx) if callable(a) else a for a in args], group=group, threads=1)

    return step


def tests(directory: str) -> Callable[[Context], None]:
    """A step that runs a model directory's unit tests in the train environment."""
    return run("unittest", "discover", "-s", str(paths.ROOT / directory), "-t", str(paths.ML / "src"))


def workers(default: int) -> Callable[[Context], str]:
    return lambda ctx: str(ctx.options.get("workers", default))


def data(*parts: str) -> Callable[[Context], str]:
    """A path in the data directory, resolved when the step runs."""
    return lambda ctx: str(ctx.data.joinpath(*parts))


PIPELINE = Pipeline(
    name="lm",
    help="Wikipedia word bigram (lm/), modified Kneser-Ney",
    output="lm",
    steps=(
        Step("test", "unit tests of segmentation, MKN and the binary formats", tests("zhuyintype_ml/training/lm")),
        Step(
            "download",
            "latest zhwiki pages-articles dump into data/raw/lm, SHA-1 checked, with a receipt",
            run("zhuyintype_ml.training.lm.download", lambda ctx: str(raw("lm")), group=None),
        ),
        Step(
            "train",
            "extract, deduplicate, hold out 5,000 sentences, hard-EM segmentation, MKN bigram (GPU host)",
            run(
                "zhuyintype_ml.training.lm.train",
                "--dump",
                lambda ctx: str(raw("lm", DUMP_URL.rsplit("/", 1)[1])),
                "--lexicon",
                data("lexicon.tsv"),
                "--work",
                data("lm", "work"),
                "--output",
                data("lm"),
                "--workers",
                workers(64),
            ),
        ),
        Step(
            "verify",
            "independent TSV, lexicon and train/test separation checks (GPU host)",
            run(
                "zhuyintype_ml.training.lm.verify",
                "--lexicon",
                data("lexicon.tsv"),
                "--work",
                data("lm", "work"),
                "--output",
                data("lm"),
                "--workers",
                workers(64),
            ),
        ),
        Step(
            "prefer-tai",
            "move probability from 臺 spellings to 台 ones (once, after verify)",
            run("zhuyintype_ml.training.lm.prefer_tai", data("lm"), data("lexicon.tsv"), group=None),
        ),
    ),
)
