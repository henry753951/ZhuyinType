"""Dev top-1 character selection of the trained v3 models (PyTorch, CUDA, bf16); never scores test text.

    zt train v3 quality
    python -m nlp.training.v3.evaluate

Every dev line (v3_030/dev_{colloquial,wikipedia}.txt), with and without context, with no noise and with 5% typing
noise.  Scores non-literal positions against the full candidate union (gold always included): selection given
recall, not decoder accuracy.  The baseline is the highest-prior single-character lexicon row.  Writes quality.json.
`cases` is also the general dev gate of the homophone continuation.
"""

from __future__ import annotations

import json
import random
import re
import time
from collections.abc import Iterator
from typing import Any

import torch

from .common import OUT, save
from .dataset import Builder, collate
from .models import load_models
from .polyphone import load_poly


def cases(builder: Builder, domain: str, context_enabled: bool, noise: float) -> Iterator[dict[str, Any]]:
    poly, labels = load_poly()
    previous = ""
    rng = random.Random(20261027)
    for line in (OUT / f"dev_{domain}.txt").read_text().splitlines():
        for match in re.finditer(r"[^。！？；]+[。！？；]?", line):
            for offset in range(match.start(), match.end(), 64):
                text = line[offset : min(offset + 64, match.end())]
                if not text:
                    continue
                reading, low = builder.lex.label(text, poly, builder.chars, labels)
                ctx = ""
                if context_enabled:
                    ctx = (previous if domain == "colloquial" else "") + line[:offset]
                yield builder.example(text, reading, ctx, rng, rate=noise, augment=False)
        previous = line


def main(argv: list[str] | None = None) -> int:
    import argparse

    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    torch.set_num_threads(4)
    context, syllable = load_models(OUT / "syllable.pt", "cuda")
    builder = Builder()
    rows = []
    start = time.monotonic()
    for domain in ["colloquial", "wikipedia"]:
        for with_context in [False, True]:
            for rate in [0.0, 0.05]:
                correct = baseline = total = all_correct = all_total = oversized = oov = changed = 0
                batch: list[dict[str, Any]] = []

                def score(batch: list[dict[str, Any]]) -> None:
                    nonlocal correct, baseline, total, all_correct, all_total, oversized, oov, changed
                    tensors = [t.cuda() for t in collate(batch, builder)]
                    sid, tone, cost, boundary, candidates, ctx, target = tensors
                    with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
                        k, v = syllable.project(context.hidden(ctx), ctx != 0)
                        logprob = syllable(sid, tone, cost, boundary, candidates, k, v)
                    pred = candidates.gather(-1, logprob.argmax(-1).unsqueeze(-1)).squeeze(-1).cpu().tolist()
                    for e, predicted in zip(batch, pred):
                        for i, (char, reading, truth) in enumerate(zip(e["text"], e["readings"], e["truth"])):
                            hit = builder.vocab[predicted[i]] == char
                            all_correct += hit
                            all_total += 1
                            if reading[0] == "<literal>":
                                continue
                            correct += hit
                            baseline += e["baseline"][i] == truth and truth != 2
                            total += 1
                            oov += truth == 2
                            oversized += len(e["candidates"][i]) > 64
                            changed += e["cost_bucket"][i] > 0

                for e in cases(builder, domain, with_context, rate):
                    batch.append(e)
                    if len(batch) == 16:
                        score(batch)
                        batch = []
                if batch:
                    score(batch)
                row = {
                    "domain": domain,
                    "line_context": with_context,
                    "requested_noise": rate,
                    "positions": total,
                    "model_top1": correct / total,
                    "baseline_top1": baseline / total,
                    "model_correct": correct,
                    "baseline_correct": baseline,
                    "all_positions": all_total,
                    "all_top1": all_correct / all_total,
                    "oov_positions": oov,
                    "candidate_count_over64": oversized,
                    "actual_valid_noise_fraction": changed / total,
                }
                rows.append(row)
                save(
                    OUT / "quality.json",
                    {
                        "rows": rows,
                        "seconds": time.monotonic() - start,
                        "protocol": (
                            "All dev lines; longest-match + polyphone labels. Score nonliteral reading positions; OOV "
                            "predictions count wrong. Candidate union includes gold, no candidate cap (selection "
                            "quality "
                            "conditional on recall). Literal-inclusive accuracy also given. Wikipedia context is "
                            "preceding text "
                            "on same line; colloquial additionally uses previous dev line. 5% requested noise can "
                            "revert to clean "
                            "if invalid."
                        ),
                    },
                )
                print(json.dumps(row), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
