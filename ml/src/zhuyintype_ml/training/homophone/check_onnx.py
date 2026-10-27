"""Compare a v3 model's ONNX export, or its saved Core ML outputs, with its PyTorch checkpoint (GPU host).

    zt --remote train homophone parity
    python -m zhuyintype_ml.training.homophone.check_onnx --v3 data/build/v3_homophone2 \
        --output homophone2/onnx-chosen-audit.json
    python -m zhuyintype_ml.training.homophone.check_onnx --v3 data/build/v3_homophone2 --output OUT.json \
        --coreml-vectors data/build/v3_homophone2/test_vectors.json --diagnostic

Uses the 20 interface cases of the original v3 (v3_030/test_vectors.json).  ONNX (ContextEncoder.onnx and
SyllableEncoder.onnx in the model directory, from zhuyintype_ml/remote's export) must match fp32 PyTorch within 2e-4.
With --coreml-vectors, the saved Core ML outputs from the Mac are decoded and compared instead (nothing runs Core ML
here): logprob, memory and probability differences and argmax changes per case, failing above the historical 0.1 bound
unless --diagnostic.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import onnxruntime as ort
import torch

from zhuyintype_ml.training.v3.models import ContextExport, SyllableExport, load_models
from zhuyintype_ml.training.v3.reference import NAMES

from .common import BASE, save


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--v3", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--coreml-vectors", type=Path, help="Check saved Mac export outputs; execute no Core ML here.")
    p.add_argument(
        "--diagnostic",
        action="store_true",
        help="Record fp16 differences for baseline calibration without applying the inherited 0.1 bound.",
    )
    args = p.parse_args(argv)
    assert sys.platform == "linux"
    torch.set_num_threads(1)
    context, syllable = load_models(args.v3 / "syllable.pt")
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    if not args.coreml_vectors:
        cr = ort.InferenceSession(str(args.v3 / "ContextEncoder.onnx"), options, providers=["CPUExecutionProvider"])
        sr = ort.InferenceSession(str(args.v3 / "SyllableEncoder.onnx"), options, providers=["CPUExecutionProvider"])
    rows: list[dict[str, Any]] = []
    vectors = json.loads((args.coreml_vectors or BASE / "test_vectors.json").read_text())["vectors"]
    template = json.loads((BASE / "test_vectors.json").read_text())["vectors"]
    assert len(vectors) == len(template) == 20
    for vector, original in zip(vectors, template, strict=True):
        assert vector["id"] == original["id"] and vector["inputs"] == original["inputs"]
        values = {n: np.asarray(v, np.int32) for n, v in vector["inputs"].items()}
        ctx = values.pop("context_ids")
        if args.coreml_vectors:
            memory: dict[str, np.ndarray] = {}
            for name in ["memory_k", "memory_v"]:
                item = vector["expected"][name]
                raw = base64.b64decode(item["data"], validate=True)
                assert item["dtype"] == "float16" and item["byte_order"] == "little" and item["encoding"] == "base64"
                assert hashlib.sha256(raw).hexdigest() == item["sha256"]
                memory[name] = np.frombuffer(raw, dtype="<f2").reshape(item["shape"]).astype(np.float32)
            actual = np.asarray(vector["expected"]["logprob"], np.float32)
        else:
            memory = dict(zip(["memory_k", "memory_v"], cr.run(None, {"context_ids": ctx})))
            actual = sr.run(None, {**values, **memory})[0]
        with torch.no_grad():
            k, v = cw(torch.from_numpy(ctx))
            expected = sw(*(torch.from_numpy(values[n]) for n in NAMES), k, v).numpy()
        valid = values["candidate_ids"] != 0
        delta = float(abs(actual[valid] - expected[valid]).max())
        assert actual.shape == expected.shape and np.isfinite(actual).all()
        assert np.all(actual[~valid] < -9990)
        if args.coreml_vectors:
            for name, value in [("memory_k", k), ("memory_v", v)]:
                assert memory[name].shape == tuple(value.shape) and np.isfinite(memory[name]).all()
            memory_delta = max(
                float(abs(memory[name] - value.numpy()).max()) for name, value in [("memory_k", k), ("memory_v", v)]
            )
            with torch.no_grad():
                isolated = sw(
                    *(torch.from_numpy(values[n]) for n in NAMES),
                    torch.from_numpy(memory["memory_k"]),
                    torch.from_numpy(memory["memory_v"]),
                ).numpy()
        else:
            np.testing.assert_allclose(actual[valid], expected[valid], atol=2e-4, rtol=2e-5)
        tolerance = 0.01 if args.coreml_vectors else 0.002
        np.testing.assert_allclose(np.exp(actual).sum(-1), 1, atol=tolerance, rtol=tolerance)
        row = dict(id=vector["id"], max_abs_error=delta)
        if args.coreml_vectors:
            row.update(
                max_memory_abs_error=memory_delta,
                same_memory_logprob_error=float(abs(actual[valid] - isolated[valid]).max()),
                max_probability_abs_error=float(abs(np.exp(actual) - np.exp(expected)).max()),
                top1_agreement=float(np.mean(actual.argmax(-1) == expected.argmax(-1))),
                top1_changed=int(np.sum(actual.argmax(-1) != expected.argmax(-1))),
                positions=int(actual.shape[1]),
                within_reference_tolerance=delta <= 0.1 and memory_delta <= 0.1,
            )
        rows.append(row)
    save(args.output, dict(cases=rows, maximum=max(r["max_abs_error"] for r in rows)))
    if args.coreml_vectors and not args.diagnostic:
        assert all(r["within_reference_tolerance"] for r in rows), (
            f"Core ML fp16 exceeds reference fp32 tolerance; full audit saved to {args.output}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
