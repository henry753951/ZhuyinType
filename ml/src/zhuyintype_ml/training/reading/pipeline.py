"""The reading-noise v3: the app's v3 continued with characters typed another way, from the reading channel.

    zt --remote --nice 10 --detach train reading test gap train evaluate   # GPU host
    zt --remote train reading train --option runs=s10,r10 --option tolerance=0.005
    zt train reading export --option candidate=s10                       # Mac: Core ML of the chosen run

Options (--option KEY=VALUE):
    runs        one training run each: s<scale> scales every class, r<scale> the rare ones (default: s3,s10,s30,s100)
    tolerance   allowed loss on the general and 在／再 dev checks (default: 0.002)
    lexicon     the lexicon whose readings are the standard for the gap rows (default: lexcov-s14/lexicon.tsv)
    candidate   the run to export (s<scale>), after its dev decision

The data directory needs v3_030/ (the 0.3.0 v3 and its shards), homophone2/ and v3_homophone2/ (the homophone run
and the shipped model it continues), lexicon_coverage/channel/ and misread/ (`zt train lexicon channel misread`),
lexicon_coverage/eval/ (the 善解 rows) and the lexicon-coverage build the matrices name (lexcov-s14/).
"""

from __future__ import annotations

import shutil

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

MODULE = "zhuyintype_ml.training.reading"
MATRICES = paths.PACKAGE / "training" / "reading" / "matrices"


def tests(ctx: Context) -> None:
    ctx.python("unittest", "discover", "-s", str(paths.PACKAGE / "training" / "reading"), "-t", str(paths.ML / "src"))


def gap(ctx: Context) -> None:
    lexicon = str(ctx.options.get("lexicon", ctx.data / "lexcov-s14" / "lexicon.tsv"))
    misread = str(ctx.data / "lexicon_coverage" / "misread")
    ctx.python(f"{MODULE}.gap", "--lexicon", lexicon, "--misread", misread, "--out", str(ctx.out / "gap"))


def train(ctx: Context) -> None:
    runs = str(ctx.options.get("runs", "s3,s10,s30,s100"))
    tolerance = str(ctx.options.get("tolerance", 0.002))
    ctx.python(f"{MODULE}.train", "--runs", runs, "--tolerance", tolerance, group="train", threads=4)


def candidates(ctx: Context) -> dict[str, str]:
    """The shipped v3 (`start`) and every finished run, by name."""
    found = {"start": str(paths.data("v3_homophone2"))}
    for model in sorted(ctx.data.glob("v3_reading_*")):
        if (model / "syllable.pt").exists():
            found[model.name.removeprefix("v3_reading_")] = str(model)
    return found


def evaluate(ctx: Context) -> None:
    """Every matrix with each candidate on the GPU model server: reports in v3_reading/eval/<candidate>/<matrix>/."""
    for name, model in candidates(ctx).items():
        for matrix in sorted(MATRICES.glob("*.json")):
            output = ctx.out / "eval" / name / matrix.stem
            if output.exists() and any(output.glob("*.json")):
                ctx.log(f"kept {paths.relative(output)}")
                continue
            ctx.python(
                "zhuyintype_ml.cli",
                *("models", "serve", "--device", "cuda", "--v3", model, "--"),
                *("eval", "sweep", str(matrix), "--output", str(output)),
                group="export",
            )


def export(ctx: Context) -> None:
    """Core ML for the chosen run on the Mac: the homophone export (licences, notices, golden vectors) unchanged."""
    import json

    from zhuyintype_ml.training.homophone.common import digest

    name = str(ctx.options["candidate"])
    model = ctx.data / f"v3_reading_{name}"
    run = ctx.out / "export" / name
    run.mkdir(parents=True, exist_ok=True)
    (run / "choice2.json").write_text(json.dumps({"checkpoint": digest(model / "syllable.pt")}) + "\n")
    ctx.python("zhuyintype_ml.training.homophone.export", "--model", str(model), "--run", str(run), group="export")
    shutil.copy2(ctx.out / f"{name}.json", model / "training.json")


PIPELINE = Pipeline(
    name="reading",
    help="the app's v3 continued with reading noise from the reading channel (v3_reading/, v3_reading_*/)",
    output="v3_reading",
    steps=(
        Step("test", "unit tests of the reading noise and the gap rows", tests),
        Step("gap", "misread-dev's sentences typed with the standard and the variant readings", gap),
        Step("train", "continue the shipped v3 with reading noise, one run per scale (CUDA)", train),
        Step(
            "evaluate", "dev, misread-dev, 善解 and gap matrices for every candidate, app configuration (GPU)", evaluate
        ),
        Step("export", "Core ML of the chosen run, compiled, with golden vectors (macOS only)", export),
    ),
)
