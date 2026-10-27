"""After the Core ML export, on the Mac: parity with PyTorch, the golden test vectors, and CPU-only latency.

    zt train v3 validate                 # macOS only
    python -m zhuyintype_ml.training.v3.validate

Runs 20 synthetic interface cases (context 1–256, syllables 1–64, candidates 1–64) through Core ML (CPU only) and
the fp32 PyTorch checkpoint; asserts shapes, finite values, normalisation and |Δ| <= 0.1.  Writes example.json,
test_vectors.json (exact Core ML outputs, float16 memories as base64 little-endian bytes with SHA256; zhuyin-cli
--check-v3 compares against them), parity.json and latency.json (10 warm-ups, 100 calls).
"""

from __future__ import annotations

import base64
import hashlib
import json
import platform
import random
import subprocess
import sys
import time
from collections.abc import Callable
from functools import partial
from typing import Any

import numpy as np
import torch

from .common import OUT, digest, save
from .models import ContextExport, SyllableExport, load_models
from .readings import Lexicon
from .reference import NAMES, Runner, inputs


def tensor(value: np.ndarray) -> dict[str, Any]:
    """A float16 array as portable JSON: base64 little-endian bytes, shape and SHA256."""
    value = np.ascontiguousarray(value.astype("<f2"))
    raw = value.tobytes()
    return {
        "dtype": "float16",
        "byte_order": "little",
        "shape": list(value.shape),
        "encoding": "base64",
        "data": base64.b64encode(raw).decode(),
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def make_cases() -> list[dict[str, Any]]:
    lex = Lexicon(OUT / "lexicon.tsv")
    vocab = json.load(open(OUT / "chars.json"))
    allowed = set(vocab[4:])
    texts = [
        "今天不要一直工作",
        "我們明天一起去公園",
        "這個方法真的很好",
        "臺灣的天氣很舒服",
        "你覺得這樣可以嗎",
        "我想喝一杯咖啡",
        "他的銀行就在旁邊",
        "長大的孩子很快樂",
        "請問現在幾點",
        "這件事情已經完成了",
    ]
    cases = []
    lengths = [1, 2, 10, 30, 60, 64]
    context_lengths = [1, 64, 128, 256]
    counts = [1, 2, 32, 64]
    for i in range(20):
        source = "好！" if i == 19 else texts[i % len(texts)]
        length = lengths[i % len(lengths)]
        text = (source * (length // len(source) + 1))[:length]
        reading, _ = lex.label(text)
        count = counts[i % len(counts)]
        candidates: list[list[str]] = []
        for char, r in zip(text, reading):
            if r[0] == "<literal>":
                candidates.append([char])
                continue
            row = [char] + [
                c
                for c in sorted(lex.candidates.get(r, {}), key=lambda c: -lex.candidates[r][c])
                if c != char and c in allowed
            ]
            row = row[:count]
            # Extra allowed ids exercise the full C range independently of lexical recall.
            if len(row) < count:
                row += [c for c in vocab[4:] if c not in row][: count - len(row)]
            random.Random(i + len(candidates)).shuffle(row)
            candidates.append(row)
        if i == 19:
            candidates[-1] = candidates[-1][:1]  # Include pad candidates in the golden output.
        lc = context_lengths[i % len(context_lengths)]
        prefix = ("前面已經提到今天的事情。" * 24)[: lc - 1]
        typed = [[s, t, i % 4, i % 2] for s, t in reading]
        cases.append({"context": prefix, "syllables": typed, "candidates": candidates})
    return cases


def measure(call: Callable[[], object], warmup: int = 10, repeats: int = 100) -> dict[str, Any]:
    for _ in range(warmup):
        call()
    timings = []
    for _ in range(repeats):
        start = time.perf_counter_ns()
        call()
        timings.append((time.perf_counter_ns() - start) / 1e6)
    return {
        "warmup": warmup,
        "repeats": repeats,
        "p50_ms": float(np.median(timings)),
        "p95_ms": float(np.percentile(timings, 95)),
        "min_ms": min(timings),
        "max_ms": max(timings),
    }


def main(argv: list[str] | None = None) -> int:
    import argparse

    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    if sys.platform != "darwin":
        raise SystemExit("v3 validate runs only on macOS (Core ML)")
    save(
        OUT / "example.json",
        {
            "context": "我們住在",
            "syllables": [["ㄊㄞ", 2, 0, 0], ["ㄨㄢ", 1, 0, 0]],
            "candidates": [["臺", "台"], ["灣", "彎"]],
        },
    )
    runner = Runner("coreml")
    context, syllable = load_models(OUT / "syllable.pt")
    torch.set_num_threads(4)
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    vectors: list[dict[str, Any]] = []
    parity: list[dict[str, Any]] = []
    for i, case in enumerate(make_cases()):
        ctx, typed = inputs(case)
        memory, actual = runner.predict(ctx, typed)
        with torch.no_grad():
            tk, tv = cw(torch.from_numpy(ctx))
            expected = sw(*(torch.from_numpy(typed[n]) for n in NAMES), tk, tv).numpy()
            isolated = sw(
                *(torch.from_numpy(typed[n]) for n in NAMES),
                torch.from_numpy(memory["memory_k"]),
                torch.from_numpy(memory["memory_v"]),
            ).numpy()
        valid = typed["candidate_ids"] != 0
        assert actual.shape == typed["candidate_ids"].shape and np.isfinite(actual).all()
        assert np.all(actual[~valid] < -9990)
        np.testing.assert_allclose(np.exp(actual).sum(-1), 1.0, rtol=0.01, atol=0.01)
        delta = float(np.max(np.abs(actual[valid] - expected[valid])))
        memory_delta = max(
            float(np.max(np.abs(memory["memory_k"].astype(np.float32) - tk.numpy().astype(np.float32)))),
            float(np.max(np.abs(memory["memory_v"].astype(np.float32) - tv.numpy().astype(np.float32)))),
        )
        row = {
            "case": i,
            "context_length": ctx.shape[1],
            "syllables": actual.shape[1],
            "candidates": actual.shape[2],
            "max_logprob_abs_error": delta,
            "max_memory_abs_error": memory_delta,
            "same_memory_logprob_error": float(np.max(np.abs(actual[valid] - isolated[valid]))),
            "top1_agreement": float(np.mean(actual.argmax(-1) == expected.argmax(-1))),
        }
        parity.append(row)
        # fp16 accumulation is not bitwise identical to fp32 PyTorch. Golden values are actual CPU Core ML outputs.
        assert delta <= 0.1 and memory_delta <= 0.1, row
        vectors.append(
            {
                "id": i,
                "case": case,
                "inputs": {"context_ids": ctx.tolist(), **{n: v.tolist() for n, v in typed.items()}},
                "expected": {**{n: tensor(v) for n, v in memory.items()}, "logprob": actual.tolist()},
            }
        )
        print(row, flush=True)
    save(
        OUT / "test_vectors.json",
        {
            "version": 1,
            "backend": "Core ML CPU_ONLY",
            "vectors": vectors,
            "memory_format": "base64 little-endian contiguous float16 in C order; shape and SHA256 provided",
            "comparison": {"coreml_logprob_atol": 0.002, "coreml_logprob_rtol": 0.002, "pytorch_fp32_atol": 0.1},
            "package_sha256": {
                str(p.relative_to(OUT)): digest(p)
                for name in ["ContextEncoder.mlpackage", "SyllableEncoder.mlpackage"]
                for p in (OUT / name).rglob("*")
                if p.is_file()
            },
        },
    )
    save(OUT / "parity.json", parity)
    latency: dict[str, Any] = {
        "machine": platform.platform(),
        "cpu": subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip(),
        "compute_units": "CPU_ONLY",
        "context": {},
        "syllable": {},
        "protocol": (
            "10 warmups then 100 wall-clock predict calls including Python/Core ML bridge; model load "
            "and first compilation excluded; projected memory reused for syllable calls"
        ),
    }
    for lc in [64, 128, 256]:
        ctx = np.array([[1] + [4] * (lc - 1)], np.int32)
        latency["context"][str(lc)] = measure(partial(runner.context.predict, {"context_ids": ctx}))
    memory = runner.context.predict({"context_ids": np.array([[1] + [4] * 255], np.int32)})
    for ls in [10, 30, 60]:
        values = {n: np.ones((1, ls), np.int32) * (2 if n == "syllable_ids" else 1) for n in NAMES[:4]}
        values["cost_bucket"][:] = 0
        values["boundary"][:] = 0
        values["candidate_ids"] = np.broadcast_to(np.arange(4, 36, dtype=np.int32), (1, ls, 32)).copy()
        values.update(memory)
        latency["syllable"][str(ls)] = measure(partial(runner.syllable.predict, values))
    latency["target_30_p50_le15ms"] = latency["syllable"]["30"]["p50_ms"] <= 15
    latency["target_30_p95_le15ms"] = latency["syllable"]["30"]["p95_ms"] <= 15
    save(OUT / "latency.json", latency)
    print(json.dumps(latency, indent=2), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
