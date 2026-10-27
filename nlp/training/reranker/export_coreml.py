#!/usr/bin/env python3
"""Convert the trained reranker to Core ML.

    python export_coreml.py --data ~/zhuyintype/reranker --out ~/zhuyintype/reranker/Reranker.mlpackage

The Core ML model takes `ids` [B, L] (input characters, starting with <bos>) and `targets` [B, L] (the next
characters) and returns `logprob` [B, L]: log P(target | everything before it).  B and L are flexible, so all
candidates are scored in one call and the app receives B·L numbers instead of B·L·V logits.
"""
import argparse
import json
import os

import coremltools as ct
import numpy as np
import torch

from train import CharLM, Config


class Scorer(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, ids, targets):
        logits = self.model(ids).float()
        return torch.log_softmax(logits, dim=-1).gather(-1, targets.unsqueeze(-1)).squeeze(-1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
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
        inputs=[ct.TensorType(name="ids", shape=shape, dtype=np.int32),
                ct.TensorType(name="targets", shape=shape, dtype=np.int32)],
        outputs=[ct.TensorType(name="logprob", dtype=np.float32)],
        minimum_deployment_target=ct.target.macOS14,
        compute_precision=ct.precision.FLOAT16,
        convert_to="mlprogram",
    )
    mlmodel.short_description = "ZhuyinType reranker: character language model log probabilities"
    mlmodel.save(args.out)
    print("saved", args.out)


if __name__ == "__main__":
    main()
