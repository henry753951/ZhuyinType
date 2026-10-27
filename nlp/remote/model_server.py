#!/usr/bin/env python3
"""Batched fp32 checkpoint inference over a private Unix JSON-lines socket."""
import argparse
from collections import OrderedDict
from concurrent.futures import Future
import json
import os
from pathlib import Path
import queue
import socketserver
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'nlp/training/reranker'), str(ROOT / 'nlp/training/v3')]
import torch
from train import CharLM, Config
from models import load_models


class Models:
    def __init__(self, reranker, v3, device, memory_gb):
        torch.set_num_threads(1)
        if device == 'cuda':
            torch.cuda.set_per_process_memory_fraction(memory_gb * 2**30 / torch.cuda.get_device_properties(0).total_memory)
            torch.backends.cuda.matmul.allow_tf32 = False
            torch.backends.cudnn.allow_tf32 = False
        self.device = device
        checkpoint = torch.load(reranker, map_location='cpu', weights_only=False)
        self.reranker = CharLM(Config(**checkpoint['config']))
        self.reranker.load_state_dict(checkpoint['model'])
        self.reranker = self.reranker.to(device).eval()
        self.context, self.syllable = load_models(v3, device)
        self.cache = OrderedDict()
        self.requests = self.batches = 0

    def tensor(self, values):
        return torch.tensor(values, dtype=torch.long, device=self.device)

    def validate(self, r):
        def vector(values, limit, length):
            if not isinstance(values, list) or len(values) != length or any(type(x) is not int or not 0 <= x < limit for x in values):
                raise ValueError('invalid integer input')
        if r['op'] == 'rerank':
            b, length = r['batch'], r['length']
            if type(b) is not int or type(length) is not int or not 1 <= b <= 32 or not 2 <= length <= 64:
                raise ValueError('reranker shape outside export limits')
            for key in ('ids', 'targets'):
                vector(r[key], self.reranker.c.vocab, b * length)
        elif r['op'] == 'syllable':
            n = len(r['ids'])
            if not 1 <= n <= 64 or not 1 <= len(r['context']) <= 256 or len(r['candidates']) != n:
                raise ValueError('invalid syllable shape')
            vector(r['context'], self.context.embed.num_embeddings, len(r['context']))
            for key, limit in [('ids', self.syllable.syllable.num_embeddings), ('tones', 6), ('costs', 4), ('boundary', 2)]:
                vector(r[key], limit, n)
            width = len(r['candidates'][0])
            if not 1 <= width <= 64:
                raise ValueError('invalid candidate width')
            for row in r['candidates']:
                vector(row, self.syllable.chars.num_embeddings, width)
        else:
            raise ValueError('unknown operation')

    @torch.inference_mode()
    def predict(self, requests):
        self.requests += len(requests)
        self.batches += 1
        if requests[0]['op'] == 'rerank':
            length = max(r['length'] for r in requests)
            rows, labels = [], []
            for r in requests:
                for start in range(0, len(r['ids']), r['length']):
                    rows.append(r['ids'][start:start + r['length']] + [0] * (length - r['length']))
                    labels.append(r['targets'][start:start + r['length']] + [0] * (length - r['length']))
            logits = self.reranker(self.tensor(rows)).float()
            scores = logits.log_softmax(-1).gather(-1, self.tensor(labels).unsqueeze(-1)).squeeze(-1).cpu()
            output, start = [], 0
            for r in requests:
                output.append(scores[start:start + r['batch'], :r['length']].reshape(-1).tolist())
                start += r['batch']
            return output
        # Grouping identical shapes avoids changing bidirectional attention or candidate normalization by padding.
        memories = []
        for r in requests:
            key = tuple(r['context'])
            if key not in self.cache:
                ids = self.tensor([key])
                k, v = self.syllable.project(self.context.hidden(ids), ids != 0)
                # Match the exported fp16 memory boundary, even though the GPU models use fp32.
                self.cache[key] = (k.half().float(), v.half().float())
                if len(self.cache) > 32:
                    self.cache.popitem(last=False)
            self.cache.move_to_end(key)
            memories.append(self.cache[key])
        k = torch.cat([m[0] for m in memories], dim=1)
        v = torch.cat([m[1] for m in memories], dim=1)
        inputs = [self.tensor([r[key] for r in requests]) for key in ('ids', 'tones', 'costs', 'boundary', 'candidates')]
        return self.syllable(*inputs, k, v).cpu().tolist()


def group_key(r):
    if r['op'] == 'rerank':
        return ('rerank',)
    return ('syllable', len(r['ids']), len(r['candidates'][0]), len(r['context']))


def worker(models, pending, batch_rows):
    while True:
        batch = [pending.get()]
        deadline = time.monotonic() + .002
        while len(batch) < 64:
            try:
                batch.append(pending.get(timeout=max(0, deadline - time.monotonic())))
            except queue.Empty:
                break
        groups = {}
        for request, future in batch:
            groups.setdefault(group_key(request), []).append((request, future))
        for group in groups.values():
            chunk, size = [], 0
            for item in group + [None]:
                cost = 0 if item is None else item[0].get('batch', 1)
                limit = batch_rows if group[0][0]['op'] == 'rerank' else 8
                if chunk and (item is None or size + cost > limit):
                    try:
                        outputs = models.predict([r for r, _ in chunk])
                        for (_, future), output in zip(chunk, outputs):
                            future.set_result(output)
                    except Exception as error:
                        for _, future in chunk:
                            future.set_exception(error)
                    chunk, size = [], 0
                if item is not None:
                    chunk.append(item)
                    size += cost


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--socket', required=True)
    p.add_argument('--reranker', type=Path, required=True)
    p.add_argument('--v3', type=Path, required=True)
    p.add_argument('--device', choices=['cuda', 'cpu'], default='cuda')
    p.add_argument('--memory-gb', type=float, default=2)
    p.add_argument('--batch-rows', type=int, default=32)
    a = p.parse_args()
    if not 1 <= a.batch_rows <= 32 or not 0 < a.memory_gb <= 4:
        p.error('require 1..32 batch rows and 0..4 GiB memory budget')
    models = Models(a.reranker, a.v3, a.device, a.memory_gb)
    pending = queue.Queue(maxsize=128)
    threading.Thread(target=worker, args=(models, pending, a.batch_rows), daemon=True).start()

    class Handler(socketserver.StreamRequestHandler):
        def handle(self):
            while True:
                line = self.rfile.readline(1_048_577)
                if not line:
                    return
                try:
                    if len(line) > 1_048_576:
                        raise ValueError('request too large')
                    request = json.loads(line)
                    models.validate(request)
                    future = Future()
                    pending.put((request, future), timeout=120)
                    response = {'output': future.result(timeout=120)}
                except Exception as error:
                    response = {'error': str(error)}
                self.wfile.write(json.dumps(response, allow_nan=False, separators=(',', ':')).encode() + b'\n')

    class Server(socketserver.ThreadingUnixStreamServer):
        daemon_threads = True
        request_queue_size = 128

    os.umask(0o077)
    # Refuse an existing socket: never unlink another evaluator's live server.
    with Server(a.socket, Handler) as server:
        print(json.dumps({'ready': a.socket, 'torch': torch.__version__, 'device': a.device, 'precision': 'fp32',
                          'memory_gb': a.memory_gb}), flush=True)
        try:
            server.serve_forever()
        finally:
            Path(a.socket).unlink(missing_ok=True)


if __name__ == '__main__':
    main()
