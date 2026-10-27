"""Train the context LM, then the context-conditioned syllable encoder, each until the dev NLL stops improving.

    zt train v3 context syllable
    python -m nlp.training.v3.train context --steps 30000 --batch 64
    python -m nlp.training.v3.train syllable --steps 60000 --batch 64
    python -m nlp.training.v3.train syllable --resume --steps 10000 --batch 64   # continue to a plateau

Validation every 1000 steps; an improvement must exceed 0.002 NLL; stop after 4 evaluations without one (and, for a
fresh run, at least 5000 steps).  The context encoder is frozen during the syllable stage; the syllable layers' cross
K/V projections train with it.  `--resume` continues the selected syllable checkpoint with a new optimizer, a new
sampling seed and initial learning rate 6e-5, keeping the history and time.  Writes <phase>.pt, <phase>-history.json
and <phase>-done.json to v3_030/.  Needs CUDA.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import re
import time
from typing import Any

import torch
from torch.nn import functional as F
from torch.utils.data import DataLoader

from .common import OUT, save
from .dataset import Batches, Builder, collate
from .models import ContextEncoder, SyllableEncoder


@torch.no_grad()
def evaluate_context(model: ContextEncoder, chars: dict[str, int]) -> dict[str, float]:
    result = {}
    for name in ["wikipedia", "colloquial"]:
        total, tokens = 0.0, 0
        for line in (OUT / f"dev_{name}.txt").read_text().splitlines()[:256]:
            ids = [1] + [chars.get(c, 2) for c in line[:256]]
            if len(ids) < 2:
                continue
            x = torch.tensor([ids], device="cuda")
            with torch.autocast("cuda", dtype=torch.bfloat16):
                logits = model(x[:, :-1])
                loss = F.cross_entropy(logits.flatten(0, 1).float(), x[:, 1:].flatten(), reduction="sum")
            total += loss.item()
            tokens += len(ids) - 1
        result[name] = total / max(tokens, 1)
    return result


@torch.no_grad()
def evaluate_syllable(
    context: ContextEncoder, syllable: SyllableEncoder, builder: Builder, cases: list[dict[str, Any]]
) -> float:
    total, tokens = 0.0, 0
    for start in range(0, len(cases), 16):
        batch = collate(cases[start : start + 16], builder)
        sid, tone, cost, boundary, candidate, ctx, target = [x.cuda() for x in batch]
        with torch.autocast("cuda", dtype=torch.bfloat16):
            k, v = syllable.project(context.hidden(ctx), ctx != 0)
            logprob = syllable(sid, tone, cost, boundary, candidate, k, v)
            loss = F.nll_loss(logprob.flatten(0, 1), target.flatten(), ignore_index=-100, reduction="sum")
        total += loss.item()
        tokens += (target != -100).sum().item()
    return total / max(1, tokens)


def dev_cases(builder: Builder, limit: int = 128) -> list[dict[str, Any]]:
    from .polyphone import load_poly

    poly, labels = load_poly()
    cases: list[dict[str, Any]] = []
    for domain in ["wikipedia", "colloquial"]:
        previous = ""
        for line in (OUT / f"dev_{domain}.txt").read_text().splitlines()[:limit]:
            for match in re.finditer(r"[^。！？；]+[。！？；]?", line):
                for offset in range(match.start(), match.end(), 64):
                    text = line[offset : min(offset + 64, match.end())]
                    if len(text) < 2:
                        continue
                    readings, _ = builder.lex.label(text, poly, builder.chars, labels)
                    ctx = (previous if domain == "colloquial" else "") + line[:offset]
                    cases.append(
                        builder.example(text, readings, ctx, random.Random(len(cases)), rate=0.05, augment=False)
                    )
            previous = line
    return cases


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("phase", choices=["context", "syllable"])
    p.add_argument("--steps", type=int, default=30000)
    p.add_argument("--batch", type=int, default=64)
    p.add_argument("--eval-every", type=int, default=1000)
    p.add_argument("--patience", type=int, default=4)
    p.add_argument(
        "--resume", action="store_true", help="Continue the selected syllable checkpoint at lower LR until plateau"
    )
    args = p.parse_args(argv)
    assert not args.resume or args.phase == "syllable"
    previous = json.load(open(OUT / "syllable-done.json")) if args.resume else {}
    base_step = previous.get("steps", 0)
    torch.manual_seed(71)
    torch.set_num_threads(4)
    torch.backends.cuda.matmul.allow_tf32 = True
    vocab = json.load(open(OUT / "chars.json"))
    syllables = json.load(open(OUT / "syllables.json"))
    context = ContextEncoder(len(vocab)).cuda()
    syllable = SyllableEncoder(len(vocab), len(syllables)).cuda()
    if args.phase == "syllable":
        checkpoint = torch.load(
            OUT / ("syllable.pt" if args.resume else "context.pt"), map_location="cpu", weights_only=False
        )
        context.load_state_dict(checkpoint["context"])
        context.requires_grad_(False).eval()
        if args.resume:
            syllable.load_state_dict(checkpoint["syllable"])
        else:
            syllable.initialize(context)
    model = context if args.phase == "context" else syllable
    model.train()
    optimizer = torch.optim.AdamW(model.parameters(), lr=4e-4, weight_decay=0.1, betas=(0.9, 0.95), fused=True)
    stream = iter(
        DataLoader(
            Batches(args.phase, args.batch, seed=31 + base_step),
            batch_size=None,
            num_workers=6,
            pin_memory=True,
            prefetch_factor=2,
        )
    )
    chars = {c: i for i, c in enumerate(vocab)}
    builder = Builder() if args.phase == "syllable" else None
    cases = dev_cases(builder) if builder else None
    history: list[dict[str, Any]] = []
    best, stale, best_step = math.inf, 0, 0
    if args.resume:
        history = json.load(open(OUT / "syllable-history.json"))["history"]
        best, best_step = previous["best_dev_nll"], checkpoint["step"]
    started = time.monotonic() - previous.get("seconds", 0)
    minimum_steps = 0 if args.resume else 5000
    base_lr = 6e-5 if args.resume else 4e-4
    for step in range(1, args.steps + 1):
        global_step = base_step + step
        lr = base_lr * min(1.0, step / 500) * (0.15 + 0.85 * 0.5 * (1 + math.cos(math.pi * step / args.steps)))
        for group in optimizer.param_groups:
            group["lr"] = lr
        batch = [x.cuda(non_blocking=True) for x in next(stream)]
        with torch.autocast("cuda", dtype=torch.bfloat16):
            if args.phase == "context":
                x, y = batch
                logits = model(x)
                loss = F.cross_entropy(logits.flatten(0, 1).float(), y.flatten(), ignore_index=0)
            else:
                sid, tone, cost, boundary, candidates, ctx, target = batch
                with torch.no_grad():
                    hidden = context.hidden(ctx)
                k, v = syllable.project(hidden, ctx != 0)
                logprob = syllable(sid, tone, cost, boundary, candidates, k, v)
                loss = F.nll_loss(logprob.flatten(0, 1), target.flatten(), ignore_index=-100)
        if not torch.isfinite(loss):
            raise RuntimeError(f"non-finite loss at {step}")
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()
        if step % 100 == 0:
            print(
                json.dumps(
                    {
                        "phase": args.phase,
                        "step": global_step,
                        "loss": loss.item(),
                        "minutes": (time.monotonic() - started) / 60,
                    }
                ),
                flush=True,
            )
        if step % args.eval_every == 0 or step == args.steps:
            model.eval()
            metrics = (
                evaluate_context(context, chars)
                if args.phase == "context"
                else {"nll": evaluate_syllable(context, syllable, builder, cases)}
            )
            score = sum(metrics.values()) / len(metrics)
            row = {
                "step": global_step,
                "loss": loss.item(),
                "dev": metrics,
                "minutes": (time.monotonic() - started) / 60,
            }
            history.append(row)
            print(json.dumps(row), flush=True)
            if score < best - 0.002:
                best, stale, best_step = score, 0, global_step
                checkpoint = {
                    "context": context.state_dict(),
                    "vocab": len(vocab),
                    "syllables": len(syllables),
                    "step": global_step,
                    "phase": args.phase,
                    "metrics": row,
                }
                if args.phase == "syllable":
                    checkpoint["syllable"] = syllable.state_dict()
                torch.save(checkpoint, OUT / (args.phase + ".pt"))
            else:
                stale += 1
            save(
                OUT / (args.phase + "-history.json"),
                {
                    "phase": args.phase,
                    "history": history,
                    "best_nll": best,
                    "stale_evaluations": stale,
                    "seconds": time.monotonic() - started,
                    "stopping": "plateau" if stale >= args.patience and step >= minimum_steps else "running",
                },
            )
            model.train()
            if stale >= args.patience and step >= minimum_steps:
                break
    save(
        OUT / (args.phase + "-done.json"),
        {
            "steps": global_step,
            "selected_step": best_step,
            "seconds": time.monotonic() - started,
            "best_dev_nll": best,
            "plateau": stale >= args.patience,
            "parameter_count": sum(p.numel() for p in model.parameters()),
            "batch": args.batch,
            "max_steps": base_step + args.steps,
            "resumed_after_steps": base_step,
            "resume_initial_lr": base_lr if args.resume else None,
            "dev_cases": len(cases) if cases else "first 256 lines per domain",
            "context_training": "frozen during syllable stage; cross K/V trained with syllable stage",
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
