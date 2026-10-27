"""Convert the trained reranker (charlm.pt) to Core ML: Reranker.mlpackage, fp16, macOS 14.

    zt train reranker export
    python -m nlp.training.reranker.export [--data data/build/reranker] [--out DIR/Reranker.mlpackage]

The Core ML model takes `ids` [B, L] (input characters, starting with <bos>) and `targets` [B, L] (the next
characters) and returns `logprob` [B, L]: log P(target | everything before it).  B and L are flexible, so all
candidates are scored in one call and the app receives B·L numbers instead of B·L·V logits.  Needs coremltools (the
export dependency group); compile with `zt train reranker compile` on macOS.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path

import coremltools as ct
import numpy as np
import torch

from nlp.common import paths

from .train import CharLM, Config


class Scorer(torch.nn.Module):
    def __init__(self, model: CharLM) -> None:
        super().__init__()
        self.model = model

    def forward(self, ids: torch.Tensor, targets: torch.Tensor) -> torch.Tensor:
        logits = self.model(ids).float()
        return torch.log_softmax(logits, dim=-1).gather(-1, targets.unsqueeze(-1)).squeeze(-1)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--data", type=Path, default=paths.data("reranker"), help="directory with charlm.pt")
    parser.add_argument("--out", type=Path, help="package to write (default: <data>/Reranker.mlpackage)")
    args = parser.parse_args(argv)
    out = args.out or args.data / "Reranker.mlpackage"
    checkpoint = torch.load(os.path.join(args.data, "charlm.pt"), map_location="cpu")
    config = Config(**checkpoint["config"])
    model = CharLM(config)
    model.load_state_dict(checkpoint["model"])
    model.eval()
    scorer = Scorer(model).eval()
    example = (torch.ones(4, 24, dtype=torch.int32), torch.ones(4, 24, dtype=torch.int32))
    traced = torch.jit.trace(scorer, (example[0].long(), example[1].long()))
    shape = ct.Shape(shape=(ct.RangeDim(1, 32, default=8), ct.RangeDim(2, config.context, default=48)))
    mlmodel = ct.convert(
        traced,
        inputs=[
            ct.TensorType(name="ids", shape=shape, dtype=np.int32),
            ct.TensorType(name="targets", shape=shape, dtype=np.int32),
        ],
        outputs=[ct.TensorType(name="logprob", dtype=np.float32)],
        minimum_deployment_target=ct.target.macOS14,
        compute_precision=ct.precision.FLOAT16,
        convert_to="mlprogram",
    )
    mlmodel.short_description = "ZhuyinType reranker: character language model log probabilities"
    mlmodel.save(str(out))
    print("saved", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
