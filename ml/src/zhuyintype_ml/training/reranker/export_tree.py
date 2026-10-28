"""Export the unchanged reranker weights as a prefix-tree scorer: depth RoPE and ancestor-only attention.

    zt train reranker tree
    python -m zhuyintype_ml.training.reranker.export_tree --format coreml [--data data/build/reranker]
    python -m zhuyintype_ml.training.reranker.export_tree --format onnx

Writes RerankerTree.mlpackage or RerankerTree.onnx beside the reranker.

Inputs are one packed trie: ids/positions [1,N], ancestors [N,N] (1 allowed, 0 masked), parents/targets [E] (root
points to itself).  Terminal leaves need no hidden state.  Output [E] scores each token at its parent's hidden state;
context/root outputs are ignored by Swift.  Before exporting, the scorer is checked against the batched model on a
small tree (2e-5).  The Core ML export keeps int32 gather indices (see REPORT_tree.md).

TreeScorer and example are also imported by ml/src/zhuyintype_ml/remote's ONNX export.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import numpy as np
import torch

from zhuyintype_ml.core import paths

from .train import CharLM, Config


class TreeScorer(torch.nn.Module):
    def __init__(self, model: CharLM) -> None:
        super().__init__()
        self.model = model

    def forward(
        self,
        ids: torch.Tensor,
        positions: torch.Tensor,
        ancestors: torch.Tensor,
        parents: torch.Tensor,
        targets: torch.Tensor,
    ) -> torch.Tensor:
        m = self.model
        x = m.embed(ids.long())
        cos = m.cos[0, 0][positions.long()].unsqueeze(1)
        sin = m.sin[0, 0][positions.long()].unsqueeze(1)
        mask = (1 - ancestors.to(x.dtype)) * -10000.0
        for block in m.blocks:
            x = block(x, cos, sin, mask)
        logits = (m.norm(x) @ m.embed.weight.T).float()
        logprob = logits.log_softmax(-1).reshape(-1)
        return logprob[parents.long() * m.c.vocab + targets.long()]


def example() -> tuple[torch.Tensor, ...]:
    # BOS -> a -> b / c -> d / e; siblings b/c have equal depth and cannot mix.
    return (
        torch.tensor([[1, 3, 4, 5]], dtype=torch.int32),
        torch.tensor([[0, 1, 2, 2]], dtype=torch.int32),
        torch.tensor([[1, 0, 0, 0], [1, 1, 0, 0], [1, 1, 1, 0], [1, 1, 0, 1]], dtype=torch.int32),
        torch.tensor([0, 0, 1, 1, 2, 3], dtype=torch.int32),
        torch.tensor([1, 3, 4, 5, 6, 7], dtype=torch.int32),
    )


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--data", type=Path, default=paths.data("reranker"), help="directory with charlm.pt")
    p.add_argument("--format", choices=["coreml", "onnx"], required=True)
    p.add_argument("--out", type=Path)
    p.add_argument("--target", choices=["macOS14", "macOS15"], default="macOS14", help="Core ML minimum macOS")
    a = p.parse_args(argv)
    torch.set_num_threads(1)
    checkpoint = torch.load(a.data / "charlm.pt", map_location="cpu", weights_only=False)
    model = CharLM(Config(**checkpoint["config"]))
    model.load_state_dict(checkpoint["model"])
    scorer = TreeScorer(model.eval()).eval()
    values = example()
    with torch.no_grad():
        actual = scorer(*values)
        for token, node in [(4, 2), (5, 3)]:
            expected = model(torch.tensor([[1, 3]]))[0, -1].log_softmax(-1)[token]
            torch.testing.assert_close(actual[node], expected, atol=2e-5, rtol=2e-5)
        for branch, token, node in [(4, 6, 4), (5, 7, 5)]:
            expected = model(torch.tensor([[1, 3, branch]]))[0, -1].log_softmax(-1)[token]
            torch.testing.assert_close(actual[node], expected, atol=2e-5, rtol=2e-5)
    names = ["ids", "positions", "ancestors", "parents", "targets"]
    if a.format == "onnx":
        import onnx
        import onnxruntime as ort

        out = a.out or a.data / "RerankerTree.onnx"
        with torch.no_grad():
            torch.onnx.export(
                scorer,
                values,
                str(out),
                input_names=names,
                output_names=["logprob"],
                dynamic_axes={
                    "ids": {1: "nodes"},
                    "positions": {1: "nodes"},
                    "ancestors": {0: "nodes", 1: "nodes"},
                    "parents": {0: "edges"},
                    "targets": {0: "edges"},
                    "logprob": {0: "edges"},
                },
                opset_version=17,
                dynamo=False,
            )
        onnx.checker.check_model(str(out))
        options = ort.SessionOptions()
        options.intra_op_num_threads = options.inter_op_num_threads = 1
        runtime = ort.InferenceSession(str(out), options, providers=["CPUExecutionProvider"])
        result = runtime.run(None, dict(zip(names, [x.numpy() for x in values])))[0]
        np.testing.assert_allclose(result, actual.detach().numpy(), atol=2e-4, rtol=2e-5)
    else:
        import coremltools as ct

        out = a.out or a.data / "RerankerTree.mlpackage"
        traced = torch.jit.trace(scorer, values)
        # 32 candidates x 63 characters + BOS; includes the candidate-list path.
        n = ct.RangeDim(1, 2017, default=128)
        e = ct.RangeDim(1, 2017, default=144)
        shapes = [(1, n), (1, n), (n, n), (e,), (e,)]
        # shortcut: fp16 score parity exceeds 0.01 nats, tighten precision before production bundling.
        # Flattened parent*vocab indices exceed 65535; symbolic shapes fool the int16 cast pass.
        pipeline = ct.PassPipeline.DEFAULT
        pipeline.remove_passes({"common::add_int16_cast"})
        mlmodel = ct.convert(
            traced,
            inputs=[ct.TensorType(name=name, shape=shape, dtype=np.int32) for name, shape in zip(names, shapes)],
            outputs=[ct.TensorType(name="logprob", dtype=np.float32)],
            minimum_deployment_target=getattr(ct.target, a.target),
            compute_precision=ct.precision.FLOAT16,
            convert_to="mlprogram",
            pass_pipeline=pipeline,
        )
        spec = mlmodel.get_spec()
        for function in spec.mlProgram.functions.values():
            for block in function.block_specializations.values():
                types = {value.name: value.type.tensorType.dataType for op in block.operations for value in op.outputs}
                types.update({value.name: value.type.tensorType.dataType for value in function.inputs})
                gather = [op for op in block.operations if op.type == "gather"][-1]
                indices = gather.inputs["indices"].arguments[0].name
                assert types[indices] == ct.proto.MIL_pb2.INT32, "tree output gather must retain int32 indices"
        mlmodel.short_description = "ZhuyinType v2b prefix tree: depth RoPE, ancestor-only attention"
        mlmodel.save(str(out))
    print("saved", out, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
