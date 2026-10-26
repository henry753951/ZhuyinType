"""Run paired Core ML packages (CPU only) or the matching PyTorch checkpoint.

python training/v3/reference.py --input data/build/v3/example.json
JSON: {"context": "前文", "syllables": [["ㄓㄨㄥ", 1, 0, 0]], "candidates": [["中", "忠"]]}
--backend torch uses data/build/v3/syllable.pt. Output contains the supplied character order and logprobs.
"""
import argparse
import json
from pathlib import Path
import numpy as np

from common import OUT

NAMES = ["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids"]


def inputs(case, directory=OUT):
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
        if syllable not in sids or syllable == "<pad>" or tone not in range(6) or cost not in range(4) or boundary not in range(2):
            raise ValueError(f"invalid syllable tuple: {(syllable, tone, cost, boundary)}")
    ctx = np.array([[1] + [cids.get(c, 2) for c in text[-255:]]], np.int32)
    values = [np.array([[sids[t[0]] for t in typed]], np.int32)]
    values += [np.array([[t[i] for t in typed]], np.int32) for i in range(1, 4)]
    return ctx, dict(zip(NAMES, values + [ids]))


class Runner:
    def __init__(self, backend="coreml", directory=OUT):
        self.backend, self.directory = backend, directory
        if backend == "coreml":
            import coremltools as ct
            self.context = ct.models.MLModel(str(directory / "ContextEncoder.mlpackage"), compute_units=ct.ComputeUnit.CPU_ONLY)
            self.syllable = ct.models.MLModel(str(directory / "SyllableEncoder.mlpackage"), compute_units=ct.ComputeUnit.CPU_ONLY)
        else:
            import torch
            from models import ContextExport, SyllableExport, load_models
            torch.set_num_threads(4)
            context, syllable = load_models(directory / "syllable.pt")
            self.context, self.syllable = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()

    def predict(self, ctx, typed):
        if self.backend == "coreml":
            memory = self.context.predict({"context_ids": ctx})
            result = self.syllable.predict({**typed, **memory})["logprob"]
            return memory, result
        import torch
        with torch.no_grad():
            k, v = self.context(torch.from_numpy(ctx))
            output = self.syllable(*(torch.from_numpy(typed[n]) for n in NAMES), k, v)
        return {"memory_k": k.numpy(), "memory_v": v.numpy()}, output.numpy()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--input", type=Path, required=True)
    p.add_argument("--backend", choices=["coreml", "torch"], default="coreml")
    p.add_argument("--data", type=Path, default=OUT)
    a = p.parse_args()
    case = json.loads(a.input.read_text())
    ctx, typed = inputs(case, a.data)
    memory, logprob = Runner(a.backend, a.data).predict(ctx, typed)
    print(json.dumps({"backend": a.backend, "candidates": case["candidates"], "logprob": logprob.tolist()}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
