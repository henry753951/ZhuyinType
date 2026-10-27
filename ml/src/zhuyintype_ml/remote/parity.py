"""Check the exported ONNX models against fp32 PyTorch, the saved Core ML vectors and real reranker sentences.

    zt models parity --coreml         # on the Mac: Core ML reference outputs → remote_eval/reranker-coreml.json
    zt models parity                  # on Linux, after `zt models export`: → remote_eval/parity.json

Needs data/build/v3/test_vectors.json (checkpoints asset) and, for the second step, reranker-coreml.json from the
first (push it with `zt --remote --push data/build/remote_eval/reranker-coreml.json models parity`).  The fp32 ONNX
models must match PyTorch within 2e-4 and Core ML within 0.1; *.fp16.onnx results are recorded, not required.
"""

from __future__ import annotations

import argparse
import base64
import json
import time
from pathlib import Path
from typing import Any

import numpy as np

from zhuyintype_ml.core import paths

Case = dict[str, Any]


def real_reranker(directory: Path) -> list[Case]:
    """Sentences, contexts and an unknown character, encoded the way the engine encodes them."""
    vocab = json.loads((directory / "vocab.json").read_text())
    index = {c: i for i, c in enumerate(vocab) if i > 2 and len(c) == 1}
    cases = []
    for context, sentences in [
        ("", ["今天的天氣很好", "今天的天氣很號", "我想喝一杯咖啡"]),
        ("我們住在", ["臺灣", "台灣", "臺彎"]),
        ("前面已經提到今天的事情。" * 3, ["這件事情已經完成了", "這件事情已經完城了"]),
        ("", ["好", "𠮷"]),
    ]:
        encoded = [[index.get(c, 2) for c in s] for s in sentences]
        prefix = [1] + [index.get(c, 2) for c in context[-24:]]
        length = max(2, len(prefix) - 1 + max(map(len, encoded)))
        ids = np.zeros((len(sentences), length), np.int32)
        targets = ids.copy()
        for i, row in enumerate(encoded):
            tokens = prefix + row
            ids[i, : len(tokens) - 1] = tokens[:-1]
            targets[i, : len(tokens) - 1] = tokens[1:]
        cases.append({"context": context, "sentences": sentences, "ids": ids.tolist(), "targets": targets.tolist()})
    return cases


def coreml_reference(reranker: Path, output: Path) -> None:
    import coremltools as ct

    model = ct.models.MLModel(str(reranker / "Reranker.mlpackage"), compute_units=ct.ComputeUnit.CPU_ONLY)
    cases = real_reranker(reranker)
    for case in cases:
        inputs = {n: np.array(case[n], np.int32) for n in ["ids", "targets"]}
        case["coreml"] = model.predict(inputs)["logprob"].tolist()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n")


def onnx_parity(reranker: Path, v3: Path, output: Path) -> dict[str, Any]:
    import torch

    from zhuyintype_ml.remote.export_onnx import NAMES, Scorer, load_reranker, session
    from zhuyintype_ml.training.v3.models import ContextExport, SyllableExport, load_models

    torch.set_num_threads(1)
    rr = Scorer(load_reranker(reranker / "charlm.pt")).eval()
    context, syllable = load_models(v3 / "syllable.pt")
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    vectors = json.loads((v3 / "test_vectors.json").read_text())["vectors"]
    real = json.loads((output.parent / "reranker-coreml.json").read_text())
    report: dict[str, Any] = {}
    for suffix in [".onnx", ".fp16.onnx"]:
        try:
            cr, sr, r = (
                session(d / (n + suffix))
                for d, n in [(v3, "ContextEncoder"), (v3, "SyllableEncoder"), (reranker, "Reranker")]
            )
            rows = []
            for vector in vectors:
                values = {n: np.array(v, np.int32) for n, v in vector["inputs"].items()}
                ctx = {"context_ids": values.pop("context_ids")}
                start = time.monotonic()
                memory = dict(zip(["memory_k", "memory_v"], cr.run(None, ctx)))
                got = sr.run(None, {**values, **memory})[0]
                seconds = time.monotonic() - start
                with torch.no_grad():
                    k, v = cw(torch.from_numpy(ctx["context_ids"]))
                    expected = sw(*(torch.from_numpy(values[n]) for n in NAMES), k, v).numpy()
                valid = values["candidate_ids"] != 0
                ml = np.array(vector["expected"]["logprob"])
                memories = {
                    n: np.frombuffer(base64.b64decode(vector["expected"][n]["data"]), dtype="<f2").reshape(
                        vector["expected"][n]["shape"]
                    )
                    for n in ["memory_k", "memory_v"]
                }
                isolated = sr.run(None, {**values, **memories})[0]
                delta = float(np.max(abs(got[valid] - expected[valid])))
                rows.append(
                    {
                        "id": vector["id"],
                        "torch_abs": delta,
                        "coreml_abs": float(np.max(abs(got[valid] - ml[valid]))),
                        "coreml_same_memory_abs": float(np.max(abs(isolated[valid] - ml[valid]))),
                        "memory_abs": max(
                            float(np.max(abs(memory[n].astype(np.float32) - memories[n].astype(np.float32))))
                            for n in memories
                        ),
                        "top1_coreml": float(np.mean(got.argmax(-1) == ml.argmax(-1))),
                        "seconds": seconds,
                    }
                )
                np.testing.assert_allclose(np.exp(got).sum(-1), 1, atol=0.002, rtol=0.002)
                assert np.isfinite(got).all() and np.all(got[~valid] < -9990)
                assert delta < 0.1, rows[-1]
            rerank = []
            for case in real:
                inputs = {n: np.array(case[n], np.int32) for n in ["ids", "targets"]}
                with torch.no_grad():
                    expected = rr(*(torch.from_numpy(inputs[n]) for n in ["ids", "targets"])).numpy()
                got = r.run(None, inputs)[0]
                rerank.append(
                    {
                        "sentences": case["sentences"],
                        "torch_abs": float(np.max(abs(got - expected))),
                        "coreml_abs": float(np.max(abs(got - np.array(case["coreml"])))),
                    }
                )
                assert rerank[-1]["coreml_abs"] < 0.1, rerank[-1]
                if suffix == ".onnx":
                    np.testing.assert_allclose(got, expected, atol=2e-4, rtol=2e-5)
            report[suffix] = {"v3": rows, "reranker": rerank}
        except Exception as error:
            report[suffix] = {"error": str(error)}
            if suffix == ".onnx":
                output.write_text(json.dumps(report, indent=2) + "\n")
                raise
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--coreml", action="store_true", help="write the Core ML reference outputs (macOS)")
    parser.add_argument(
        "--output",
        type=Path,
        help="report file (default: remote_eval/parity.json or remote_eval/reranker-coreml.json in the data directory)",
    )
    parser.add_argument("--reranker", type=Path, default=paths.data("reranker"))
    parser.add_argument("--v3", type=Path, default=paths.data("v3"))
    args = parser.parse_args(argv)
    if args.coreml:
        coreml_reference(args.reranker, args.output or paths.data("remote_eval", "reranker-coreml.json"))
        return 0
    report = onnx_parity(args.reranker, args.v3, args.output or paths.data("remote_eval", "parity.json"))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
