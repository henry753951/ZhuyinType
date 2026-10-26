"""Export stateless macOS 14 fp16 packages with bounded flexible dimensions."""
import argparse
import json
from pathlib import Path

import coremltools as ct
import numpy as np
import torch

from common import OUT, digest, save
from models import ContextExport, SyllableExport, load_models, LAYERS, WIDTH, HEADS, FFN


NAMES = ["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids", "memory_k", "memory_v"]


def export(checkpoint, out):
    torch.set_num_threads(4)
    out.mkdir(exist_ok=True, parents=True)
    context, syllable = load_models(checkpoint)
    wrapper = ContextExport(context, syllable).eval()
    sample = torch.ones(1, 32, dtype=torch.int32)
    traced = torch.jit.trace(wrapper, (sample,), check_trace=False)
    lc = ct.RangeDim(1, 256, default=128, symbol="context_length")
    ml = ct.convert(traced, inputs=[ct.TensorType(name="context_ids", shape=(1, lc), dtype=np.int32)],
                    outputs=[ct.TensorType(name="memory_k", dtype=np.float16), ct.TensorType(name="memory_v", dtype=np.float16)],
                    minimum_deployment_target=ct.target.macOS14, compute_precision=ct.precision.FLOAT16,
                    convert_to="mlprogram", skip_model_load=True)
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
    ml = ct.convert(traced, inputs=inputs, outputs=[ct.TensorType(name="logprob", dtype=np.float32)],
                    minimum_deployment_target=ct.target.macOS14, compute_precision=ct.precision.FLOAT16,
                    convert_to="mlprogram", skip_model_load=True)
    ml.short_description = "ZhuyinType v3: bidirectional syllables, projected context K/V, candidate-only log-softmax"
    ml.license = "CC BY-SA 4.0"
    ml.save(str(out / "SyllableEncoder.mlpackage"))
    save(out / "config.json", {"version": 3, "minimum_macos": "14.0", "format": "mlprogram", "precision": "float16",
         "layers": LAYERS, "width": WIDTH, "heads": HEADS, "head_dim": WIDTH // HEADS, "ffn": FFN,
         "rope_base": 10000, "layer_norm_epsilon": 1e-5, "vocab_size": context.embed.num_embeddings,
         "syllable_vocab_size": syllable.syllable.num_embeddings, "max_context": 256, "max_syllables": 64, "max_candidates": 64,
         "char_ids": {"pad": 0, "bos": 1, "unk": 2, "sep": 3}, "syllable_ids": {"pad": 0, "literal": 1},
         "tone_ids": {"none": 0, "first": 1, "ˊ": 2, "ˇ": 3, "ˋ": 4, "˙": 5},
         "cost_bucket": {"exact": 0, "small": 1, "medium": 2, "large": 3}, "boundary": {"explicit": 0, "implicit": 1},
         "context_model": "ContextEncoder.mlpackage", "syllable_model": "SyllableEncoder.mlpackage",
         "context_compiled": "ContextEncoder.mlmodelc", "syllable_compiled": "SyllableEncoder.mlmodelc",
         "chars": "chars.json", "syllables": "syllables.json", "test_vectors": "test_vectors.json",
         "checkpoint": checkpoint.name, "checkpoint_sha256": digest(checkpoint),
         "inputs": {"context_ids": {"dtype": "int32", "shape": [1, "Lc"]},
                    **{n: {"dtype": "int32", "shape": [1, "Ls"]} for n in NAMES[:4]},
                    "candidate_ids": {"dtype": "int32", "shape": [1, "Ls", "C"]},
                    **{n: {"dtype": "float16", "shape": [6, "Lc", 512]} for n in NAMES[5:]}},
         "outputs": {"memory_k": {"dtype": "float16", "shape": [6, "Lc", 512]},
                     "memory_v": {"dtype": "float16", "shape": [6, "Lc", 512]},
                     "logprob": {"dtype": "float32", "shape": [1, "Ls", "C"]}},
         "contract": ["context_ids begins with BOS; retain at most 255 preceding characters",
                      "Map paragraph newlines in context text to SEP character id 3; never concatenate paragraphs without a separator",
                      "Each syllable position has at least one non-pad candidate; unknown characters share UNK id 2",
                      "Memory is projected using the final syllable-layer weights; always use matching model packages",
                      "All Ls dimensions agree, both memory Lc dimensions agree; RangeDim alone does not enforce this at runtime",
                      "Pad candidates are 0 and have log-probability near -10000; caller supplies and prunes candidates to <=64",
                      "Tones are separate from syllable strings; literals use syllable id 1 and tone 0"]})
    print("exported", out, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--checkpoint", type=Path, default=OUT / "syllable.pt")
    p.add_argument("--out", type=Path, default=OUT)
    a = p.parse_args()
    export(a.checkpoint, a.out)
