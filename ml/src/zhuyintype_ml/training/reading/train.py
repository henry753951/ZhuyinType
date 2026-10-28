"""Continue the app's v3 (the 0.4.0 homophone continuation) with reading noise from the reading channel.

    zt train reading train
    python -m zhuyintype_ml.training.reading.train --runs s3,s10,r10 [--tolerance 0.002] [--steps 10000] \\
        [--out data/build/v3_reading]

The homophone recipe (ml/src/zhuyintype_ml/training/homophone/train.py) unchanged except for the noise: the same
batches (30% on confusion groups, a quarter real-source clauses), confusion positions weighing 3, AdamW 2e-5 with a
100-step warm-up, batch 64, 30% context dropout, the context encoder frozen.  It starts from the shipped model
(v3_homophone2/, released as v3/) and adds training/reading/noise.py: each position is typed another way with
probability min(0.5, Σ w·p) over the variants that apply there (reading/noise.py).  A run's name gives the noise:
`s<scale>` scales every class, `r<scale>` only the rare classes (m and the neutral rate; noise.RARE).

One run per name, one after another.  Every 500 steps three dev checks (PyTorch, bf16): the eight general dev
conditions and the 在／再 dev positions of the homophone run, and reading dev — every dev chunk of v3_030's two dev
sets with one position typed another way (reading/noise.py `one`), scored at that position, with line context.  A
checkpoint is admissible while every general condition and 在／再 dev accuracy stay within --tolerance (the homophone
run's 0.2 points by default) of the starting model; among admissible checkpoints the lowest reading-dev NLL is
kept; four checks without improvement stop the run.  Writes v3_reading_<name>/ (syllable.pt, the context checkpoint,
vocabularies and config.json of the start) and, in --out, start.json and <name>.json (history, selection,
settings).  Needs CUDA.
"""

from __future__ import annotations

import argparse
import functools
import json
import random
import re
import shutil
import sys
import time
from collections import Counter
from pathlib import Path
from typing import Any

import torch
from torch.nn import functional as F
from torch.utils.data import DataLoader

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.homophone.common import MODEL as START
from zhuyintype_ml.training.homophone.common import RUN as HOMOPHONE
from zhuyintype_ml.training.homophone.common import save
from zhuyintype_ml.training.homophone.train import Batches, general_cases, homophone_cases, predict, score
from zhuyintype_ml.training.v3.common import OUT as V3
from zhuyintype_ml.training.v3.dataset import Builder
from zhuyintype_ml.training.v3.models import ContextEncoder, SyllableEncoder, load_models
from zhuyintype_ml.training.v3.polyphone import load_poly

from .noise import RARE, ReadingNoise

CHANNEL = paths.data("lexicon_coverage", "channel")
OUT = paths.data("v3_reading")


def noise_of(name: str) -> tuple[float, frozenset[str] | None]:
    """'s10' -> (10, every class); 'r10' -> (10, the rare classes)."""
    if name[:1] not in ("s", "r"):
        raise ValueError(f"a run is s<scale> or r<scale>, not {name!r}")
    return float(name[1:]), (RARE if name[0] == "r" else None)


def noisy_builder(channel: Path, name: str) -> Builder:
    builder = Builder()
    scale, scaled = noise_of(name)
    builder.reading = ReadingNoise(channel, scale, valid=lambda r: r in builder.noise.encode, scaled=scaled)
    return builder


def reading_cases(builder: Builder, noise: ReadingNoise) -> list[dict[str, Any]]:
    """v3_030's dev chunks (as v3/evaluate.py cuts them, with line context) with one position typed another way."""
    poly, labels = load_poly()
    rng = random.Random(20261028)
    cases = []
    for domain in ["colloquial", "wikipedia"]:
        previous = ""
        for line in (V3 / f"dev_{domain}.txt").read_text().splitlines():
            for match in re.finditer(r"[^。！？；]+[。！？；]?", line):
                for offset in range(match.start(), match.end(), 64):
                    text = line[offset : min(offset + 64, match.end())]
                    if not text:
                        continue
                    readings, _ = builder.lex.label(text, poly, builder.chars, labels)
                    chosen = noise.one(text, readings, builder.lex, rng)
                    if chosen is None:
                        continue
                    i, option = chosen
                    context = (previous if domain == "colloquial" else "") + line[:offset]
                    fixed = {i: (option.typed, option.bucket)}
                    builder.reading = lambda *_args, fixed=fixed: fixed
                    e = builder.example(text, readings, context, rng, rate=0, augment=False)
                    e["focus"], e["kind"], e["state"] = i, option.kind, option.state
                    cases.append(e)
            previous = line
    builder.reading = None
    return cases


@torch.no_grad()
def reading_score(
    context: ContextEncoder, model: SyllableEncoder, builder: Builder, cases: list[dict[str, Any]]
) -> dict[str, Any]:
    """NLL and top-1 accuracy at the position typed another way, overall and by class and state."""
    model.eval()
    nll, hits = 0.0, 0
    by: Counter[str] = Counter()
    right: Counter[str] = Counter()
    for offset in range(0, len(cases), 16):
        batch = cases[offset : offset + 16]
        scores, _candidates, target = predict(context, model, builder, batch)
        for i, e in enumerate(batch):
            j = e["focus"]
            nll -= scores[i, j, target[i, j]].item()
            hit = int(scores[i, j].argmax().item() == target[i, j].item())
            hits += hit
            for key in (e["kind"], e["state"]):
                by[key] += 1
                right[key] += hit
    n = len(cases)
    return dict(
        nll=nll / n, accuracy=hits / n, positions=n, by={k: dict(total=by[k], accuracy=right[k] / by[k]) for k in by}
    )


def evaluate(
    context: ContextEncoder,
    model: SyllableEncoder,
    builder: Builder,
    focused: list[dict[str, Any]],
    general: dict[str, list[dict[str, Any]]],
    reading: list[dict[str, Any]],
) -> dict[str, Any]:
    metrics = score(context, model, builder, focused, general)
    metrics["reading"] = reading_score(context, model, builder, reading)
    return metrics


def admissible(metrics: dict[str, Any], start: dict[str, Any], tolerance: float) -> bool:
    general = all(
        metrics["general"][k]["accuracy"] >= start["general"][k]["accuracy"] - tolerance for k in start["general"]
    )
    return general and metrics["zai_accuracy"] >= start["zai_accuracy"] - tolerance


def train(
    name: str, args: argparse.Namespace, builder: Builder, cases: dict[str, Any], start: dict[str, Any]
) -> dict[str, Any]:
    scale, scaled = noise_of(name)
    destination = paths.data(f"v3_reading_{name}")
    destination.mkdir(exist_ok=False)
    for file in ["context.pt", "chars.json", "syllables.json", "config.json"]:
        shutil.copy2(START / file, destination / file)
    torch.manual_seed(20261028)
    context, model = load_models(START / "syllable.pt", "cuda")
    context.requires_grad_(False).eval()
    optimizer = torch.optim.AdamW(model.parameters(), lr=2e-5, weight_decay=0.1, betas=(0.9, 0.95), fused=True)
    make = functools.partial(noisy_builder, args.channel, name)
    stream = iter(
        DataLoader(
            Batches(args.batch, HOMOPHONE, make=make, seed=812),
            batch_size=None,
            num_workers=4,
            pin_memory=True,
            prefetch_factor=2,
        )
    )
    inventory = json.loads((HOMOPHONE / "inventory.json").read_text())["kept"]
    focus_ids = torch.tensor([builder.chars[c] for g in inventory for c in g["chars"]], device="cuda")
    history: list[dict[str, Any]] = []
    best, stale, best_step = start["reading"]["nll"], 0, 0
    started = time.monotonic()
    record = dict(
        scale=scale,
        scaled=sorted(scaled) if scaled else "all",
        cap=0.5,
        tolerance=args.tolerance,
        batch=args.batch,
        learning_rate=2e-5,
        start=str(START / "syllable.pt"),
    )
    for step in range(1, args.steps + 1):
        model.train()
        for group in optimizer.param_groups:
            group["lr"] = 2e-5 * min(1.0, step / 100)
        sid, tone, cost, boundary, candidates, ctx, target = [x.cuda(non_blocking=True) for x in next(stream)]
        with torch.autocast("cuda", dtype=torch.bfloat16):
            with torch.no_grad():
                hidden = context.hidden(ctx)
            k, v = model.project(hidden, ctx != 0)
            scores = model(sid, tone, cost, boundary, candidates, k, v)
            loss = F.nll_loss(scores.flatten(0, 1), target.flatten(), ignore_index=-100, reduction="none").view_as(
                target
            )
            truth = candidates.gather(-1, target.clamp_min(0).unsqueeze(-1)).squeeze(-1)
            weights = (1 + 2 * torch.isin(truth, focus_ids)).float() * (target != -100)
            loss = (loss * weights).sum() / weights.sum()
        if not torch.isfinite(loss):
            raise RuntimeError(f"nonfinite loss step {step}")
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()
        if step % 100 == 0:
            print(name, step, loss.item(), flush=True)
        if step % args.eval_every:
            continue
        metrics = evaluate(context, model, builder, cases["focused"], cases["general"], cases["reading"])
        ok = admissible(metrics, start, args.tolerance)
        if ok and metrics["reading"]["nll"] < best - 0.001:
            best, stale, best_step = metrics["reading"]["nll"], 0, step
            torch.save(
                dict(
                    context=context.state_dict(),
                    syllable=model.state_dict(),
                    vocab=len(builder.vocab),
                    syllables=len(builder.syllables),
                    step=step,
                    metrics=metrics,
                ),
                destination / "syllable.pt",
            )
        else:
            stale += 1
        minutes = (time.monotonic() - started) / 60
        row = dict(step=step, minutes=minutes, loss=loss.item(), admissible=ok, metrics=metrics)
        history.append(row)
        print(name, json.dumps(row), flush=True)
        save(args.out / f"{name}.json", dict(record, history=history, selected_step=best_step, stale=stale))
        if stale >= 4:
            break
    result = dict(record, history=history, selected_step=best_step, stale=stale, plateau=stale >= 4)
    save(args.out / f"{name}.json", result)
    if not best_step:
        shutil.rmtree(destination)
    return result


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--runs", default="s3,s10,s30,s100", help="s<scale> or r<scale>, one run each")
    p.add_argument("--tolerance", type=float, default=0.002, help="allowed loss on the general and 在／再 dev checks")
    p.add_argument("--steps", type=int, default=10000)
    p.add_argument("--batch", type=int, default=64)
    p.add_argument("--eval-every", type=int, default=500)
    p.add_argument("--channel", type=Path, default=CHANNEL, help="the reading channel (lexicon_coverage/channel)")
    p.add_argument("--out", type=Path, default=OUT, help="run records (default: data/build/v3_reading)")
    args = p.parse_args(argv)
    assert sys.platform == "linux", "Training and model evaluation run remotely."
    args.out.mkdir(parents=True, exist_ok=True)
    torch.set_num_threads(4)
    builder = Builder()
    probe = ReadingNoise(args.channel, valid=lambda r: r in builder.noise.encode)
    cases: dict[str, Any] = dict(
        focused=homophone_cases(builder, HOMOPHONE),
        general=general_cases(builder),
        reading=reading_cases(builder, probe),
    )
    if (args.out / "start.json").exists():
        start = json.loads((args.out / "start.json").read_text())
    else:
        context, model = load_models(START / "syllable.pt", "cuda")
        start = evaluate(context, model, builder, cases["focused"], cases["general"], cases["reading"])
        save(args.out / "start.json", start)
        del context, model
    print("start", json.dumps(start), flush=True)
    for name in args.runs.split(","):
        record = args.out / f"{name}.json"
        if record.exists() and json.loads(record.read_text()).get("plateau") is not None:
            print(f"{name}: done already", flush=True)
            continue
        result = train(name, args, builder, cases, start)
        print(f"{name}: selected step {result['selected_step']}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
