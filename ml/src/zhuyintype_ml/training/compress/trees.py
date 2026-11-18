"""Prefix trees shaped like the engine's reranker calls, and the check of a tree model against PyTorch on them.

`zt train compress parity` runs it on the Core ML tree (CPU, the macOS app's compute unit) and `desktop_models` on
the ORT model the Linux and Windows apps load.  In fp16, Core ML on the CPU scored some node counts 4 to 8 nats off
or NaN, which one small tree never reaches (research log 2026-11-18, `chat-reranker`).
"""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

import numpy as np

INPUTS = ["ids", "positions", "ancestors", "parents", "targets"]


def pack(prefix: list[int], candidates: list[list[int]]) -> dict[str, np.ndarray]:
    """The model inputs of one prefix tree, as engine/crates/zhuyin-models/src/tree.rs packs them."""
    ids, positions = list(prefix), list(range(len(prefix)))
    parents = [max(0, i - 1) for i in range(len(prefix))]
    children: dict[tuple[int, int], int] = {}
    for candidate in candidates:
        parent = len(prefix) - 1
        for token in candidate:
            if (parent, token) not in children:
                children[parent, token] = len(ids)
                ids.append(token)
                positions.append(positions[parent] + 1)
                parents.append(parent)
            parent = children[parent, token]
    states = sorted(set(parents))
    index = {node: i for i, node in enumerate(states)}
    ancestors = np.zeros((len(states), len(states)), np.int32)
    for row, node in enumerate(states):
        while True:
            ancestors[row, index[node]] = 1
            if node == 0:
                break
            node = parents[node]
    return {
        "ids": np.array([[ids[n] for n in states]], np.int32),
        "positions": np.array([[positions[n] for n in states]], np.int32),
        "ancestors": ancestors,
        "parents": np.array([index[p] for p in parents], np.int32),
        "targets": np.array(ids, np.int32),
    }


def sample_trees(count: int = 600, seed: int = 0) -> list[dict[str, np.ndarray]]:
    """Trees shaped like the engine's calls: up to 24 context characters, then 1 to 16 candidates that share
    prefixes and branch, within the model's 64 positions."""
    rng = np.random.default_rng(seed)
    trees: list[dict[str, np.ndarray]] = []
    while len(trees) < count:
        prefix = [1, *rng.integers(3, 3000, int(rng.integers(0, 25))).tolist()]
        base = rng.integers(3, 3000, int(rng.integers(2, 31))).tolist()
        candidates = []
        for length in range(max(1, len(base) - int(rng.integers(1, 17)) + 1), len(base) + 1):
            candidate = base[:length]
            if rng.random() < 0.5:
                candidate[int(rng.integers(0, length))] = int(rng.integers(3, 3000))
            candidates.append(candidate)
        if len(prefix) + len(base) <= 64:
            trees.append(pack(prefix, candidates))
    return trees


def check_trees(
    checkpoint: Path, predict: Callable[[dict[str, np.ndarray]], Any], limit: float = 0.5, count: int = 600
) -> dict[str, Any]:
    """`predict` (one packed tree → the log-probability of every target) against PyTorch's TreeScorer on the same
    checkpoint: the largest error and the node counts where a tree is off by more than `limit` nats or not finite.
    The first target scores the root against itself and is ignored (reranker/export_tree.py)."""
    import torch

    from zhuyintype_ml.training.reranker.export_tree import TreeScorer
    from zhuyintype_ml.training.reranker.train import CharLM, Config

    saved = torch.load(checkpoint, map_location="cpu", weights_only=False)
    charlm = CharLM(Config(**saved["config"]))
    charlm.load_state_dict(saved["model"])
    scorer = TreeScorer(charlm.eval())
    worst, errors, failed = 0.0, list[float](), dict[int, float]()
    for inputs in sample_trees(count):
        with torch.no_grad():
            expected = scorer(*(torch.from_numpy(inputs[k]) for k in INPUTS)).numpy()
        got = np.asarray(predict(inputs), np.float64).reshape(-1)
        error = float(np.max(np.abs(got - expected)[1:])) if np.all(np.isfinite(got)) else float("inf")
        worst = max(worst, error)
        errors.append(error)
        if not error <= limit:
            nodes = int(inputs["ids"].shape[1])
            failed[nodes] = max(failed.get(nodes, 0.0), error)
    return {
        "trees": count,
        "max_abs": worst,
        "median_max_abs": float(np.median(errors)),
        "failed_node_counts": dict(sorted(failed.items())),
    }
