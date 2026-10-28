"""The weight compression methods compared for the shipped models, and where each one's files go.

    from zhuyintype_ml.training.compress.methods import METHODS, method

A method names how every large weight matrix (nn.Linear and nn.Embedding, at least 65,536 values) is stored in Core
ML: rounded to fp16 (what the app ships), linear int8/int4 with a scale per row or per block of 32 columns, or a
palette (look-up table) of 2^bits values per matrix or per group of 16 rows.  Layer norms, the small embeddings and
the rotary tables stay fp16.  Methods marked macOS 15 use Core ML ops that macOS 14 (the app's minimum) lacks.

This module is standard library only, so `zt train compress` can list and plan without PyTorch.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Method:
    name: str
    kind: str  # "fp16", "linear" or "palette"
    bits: int = 16
    # linear: 0 = a scale per row, N = a scale per block of N columns in each row;
    # palette: 0 = one table per matrix, N = one table per group of N rows (or of N columns when rows do not divide)
    size: int = 0
    macos: str = "14.0"
    help: str = ""

    @property
    def granularity(self) -> str:
        if self.kind == "linear":
            return "per_block" if self.size else "per_channel"
        if self.kind == "palette":
            return "per_grouped_channel" if self.size else "per_tensor"
        return "none"


METHODS: tuple[Method, ...] = (
    Method("fp16", "fp16", help="every weight rounded to fp16, as the shipped Core ML models"),
    Method("int8-channel", "linear", 8, 0, help="symmetric int8, a scale per row"),
    Method("int8-block32", "linear", 8, 32, "15.0", "symmetric int8, a scale per 32 columns"),
    Method("int4-block32", "linear", 4, 32, "15.0", "symmetric int4, a scale per 32 columns"),
    Method("pal6", "palette", 6, help="64-value k-means table per matrix"),
    Method("pal4", "palette", 4, help="16-value k-means table per matrix"),
    Method("pal6-group16", "palette", 6, 16, "15.0", "64-value table per 16 rows"),
    Method("pal4-group16", "palette", 4, 16, "15.0", "16-value table per 16 rows"),
)

# ONNX Runtime models for Linux (onnx_models.py): dynamic int8 quantization of the fp16 method's models (weights int8
# per channel or per tensor, activations quantized at run time), and the int8-channel weights stored as int8 with a
# DequantizeLinear (fp32 activations, the same values as the Core ML int8-channel models).
ONNX_METHODS: tuple[str, ...] = ("onnx-int8", "onnx-int8-tensor", "onnx-int8-weights")

# Weight matrices with fewer values stay fp16 (layer norms, tone/cost/boundary embeddings, the per-character bias).
MINIMUM_VALUES = 65_536


def method(name: str) -> Method:
    for candidate in METHODS:
        if candidate.name == name:
            return candidate
    raise SystemExit(f"zt: unknown compression method {name!r}; known: {', '.join(m.name for m in METHODS)}")


def output(out: Path, name: str) -> tuple[Path, Path]:
    """The reranker and v3 directories of a method under the pipeline's output directory."""
    return out / name / "reranker", out / name / "v3"
