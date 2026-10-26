#!/usr/bin/env python3
"""Compare ONNX against fp32 Torch, saved Core ML vectors, and real reranker sentences."""
import argparse
import base64
import json
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'training/reranker'), str(ROOT / 'training/v3'), str(Path(__file__).parent)]
import numpy as np


def real_reranker(directory):
    vocab = json.loads((directory / 'vocab.json').read_text())
    index = {c: i for i, c in enumerate(vocab) if i > 2 and len(c) == 1}
    cases = []
    for context, sentences in [('', ['今天的天氣很好', '今天的天氣很號', '我想喝一杯咖啡']),
                               ('我們住在', ['臺灣', '台灣', '臺彎']),
                               ('前面已經提到今天的事情。' * 3, ['這件事情已經完成了', '這件事情已經完城了']),
                               ('', ['好', '𠮷'])]:
        encoded = [[index.get(c, 2) for c in s] for s in sentences]
        prefix = [1] + [index.get(c, 2) for c in context[-24:]]
        length = max(2, len(prefix) - 1 + max(map(len, encoded)))
        ids = np.zeros((len(sentences), length), np.int32)
        targets = ids.copy()
        for i, row in enumerate(encoded):
            tokens = prefix + row
            ids[i, :len(tokens) - 1] = tokens[:-1]
            targets[i, :len(tokens) - 1] = tokens[1:]
        cases.append({'context': context, 'sentences': sentences, 'ids': ids.tolist(), 'targets': targets.tolist()})
    return cases


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--coreml', action='store_true')
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--reranker', type=Path, default=ROOT / 'data/build/reranker')
    p.add_argument('--v3', type=Path, default=ROOT / 'data/build/v3')
    a = p.parse_args()
    if a.coreml:
        import coremltools as ct
        model = ct.models.MLModel(str(a.reranker / 'Reranker.mlpackage'), compute_units=ct.ComputeUnit.CPU_ONLY)
        cases = real_reranker(a.reranker)
        for case in cases:
            case['coreml'] = model.predict({n: np.array(case[n], np.int32) for n in ['ids', 'targets']})['logprob'].tolist()
        a.output.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + '\n')
        return
    import torch
    from export_onnx import Scorer, session, NAMES
    from train import CharLM, Config
    from models import load_models, ContextExport, SyllableExport
    torch.set_num_threads(1)
    checkpoint = torch.load(a.reranker / 'charlm.pt', map_location='cpu', weights_only=False)
    model = CharLM(Config(**checkpoint['config']))
    model.load_state_dict(checkpoint['model'])
    rr = Scorer(model.eval()).eval()
    context, syllable = load_models(a.v3 / 'syllable.pt')
    cw, sw = ContextExport(context, syllable).eval(), SyllableExport(syllable).eval()
    vectors = json.loads((a.v3 / 'test_vectors.json').read_text())['vectors']
    real = json.loads((a.output.parent / 'reranker-coreml.json').read_text())
    report = {}
    for suffix in ['.onnx', '.fp16.onnx']:
        try:
            cr, sr, r = (session(d / (n + suffix)) for d, n in [(a.v3, 'ContextEncoder'), (a.v3, 'SyllableEncoder'), (a.reranker, 'Reranker')])
            rows = []
            for vector in vectors:
                values = {n: np.array(v, np.int32) for n, v in vector['inputs'].items()}
                ctx = {'context_ids': values.pop('context_ids')}
                start = time.monotonic()
                memory = dict(zip(['memory_k', 'memory_v'], cr.run(None, ctx)))
                got = sr.run(None, {**values, **memory})[0]
                seconds = time.monotonic() - start
                with torch.no_grad():
                    k, v = cw(torch.from_numpy(ctx['context_ids']))
                    expected = sw(*(torch.from_numpy(values[n]) for n in NAMES), k, v).numpy()
                valid = values['candidate_ids'] != 0
                ml = np.array(vector['expected']['logprob'])
                memories = {n: np.frombuffer(base64.b64decode(vector['expected'][n]['data']), dtype='<f2').reshape(vector['expected'][n]['shape']) for n in ['memory_k', 'memory_v']}
                isolated = sr.run(None, {**values, **memories})[0]
                delta = float(np.max(abs(got[valid] - expected[valid])))
                rows.append({'id': vector['id'], 'torch_abs': delta,
                             'coreml_abs': float(np.max(abs(got[valid] - ml[valid]))),
                             'coreml_same_memory_abs': float(np.max(abs(isolated[valid] - ml[valid]))),
                             'memory_abs': max(float(np.max(abs(memory[n].astype(np.float32) - memories[n].astype(np.float32)))) for n in memories),
                             'top1_coreml': float(np.mean(got.argmax(-1) == ml.argmax(-1))), 'seconds': seconds})
                np.testing.assert_allclose(np.exp(got).sum(-1), 1, atol=.002, rtol=.002)
                assert np.isfinite(got).all() and np.all(got[~valid] < -9990)
                assert delta < .1, rows[-1]
            rerank = []
            for case in real:
                inputs = {n: np.array(case[n], np.int32) for n in ['ids', 'targets']}
                with torch.no_grad():
                    expected = rr(*(torch.from_numpy(inputs[n]) for n in ['ids', 'targets'])).numpy()
                got = r.run(None, inputs)[0]
                rerank.append({'sentences': case['sentences'], 'torch_abs': float(np.max(abs(got - expected))),
                               'coreml_abs': float(np.max(abs(got - np.array(case['coreml']))))})
                assert rerank[-1]['coreml_abs'] < .1, rerank[-1]
                if suffix == '.onnx':
                    np.testing.assert_allclose(got, expected, atol=2e-4, rtol=2e-5)
            report[suffix] = {'v3': rows, 'reranker': rerank}
        except Exception as error:
            report[suffix] = {'error': str(error)}
            if suffix == '.onnx':
                a.output.write_text(json.dumps(report, indent=2) + '\n')
                raise
    a.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))

if __name__ == '__main__':
    main()
