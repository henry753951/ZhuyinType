"""Six-layer RoPE encoders with cached cross-attention projections."""
import math
import torch
from torch import nn
from torch.nn import functional as F

LAYERS, WIDTH, HEADS, FFN = 6, 512, 8, 2048


def rope(x, cos, sin):
    a, b = x[..., ::2], x[..., 1::2]
    return torch.stack((a * cos - b * sin, a * sin + b * cos), -1).flatten(-2)


def attention(q, k, v, causal=False, valid=None):
    if torch.jit.is_tracing():
        score = q @ k.transpose(-1, -2) / math.sqrt(WIDTH // HEADS)
        if causal:
            length = q.shape[-2]
            mask = torch.arange(length, device=q.device)
            score = score.masked_fill(mask[None, :] > mask[:, None], -1e4)
        if valid is not None:
            score = score.masked_fill(~valid[:, None, None, :], -1e4)
        return score.softmax(-1) @ v
    mask = valid[:, None, None, :] if valid is not None else None
    return F.scaled_dot_product_attention(q, k, v, attn_mask=mask, is_causal=causal)


def initialize(module):
    if isinstance(module, (nn.Linear, nn.Embedding)):
        nn.init.normal_(module.weight, std=.02)


class Block(nn.Module):
    def __init__(self, cross=False):
        super().__init__()
        self.norm1 = nn.LayerNorm(WIDTH)
        self.qkv = nn.Linear(WIDTH, 3 * WIDTH, bias=False)
        self.proj = nn.Linear(WIDTH, WIDTH, bias=False)
        self.norm2 = nn.LayerNorm(WIDTH)
        self.up = nn.Linear(WIDTH, FFN, bias=False)
        self.down = nn.Linear(FFN, WIDTH, bias=False)
        if cross:
            self.cross_norm = nn.LayerNorm(WIDTH)
            self.cross_q = nn.Linear(WIDTH, WIDTH, bias=False)
            self.cross_k = nn.Linear(WIDTH, WIDTH, bias=False)
            self.cross_v = nn.Linear(WIDTH, WIDTH, bias=False)
            self.cross_out = nn.Linear(WIDTH, WIDTH, bias=False)

    def forward(self, x, cos, sin, causal=False, memory=None, valid=None):
        batch, length, _ = x.shape
        q, k, v = self.qkv(self.norm1(x)).reshape(batch, length, 3, HEADS, WIDTH // HEADS).permute(2, 0, 3, 1, 4).unbind(0)
        q, k = rope(q, cos, sin), rope(k, cos, sin)
        x = x + self.proj(attention(q, k, v, causal, valid).transpose(1, 2).reshape(batch, length, WIDTH))
        if memory is not None:
            k, v = memory
            memory_valid = k.abs().sum(-1) > 0
            q = self.cross_q(self.cross_norm(x)).reshape(batch, length, HEADS, WIDTH // HEADS).transpose(1, 2)
            k = k.reshape(batch, -1, HEADS, WIDTH // HEADS).transpose(1, 2)
            v = v.reshape(batch, -1, HEADS, WIDTH // HEADS).transpose(1, 2)
            x = x + self.cross_out(attention(q, k, v, valid=memory_valid).transpose(1, 2).reshape(batch, length, WIDTH))
        return x + self.down(F.gelu(self.up(self.norm2(x))))


class Rotary(nn.Module):
    def __init__(self):
        super().__init__()
        half = WIDTH // HEADS // 2
        angles = torch.arange(256)[:, None] / (10000 ** (torch.arange(half) / half))[None]
        self.register_buffer("cos", angles.cos()[None, None], persistent=False)
        self.register_buffer("sin", angles.sin()[None, None], persistent=False)


class ContextEncoder(Rotary):
    def __init__(self, vocab):
        super().__init__()
        self.embed = nn.Embedding(vocab, WIDTH)
        self.blocks = nn.ModuleList(Block() for _ in range(LAYERS))
        self.norm = nn.LayerNorm(WIDTH)
        self.apply(initialize)

    def hidden(self, ids):
        length = ids.shape[1]
        x = self.embed(ids)
        for block in self.blocks:
            x = block(x, self.cos[:, :, :length], self.sin[:, :, :length], causal=True)
        return self.norm(x)

    def forward(self, ids):
        return F.linear(self.hidden(ids), self.embed.weight)


class SyllableEncoder(Rotary):
    def __init__(self, vocab, syllables):
        super().__init__()
        self.syllable = nn.Embedding(syllables, WIDTH)
        self.tone = nn.Embedding(6, WIDTH)
        self.cost = nn.Embedding(4, WIDTH)
        self.boundary = nn.Embedding(2, WIDTH)
        self.chars = nn.Embedding(vocab, WIDTH)
        self.bias = nn.Embedding(vocab, 1)
        self.blocks = nn.ModuleList(Block(cross=True) for _ in range(LAYERS))
        self.norm = nn.LayerNorm(WIDTH)
        self.apply(initialize)

    def project(self, hidden, valid=None):
        k = torch.stack([b.cross_k(hidden) for b in self.blocks])
        v = torch.stack([b.cross_v(hidden) for b in self.blocks])
        if valid is not None:
            k = k * valid[None, :, :, None]
            v = v * valid[None, :, :, None]
        return k, v

    def forward(self, syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids, memory_k, memory_v):
        length = syllable_ids.shape[1]
        x = self.syllable(syllable_ids) + self.tone(tone_ids) + self.cost(cost_bucket) + self.boundary(boundary)
        for i, block in enumerate(self.blocks):
            x = block(x, self.cos[:, :, :length], self.sin[:, :, :length],
                      memory=(memory_k[i], memory_v[i]), valid=syllable_ids != 0)
        x = self.norm(x)
        logits = torch.matmul(self.chars(candidate_ids), x.unsqueeze(-1)).squeeze(-1) / math.sqrt(WIDTH)
        logits = logits + self.bias(candidate_ids).squeeze(-1)
        return logits.float().masked_fill(candidate_ids == 0, -1e4).log_softmax(-1)

    def initialize(self, context):
        self.chars.weight.data.copy_(context.embed.weight)
        self.norm.load_state_dict(context.norm.state_dict())
        for block, source in zip(self.blocks, context.blocks):
            block.load_state_dict(source.state_dict(), strict=False)
            _, k, v = source.qkv.weight.chunk(3, 0)
            block.cross_k.weight.data.copy_(k)
            block.cross_v.weight.data.copy_(v)
        nn.init.zeros_(self.bias.weight)


class ContextExport(nn.Module):
    def __init__(self, context, syllable):
        super().__init__()
        self.context, self.syllable = context, syllable

    def forward(self, context_ids):
        k, v = self.syllable.project(self.context.hidden(context_ids), context_ids != 0)
        return k[:, 0].half(), v[:, 0].half()


class SyllableExport(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids, memory_k, memory_v):
        return self.model(syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids,
                          memory_k.float().unsqueeze(1), memory_v.float().unsqueeze(1))


def load_models(path, device="cpu"):
    ckpt = torch.load(path, map_location="cpu", weights_only=False)
    context = ContextEncoder(ckpt["vocab"])
    syllable = SyllableEncoder(ckpt["vocab"], ckpt["syllables"])
    context.load_state_dict(ckpt["context"])
    syllable.load_state_dict(ckpt["syllable"])
    return context.to(device).eval(), syllable.to(device).eval()
