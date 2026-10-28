"""ONNX models of every compression method, ONNX Runtime's dynamic int8 models, and reference outputs for the Mac.

    zt train compress onnx                      # after `quantize`, on the GPU host (export dependency group)
    python -m zhuyintype_ml.training.compress.onnx_models --out data/build/compress --method fp16 pal6

1. Each method's fake-quantized checkpoints are exported with ml/src/zhuyintype_ml/remote/export_onnx.py (fp32 graphs
   holding exactly the compressed values), so `zhuyin-cli --reranker DIR --v3 DIR` evaluates them on Linux.
2. ONNX Runtime models for Linux: onnx-int8 and onnx-int8-tensor, onnxruntime.quantization.quantize_dynamic of the
   fp16 method's models (int8 weights per channel or per tensor, activations quantized at run time); onnx-int8-weights,
   the int8-channel method's models with each int8 matrix stored as int8 and a DequantizeLinear (the Core ML values).
3. reference.json per method: the log-probabilities these ONNX models give for the v3 test vectors, the parity
   sentences of ml/src/zhuyintype_ml/remote/parity.py and a small prefix tree, which `zt train compress parity` on the
   Mac compares with the compressed Core ML models, and the one-thread seconds of those calls (for
   onnx-int8-weights also with the DequantizeLinear folded at load, reference-folded.json).
4. no-reranker/: the data directory's models without the reranker (symbolic links), for `--models`.

<out>/onnx.json records the size of every ONNX file.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import time
from pathlib import Path
from typing import Any

import numpy as np

from zhuyintype_ml.core import paths

from .methods import METHODS, ONNX_METHODS, output

ONNX_FILES = {
    "reranker": ["Reranker.onnx", "RerankerTree.onnx"],
    "v3": ["ContextEncoder.onnx", "SyllableEncoder.onnx"],
}


def export(reranker: Path, v3: Path) -> None:
    from zhuyintype_ml.remote import export_onnx

    export_onnx.main(["--reranker", str(reranker), "--v3", str(v3), "--tree"])


def files(out: Path, source_name: str, name: str) -> list[tuple[Path, Path]]:
    """A fresh <out>/<name>/ with the source method's small files; the (source, target) ONNX paths."""
    pairs = []
    for source, target in zip(output(out, source_name), output(out, name)):
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(source, target, symlinks=True, ignore=shutil.ignore_patterns("*.onnx", "*.pt", "*.ml*"))
        pairs += [(source / f, target / f) for f in ONNX_FILES[target.name]]
    return pairs


def dynamic(out: Path, name: str, per_channel: bool) -> None:
    """quantize_dynamic of the fp16 method's ONNX models into <out>/<name>/."""
    from onnxruntime.quantization import QuantType, quantize_dynamic

    for source, target in files(out, "fp16", name):
        quantize_dynamic(str(source), str(target), per_channel=per_channel, weight_type=QuantType.QInt8)
        print(f"{name}: {paths.relative(target)} {target.stat().st_size / 1e6:.1f} MB", flush=True)


def int8_rows(weight: np.ndarray) -> tuple[np.ndarray, np.ndarray, int] | None:
    """When `weight` holds int8-channel values (quantize.linear: a symmetric int8 scale per row of the PyTorch
    weight, which is axis 0, or axis 1 of a transposed copy), its int8 values, fp32 scales and axis; else None.
    Quantizing those values again gives the same values (test_compress), so the scales are recovered exactly."""
    for axis in (0, 1):
        rows = (weight if axis == 0 else weight.T).astype(np.float16)
        scale = ((2 * np.abs(rows).max(-1, keepdims=True)).astype(np.float16) / np.float16(254)).astype(np.float16)
        safe = np.where(scale > 0, scale, np.float16(1))
        q = np.clip(np.round((rows / safe).astype(np.float16)), -127, 127)
        values = (q.astype(np.float32) * scale.astype(np.float32)).astype(np.float16)
        if np.array_equal(values.astype(np.float32), (weight if axis == 0 else weight.T)):
            return q.astype(np.int8), scale[:, 0].astype(np.float32), axis
    return None


def weight_only(out: Path, name: str, source_name: str = "int8-channel") -> None:
    """The int8-channel method's ONNX models with every int8 matrix stored as int8 plus a scale per row and a
    DequantizeLinear node: the weights the evaluation used (to within fp16 rounding of each product) at a quarter of
    the fp32 file size, with fp32 activations."""
    import onnx
    from onnx import helper, numpy_helper

    from .methods import MINIMUM_VALUES

    for source, target in files(out, source_name, name):
        model = onnx.load(str(source))
        graph = model.graph
        replaced = 0
        for initializer in list(graph.initializer):
            weight = numpy_helper.to_array(initializer)
            if weight.dtype != np.float32 or weight.ndim != 2 or weight.size < MINIMUM_VALUES:
                continue
            found = int8_rows(weight)
            if found is None:
                continue
            q, scale, axis = found
            graph.initializer.remove(initializer)
            graph.initializer.extend(
                [
                    numpy_helper.from_array(q if axis == 0 else q.T, initializer.name + "_int8"),
                    numpy_helper.from_array(scale, initializer.name + "_scale"),
                ]
            )
            inputs = [initializer.name + "_int8", initializer.name + "_scale"]
            graph.node.insert(0, helper.make_node("DequantizeLinear", inputs, [initializer.name], axis=axis))
            replaced += 1
        onnx.checker.check_model(model)
        onnx.save(model, str(target))
        print(f"{name}: {paths.relative(target)} {target.stat().st_size / 1e6:.1f} MB, {replaced} int8", flush=True)


def session(path: Path, fold: bool = False) -> Any:
    """A one-thread CPU session as the engine opens it; `fold` also turns off ONNX Runtime's QDQ handling
    (session.disable_quant_qdq), so DequantizeLinear of a constant weight is folded once at load."""
    import onnxruntime as ort

    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    if fold:
        options.add_session_config_entry("session.disable_quant_qdq", "1")
    return ort.InferenceSession(str(path), options, providers=["CPUExecutionProvider"])


def reference(
    out: Path, name: str, vectors: list[dict[str, Any]], cases: list[dict[str, Any]], fold: bool = False
) -> None:
    """The ONNX log-probabilities of one method on the parity inputs, written to <out>/<name>/reference.json, with
    the one-thread seconds they took."""
    from zhuyintype_ml.training.reranker.export_tree import example

    reranker, v3 = output(out, name)
    context, syllable = session(v3 / "ContextEncoder.onnx", fold), session(v3 / "SyllableEncoder.onnx", fold)
    batch, tree = session(reranker / "Reranker.onnx", fold), session(reranker / "RerankerTree.onnx", fold)
    result: dict[str, Any] = {"method": name, "v3": [], "reranker": [], "tree": None, "seconds": {}, "folded": fold}

    def run_v3() -> list[dict[str, Any]]:
        rows = []
        for vector in vectors:
            values = {n: np.array(v, np.int32) for n, v in vector["inputs"].items()}
            ids = {"context_ids": values.pop("context_ids")}
            memory = dict(zip(["memory_k", "memory_v"], context.run(None, ids)))
            logprob = syllable.run(None, {**values, **memory})[0]
            rows.append({"id": vector["id"], "logprob": logprob.round(6).tolist()})
        return rows

    def run_reranker() -> list[dict[str, Any]]:
        rows = []
        for case in cases:
            inputs = {n: np.array(case[n], np.int32) for n in ["ids", "targets"]}
            rows.append({"sentences": case["sentences"], "logprob": batch.run(None, inputs)[0].tolist()})
        return rows

    # One-thread CPU seconds for all inputs, the fastest of five runs (a rough per-call cost on the host).
    for key, function in [("v3", run_v3), ("reranker", run_reranker)]:
        times = []
        for _ in range(5):
            start = time.perf_counter()
            result[key] = function()
            times.append(time.perf_counter() - start)
        result["seconds"][key] = round(min(times), 4)
    names = ["ids", "positions", "ancestors", "parents", "targets"]
    result["tree"] = tree.run(None, dict(zip(names, [x.numpy() for x in example()])))[0].tolist()
    target = out / name / ("reference-folded.json" if fold else "reference.json")
    target.write_text(json.dumps(result, ensure_ascii=False) + "\n")
    print(f"{name}: one-thread seconds {result['seconds']}{' (QDQ folded)' if fold else ''}", flush=True)


def parity_inputs(source_reranker: Path, source_v3: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """The v3 test vectors' inputs (checkpoints asset) and the reranker parity sentences, with only the inputs kept."""
    from zhuyintype_ml.remote.parity import real_reranker

    vectors = json.loads((source_v3 / "test_vectors.json").read_text())["vectors"]
    vectors = [{"id": v["id"], "inputs": v["inputs"]} for v in vectors]
    cases = [{k: c[k] for k in ["context", "sentences", "ids", "targets"]} for c in real_reranker(source_reranker)]
    return vectors, cases


def no_reranker(out: Path, data: Path) -> None:
    """<out>/no-reranker: links to the data directory's lexicon, language models and v3, without the reranker."""
    target = out / "no-reranker"
    target.mkdir(parents=True, exist_ok=True)
    for name in ["lexicon.tsv", "lexicon.bin", "lexicon.json", "lm", "lm_chat", "v3"]:
        link = target / name
        if link.is_symlink():
            link.unlink()
        link.symlink_to(os.path.relpath(data / name, target))


def sizes(out: Path, names: list[str]) -> dict[str, dict[str, int]]:
    result = {}
    for name in names:
        reranker, v3 = output(out, name)
        files = [reranker / f for f in ONNX_FILES["reranker"]] + [v3 / f for f in ONNX_FILES["v3"]]
        result[name] = {f.name: f.stat().st_size for f in files if f.exists()}
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=paths.data("compress"))
    parser.add_argument("--reranker", type=Path, default=paths.data("reranker"), help="source reranker (inputs)")
    parser.add_argument("--v3", type=Path, default=paths.data("v3"), help="source v3 (test vectors)")
    parser.add_argument("--method", nargs="*", default=[m.name for m in METHODS], help="methods to export")
    parser.add_argument(
        "--onnx", nargs="*", default=list(ONNX_METHODS), help="ONNX Runtime methods (their source must be exported)"
    )
    args = parser.parse_args(argv)
    import torch

    torch.set_num_threads(1)
    for name in args.method:
        export(*output(args.out, name))
    for name in args.onnx:
        if name == "onnx-int8-weights":
            weight_only(args.out, name)
        else:
            dynamic(args.out, name, per_channel=name == "onnx-int8")
    names = args.method + args.onnx
    vectors, cases = parity_inputs(args.reranker, args.v3)
    for name in names:
        reference(args.out, name, vectors, cases)
        if name == "onnx-int8-weights":
            reference(args.out, name, vectors, cases, fold=True)
    no_reranker(args.out, paths.data_dir())
    record = args.out / "onnx.json"
    previous = json.loads(record.read_text()) if record.exists() else {}
    record.write_text(json.dumps({**previous, **sizes(args.out, names)}, indent=2) + "\n")
    print(json.dumps(sizes(args.out, names), indent=2), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
