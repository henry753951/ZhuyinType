#!/usr/bin/env python3
"""Export matched checkpoint weights; dynamic int32 inputs, fp32 logprob, fp16 v3 memory."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'training/reranker'), str(ROOT / 'training/v3')]
import numpy as np
import onnx
import onnxruntime as ort
import torch
from train import CharLM, Config
from models import ContextExport, SyllableExport, load_models

NAMES = ['syllable_ids', 'tone_ids', 'cost_bucket', 'boundary', 'candidate_ids']

class Scorer(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, ids, targets):
        return self.model(ids).float().log_softmax(-1).gather(-1, targets.long().unsqueeze(-1)).squeeze(-1)


def session(path):
    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    return ort.InferenceSession(str(path), options, providers=['CPUExecutionProvider'])


def export(model, values, path, names, outputs, axes, fp16):
    with torch.no_grad():
        torch.onnx.export(model, values, str(path), input_names=names, output_names=outputs,
                          dynamic_axes=axes, opset_version=17, dynamo=False)
    onnx.checker.check_model(str(path))
    if fp16:
        half = copy.deepcopy(model).half().eval()
        if isinstance(half, SyllableExport):
            half = HalfSyllable(half.model).eval()
        half_values = tuple(v.half() if v.is_floating_point() else v for v in values)
        with torch.no_grad():
            torch.onnx.export(half, half_values, str(path.with_suffix('.fp16.onnx')),
                              input_names=names, output_names=outputs, dynamic_axes=axes,
                              opset_version=17, dynamo=False)
        onnx.checker.check_model(str(path.with_suffix('.fp16.onnx')))


class HalfSyllable(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids, memory_k, memory_v):
        return self.model(syllable_ids, tone_ids, cost_bucket, boundary, candidate_ids,
                          memory_k.unsqueeze(1), memory_v.unsqueeze(1))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--reranker-checkpoint', type=Path, required=True)
    p.add_argument('--reranker', type=Path, default=ROOT / 'data/build/reranker')
    p.add_argument('--v3', type=Path, default=ROOT / 'data/build/v3')
    p.add_argument('--fp16', action='store_true')
    a = p.parse_args()
    torch.set_num_threads(1)
    checkpoint = torch.load(a.reranker_checkpoint, map_location='cpu', weights_only=False)
    model = CharLM(Config(**checkpoint['config']))
    model.load_state_dict(checkpoint['model'])
    scorer = Scorer(model.eval()).eval()
    ids = torch.ones((4, 24), dtype=torch.int32)
    export(scorer, (ids, ids), a.reranker / 'Reranker.onnx', ['ids', 'targets'], ['logprob'],
           {n: {0: 'batch', 1: 'length'} for n in ['ids', 'targets', 'logprob']}, a.fp16)
    context, syllable = load_models(a.v3 / 'syllable.pt')
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    ctx = torch.ones((1, 12), dtype=torch.int32)
    with torch.no_grad():
        k, v = cw(ctx)
    typed = [torch.ones((1, 10), dtype=torch.int32) for _ in range(4)]
    candidates = torch.ones((1, 10, 8), dtype=torch.int32) * 4
    export(cw, (ctx,), a.v3 / 'ContextEncoder.onnx', ['context_ids'], ['memory_k', 'memory_v'],
           {'context_ids': {1: 'context'}, 'memory_k': {1: 'context'}, 'memory_v': {1: 'context'}}, a.fp16)
    axes = {n: {1: 'length'} for n in NAMES[:4]}
    axes.update({n: {1: 'length', 2: 'candidates'} for n in ['candidate_ids', 'logprob']})
    axes.update({n: {1: 'context'} for n in ['memory_k', 'memory_v']})
    export(sw, tuple(typed + [candidates, k, v]), a.v3 / 'SyllableEncoder.onnx',
           NAMES + ['memory_k', 'memory_v'], ['logprob'], axes, a.fp16)
    # Dynamic dimensions and odd lengths catch frozen trace shapes before any full evaluation.
    rr = session(a.reranker / 'Reranker.onnx')
    rng = np.random.default_rng(7)
    checks = []
    for batch, length in [(1, 2), (3, 7), (16, 48), (32, 64)]:
        x = rng.integers(0, model.c.vocab, (batch, length), dtype=np.int32)
        with torch.no_grad():
            expected = scorer(torch.from_numpy(x), torch.from_numpy(x)).numpy()
        actual = rr.run(None, {'ids': x, 'targets': x})[0]
        np.testing.assert_allclose(actual, expected, atol=2e-4, rtol=2e-5)
        checks.append({'batch': batch, 'length': length, 'max_abs_error': float(np.max(abs(actual - expected)))})
    paths = [a.reranker_checkpoint, a.v3 / 'context.pt', a.v3 / 'syllable.pt']
    paths += list(a.reranker.glob('*.onnx')) + list(a.v3.glob('*.onnx'))
    report = {'torch': torch.__version__, 'onnx': onnx.__version__, 'onnxruntime': ort.__version__,
              'sha256': {str(x): hashlib.sha256(x.read_bytes()).hexdigest() for x in paths}, 'dynamic_reranker': checks}
    (a.v3 / 'onnx-export.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2), flush=True)

if __name__ == '__main__':
    main()
