#!/usr/bin/env python3
"""Train the reranker: a small character-level causal Transformer language model.

    python train.py --data ~/zhuyintype/reranker --steps 30000

It scores whole candidate sentences given the text before them; the engine combines that score with its own to
choose the displayed conversion among its N best.  Pre-LayerNorm GPT with RoPE, tied embeddings, bf16.
"""
import argparse
import json
import math
import os
import time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F


class Config:
    def __init__(self, vocab, layers=6, width=384, heads=6, ffn=1536, context=64):
        self.vocab, self.layers, self.width, self.heads, self.ffn, self.context = vocab, layers, width, heads, ffn, context


def rope(x, cos, sin):
    a, b = x[..., ::2], x[..., 1::2]
    return torch.stack((a * cos - b * sin, a * sin + b * cos), dim=-1).flatten(-2)


class Block(nn.Module):
    def __init__(self, c):
        super().__init__()
        self.heads = c.heads
        self.norm1 = nn.LayerNorm(c.width)
        self.qkv = nn.Linear(c.width, 3 * c.width, bias=False)
        self.proj = nn.Linear(c.width, c.width, bias=False)
        self.norm2 = nn.LayerNorm(c.width)
        self.up = nn.Linear(c.width, c.ffn, bias=False)
        self.down = nn.Linear(c.ffn, c.width, bias=False)

    def forward(self, x, cos, sin, mask):
        B, L, D = x.shape
        q, k, v = self.qkv(self.norm1(x)).view(B, L, 3, self.heads, D // self.heads).permute(2, 0, 3, 1, 4)
        q, k = rope(q, cos, sin), rope(k, cos, sin)
        attn = (q @ k.transpose(-2, -1)) / math.sqrt(D // self.heads) + mask
        y = (attn.softmax(-1) @ v).transpose(1, 2).reshape(B, L, D)
        x = x + self.proj(y)
        return x + self.down(F.gelu(self.up(self.norm2(x))))


class CharLM(nn.Module):
    def __init__(self, c):
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
        self.register_buffer("causal", torch.full((c.context, c.context), float("-inf")).triu(1)[None, None], persistent=False)
        self.apply(self._init)

    @staticmethod
    def _init(m):
        if isinstance(m, (nn.Linear, nn.Embedding)):
            nn.init.normal_(m.weight, std=0.02)

    def forward(self, ids):
        L = ids.shape[1]
        x = self.embed(ids)
        cos, sin, mask = self.cos[:, :, :L], self.sin[:, :, :L], self.causal[:, :, :L, :L]
        for block in self.blocks:
            x = block(x, cos, sin, mask)
        return self.norm(x) @ self.embed.weight.T


def batches(path, context, batch, device, seed):
    data = np.memmap(path, dtype=np.uint16, mode="r")
    rng = np.random.default_rng(seed)
    while True:
        starts = rng.integers(0, len(data) - context - 1, size=batch)
        chunk = np.stack([data[s:s + context + 1] for s in starts]).astype(np.int64)
        x = torch.from_numpy(chunk).to(device, non_blocking=True)
        yield x[:, :-1], x[:, 1:]


@torch.no_grad()
def evaluate(model, path, context, device, limit=2_000_000):
    data = np.fromfile(path, dtype=np.uint16)[:limit].astype(np.int64)
    n = (len(data) - 1) // context
    x = torch.from_numpy(data[:n * context + 1]).to(device)
    inputs, targets = x[:-1].view(n, context), x[1:].view(n, context)
    total, count = 0.0, 0
    for i in range(0, n, 256):
        with torch.autocast("cuda", dtype=torch.bfloat16):
            logits = model(inputs[i:i + 256])
        loss = F.cross_entropy(logits.float().reshape(-1, logits.shape[-1]), targets[i:i + 256].reshape(-1),
                               ignore_index=0, reduction="sum")
        total += loss.item()
        count += (targets[i:i + 256] != 0).sum().item()
    return total / count


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", required=True)
    parser.add_argument("--steps", type=int, default=30000)
    parser.add_argument("--batch", type=int, default=512)
    parser.add_argument("--lr", type=float, default=1e-3)
    parser.add_argument("--layers", type=int, default=6)
    parser.add_argument("--width", type=int, default=384)
    parser.add_argument("--heads", type=int, default=6)
    parser.add_argument("--ffn", type=int, default=1536)
    args = parser.parse_args()
    torch.manual_seed(7)
    device = "cuda"
    vocab = json.load(open(os.path.join(args.data, "vocab.json"), encoding="utf-8"))
    config = Config(len(vocab), args.layers, args.width, args.heads, args.ffn)
    model = CharLM(config).to(device)
    params = sum(p.numel() for p in model.parameters())
    print(f"parameters: {params / 1e6:.1f} M", flush=True)
    decay = [p for n, p in model.named_parameters() if p.dim() >= 2]
    no_decay = [p for n, p in model.named_parameters() if p.dim() < 2]
    optimizer = torch.optim.AdamW([{"params": decay, "weight_decay": 0.1}, {"params": no_decay, "weight_decay": 0}],
                                  lr=args.lr, betas=(0.9, 0.95), fused=True)
    compiled = torch.compile(model)
    stream = batches(os.path.join(args.data, "train.bin"), config.context, args.batch, device, 7)
    warmup = 1000
    start = time.time()
    log = []
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
                torch.save({"model": model.state_dict(), "config": vars(config), "step": step},
                           os.path.join(args.data, "charlm.pt"))
            log.append(row)
            print(json.dumps(row), flush=True)
    json.dump(log, open(os.path.join(args.data, "train-log.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
