"""Continue the 0.3.0 v3 syllable encoder on 在／再 and the other confusion groups: the 0.4.0 app model.

    zt train homophone train
    python -m zhuyintype_ml.training.homophone.train [--steps 10000] [--out data/build/homophone2] \\
        [--model data/build/v3_homophone2]

Starts from v3_030/syllable.pt with the context encoder frozen.  AdamW 2e-5 (100-step warm-up), batch 64, the
original augmentation and 30% context dropout.  30% of batches focus on a confusion: half on 在／再 (half of those on
clause-initial 在／再), half on another group; confusion positions weigh 3 in the loss.  A quarter of the examples of
normal and 在／再 batches come from real-source clauses (real-train.jsonl), never Taiwan Tongues.  Every 500 steps:
the homophone dev NLL decides, but only while all eight general dev conditions of zhuyintype_ml/training/v3/evaluate.py
stay within 0.2 points of the original; stops after four checks without improvement.  Writes the selected checkpoint and
copies of the original context checkpoint and vocabularies to --model, and v3-baseline.json, v3-training.json and
v3-complete.json to --out.  Needs CUDA (GPU host).
"""

from __future__ import annotations

import argparse
import json
import random
import shutil
import sys
import time
from collections.abc import Iterator
from functools import lru_cache
from pathlib import Path
from typing import Any

import numpy as np
import torch
from torch.nn import functional as F
from torch.utils.data import DataLoader, IterableDataset

from zhuyintype_ml.training.v3.dataset import Builder, Shard, collate
from zhuyintype_ml.training.v3.models import ContextEncoder, SyllableEncoder

from .common import BASE, MODEL, RUN, jsonl, save


class Batches(IterableDataset):
    """Training batches from the filtered shard indexes and the real-source clauses of `directory`."""

    def __init__(self, batch: int, directory: Path = RUN) -> None:
        self.batch, self.directory = batch, directory

    def __iter__(self) -> Iterator[tuple[torch.Tensor, ...]]:
        worker = torch.utils.data.get_worker_info()
        rng = random.Random(812 + (worker.id if worker else 0))
        builder = Builder()
        directory = self.directory
        manifest = json.loads((directory / "filtered.json").read_text())["shards"]
        groups = json.loads((directory / "inventory.json").read_text())["kept"]
        real = list(jsonl(directory / "real-train.jsonl"))
        real_by_char = {c: [r for r in real if c in r["text"]] for c in "在再"}
        initial = {c: [r for r in real if r["text"].startswith(c)] for c in "在再"}
        load = lru_cache(maxsize=8)(Shard)

        @lru_cache(maxsize=512)
        def index(name: str, group: str) -> np.ndarray:
            return np.load(directory / "filtered" / f"{name}.{group}.npy", mmap_mode="r")

        while True:
            group = "normal"
            if rng.random() < 0.3:
                group = "primary-" + rng.choice("在再") if rng.random() < 0.5 else rng.choice(groups)["chars"]
                if group.startswith("primary-") and rng.random() < 0.5:
                    group = "initial-" + group[-1]
            weights = [
                (r["allowed"] if group == "normal" else r["groups"][group])
                * (2 if r["name"].startswith("colloquial") else 1)
                for r in manifest
            ]
            examples = []
            for _ in range(self.batch):
                if group.startswith("initial-"):
                    pool = initial[group[-1]]
                elif group.startswith("primary-"):
                    pool = real_by_char[group[-1]]
                else:
                    pool = real if group == "normal" else []
                if pool and rng.random() < 0.25:
                    row = rng.choice(pool)
                    readings = [(r[0], r[1]) for r in row["readings"]]
                    examples.append(builder.example(row["text"], readings, row.get("context", ""), rng))
                    continue
                row = rng.choices(manifest, weights)[0]
                shard = load(row["name"])
                choices = index(row["name"], group)
                start, end, doc = shard.examples[int(choices[rng.randrange(len(choices))])]
                text = "".join(builder.vocab[n] if n >= 4 else "�" for n in shard.ids[start:end])
                readings = [
                    (builder.syllables[n], int(t)) for n, t in zip(shard.syllables[start:end], shard.tones[start:end])
                ]
                context = "".join(
                    builder.vocab[n] if n >= 4 else "\n" if n == 3 else "�" if n == 2 else ""
                    for n in shard.ids[max(doc, start - 255) : start]
                )
                examples.append(builder.example(text, readings, context, rng))
            yield collate(examples, builder, drop_context=rng.random() < 0.3)


def predict(
    context: ContextEncoder, model: SyllableEncoder, builder: Builder, batch: list[dict[str, Any]]
) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    sid, tone, cost, boundary, candidates, ctx, target = [x.cuda() for x in collate(batch, builder)]
    with torch.autocast("cuda", dtype=torch.bfloat16):
        k, v = model.project(context.hidden(ctx), ctx != 0)
        scores = model(sid, tone, cost, boundary, candidates, k, v)
    return scores, candidates, target


def homophone_cases(builder: Builder, directory: Path = RUN) -> list[dict[str, Any]]:
    """The 在／再 dev clauses, without and with their context, each with its confusion positions."""
    from zhuyintype_ml.training.v3.polyphone import load_poly

    poly, labels = load_poly()
    cases = []
    for i, row in enumerate(jsonl(directory / "dev_zai.jsonl")):
        readings, _ = builder.lex.label(row["text"], poly, builder.chars, labels)
        for enabled in [False, True]:
            e = builder.example(
                row["text"], readings, row["context"] if enabled else "", random.Random(i), rate=0, augment=False
            )
            e["focus"] = [p["index"] for p in row["positions"] if p["group"] == "在再"]
            cases.append(e)
    return cases


def general_cases(builder: Builder) -> dict[str, list[dict[str, Any]]]:
    """The eight general dev conditions of the v3 quality step: domain, context, noise."""
    from zhuyintype_ml.training.v3.evaluate import cases

    return {
        f"{domain}-{ctx}-{noise}": list(cases(builder, domain, ctx, noise))
        for domain in ["colloquial", "wikipedia"]
        for ctx in [False, True]
        for noise in [0.0, 0.05]
    }


@torch.no_grad()
def score(
    context: ContextEncoder,
    model: SyllableEncoder,
    builder: Builder,
    focused: list[dict[str, Any]],
    general: dict[str, list[dict[str, Any]]],
) -> dict[str, Any]:
    """Homophone dev NLL and accuracy at the focus positions, and top-1 accuracy on each general dev condition."""
    model.eval()
    nll, total, correct = 0.0, 0, 0
    for offset in range(0, len(focused), 16):
        batch = focused[offset : offset + 16]
        scores, candidates, target = predict(context, model, builder, batch)
        for i, e in enumerate(batch):
            for j in e["focus"]:
                nll -= scores[i, j, target[i, j]].item()
                correct += scores[i, j].argmax().item() == target[i, j].item()
                total += 1
    result: dict[str, Any] = dict(zai_nll=nll / total, zai_accuracy=correct / total, general={})
    for key, cases in general.items():
        hits, n = 0, 0
        for offset in range(0, len(cases), 16):
            batch = cases[offset : offset + 16]
            scores, candidates, _ = predict(context, model, builder, batch)
            pred = candidates.gather(-1, scores.argmax(-1).unsqueeze(-1)).squeeze(-1).cpu().tolist()
            for e, p in zip(batch, pred):
                for i, (char, reading) in enumerate(zip(e["text"], e["readings"])):
                    if reading[0] != "<literal>":
                        hits += builder.vocab[p[i]] == char
                        n += 1
        result["general"][key] = dict(correct=hits, total=n, accuracy=hits / n)
    return result


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--steps", type=int, default=10000)
    p.add_argument("--batch", type=int, default=64)
    p.add_argument("--eval-every", type=int, default=500)
    p.add_argument("--out", type=Path, default=RUN, help="the prepared run directory (default: data/build/homophone2)")
    p.add_argument("--model", type=Path, default=MODEL, help="new model directory (default: data/build/v3_homophone2)")
    args = p.parse_args(argv)
    out = args.out.resolve()
    assert sys.platform == "linux", "Training and model evaluation run remotely."
    assert not (out / "v3-training.json").exists(), "Inspect existing training before explicitly resuming."
    from zhuyintype_ml.training.v3.models import load_models

    torch.set_num_threads(4)
    torch.manual_seed(20261027)
    context, model = load_models(BASE / "syllable.pt", "cuda")
    context.requires_grad_(False).eval()
    builder = Builder()
    focused = homophone_cases(builder, out)
    general = general_cases(builder)
    baseline = score(context, model, builder, focused, general)
    save(out / "v3-baseline.json", baseline)
    print("baseline", json.dumps(baseline), flush=True)
    optimizer = torch.optim.AdamW(model.parameters(), lr=2e-5, weight_decay=0.1, betas=(0.9, 0.95), fused=True)
    stream = iter(
        DataLoader(Batches(args.batch, out), batch_size=None, num_workers=4, pin_memory=True, prefetch_factor=2)
    )
    inventory = json.loads((out / "inventory.json").read_text())["kept"]
    focus_ids = torch.tensor([builder.chars[c] for g in inventory for c in g["chars"]], device="cuda")
    history: list[dict[str, Any]] = []
    best, stale, best_step = baseline["zai_nll"], 0, 0
    started = time.monotonic()
    destination = args.model
    destination.mkdir(exist_ok=False)
    for name in ["context.pt", "chars.json", "syllables.json", "config.json"]:
        shutil.copy2(BASE / name, destination / name)
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
            print(step, loss.item(), flush=True)
        if step % args.eval_every == 0:
            metrics = score(context, model, builder, focused, general)
            admissible = all(
                metrics["general"][k]["accuracy"] >= baseline["general"][k]["accuracy"] - 0.002 for k in general
            )
            improved = admissible and metrics["zai_nll"] < best - 0.001
            if improved:
                best, stale, best_step = metrics["zai_nll"], 0, step
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
            row = dict(
                step=step,
                minutes=(time.monotonic() - started) / 60,
                loss=loss.item(),
                admissible=admissible,
                metrics=metrics,
            )
            history.append(row)
            print(json.dumps(row), flush=True)
            save(
                out / "v3-training.json",
                dict(
                    history=history,
                    selected_step=best_step,
                    stale=stale,
                    plateau=stale >= 4,
                    batch=args.batch,
                    oversample=0.3,
                    confusion_loss_weight=3,
                    learning_rate=2e-5,
                ),
            )
            if stale >= 4:
                break
    if stale < 4:
        raise RuntimeError("Step budget reached before dev plateau; do not export as complete.")
    if not best_step:
        raise RuntimeError("No checkpoint improved homophone dev under the general accuracy gate.")
    save(out / "v3-complete.json", dict(selected_step=best_step, plateau=True, seconds=time.monotonic() - started))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
