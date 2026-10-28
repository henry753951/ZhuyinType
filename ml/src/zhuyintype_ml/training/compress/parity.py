"""The one-off check on the Mac: compressed Core ML outputs against the GPU host's fake-quantized ONNX outputs.

    zt train compress parity --option methods=int8-channel,pal6
    python -m zhuyintype_ml.training.compress.parity --out data/build/compress --method pal6

Inputs are those of <out>/<method>/reference.json (from `zt train compress onnx` on the host: the v3 test vectors, the
reranker parity sentences and a small prefix tree).  For each method, on the CPU:

- stored weights: every weight of the compressed packages, decompressed, against the uncompressed export of the same
  checkpoint (coreml.py).  0 means Core ML holds exactly the values the host evaluated;
- uncompressed: the outputs of the compressed packages against those of the uncompressed export.  Core ML may run
  compressed weights with other kernels, so these can differ by about as much as fp16 and fp32 do;
- host: the compressed packages against the host's ONNX outputs of the same weights (fp32 there, fp16 here), beside
  the same comparison for the shipped models (`current`, against the fp16 method's reference), the usual Mac/Linux
  difference.

Reported per model: the largest absolute difference of a log-probability the engine reads (valid candidates, real
targets, tree edges) and, for v3, how often the top candidate agrees.  The report goes to <out>/parity-mac.json.
Each method's v3/test_vectors.json is rewritten with its compressed Core ML outputs (the golden vectors of
`--check-v3` for that model directory; the shipped ones belong to the fp16 models).
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import coremltools as ct
import numpy as np

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.v3.coreml import require_macos

from .methods import output

TREE = {
    "ids": [[1, 3, 4, 5]],
    "positions": [[0, 1, 2, 2]],
    "ancestors": [[1, 0, 0, 0], [1, 1, 0, 0], [1, 1, 1, 0], [1, 1, 0, 1]],
    "parents": [0, 0, 1, 1, 2, 3],
    "targets": [1, 3, 4, 5, 6, 7],
}  # reranker/export_tree.py example()


def model(path: Path) -> Any:
    if path.suffix == ".mlmodelc":
        return ct.models.CompiledMLModel(str(path), compute_units=ct.ComputeUnit.CPU_ONLY)
    return ct.models.MLModel(str(path), compute_units=ct.ComputeUnit.CPU_ONLY)


def outputs(reranker: Path, v3: Path, suffix: str, reference: dict[str, Any]) -> dict[str, Any]:
    """The Core ML outputs of one model set (`suffix` .mlmodelc or .uncompressed.mlpackage) on the reference inputs."""
    context, syllable = model(v3 / f"ContextEncoder{suffix}"), model(v3 / f"SyllableEncoder{suffix}")
    batch, tree = model(reranker / f"Reranker{suffix}"), model(reranker / f"RerankerTree{suffix}")
    vectors = json.loads((paths.data("v3") / "test_vectors.json").read_text())["vectors"]
    inputs = {v["id"]: v["inputs"] for v in vectors}
    result: dict[str, Any] = {"v3": [], "reranker": [], "tree": None}
    for row in reference["v3"]:
        values = {n: np.array(v, np.int32) for n, v in inputs[row["id"]].items()}
        memory = context.predict({"context_ids": values.pop("context_ids")})
        logprob = syllable.predict({**values, "memory_k": memory["memory_k"], "memory_v": memory["memory_v"]})
        result["v3"].append(
            {
                "id": row["id"],
                "inputs": inputs[row["id"]],
                "logprob": logprob["logprob"],
                "candidates": values["candidate_ids"],
                "memory": {n: memory[n] for n in ["memory_k", "memory_v"]},
            }
        )
    sources = {tuple(c["sentences"]): c for c in cases()}
    for case in reference["reranker"]:
        source = sources[tuple(case["sentences"])]
        got = batch.predict({n: np.array(source[n], np.int32) for n in ["ids", "targets"]})["logprob"]
        # Padding targets (id 0) are never read by the engine.
        result["reranker"].append({"logprob": got, "valid": np.array(source["targets"]) != 0})
    result["tree"] = tree.predict({n: np.array(v, np.int32) for n, v in TREE.items()})["logprob"]
    return result


def cases() -> list[dict[str, Any]]:
    from zhuyintype_ml.remote.parity import real_reranker

    return real_reranker(paths.data("reranker"))


def differences(got: dict[str, Any], expected: dict[str, Any]) -> dict[str, float]:
    """Largest absolute difference of the log-probabilities the engine reads, and top-1 agreement of v3."""
    v3, agree, total = 0.0, 0, 0
    for a, b in zip(got["v3"], expected["v3"]):
        valid = a["candidates"] != 0
        left, right = np.asarray(a["logprob"], np.float64), np.asarray(b["logprob"], np.float64)
        v3 = max(v3, float(np.max(np.abs(left[valid] - right[valid]))))
        rows = valid.any(-1)
        agree += int(np.sum(left.argmax(-1)[rows] == right.argmax(-1)[rows]))
        total += int(rows.sum())
    reranker = max(
        float(np.max(np.abs(np.asarray(a["logprob"], np.float64) - np.asarray(b["logprob"]))[a["valid"]]))
        for a, b in zip(got["reranker"], expected["reranker"])
    )
    # The first edge scores the root against itself and is ignored (reranker/export_tree.py).
    tree = float(np.max(np.abs(np.asarray(got["tree"], np.float64) - np.asarray(expected["tree"]))[1:]))
    return {
        "v3_max_abs": v3,
        "v3_top1_agreement": agree / max(total, 1),
        "reranker_max_abs": reranker,
        "tree_max_abs": tree,
    }


def golden_vectors(v3: Path, got: dict[str, Any]) -> None:
    """<v3>/test_vectors.json for the compressed models: the v3 test vectors' inputs with these Core ML outputs, in
    v3/validate.py's format, so `EngineBench --check-v3` and `zhuyin-cli --check-v3` check this model directory."""
    from zhuyintype_ml.training.v3.validate import tensor

    vectors = [
        {
            "id": row["id"],
            "inputs": row["inputs"],
            "expected": {
                **{n: tensor(np.asarray(v, np.float16)) for n, v in row["memory"].items()},
                "logprob": np.asarray(row["logprob"]).tolist(),
            },
        }
        for row in got["v3"]
    ]
    record = {
        "version": 1,
        "backend": "Core ML CPU_ONLY",
        "vectors": vectors,
        "memory_format": "base64 little-endian contiguous float16 in C order; shape and SHA256 provided",
        "comparison": {"coreml_logprob_atol": 0.002, "coreml_logprob_rtol": 0.002},
    }
    (v3 / "test_vectors.json").write_text(json.dumps(record) + "\n")


def stored_weights(reranker: Path, v3: Path) -> dict[str, float]:
    """For each package, the largest difference between its decompressed weights and the uncompressed export's:
    0 means Core ML stores exactly the values the host evaluated."""
    import coremltools.optimize.coreml as cto

    result = {}
    for directory, name in [(reranker, n) for n in ["Reranker", "RerankerTree"]] + [
        (v3, n) for n in ["ContextEncoder", "SyllableEncoder"]
    ]:
        plain = ct.models.MLModel(str(directory / f"{name}.uncompressed.mlpackage"), skip_model_load=True)
        packed = ct.models.MLModel(str(directory / f"{name}.mlpackage"), skip_model_load=True)
        stored = [m.val for m in cto.get_weights_metadata(cto.decompress_weights(packed), 65_535).values()]
        worst = 0.0
        for value in (m.val for m in cto.get_weights_metadata(plain, 65_535).values()):
            same = [s for s in stored if s.shape == value.shape]
            difference = min((float(np.max(np.abs(s.astype(np.float32) - value))) for s in same), default=np.inf)
            worst = max(worst, difference)
        result[name] = worst
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=paths.data("compress"))
    parser.add_argument("--method", nargs="+", required=True)
    args = parser.parse_args(argv)
    require_macos("the Core ML parity check")
    report: dict[str, Any] = {}
    fp16 = json.loads((args.out / "fp16" / "reference.json").read_text())
    shipped = outputs(paths.data("reranker"), paths.data("v3"), ".mlmodelc", fp16)
    report["current"] = {"host": differences(shipped, fp16)}
    for name in args.method:
        reference = json.loads((args.out / name / "reference.json").read_text())
        reranker, v3 = output(args.out, name)
        compressed = outputs(reranker, v3, ".mlmodelc", reference)
        golden_vectors(v3, compressed)
        entry: dict[str, Any] = {"host": differences(compressed, reference)}
        if (v3 / "ContextEncoder.uncompressed.mlpackage").exists():
            entry["stored_weights_max_abs"] = stored_weights(reranker, v3)
            uncompressed = outputs(reranker, v3, ".uncompressed.mlpackage", reference)
            entry["uncompressed"] = differences(compressed, uncompressed)
        report[name] = entry
        print(name, json.dumps(entry), flush=True)
    record = args.out / "parity-mac.json"
    previous = json.loads(record.read_text()) if record.exists() else {}
    record.write_text(json.dumps({**previous, **report}, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
