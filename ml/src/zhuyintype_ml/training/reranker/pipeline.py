"""The reranker pipeline: token streams, the v2b character language model, its Core ML exports.

    zt --remote train reranker                      # GPU host: prepare … tree (stops at compile off macOS)
    zt train reranker export tree compile           # Mac, with charlm.pt fetched to data/build/reranker/
    zt train reranker --option workers=60           # prepare processes (default 32)

The shipped model is v2b (0.3.0): 8 layers, 512 wide, 90,000 steps, sentence weight 1.0 and 16 candidates
(config.json).  REPORT_v2.md compares it with v1/v2a/v2c; REPORT_tree.md covers the prefix-tree scorer (0.4.0).
ONNX for Linux comes from ml/src/zhuyintype_ml/remote's export.  Training needs CUDA; compile needs macOS.
"""

from __future__ import annotations

import json
from collections.abc import Callable

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

MODULE = "zhuyintype_ml.training.reranker"
# v2b, the shipped size (REPORT_v2.md).
MODEL = ["--steps", "90000", "--layers", "8", "--width", "512", "--heads", "8", "--ffn", "2048"]
# How the app uses it: sentence weight and how many of the engine's best sentences to rerank.
CONFIG = {"weight": 1.0, "count": 16}


def module(name: str, *args: str, group: str = "train") -> Callable[[Context], None]:
    def run(ctx: Context) -> None:
        ctx.python(f"{MODULE}.{name}", "--data", str(ctx.out), *args, group=group)

    return run


def tests(ctx: Context) -> None:
    ctx.python(
        "unittest",
        "discover",
        "-s",
        str(paths.PACKAGE / "training" / "reranker"),
        "-t",
        str(paths.ML / "src"),
        group="train",
    )


def prepare(ctx: Context) -> None:
    workers = str(ctx.options.get("workers", 32))
    ctx.python(f"{MODULE}.prepare", "--out", str(ctx.out), "--workers", workers, group="train")


def export(ctx: Context) -> None:
    ctx.python(f"{MODULE}.export", "--data", str(ctx.out), group="export")
    (ctx.out / "config.json").write_text(json.dumps(CONFIG) + "\n")


def compile_models(ctx: Context) -> None:
    from zhuyintype_ml.training.v3.coreml import compile_package

    for name in ["Reranker", "RerankerTree"]:
        compile_package(ctx.out / f"{name}.mlpackage", ctx.out)


PIPELINE = Pipeline(
    name="reranker",
    help="v2b character LM for whole-sentence reranking (reranker/)",
    output="reranker",
    steps=(
        Step("test", "unit tests of the held-out filter and the prefix-tree scorer", tests),
        Step("prepare", "vocabulary and token streams without held-out 8-grams (GPU host)", prepare),
        Step("train", "v2b character LM, 90,000 steps (CUDA)", module("train", *MODEL)),
        Step("export", "Reranker.mlpackage and config.json", export),
        Step(
            "tree",
            "RerankerTree.mlpackage: the same weights as a prefix-tree scorer",
            module("export_tree", "--format", "coreml", group="export"),
        ),
        Step("compile", "compile both packages with coremlcompiler (macOS only)", compile_models),
    ),
)
