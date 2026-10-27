"""Train the reranker: a character-level causal Transformer language model (v2b: 8 layers, 512 wide).

    zt train reranker train
    python -m nlp.training.reranker.train --steps 90000 --layers 8 --width 512 --heads 8 --ffn 2048
    python -m nlp.training.reranker.train --data data/build/reranker_v2c --steps 90000 --layers 10 --width 640 \
        --heads 10 --ffn 2560

It scores whole candidate sentences given the text before them; the engine combines that score with its own to
choose the displayed conversion among its N best.  Pre-LayerNorm GPT with RoPE, tied embeddings, bf16, 64-token
context.  Reads vocab.json, train.bin and val_*.bin from --data (nlp.training.reranker.prepare); writes charlm.pt
(every 5,000 steps) and train-log.json there.  Needs CUDA.

CharLM and Config are also imported by export_tree, nlp/remote (ONNX export, model server) and ZhuyinBench.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import time
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

from nlp.common import paths


class Config:
    def __init__(
        self, vocab: int, layers: int = 6, width: int = 384, heads: int = 6, ffn: int = 1536, context: int = 64
    ) -> None:
        self.vocab, self.layers, self.width, self.heads, self.ffn, self.context = (
            vocab,
            layers,
            width,
            heads,
            ffn,
            context,
        )


def rope(x: torch.Tensor, cos: torch.Tensor, sin: torch.Tensor) -> torch.Tensor:
    a, b = x[..., ::2], x[..., 1::2]
    return torch.stack((a * cos - b * sin, a * sin + b * cos), dim=-1).flatten(-2)


class Block(nn.Module):
    def __init__(self, c: Config) -> None:
        super().__init__()
        self.heads = c.heads
        self.norm1 = nn.LayerNorm(c.width)
        self.qkv = nn.Linear(c.width, 3 * c.width, bias=False)
        self.proj = nn.Linear(c.width, c.width, bias=False)
        self.norm2 = nn.LayerNorm(c.width)
        self.up = nn.Linear(c.width, c.ffn, bias=False)
        self.down = nn.Linear(c.ffn, c.width, bias=False)

    def forward(self, x: torch.Tensor, cos: torch.Tensor, sin: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
        B, L, D = x.shape
        q, k, v = self.qkv(self.norm1(x)).view(B, L, 3, self.heads, D // self.heads).permute(2, 0, 3, 1, 4)
        q, k = rope(q, cos, sin), rope(k, cos, sin)
        attn = (q @ k.transpose(-2, -1)) / math.sqrt(D // self.heads) + mask
        y = (attn.softmax(-1) @ v).transpose(1, 2).reshape(B, L, D)
        x = x + self.proj(y)
        return x + self.down(F.gelu(self.up(self.norm2(x))))


class CharLM(nn.Module):
    cos: torch.Tensor
    sin: torch.Tensor
    causal: torch.Tensor

    def __init__(self, c: Config) -> None:
        super().__init__()
        self.c = c
        self.embed = nn.Embedding(c.vocab, c.width)
        self.blocks = nn.ModuleList(Block(c) for _ in range(c.layers))
        self.norm = nn.LayerNorm(c.width)
        half = c.width // c.heads // 2
        freq = 1.0 / (10000 ** (torch.arange(half) / half))
        angles = torch.arange(c.context)[:, None] * freq[None]
        self.register_buffer("cos", angles.cos()[None, None], persistent=False)
        self.register_buffer("sin", angles.sin()[None, None], persistent=False)
        self.register_buffer(
            "causal", torch.full((c.context, c.context), float("-inf")).triu(1)[None, None], persistent=False
        )
        self.apply(self._init)

    @staticmethod
    def _init(m: nn.Module) -> None:
        if isinstance(m, (nn.Linear, nn.Embedding)):
            nn.init.normal_(m.weight, std=0.02)

    def forward(self, ids: torch.Tensor) -> torch.Tensor:
        L = ids.shape[1]
        x = self.embed(ids)
        cos, sin, mask = self.cos[:, :, :L], self.sin[:, :, :L], self.causal[:, :, :L, :L]
        for block in self.blocks:
            x = block(x, cos, sin, mask)
        return self.norm(x) @ self.embed.weight.T


def batches(path: str, context: int, batch: int, device: str, seed: int) -> Iterator[tuple[torch.Tensor, torch.Tensor]]:
    data = np.memmap(path, dtype=np.uint16, mode="r")
    rng = np.random.default_rng(seed)
    while True:
        starts = rng.integers(0, len(data) - context - 1, size=batch)
        chunk = np.stack([data[s : s + context + 1] for s in starts]).astype(np.int64)
        x = torch.from_numpy(chunk).to(device, non_blocking=True)
        yield x[:, :-1], x[:, 1:]


@torch.no_grad()
def evaluate(model: nn.Module, path: str, context: int, device: str, limit: int = 2_000_000) -> float:
    data = np.fromfile(path, dtype=np.uint16)[:limit].astype(np.int64)
    n = (len(data) - 1) // context
    x = torch.from_numpy(data[: n * context + 1]).to(device)
    inputs, targets = x[:-1].view(n, context), x[1:].view(n, context)
    total, count = 0.0, 0
    for i in range(0, n, 256):
        with torch.autocast("cuda", dtype=torch.bfloat16):
            logits = model(inputs[i : i + 256])
        loss = F.cross_entropy(
            logits.float().reshape(-1, logits.shape[-1]),
            targets[i : i + 256].reshape(-1),
            ignore_index=0,
            reduction="sum",
        )
        total += loss.item()
        count += (targets[i : i + 256] != 0).sum().item()
    return total / count


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--data",
        type=Path,
        default=paths.data("reranker"),
        help="directory with vocab.json and the token streams (default: data/build/reranker)",
    )
    parser.add_argument("--steps", type=int, default=30000)
    parser.add_argument("--batch", type=int, default=512)
    parser.add_argument("--lr", type=float, default=1e-3)
    parser.add_argument("--layers", type=int, default=6)
    parser.add_argument("--width", type=int, default=384)
    parser.add_argument("--heads", type=int, default=6)
    parser.add_argument("--ffn", type=int, default=1536)
    args = parser.parse_args(argv)
    torch.manual_seed(7)
    device = "cuda"
    vocab = json.load(open(os.path.join(args.data, "vocab.json"), encoding="utf-8"))
    config = Config(len(vocab), args.layers, args.width, args.heads, args.ffn)
    model = CharLM(config).to(device)
    params = sum(p.numel() for p in model.parameters())
    print(f"parameters: {params / 1e6:.1f} M", flush=True)
    decay = [p for n, p in model.named_parameters() if p.dim() >= 2]
    no_decay = [p for n, p in model.named_parameters() if p.dim() < 2]
    optimizer = torch.optim.AdamW(
        [{"params": decay, "weight_decay": 0.1}, {"params": no_decay, "weight_decay": 0}],
        lr=args.lr,
        betas=(0.9, 0.95),
        fused=True,
    )
    compiled = torch.compile(model)
    stream = batches(os.path.join(args.data, "train.bin"), config.context, args.batch, device, 7)
    warmup = 1000
    start = time.time()
    log: list[dict[str, Any]] = []
    for step in range(1, args.steps + 1):
        lr = args.lr * min(1, step / warmup) * (0.1 + 0.9 * 0.5 * (1 + math.cos(math.pi * min(1, step / args.steps))))
        for group in optimizer.param_groups:
            group["lr"] = lr
        x, y = next(stream)
        with torch.autocast("cuda", dtype=torch.bfloat16):
            logits = compiled(x)
        loss = F.cross_entropy(logits.float().reshape(-1, logits.shape[-1]), y.reshape(-1), ignore_index=0)
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()
        if step % 500 == 0 or step == args.steps:
            row = {"step": step, "loss": loss.item(), "lr": lr, "minutes": (time.time() - start) / 60}
            if step % 5000 == 0 or step == args.steps:
                model.eval()
                for name in ["wiki", "colloquial"]:
                    nll = evaluate(model, os.path.join(args.data, f"val_{name}.bin"), config.context, device)
                    row[f"val_{name}_ppl"] = math.exp(nll)
                model.train()
                torch.save(
                    {"model": model.state_dict(), "config": vars(config), "step": step},
                    os.path.join(args.data, "charlm.pt"),
                )
            log.append(row)
            print(json.dumps(row), flush=True)
    with open(os.path.join(args.data, "train-log.json"), "w") as f:
        json.dump(log, f, indent=1)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
