"""Fake-quantized checkpoints: the reranker and v3 weights replaced by exactly the values Core ML will hold.

    zt train compress quantize                                      # every method, data/build/{reranker,v3}
    python -m zhuyintype_ml.training.compress.quantize --out data/build/compress --method pal6 int8-channel \
        --reranker data/build/reranker --v3 v3_new

For each method, writes <out>/<method>/reranker/charlm.pt and <out>/<method>/v3/syllable.pt with every large weight
matrix (methods.py) compressed and decompressed again, stored as fp16 (all values are fp16 numbers), beside copies of
the JSON files the engine reads.  Any PyTorch or ONNX tool loads them as ordinary checkpoints, so the GPU host
evaluates exactly the weights the compressed Core ML models decompress to:

- linear: symmetric int8/int4 as coremltools computes it on the fp16 weight (scale = 2·max|w| / (2^bits − 2) rounded
  to fp16, q = round(w / scale) in fp16, value = fp16(q · scale)), per row or per block of columns;
- palette: k-means (Lloyd's algorithm in float64 from quantile starts) on the fp16 values of each matrix or group of
  rows, the table rounded to fp16, each value replaced by its nearest table entry.  The Core ML export then stores the
  matrix with coremltools' "unique" palettization, which is lossless for these values.

The row of a weight is its output channel (nn.Linear) or its character (nn.Embedding), the axis coremltools picks
for linear, matmul and gather.  <out>/<method>/compression.json records every matrix: shape, method, error, unique
values, SHA-256 of the fp16 values, and the estimated Core ML weight bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path
from typing import Any

import torch

from zhuyintype_ml.core import paths

from .methods import METHODS, MINIMUM_VALUES, Method, method, output

V3_FILES = ["chars.json", "syllables.json", "config.json", "NOTICE.md", "attribution.jsonl", "source-manifest.json"]
RERANKER_FILES = ["vocab.json", "config.json"]


def fp16(x: torch.Tensor) -> torch.Tensor:
    """Rounded to the nearest fp16 value, kept in the input's dtype."""
    return x.to(torch.float16).to(x.dtype)


def linear(weight: torch.Tensor, bits: int, block: int) -> torch.Tensor:
    """Symmetric linear quantization of a [rows, columns] matrix as coremltools does it on fp16 weights
    (optimize/_utils.py quantize_weight_by_dtype: numpy float16 arithmetic, each operation rounded to fp16)."""
    w = fp16(weight.float())
    rows, columns = w.shape
    blocks = w.reshape(rows, columns // block, block) if block else w.reshape(rows, 1, columns)
    q_max = 2 ** (bits - 1) - 1
    max_abs = blocks.abs().amax(-1, keepdim=True)
    scale = fp16(fp16(2 * max_abs) / (2 * q_max))
    safe = torch.where(scale > 0, scale, torch.ones_like(scale))
    q = torch.round(fp16(blocks / safe)).clamp(-q_max, q_max)
    q = torch.where(scale > 0, q, torch.zeros_like(q))
    return fp16(q * scale).reshape(rows, columns)


def kmeans(values: torch.Tensor, k: int, iterations: int = 300) -> torch.Tensor:
    """One-dimensional k-means of each row of `values` [groups, n] (float64): Lloyd's algorithm on the sorted values,
    started at the quantiles.  Returns the centroids [groups, k], ascending."""
    ordered, _ = values.sort(-1)
    groups, n = ordered.shape
    sums = torch.cat([torch.zeros(groups, 1, dtype=ordered.dtype), ordered.cumsum(-1)], -1)
    starts = ((torch.arange(k, dtype=torch.float64) + 0.5) / k * n).long().clamp(0, n - 1)
    centroids = ordered[:, starts]
    for _ in range(iterations):
        bounds = (centroids[:, 1:] + centroids[:, :-1]) / 2
        cuts = torch.searchsorted(ordered, bounds.contiguous())
        edges = torch.cat([torch.zeros(groups, 1, dtype=torch.long), cuts, torch.full((groups, 1), n)], -1)
        counts = edges[:, 1:] - edges[:, :-1]
        totals = sums.gather(1, edges[:, 1:]) - sums.gather(1, edges[:, :-1])
        updated = torch.where(counts > 0, totals / counts.clamp(min=1), centroids)
        updated, _ = updated.sort(-1)
        if torch.equal(updated, centroids):
            break
        centroids = updated
    return centroids


def nearest(values: torch.Tensor, table: torch.Tensor) -> torch.Tensor:
    """Each value of `values` [groups, n] replaced by the nearest entry of its group's ascending `table` [groups, k]."""
    bounds = (table[:, 1:] + table[:, :-1]) / 2
    index = torch.searchsorted(bounds.contiguous(), values.contiguous())
    return table.gather(1, index)


def palette_axis(shape: tuple[int, ...], group: int) -> int | None:
    """The axis whose groups of `group` channels share a table: rows when they divide, else columns, else None."""
    if not group:
        return None
    if shape[0] % group == 0:
        return 0
    if shape[1] % group == 0:
        return 1
    return None


def palette(weight: torch.Tensor, bits: int, group: int) -> torch.Tensor:
    """Palettization of a [rows, columns] matrix: one fp16 table of 2^bits values per matrix (group 0) or per `group`
    rows (columns when the rows do not divide by `group`)."""
    w = fp16(weight.double())
    axis = palette_axis(tuple(w.shape), group)
    oriented = w if axis != 1 else w.T
    rows, columns = oriented.shape
    size = group if axis is not None else rows
    values = oriented.reshape(rows // size, size * columns)
    table = fp16(kmeans(values, 2**bits))
    result = nearest(values, table).reshape(rows, columns)
    return (result if axis != 1 else result.T).float()


def compressible(name: str, tensor: torch.Tensor) -> bool:
    return (
        name.endswith(".weight")
        and tensor.is_floating_point()
        and tensor.dim() == 2
        and tensor.numel() >= MINIMUM_VALUES
        and min(tensor.shape) >= 16
    )


def compress(weight: torch.Tensor, m: Method) -> torch.Tensor:
    if m.kind == "linear":
        return linear(weight, m.bits, m.size)
    if m.kind == "palette":
        return palette(weight, m.bits, m.size)
    return fp16(weight.float())


def stored_bytes(shape: tuple[int, ...], m: Method) -> int:
    """Core ML weight bytes of one compressed matrix (data, scales or tables in fp16)."""
    rows, columns = shape
    values = rows * columns
    if m.kind == "linear":
        scales = rows * (columns // m.size if m.size else 1)
        return values * m.bits // 8 + 2 * scales
    if m.kind == "palette":
        axis = palette_axis(shape, m.size)
        tables = 1 if axis is None else shape[axis] // m.size
        return (values * m.bits + 7) // 8 + 2 * tables * 2**m.bits
    return 2 * values


def digest(tensor: torch.Tensor) -> str:
    return hashlib.sha256(tensor.to(torch.float16).contiguous().numpy().tobytes()).hexdigest()


def state(source: dict[str, torch.Tensor], m: Method, prefix: str, record: list[dict[str, Any]]) -> dict[str, Any]:
    """The state dict with every compressible matrix compressed, every tensor stored as fp16."""
    result: dict[str, Any] = {}
    for name, tensor in source.items():
        if not tensor.is_floating_point():
            result[name] = tensor
            continue
        if compressible(name, tensor):
            original = tensor.float()
            value = compress(original, m)
            error = (value - fp16(original)).float()
            record.append(
                {
                    "name": f"{prefix}{name}",
                    "shape": list(tensor.shape),
                    "compressed": True,
                    "unique": int(value.unique().numel()) if m.kind == "palette" else None,
                    "palette_axis": palette_axis(tuple(tensor.shape), m.size) if m.kind == "palette" else None,
                    "max_abs_error": float(error.abs().max()),
                    "relative_rms_error": float(error.norm() / fp16(original).norm().clamp(min=1e-12)),
                    "fp16_bytes": 2 * tensor.numel(),
                    "stored_bytes": stored_bytes(tuple(tensor.shape), m),
                    "sha256": digest(value),
                }
            )
            result[name] = value.to(torch.float16)
        else:
            record.append(
                {
                    "name": f"{prefix}{name}",
                    "shape": list(tensor.shape),
                    "compressed": False,
                    "fp16_bytes": 2 * tensor.numel(),
                    "stored_bytes": 2 * tensor.numel(),
                }
            )
            result[name] = tensor.to(torch.float16)
    return result


def copy(source: Path, target: Path, names: list[str]) -> None:
    target.mkdir(parents=True, exist_ok=True)
    for name in names:
        path = source / name
        if path.is_file():
            shutil.copy2(path, target / name)
    licenses = source / "Licenses"
    if licenses.is_dir() and target.name == "v3":
        shutil.copytree(licenses, target / "Licenses", dirs_exist_ok=True)


def quantize(m: Method, reranker: Path, v3: Path, out: Path) -> dict[str, Any]:
    reranker_out, v3_out = output(out, m.name)
    copy(reranker, reranker_out, RERANKER_FILES)
    copy(v3, v3_out, V3_FILES)
    records: dict[str, list[dict[str, Any]]] = {"reranker": [], "v3": []}
    saved = torch.load(reranker / "charlm.pt", map_location="cpu", weights_only=False)
    saved = dict(saved, model=state(saved["model"], m, "", records["reranker"]))
    torch.save(saved, reranker_out / "charlm.pt")
    checkpoint = torch.load(v3 / "syllable.pt", map_location="cpu", weights_only=False)
    checkpoint = dict(
        checkpoint,
        context=state(checkpoint["context"], m, "context.", records["v3"]),
        syllable=state(checkpoint["syllable"], m, "syllable.", records["v3"]),
    )
    torch.save(checkpoint, v3_out / "syllable.pt")
    # The context-stage checkpoint is not part of any export; the ONNX export only records its hash.
    context = v3_out / "context.pt"
    if context.is_symlink() or context.exists():
        context.unlink()
    context.symlink_to((v3 / "context.pt").resolve())
    summary = {
        "method": m.name,
        "kind": m.kind,
        "bits": m.bits,
        "granularity": m.granularity,
        "size": m.size,
        "minimum_macos": m.macos,
        "sources": {
            "reranker": paths.relative(reranker / "charlm.pt"),
            "v3": paths.relative(v3 / "syllable.pt"),
            "reranker_sha256": sha256(reranker / "charlm.pt"),
            "v3_sha256": sha256(v3 / "syllable.pt"),
        },
        # The Core ML ContextEncoder holds context.* and the syllable encoder's cross K/V; SyllableEncoder the rest.
        "estimated_bytes": {
            model: {"fp16": sum(r["fp16_bytes"] for r in rows), "stored": sum(r["stored_bytes"] for r in rows)}
            for model, rows in records.items()
        },
        "tensors": records,
    }
    record = out / m.name / "compression.json"
    check(record, summary)
    record.write_text(json.dumps(summary, indent=2) + "\n")
    return summary


def check(record: Path, summary: dict[str, Any]) -> None:
    """When `record` holds an earlier run from the same sources (e.g. the GPU host's, pulled to the Mac, so that the
    Core ML export needs no 200 MB checkpoint transfer), every compressed matrix must hash the same."""
    if not record.exists():
        return
    previous = json.loads(record.read_text())
    keys = ["reranker_sha256", "v3_sha256"]
    if [previous["sources"].get(k) for k in keys] != [summary["sources"][k] for k in keys]:
        return
    hashes = {r["name"]: r.get("sha256") for rows in previous["tensors"].values() for r in rows}
    current = {r["name"]: r.get("sha256") for rows in summary["tensors"].values() for r in rows}
    different = sorted(n for n in current if hashes.get(n) != current[n])
    if different:
        raise SystemExit(f"zt: {summary['method']}: {len(different)} matrices differ from {record}: {different[:5]}")
    print(f"{summary['method']}: all {len(current)} tensors match the earlier record", flush=True)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reranker", type=Path, default=paths.data("reranker"), help="directory with charlm.pt")
    parser.add_argument("--v3", type=Path, default=paths.data("v3"), help="directory with syllable.pt")
    parser.add_argument("--out", type=Path, default=paths.data("compress"))
    parser.add_argument("--method", nargs="*", default=[m.name for m in METHODS])
    args = parser.parse_args(argv)
    torch.set_num_threads(8)
    for name in args.method:
        summary = quantize(method(name), args.reranker, args.v3, args.out)
        sizes = ", ".join(
            f"{model} {b['stored'] / 1e6:.1f} MB (fp16 {b['fp16'] / 1e6:.1f})"
            for model, b in summary["estimated_bytes"].items()
        )
        print(f"{name}: {sizes}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
