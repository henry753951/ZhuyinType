"""Mac CPU-only parity, portable golden vectors and latency after Core ML compile."""
import base64
import hashlib
import json
import platform
import random
import subprocess
import time

import numpy as np
import torch

from common import OUT, digest, save
from models import ContextExport, SyllableExport, load_models
from reference import Runner, inputs, NAMES
from readings import Lexicon


def tensor(value):
    value = np.ascontiguousarray(value.astype("<f2"))
    raw = value.tobytes()
    return {"dtype": "float16", "byte_order": "little", "shape": list(value.shape),
            "encoding": "base64", "data": base64.b64encode(raw).decode(), "sha256": hashlib.sha256(raw).hexdigest()}


def make_cases():
    lex = Lexicon(OUT / "lexicon.tsv")
    vocab = json.load(open(OUT / "chars.json"))
    allowed = set(vocab[4:])
    texts = ["今天不要一直工作", "我們明天一起去公園", "這個方法真的很好", "臺灣的天氣很舒服",
             "你覺得這樣可以嗎", "我想喝一杯咖啡", "他的銀行就在旁邊", "長大的孩子很快樂",
             "請問現在幾點", "這件事情已經完成了"]
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
        candidates = []
        for char, r in zip(text, reading):
            if r[0] == "<literal>":
                candidates.append([char])
                continue
            row = [char] + [c for c in sorted(lex.candidates.get(r, {}), key=lambda c: -lex.candidates[r][c]) if c != char and c in allowed]
            row = row[:count]
            # Extra allowed ids exercise the full C range independently of lexical recall.
            if len(row) < count:
                row += [c for c in vocab[4:] if c not in row][:count - len(row)]
            random.Random(i + len(candidates)).shuffle(row)
            candidates.append(row)
        if i == 19:
            candidates[-1] = candidates[-1][:1]  # Include pad candidates in the golden output.
        lc = context_lengths[i % len(context_lengths)]
        prefix = ("前面已經提到今天的事情。" * 24)[:lc - 1]
        typed = [[s, t, i % 4, i % 2] for s, t in reading]
        cases.append({"context": prefix, "syllables": typed, "candidates": candidates})
    return cases


def measure(call, warmup=10, repeats=100):
    for _ in range(warmup):
        call()
    timings = []
    for _ in range(repeats):
        start = time.perf_counter_ns()
        call()
        timings.append((time.perf_counter_ns() - start) / 1e6)
    return {"warmup": warmup, "repeats": repeats, "p50_ms": float(np.median(timings)),
            "p95_ms": float(np.percentile(timings, 95)), "min_ms": min(timings), "max_ms": max(timings)}


def main():
    save(OUT / "example.json", {"context": "我們住在", "syllables": [["ㄊㄞ", 2, 0, 0], ["ㄨㄢ", 1, 0, 0]],
                               "candidates": [["臺", "台"], ["灣", "彎"]]})
    runner = Runner("coreml")
    context, syllable = load_models(OUT / "syllable.pt")
    torch.set_num_threads(4)
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    vectors, parity = [], []
    for i, case in enumerate(make_cases()):
        ctx, typed = inputs(case)
        memory, actual = runner.predict(ctx, typed)
        with torch.no_grad():
            tk, tv = cw(torch.from_numpy(ctx))
            expected = sw(*(torch.from_numpy(typed[n]) for n in NAMES), tk, tv).numpy()
            isolated = sw(*(torch.from_numpy(typed[n]) for n in NAMES),
                          torch.from_numpy(memory["memory_k"]), torch.from_numpy(memory["memory_v"])).numpy()
        valid = typed["candidate_ids"] != 0
        assert actual.shape == typed["candidate_ids"].shape and np.isfinite(actual).all()
        assert np.all(actual[~valid] < -9990)
        np.testing.assert_allclose(np.exp(actual).sum(-1), 1., rtol=.01, atol=.01)
        delta = float(np.max(np.abs(actual[valid] - expected[valid])))
        memory_delta = max(float(np.max(np.abs(memory["memory_k"].astype(np.float32) - tk.numpy().astype(np.float32)))),
                           float(np.max(np.abs(memory["memory_v"].astype(np.float32) - tv.numpy().astype(np.float32)))))
        row = {"case": i, "context_length": ctx.shape[1], "syllables": actual.shape[1], "candidates": actual.shape[2],
               "max_logprob_abs_error": delta, "max_memory_abs_error": memory_delta,
               "same_memory_logprob_error": float(np.max(np.abs(actual[valid] - isolated[valid]))),
               "top1_agreement": float(np.mean(actual.argmax(-1) == expected.argmax(-1)))}
        parity.append(row)
        # fp16 accumulation is not bitwise identical to fp32 PyTorch. Golden values are actual CPU Core ML outputs.
        assert delta <= .1 and memory_delta <= .1, row
        vectors.append({"id": i, "case": case,
                        "inputs": {"context_ids": ctx.tolist(), **{n: v.tolist() for n, v in typed.items()}},
                        "expected": {**{n: tensor(v) for n, v in memory.items()}, "logprob": actual.tolist()}})
        print(row, flush=True)
    save(OUT / "test_vectors.json", {"version": 1, "backend": "Core ML CPU_ONLY", "vectors": vectors,
         "memory_format": "base64 little-endian contiguous float16 in C order; shape and SHA256 provided",
         "comparison": {"coreml_logprob_atol": .002, "coreml_logprob_rtol": .002, "pytorch_fp32_atol": .1},
         "package_sha256": {str(p.relative_to(OUT)): digest(p) for name in ["ContextEncoder.mlpackage", "SyllableEncoder.mlpackage"] for p in (OUT / name).rglob("*") if p.is_file()}})
    save(OUT / "parity.json", parity)
    latency = {"machine": platform.platform(), "cpu": subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip(),
               "compute_units": "CPU_ONLY", "context": {}, "syllable": {},
               "protocol": "10 warmups then 100 wall-clock predict calls including Python/Core ML bridge; model load and first compilation excluded; projected memory reused for syllable calls"}
    for lc in [64, 128, 256]:
        ctx = np.array([[1] + [4] * (lc - 1)], np.int32)
        latency["context"][str(lc)] = measure(lambda: runner.context.predict({"context_ids": ctx}))
    memory = runner.context.predict({"context_ids": np.array([[1] + [4] * 255], np.int32)})
    for ls in [10, 30, 60]:
        values = {n: np.ones((1, ls), np.int32) * (2 if n == "syllable_ids" else 1) for n in NAMES[:4]}
        values["cost_bucket"][:] = 0
        values["boundary"][:] = 0
        values["candidate_ids"] = np.broadcast_to(np.arange(4, 36, dtype=np.int32), (1, ls, 32)).copy()
        values.update(memory)
        latency["syllable"][str(ls)] = measure(lambda: runner.syllable.predict(values))
    latency["target_30_p50_le15ms"] = latency["syllable"]["30"]["p50_ms"] <= 15
    latency["target_30_p95_le15ms"] = latency["syllable"]["30"]["p95_ms"] <= 15
    save(OUT / "latency.json", latency)
    print(json.dumps(latency, indent=2), flush=True)


if __name__ == "__main__":
    main()
