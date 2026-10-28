"""The compression pipeline: smaller shipped models from any reranker and v3 checkpoint, without retraining.

    zt --remote --nice 10 train compress                         # GPU host: test, quantize, onnx
    zt --remote --backend cpu --export eval sweep ml/src/zhuyintype_ml/eval/matrices/compress.json   # dev sets
    zt train compress coreml parity measure --option methods=int8-channel,pal6   # Mac, after pulling the methods
    zt --remote train compress --option v3=v3_new --option reranker=reranker      # another checkpoint (e.g. a new v3)

Options: `reranker=DIR`, `v3=DIR` (source model directories in the data directory, default reranker and v3),
`methods=a,b` (default: every method in methods.py; the Mac steps need it).  Outputs in compress/: one directory per
method with reranker/ and v3/ ready for `zhuyin-cli --reranker --v3` (ONNX) or the app (Core ML), and the JSON
records of each step.
"""

from __future__ import annotations

import os
import subprocess
from collections.abc import Callable
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

from .methods import METHODS, output

MODULE = "zhuyintype_ml.training.compress"


def sources(ctx: Context) -> list[str]:
    return [
        "--reranker",
        str(paths.data(str(ctx.options.get("reranker", "reranker")))),
        "--v3",
        str(paths.data(str(ctx.options.get("v3", "v3")))),
    ]


def methods(ctx: Context) -> list[str]:
    chosen = ctx.options.get("methods")
    return str(chosen).split(",") if chosen else [m.name for m in METHODS]


def module(name: str, with_sources: bool = True, mac: bool = False) -> Callable[[Context], None]:
    def run(ctx: Context) -> None:
        if mac:
            from zhuyintype_ml.training.v3.coreml import require_macos

            require_macos(f"`zt train compress {name}`")
        args = ["--out", str(ctx.out), "--method", *methods(ctx)]
        ctx.python(f"{MODULE}.{name}", *args, *(sources(ctx) if with_sources else []), group="export", threads=8)

    return run


def tests(ctx: Context) -> None:
    ctx.python(
        "unittest",
        "discover",
        "-s",
        str(paths.PACKAGE / "training" / "compress"),
        "-t",
        str(paths.ML / "src"),
        group="export",
    )


def app_directory(out: Path, name: str, base: Path) -> Path:
    """<out>/<name>/app: `base` (a data directory or an app's Resources) with this method's reranker and v3, as
    links."""
    directory = out / name / "app"
    directory.mkdir(parents=True, exist_ok=True)
    reranker, v3 = output(out, name)
    links = {n: base / n for n in ["lexicon.tsv", "lexicon.bin", "lexicon.json", "lm", "lm_chat"]}
    links.update(reranker=reranker, v3=v3)
    for link_name, target in links.items():
        link = directory / link_name
        if link.is_symlink():
            link.unlink()
        if target.exists():
            link.symlink_to(os.path.relpath(target, directory))
    return directory


def measure(ctx: Context) -> None:
    """Per-key latency, CPU, energy and memory of the app's engine path (EngineBench) with each method's Core ML
    models and with the shipped ones, alternating, `rounds` times (default 2); reports in <method>/latency-N.json.
    `base=DIR` takes the lexicon, language models and current models from DIR, e.g. the installed app's Resources
    (compiled like the app's); default the data directory."""
    from zhuyintype_ml.training.v3.coreml import require_macos

    require_macos("latency and energy")
    rounds = int(str(ctx.options.get("rounds", 2)))
    base = Path(str(ctx.options.get("base", paths.data_dir()))).expanduser()
    for round_ in range(1, rounds + 1):
        for name in ["current", *methods(ctx)]:
            data = base if name == "current" else app_directory(ctx.out, name, base)
            target = ctx.out / name / f"latency-{round_}.json"
            target.parent.mkdir(parents=True, exist_ok=True)
            ctx.log(f"latency {name} round {round_}")
            command = [str(paths.ROOT / "tools" / "zt"), "eval", "latency", "--data", str(data), "--json", str(target)]
            subprocess.run(command, check=True)


PIPELINE = Pipeline(
    name="compress",
    help="smaller Core ML and ONNX models of the reranker and v3 (int8, palettes) without retraining",
    output="compress",
    steps=(
        Step("test", "unit tests of the quantizers (export dependency group)", tests),
        Step("quantize", "fake-quantized fp16 checkpoints of every method", module("quantize")),
        Step("onnx", "ONNX of every method, dynamic int8 ONNX, reference outputs (GPU host)", module("onnx_models")),
        Step(
            "coreml",
            "compressed Core ML packages, compiled, sizes and load times (macOS only)",
            module("coreml", mac=True),
        ),
        Step(
            "parity",
            "compressed Core ML against the host's outputs and the uncompressed export (macOS only)",
            module("parity", with_sources=False, mac=True),
        ),
        Step("measure", "per-key latency and energy with each method's models (macOS only)", measure),
    ),
)
