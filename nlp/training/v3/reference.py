"""Run the paired v3 Core ML packages (CPU only) or the matching PyTorch checkpoint on one JSON case.

    python -m nlp.training.v3.reference --input data/build/v3_030/example.json
    python -m nlp.training.v3.reference --backend torch --data data/build/v3 --input case.json

JSON: {"context": "前文", "syllables": [["ㄓㄨㄥ", 1, 0, 0]], "candidates": [["中", "忠"]]}; each syllable is
(toneless reading, tone, cost bucket, implicit boundary).  --backend torch uses <data>/syllable.pt.  Prints the
supplied candidates and their log probabilities.  The Core ML backend needs macOS.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import numpy as np

from .common import OUT

NAMES = ["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids"]


def inputs(case: dict[str, Any], directory: Path = OUT) -> tuple[np.ndarray, dict[str, np.ndarray]]:
    """Model inputs for a case: context_ids and the five syllable inputs, validated against the interface limits."""
    chars = json.load(open(directory / "chars.json"))
    syllables = json.load(open(directory / "syllables.json"))
    cids, sids = {c: i for i, c in enumerate(chars)}, {s: i for i, s in enumerate(syllables)}
    cids["\n"] = 3
    text, typed, candidates = case["context"], case["syllables"], case["candidates"]
    if not isinstance(text, str) or not 1 <= len(typed) <= 64 or len(typed) != len(candidates):
        raise ValueError("require context text and 1..64 aligned syllables/candidate rows")
    count = max(map(len, candidates))
    if not 1 <= count <= 64 or any(not row or len(row) != len(set(row)) for row in candidates):
        raise ValueError("require 1..64 unique candidate characters per position")
    ids = np.zeros((1, len(typed), count), np.int32)
    for i, row in enumerate(candidates):
        for j, char in enumerate(row):
            if not isinstance(char, str) or len(char) != 1 or cids.get(char, 2) in (0, 1, 3):
                raise ValueError(f"invalid candidate character: {char!r}")
            ids[0, i, j] = cids.get(char, 2)
    for syllable, tone, cost, boundary in typed:
        if (
            syllable not in sids
            or syllable == "<pad>"
            or tone not in range(6)
            or cost not in range(4)
            or boundary not in range(2)
        ):
            raise ValueError(f"invalid syllable tuple: {(syllable, tone, cost, boundary)}")
    ctx = np.array([[1] + [cids.get(c, 2) for c in text[-255:]]], np.int32)
    values = [np.array([[sids[t[0]] for t in typed]], np.int32)]
    values += [np.array([[t[i] for t in typed]], np.int32) for i in range(1, 4)]
    return ctx, dict(zip(NAMES, values + [ids]))


class Runner:
    context: Any
    syllable: Any

    def __init__(self, backend: str = "coreml", directory: Path = OUT) -> None:
        self.backend, self.directory = backend, directory
        if backend == "coreml":
            import coremltools as ct

            self.context = ct.models.MLModel(
                str(directory / "ContextEncoder.mlpackage"), compute_units=ct.ComputeUnit.CPU_ONLY
            )
            self.syllable = ct.models.MLModel(
                str(directory / "SyllableEncoder.mlpackage"), compute_units=ct.ComputeUnit.CPU_ONLY
            )
        else:
            import torch

            from .models import ContextExport, SyllableExport, load_models

            torch.set_num_threads(4)
            context, syllable = load_models(directory / "syllable.pt")
            self.context, self.syllable = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()

    def predict(self, ctx: np.ndarray, typed: dict[str, np.ndarray]) -> tuple[dict[str, np.ndarray], np.ndarray]:
        if self.backend == "coreml":
            memory = self.context.predict({"context_ids": ctx})
            result = self.syllable.predict({**typed, **memory})["logprob"]
            return memory, result
        import torch

        with torch.no_grad():
            k, v = self.context(torch.from_numpy(ctx))
            output = self.syllable(*(torch.from_numpy(typed[n]) for n in NAMES), k, v)
        return {"memory_k": k.numpy(), "memory_v": v.numpy()}, output.numpy()


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--input", type=Path, required=True)
    p.add_argument("--backend", choices=["coreml", "torch"], default="coreml")
    p.add_argument("--data", type=Path, default=OUT)
    a = p.parse_args(argv)
    case = json.loads(a.input.read_text())
    ctx, typed = inputs(case, a.data)
    memory, logprob = Runner(a.backend, a.data).predict(ctx, typed)
    print(
        json.dumps(
            {"backend": a.backend, "candidates": case["candidates"], "logprob": logprob.tolist()},
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
