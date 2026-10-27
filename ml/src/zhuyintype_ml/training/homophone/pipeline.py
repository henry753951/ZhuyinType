"""The homophone pipeline: 在／再 and 26 other confusion sets, and the v3 continuation the app ships from 0.4.0.

    zt --remote train homophone                         # GPU host: snapshot … evaluate (stops at the dev decision)
    zt --remote train homophone evaluate-test           # after homophone2/dev-decision.json
    zt train homophone export check                     # Mac, after homophone2/choice2.json
    zt --remote train homophone parity                  # GPU host, after ml/src/zhuyintype_ml/remote's ONNX export
    zt train homophone judge --option judge=URL         # the Qwen3-8B server (default http://127.0.0.1:8102/…)

Data (homophone/): snapshot the lexicon, held-out sets and the Taiwan Tongues train TSV, count the seed groups,
mine clauses, have Qwen3-8B judge 在／再, split dev/test.  Continuation (homophone2/, v3_homophone2/): leak-free
shard indexes and real-source clauses without Taiwan Tongues, continue the v3_030 syllable encoder, compare with the
original on dev and, once, on test, export on the Mac.  v3_homophone2/ is released as v3/ (`zt data pack`).  Two
human decisions gate the end: dev-decision.json before test, choice2.json (the checkpoint's SHA256) before export.
"""

from __future__ import annotations

import shutil
import subprocess
from collections.abc import Callable

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

from .common import DATA, HELD, MODEL

MODULE = "zhuyintype_ml.training.homophone"
JUDGE = "http://127.0.0.1:8102/v1/chat/completions"


def module(name: str, *args: str, group: str | None = "train") -> Callable[[Context], None]:
    def run(ctx: Context) -> None:
        ctx.python(f"{MODULE}.{name}", *args, group=group)

    return run


def tests(ctx: Context) -> None:
    ctx.python(
        "unittest",
        "discover",
        "-s",
        str(paths.PACKAGE / "training" / "homophone"),
        "-t",
        str(paths.ML / "src"),
        group="train",
    )


def snapshot(ctx: Context) -> None:
    """Freezes the inputs of the data steps in homophone/: kept as they are when already there."""
    copies = [(paths.data("lexicon.tsv"), DATA / "lexicon.tsv")]
    copies += [(paths.data(p), DATA / "inputs" / paths.data(p).name) for p in HELD]
    copies += [(paths.data_dir().parent / "raw" / "taiwan-tongues" / "train.tsv", DATA / "inputs" / "train.tsv")]
    for source, target in copies:
        if target.exists():
            ctx.log(f"kept {paths.relative(target)}")
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        ctx.log(f"{paths.relative(source)} → {paths.relative(target)}")


def judge(ctx: Context) -> None:
    ctx.python(f"{MODULE}.judge", "--server", str(ctx.options.get("judge", JUDGE)), group=None)


def prepare(ctx: Context) -> None:
    workers = str(ctx.options.get("workers", 4))
    ctx.python(f"{MODULE}.compatible", "prepare", "--workers", workers, group="train")


def check(ctx: Context) -> None:
    """zhuyin-cli --check-v3: the Swift scorer against the golden vectors; output in homophone2/check-v3.log."""
    from zhuyintype_ml.training.v3.coreml import require_macos

    require_macos("zhuyin-cli --check-v3 with Core ML")
    done = subprocess.run([str(paths.cli()), "--check-v3", str(MODEL)], capture_output=True, text=True)
    (ctx.out / "check-v3.log").write_text(done.stdout + done.stderr)
    print(done.stdout + done.stderr, end="")
    done.check_returncode()


def parity(ctx: Context) -> None:
    ctx.python(
        f"{MODULE}.check_onnx", "--v3", str(MODEL), "--output", str(ctx.out / "onnx-chosen-audit.json"), group="export"
    )
    ctx.python(
        f"{MODULE}.check_onnx",
        "--v3",
        str(MODEL),
        "--output",
        str(ctx.out / "coreml-chosen-audit.json"),
        "--coreml-vectors",
        str(MODEL / "test_vectors.json"),
        "--diagnostic",
        group="export",
    )


PIPELINE = Pipeline(
    name="homophone",
    help="homophone sets and the 0.4.0 v3 continuation (homophone/, homophone2/, v3_homophone2/)",
    output="homophone2",
    steps=(
        Step("test", "unit tests of exclusions, categories, splits, filtering and scoring", tests),
        Step("snapshot", "copy the lexicon, held-out sets and Taiwan Tongues train.tsv into homophone/", snapshot),
        Step("inventory", "count the 27 seed groups over the whole corpus; keep 26", module("mine")),
        Step(
            "mine",
            "sample candidate clauses (every 20th wiki line, Taiwan Tongues)",
            module("mine", "--reuse-inventory"),
        ),
        Step("judge", "Qwen3-8B masked 在／再 review (needs the judge server)", judge),
        Step("split", "dev/test sets, smoke and a 100-clause review sample", module("split", group=None)),
        Step("prepare", "leak-free shard indexes and real-source clauses in homophone2/ (GPU host)", prepare),
        Step("train", "continue the v3_030 syllable encoder to the dev plateau (CUDA)", module("train")),
        Step(
            "evaluate",
            "original v3 against the continuation on dev, app configuration",
            module("evaluate", "compare", "--phase", "dev", group=None),
        ),
        Step(
            "evaluate-test",
            "the same on the test sets, once, after dev-decision.json",
            module("evaluate", "compare", "--phase", "test", group=None),
        ),
        Step(
            "export", "Core ML, compiled models, golden vectors, NOTICE (macOS only)", module("export", group="export")
        ),
        Step("check", "zhuyin-cli --check-v3 against the golden vectors (macOS only)", check),
        Step("parity", "ONNX and saved Core ML outputs against PyTorch (GPU host)", parity),
    ),
)
