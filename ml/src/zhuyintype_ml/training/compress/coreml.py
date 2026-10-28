"""Compressed Core ML models of a method (macOS): export, compress, compile, and check that nothing else changed.

    zt train compress coreml --option methods=int8-channel,pal6
    python -m zhuyintype_ml.training.compress.coreml --out data/build/compress --method pal6

For each method (after `quantize`, with <out>/<method>/ pulled from the GPU host):

1. The fake-quantized checkpoints are exported with the usual exporters (v3/export.py, reranker/export.py,
   reranker/export_tree.py; macOS 15 for the methods that need it).  Rounded to fp16, every weight equals the value
   the host evaluated.
2. Every weight constant of 65,536 values or more is matched, value for value, with a compressed matrix of the
   checkpoint (as stored or transposed) and compressed with coremltools.optimize.coreml: linear methods with
   linear_quantize_weights (symmetric, per channel or per block; coremltools recomputes the same scales from these
   values), palettes with palettize_weights in "unique" mode (the table is the matrix's own 2^bits values).  A
   constant that matches no matrix stays fp16 and is listed.
3. The uncompressed export is kept as <Name>.uncompressed.mlpackage for `parity`, which checks that compressing
   changed no output; the compressed package is compiled to <Name>.mlmodelc beside it.

<out>/<method>/coreml.json records the matched constants, package and compiled sizes, and load times.
"""

from __future__ import annotations

import argparse
import json
import shutil
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any

import coremltools as ct
import coremltools.optimize.coreml as cto
import numpy as np
import torch

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.reranker import export as reranker_export
from zhuyintype_ml.training.reranker import export_tree
from zhuyintype_ml.training.v3 import export as v3_export
from zhuyintype_ml.training.v3.coreml import compile_package, require_macos

from .methods import MINIMUM_VALUES, Method, method, output
from .quantize import RERANKER_FILES, V3_FILES, compressible, copy

PACKAGES = {"reranker": ["Reranker", "RerankerTree"], "v3": ["ContextEncoder", "SyllableEncoder"]}
TARGETS = {"14.0": "macOS14", "15.0": "macOS15"}


def checkpoint_matrices(reranker: Path, v3: Path) -> dict[str, np.ndarray]:
    """The compressed matrices of the fake-quantized checkpoints, as fp16 arrays by name."""
    matrices: dict[str, np.ndarray] = {}
    saved = torch.load(reranker / "charlm.pt", map_location="cpu", weights_only=False)
    for name, tensor in saved["model"].items():
        if compressible(name, tensor):
            matrices[f"{name}"] = tensor.to(torch.float16).numpy()
    checkpoint = torch.load(v3 / "syllable.pt", map_location="cpu", weights_only=False)
    for part in ["context", "syllable"]:
        for name, tensor in checkpoint[part].items():
            if compressible(name, tensor):
                matrices[f"{part}.{name}"] = tensor.to(torch.float16).numpy()
    return matrices


def match(value: np.ndarray, matrices: dict[str, np.ndarray]) -> tuple[str, bool] | None:
    """The checkpoint matrix equal to a Core ML constant, and whether the constant holds it transposed."""
    value = value.astype(np.float16)
    if value.ndim != 2:
        return None
    for name, matrix in matrices.items():
        if matrix.shape == value.shape and np.array_equal(matrix, value):
            return name, False
        if matrix.shape == value.shape[::-1] and np.array_equal(matrix.T, value):
            return name, True
    return None


def palette_axis(record: dict[str, Any] | None) -> int | None:
    return None if record is None else record.get("palette_axis")


def op_config(m: Method, axis: int | None, transposed: bool) -> Any:
    """The coremltools config that stores exactly the checkpoint's values."""
    if m.kind == "palette":
        if m.size and axis is not None:
            return cto.OpPalettizerConfig(
                mode="unique",
                granularity="per_grouped_channel",
                group_size=m.size,
                channel_axis=axis if not transposed else 1 - axis,
            )
        return cto.OpPalettizerConfig(mode="unique", granularity="per_tensor")
    dtype = "int8" if m.bits == 8 else "int4"
    if m.size:
        block = [1, m.size] if not transposed else [m.size, 1]
        return cto.OpLinearQuantizerConfig(
            mode="linear_symmetric", dtype=dtype, granularity="per_block", block_size=block
        )
    return cto.OpLinearQuantizerConfig(mode="linear_symmetric", dtype=dtype, granularity="per_channel")


def compress_package(
    package: Path, m: Method, matrices: dict[str, np.ndarray], records: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    """Compresses one exported package in place, keeping the export as <Name>.uncompressed.mlpackage."""
    kept = package.with_name(package.stem + ".uncompressed.mlpackage")
    if kept.exists():
        shutil.rmtree(kept)
    package.rename(kept)
    model = ct.models.MLModel(str(kept), skip_model_load=True)
    metadata = cto.get_weights_metadata(model, weight_threshold=MINIMUM_VALUES - 1)
    configs: dict[str, Any] = {}
    matched, unmatched = [], []
    for const, meta in metadata.items():
        found = match(meta.val, matrices)
        if found is None:
            unmatched.append({"const": const, "shape": list(meta.val.shape)})
            continue
        name, transposed = found
        configs[const] = op_config(m, palette_axis(records.get(name)), transposed)
        matched.append({"const": const, "matrix": name, "transposed": transposed})
    config = cto.OptimizationConfig(op_name_configs=configs)
    if m.kind == "palette":
        compressed = cto.palettize_weights(model, config)
    else:
        compressed = cto.linear_quantize_weights(model, config)
    compressed.save(str(package))
    return {"matched": matched, "unmatched": unmatched}


def size(path: Path) -> int:
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file()) if path.is_dir() else path.stat().st_size


LOAD = """
import sys, time
import coremltools as ct
start = time.perf_counter()
ct.models.CompiledMLModel(sys.argv[1], compute_units=ct.ComputeUnit.CPU_ONLY)
print(time.perf_counter() - start)
"""


def load_seconds(compiled: Path, repeats: int = 5) -> dict[str, float]:
    """Seconds to load a compiled model on the CPU, each time in a new process as the input method starts: the first
    load after compiling (Core ML prepares and caches the model) and the median of the next `repeats`."""
    times = []
    for _ in range(repeats + 1):
        done = subprocess.run([sys.executable, "-c", LOAD, str(compiled)], capture_output=True, text=True, check=True)
        times.append(float(done.stdout.strip().splitlines()[-1]))
    return {"first": round(times[0], 4), "median": round(statistics.median(times[1:]), 4)}


def export_method(m: Method, out: Path, source_reranker: Path, source_v3: Path) -> dict[str, Any]:
    reranker, v3 = output(out, m.name)
    # Only the checkpoints and records come from the host; the engine's small files are the source model's.
    copy(source_reranker, reranker, RERANKER_FILES)
    copy(source_v3, v3, V3_FILES)
    target = TARGETS[m.macos]
    config = json.loads((v3 / "config.json").read_text())
    v3_export.export(v3 / "syllable.pt", v3, target=getattr(ct.target, target))
    # Keep the shipped model's config (written by the pipeline that trained it), noting the compression.
    config.update({"minimum_macos": m.macos, "compression": m.name})
    (v3 / "config.json").write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n")
    reranker_export.main(["--data", str(reranker), "--target", target])
    export_tree.main(["--data", str(reranker), "--format", "coreml", "--target", target])
    report: dict[str, Any] = {"method": m.name, "minimum_macos": m.macos, "packages": {}}
    matrices = checkpoint_matrices(reranker, v3)
    summary = json.loads((out / m.name / "compression.json").read_text())
    records = {r["name"]: r for rows in summary["tensors"].values() for r in rows}
    for model, names in PACKAGES.items():
        directory = reranker if model == "reranker" else v3
        for name in names:
            package = directory / f"{name}.mlpackage"
            entry: dict[str, Any] = {}
            if m.kind != "fp16":
                entry.update(compress_package(package, m, matrices, records))
            compiled = directory / f"{name}.mlmodelc"
            if compiled.exists():
                shutil.rmtree(compiled)
            compile_package(package, directory)
            entry.update(
                package_bytes=size(package), compiled_bytes=size(compiled), load_seconds=load_seconds(compiled)
            )
            report["packages"][name] = entry
            print(f"{m.name} {name}: {entry['compiled_bytes'] / 1e6:.1f} MB, unmatched {entry.get('unmatched')}")
    report["compiled_bytes"] = {
        model: sum(report["packages"][n]["compiled_bytes"] for n in names) for model, names in PACKAGES.items()
    }
    (out / m.name / "coreml.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def current_report(data: Path) -> dict[str, Any]:
    """Sizes and load times of the shipped models in the data directory, for comparison."""
    report: dict[str, Any] = {"method": "current", "packages": {}}
    for model, names in PACKAGES.items():
        for name in names:
            compiled = data / model / f"{name}.mlmodelc"
            report["packages"][name] = {"compiled_bytes": size(compiled), "load_seconds": load_seconds(compiled)}
    report["compiled_bytes"] = {
        model: sum(report["packages"][n]["compiled_bytes"] for n in names) for model, names in PACKAGES.items()
    }
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=paths.data("compress"))
    parser.add_argument("--method", nargs="+", required=True)
    parser.add_argument("--reranker", type=Path, default=paths.data("reranker"), help="source reranker directory")
    parser.add_argument("--v3", type=Path, default=paths.data("v3"), help="source v3 directory")
    args = parser.parse_args(argv)
    require_macos("Core ML compression")
    torch.set_num_threads(4)
    reports = {"current": current_report(paths.data_dir())}
    for name in args.method:
        reports[name] = export_method(method(name), args.out, args.reranker, args.v3)
    record = args.out / "coreml.json"
    previous = json.loads(record.read_text()) if record.exists() else {}
    record.write_text(json.dumps({**previous, **reports}, indent=2) + "\n")
    for name, report in reports.items():
        sizes = ", ".join(f"{model} {b / 1e6:.1f} MB" for model, b in report["compiled_bytes"].items())
        print(f"{name}: {sizes}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
