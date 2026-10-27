"""Export the reranker and the v3 encoders from their checkpoints to ONNX for the Linux engine (ONNX Runtime).

    zt models export [--tree] [--fp16]          # the usual way: cached by content, runs this with the export group
    uv run --group export python -m zhuyintype_ml.remote.export_onnx --reranker data/build/reranker --v3 data/build/v3

Inputs are dynamic int32, the reranker returns fp32 log-probabilities and the v3 encoders keep the fp16 memory
boundary of the Core ML models.  --fp16 also writes *.fp16.onnx experiments.  After exporting, the reranker is run on
odd shapes against PyTorch, so a frozen trace shape fails here instead of in an evaluation.  A report with versions,
hashes and errors goes to <v3>/onnx-export.json.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

import numpy as np
import onnx
import onnxruntime as ort
import torch

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.reranker.export_tree import TreeScorer
from zhuyintype_ml.training.reranker.export_tree import example as tree_example
from zhuyintype_ml.training.reranker.train import CharLM, Config
from zhuyintype_ml.training.v3.models import ContextExport, SyllableExport, load_models

NAMES = ["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids"]
Axes = dict[str, dict[int, str]]


class Scorer(torch.nn.Module):
    """The reranker as the engine calls it: log-probability of each target token."""

    def __init__(self, model: torch.nn.Module) -> None:
        super().__init__()
        self.model = model

    def forward(self, ids: torch.Tensor, targets: torch.Tensor) -> torch.Tensor:
        return self.model(ids).float().log_softmax(-1).gather(-1, targets.long().unsqueeze(-1)).squeeze(-1)


class HalfSyllable(torch.nn.Module):
    """The syllable encoder in fp16, taking the memory without its head axis (as the fp32 export does)."""

    def __init__(self, model: torch.nn.Module) -> None:
        super().__init__()
        self.model = model

    def forward(
        self,
        syllable_ids: torch.Tensor,
        tone_ids: torch.Tensor,
        cost_bucket: torch.Tensor,
        boundary: torch.Tensor,
        candidate_ids: torch.Tensor,
        memory_k: torch.Tensor,
        memory_v: torch.Tensor,
    ) -> torch.Tensor:
        return self.model(
            syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids, memory_k.unsqueeze(1), memory_v.unsqueeze(1)
        )


def session(path: Path) -> ort.InferenceSession:
    """A one-thread CPU session, as each engine process uses."""
    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    return ort.InferenceSession(str(path), options, providers=["CPUExecutionProvider"])


def load_reranker(checkpoint: Path) -> Any:
    saved = torch.load(checkpoint, map_location="cpu", weights_only=False)
    model = CharLM(Config(**saved["config"]))
    model.load_state_dict(saved["model"])
    return model.eval()


def export(
    model: torch.nn.Module,
    values: tuple[torch.Tensor, ...],
    path: Path,
    names: list[str],
    outputs: list[str],
    axes: Axes,
    fp16: bool,
) -> None:
    with torch.no_grad():
        torch.onnx.export(
            model,
            values,
            str(path),
            input_names=names,
            output_names=outputs,
            dynamic_axes=axes,
            opset_version=17,
            dynamo=False,
        )
    onnx.checker.check_model(str(path))
    if fp16:
        half: torch.nn.Module = copy.deepcopy(model).half().eval()
        if isinstance(half, SyllableExport):
            half = HalfSyllable(half.model).eval()
        half_values = tuple(v.half() if v.is_floating_point() else v for v in values)
        target = path.with_suffix(".fp16.onnx")
        with torch.no_grad():
            torch.onnx.export(
                half,
                half_values,
                str(target),
                input_names=names,
                output_names=outputs,
                dynamic_axes=axes,
                opset_version=17,
                dynamo=False,
            )
        onnx.checker.check_model(str(target))


def check_reranker(scorer: Scorer, vocabulary: int, path: Path) -> list[dict[str, float]]:
    """Dynamic dimensions and odd lengths against PyTorch."""
    rr = session(path)
    rng = np.random.default_rng(7)
    checks = []
    for batch, length in [(1, 2), (3, 7), (16, 48), (32, 64)]:
        x = rng.integers(0, vocabulary, (batch, length), dtype=np.int32)
        with torch.no_grad():
            expected = scorer(torch.from_numpy(x), torch.from_numpy(x)).numpy()
        actual = rr.run(None, {"ids": x, "targets": x})[0]
        np.testing.assert_allclose(actual, expected, atol=2e-4, rtol=2e-5)
        checks.append({"batch": batch, "length": length, "max_abs_error": float(np.max(abs(actual - expected)))})
    return checks


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--reranker",
        type=Path,
        default=paths.data("reranker"),
        help="reranker directory with charlm.pt (default: data/build/reranker)",
    )
    parser.add_argument("--reranker-checkpoint", type=Path, help="checkpoint (default: <reranker>/charlm.pt)")
    parser.add_argument(
        "--v3", type=Path, default=paths.data("v3"), help="v3 directory with syllable.pt (default: data/build/v3)"
    )
    parser.add_argument("--fp16", action="store_true", help="also write *.fp16.onnx experiments")
    parser.add_argument("--tree", action="store_true", help="also export prefix-tree scoring with the same weights")
    args = parser.parse_args(argv)
    torch.set_num_threads(1)
    checkpoint = args.reranker_checkpoint or args.reranker / "charlm.pt"
    model = load_reranker(checkpoint)
    scorer = Scorer(model).eval()
    ids = torch.ones((4, 24), dtype=torch.int32)
    export(
        scorer,
        (ids, ids),
        args.reranker / "Reranker.onnx",
        ["ids", "targets"],
        ["logprob"],
        {n: {0: "batch", 1: "length"} for n in ["ids", "targets", "logprob"]},
        args.fp16,
    )
    if args.tree:
        export(
            TreeScorer(model).eval(),
            tree_example(),
            args.reranker / "RerankerTree.onnx",
            ["ids", "positions", "ancestors", "parents", "targets"],
            ["logprob"],
            {
                "ids": {1: "nodes"},
                "positions": {1: "nodes"},
                "ancestors": {0: "nodes", 1: "nodes"},
                "parents": {0: "edges"},
                "targets": {0: "edges"},
                "logprob": {0: "edges"},
            },
            args.fp16,
        )
    context, syllable = load_models(args.v3 / "syllable.pt")
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    ctx = torch.ones((1, 12), dtype=torch.int32)
    with torch.no_grad():
        k, v = cw(ctx)
    typed = [torch.ones((1, 10), dtype=torch.int32) for _ in range(4)]
    candidates = torch.ones((1, 10, 8), dtype=torch.int32) * 4
    export(
        cw,
        (ctx,),
        args.v3 / "ContextEncoder.onnx",
        ["context_ids"],
        ["memory_k", "memory_v"],
        {"context_ids": {1: "context"}, "memory_k": {1: "context"}, "memory_v": {1: "context"}},
        args.fp16,
    )
    axes: Axes = {n: {1: "length"} for n in NAMES[:4]}
    axes.update({n: {1: "length", 2: "candidates"} for n in ["candidate_ids", "logprob"]})
    axes.update({n: {1: "context"} for n in ["memory_k", "memory_v"]})
    export(
        sw,
        (*typed, candidates, k, v),
        args.v3 / "SyllableEncoder.onnx",
        NAMES + ["memory_k", "memory_v"],
        ["logprob"],
        axes,
        args.fp16,
    )
    checks = check_reranker(scorer, model.c.vocab, args.reranker / "Reranker.onnx")
    files = [checkpoint, args.v3 / "context.pt", args.v3 / "syllable.pt"]
    files += sorted(args.reranker.glob("*.onnx")) + sorted(args.v3.glob("*.onnx"))
    report = {
        "torch": torch.__version__,
        "onnx": onnx.__version__,
        "onnxruntime": ort.__version__,
        "sha256": {paths.relative(x): hashlib.sha256(x.read_bytes()).hexdigest() for x in files},
        "dynamic_reranker": checks,
    }
    (args.v3 / "onnx-export.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
