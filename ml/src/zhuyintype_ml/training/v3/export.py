"""Export a v3 checkpoint to two stateless Core ML packages (macOS 14, fp16) and config.json.

    zt train v3 export
    python -m zhuyintype_ml.training.v3.export [--checkpoint v3_030/syllable.pt] [--out v3_030]

ContextEncoder.mlpackage maps context_ids [1, Lc<=256] to memory_k/memory_v [6, Lc, 512] float16, already projected
by every syllable layer's cross K/V weights.  SyllableEncoder.mlpackage scores candidate_ids [1, Ls<=64, C<=64] and
returns their log-softmax.  config.json records the dimensions, ids and the calling contract.  Needs coremltools
(the export dependency group); compile with `zt train v3 compile` on macOS.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

import coremltools as ct
import numpy as np
import torch

from .common import OUT, digest, save
from .models import FFN, HEADS, LAYERS, WIDTH, ContextExport, SyllableExport, load_models

NAMES = ["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids", "memory_k", "memory_v"]


def export(checkpoint: Path, out: Path, target: Any = ct.target.macOS14) -> None:
    """Writes the two packages and config.json; `target` is the minimum macOS (compressed weights in blocks or
    grouped palettes need macOS 15, ml/src/zhuyintype_ml/training/compress)."""
    torch.set_num_threads(4)
    out.mkdir(exist_ok=True, parents=True)
    context, syllable = load_models(checkpoint)
    wrapper = ContextExport(context, syllable).eval()
    sample = torch.ones(1, 32, dtype=torch.int32)
    traced = torch.jit.trace(wrapper, (sample,), check_trace=False)
    lc = ct.RangeDim(1, 256, default=128, symbol="context_length")
    ml = ct.convert(
        traced,
        inputs=[ct.TensorType(name="context_ids", shape=(1, lc), dtype=np.int32)],
        outputs=[ct.TensorType(name="memory_k", dtype=np.float16), ct.TensorType(name="memory_v", dtype=np.float16)],
        minimum_deployment_target=target,
        compute_precision=ct.precision.FLOAT16,
        convert_to="mlprogram",
        skip_model_load=True,
    )
    ml.short_description = "ZhuyinType v3: causal context with syllable-layer cross-attention K/V projections"
    ml.license = "CC BY-SA 4.0"
    ml.save(str(out / "ContextEncoder.mlpackage"))
    ls = ct.RangeDim(1, 64, default=30, symbol="syllable_length")
    count = ct.RangeDim(1, 64, default=32, symbol="candidate_count")
    memory_shape = (LAYERS, lc, WIDTH)
    inputs = [ct.TensorType(name=n, shape=(1, ls), dtype=np.int32) for n in NAMES[:4]]
    inputs += [ct.TensorType(name="candidate_ids", shape=(1, ls, count), dtype=np.int32)]
    inputs += [ct.TensorType(name=n, shape=memory_shape, dtype=np.float16) for n in NAMES[5:]]
    x = torch.ones(1, 10, dtype=torch.int32)
    candidates = torch.ones(1, 10, 16, dtype=torch.int32) * 4
    with torch.no_grad():
        k, v = wrapper(sample)
    example = (x, x, x * 0, x * 0, candidates, k, v)
    traced = torch.jit.trace(SyllableExport(syllable).eval(), example, check_trace=False)
    ml = ct.convert(
        traced,
        inputs=inputs,
        outputs=[ct.TensorType(name="logprob", dtype=np.float32)],
        minimum_deployment_target=target,
        compute_precision=ct.precision.FLOAT16,
        convert_to="mlprogram",
        skip_model_load=True,
    )
    ml.short_description = "ZhuyinType v3: bidirectional syllables, projected context K/V, candidate-only log-softmax"
    ml.license = "CC BY-SA 4.0"
    ml.save(str(out / "SyllableEncoder.mlpackage"))
    save(
        out / "config.json",
        {
            "version": 3,
            "minimum_macos": "15.0" if target == ct.target.macOS15 else "14.0",
            "format": "mlprogram",
            "precision": "float16",
            "layers": LAYERS,
            "width": WIDTH,
            "heads": HEADS,
            "head_dim": WIDTH // HEADS,
            "ffn": FFN,
            "rope_base": 10000,
            "layer_norm_epsilon": 1e-5,
            "vocab_size": context.embed.num_embeddings,
            "syllable_vocab_size": syllable.syllable.num_embeddings,
            "max_context": 256,
            "max_syllables": 64,
            "max_candidates": 64,
            "char_ids": {"pad": 0, "bos": 1, "unk": 2, "sep": 3},
            "syllable_ids": {"pad": 0, "literal": 1},
            "tone_ids": {"none": 0, "first": 1, "ˊ": 2, "ˇ": 3, "ˋ": 4, "˙": 5},
            "cost_bucket": {"exact": 0, "small": 1, "medium": 2, "large": 3},
            "boundary": {"explicit": 0, "implicit": 1},
            "context_model": "ContextEncoder.mlpackage",
            "syllable_model": "SyllableEncoder.mlpackage",
            "context_compiled": "ContextEncoder.mlmodelc",
            "syllable_compiled": "SyllableEncoder.mlmodelc",
            "chars": "chars.json",
            "syllables": "syllables.json",
            "test_vectors": "test_vectors.json",
            "checkpoint": checkpoint.name,
            "checkpoint_sha256": digest(checkpoint),
            "inputs": {
                "context_ids": {"dtype": "int32", "shape": [1, "Lc"]},
                **{n: {"dtype": "int32", "shape": [1, "Ls"]} for n in NAMES[:4]},
                "candidate_ids": {"dtype": "int32", "shape": [1, "Ls", "C"]},
                **{n: {"dtype": "float16", "shape": [6, "Lc", 512]} for n in NAMES[5:]},
            },
            "outputs": {
                "memory_k": {"dtype": "float16", "shape": [6, "Lc", 512]},
                "memory_v": {"dtype": "float16", "shape": [6, "Lc", 512]},
                "logprob": {"dtype": "float32", "shape": [1, "Ls", "C"]},
            },
            "contract": [
                "context_ids begins with BOS; retain at most 255 preceding characters",
                (
                    "Map paragraph newlines in context text to SEP character id 3; never concatenate paragraphs "
                    "without a "
                    "separator"
                ),
                "Each syllable position has at least one non-pad candidate; unknown characters share UNK id 2",
                "Memory is projected using the final syllable-layer weights; always use matching model packages",
                (
                    "All Ls dimensions agree, both memory Lc dimensions agree; RangeDim alone does not enforce this at "
                    "runtime"
                ),
                (
                    "Pad candidates are 0 and have log-probability near -10000; caller supplies and prunes "
                    "candidates to "
                    "<=64"
                ),
                "Tones are separate from syllable strings; literals use syllable id 1 and tone 0",
            ],
        },
    )
    print("exported", out, flush=True)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--checkpoint", type=Path, default=OUT / "syllable.pt")
    p.add_argument("--out", type=Path, default=OUT)
    a = p.parse_args(argv)
    export(a.checkpoint, a.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
