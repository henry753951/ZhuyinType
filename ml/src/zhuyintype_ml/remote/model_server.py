"""Batched fp32 PyTorch inference for the reranker and v3 over a private Unix socket, for evaluations on a GPU: every
zhuyin-cli process sends its model calls here instead of running ONNX on its own core.

    zt models serve --device cuda -- eval conversion --set colloquial-dev     # the usual way
    zt --remote eval conversion --set colloquial-dev                           # the same on the GPU host (default)
    uv run --group export python -m zhuyintype_ml.remote.model_server --socket S \
        --reranker R/charlm.pt --v3 V/syllable.pt

One thread serves every connection.  It reads whatever requests have arrived from all clients, runs all reranker
requests as one batch and all character-model requests as another, and answers.  While the GPU works, the next
requests queue up in the sockets, so batches grow with the number of clients and no request waits for a timer.

- Reranker: each request's rows (the n-best sentences after one context) become a prefix tree, and the round's trees
  are packed into one sequence where a node attends to its ancestors only (RerankerTree's scoring, which the ONNX path
  uses too).  About a quarter of the rows' tokens remain.  Positions after a row's last target are answered with 0.
- Character model: requests are padded to bucket shapes; padded syllables, memory rows and candidates are masked.
- Every forward shape is captured once as a CUDA graph and replayed with one launch.
- fp32 with TF32 off, as the ONNX models.  --workers N forks N servers on the same socket (each with its own CUDA
  context; they time-share the GPU, which measured barely faster than one).

Two wire formats on the same socket; the first byte of each request tells them apart:

- JSON lines (the original protocol):
  {"op": "rerank", "ids": [...], "targets": [...], "batch": B, "length": L}  ->  {"output": [B*L floats]}
  {"op": "syllable", "context": [...], "ids": [...], "tones": [...], "costs": [...], "boundary": [...],
   "candidates": [[...], ...]}  ->  {"output": [[floats], ...]}
  An error is answered with {"error": "..."}.
- Binary frames (zhuyin-cli's default; no text encoding on either side).  Little-endian.  Every frame is an 8-byte
  header, marker 0x00 (a JSON request starts with "{"), kind, version 1 (u16), payload length in bytes (u32), then the
  payload.  Requests: kind 1 rerank, payload int32 B, L, ids[B*L], targets[B*L]; kind 2 syllable, payload int32 C, N, W,
  context[C], ids[N], tones[N], costs[N], boundary[N], candidates[N*W] (rows padded with 0).  Responses: kind 0 with
  float32 values (rerank B*L, syllable N*W, rows of W), or kind 1 with a UTF-8 error message.

The server writes a JSON line of counters to its log every minute and when it stops: requests and batches per model,
average batch size and the share of the wall time spent computing.
"""

from __future__ import annotations

import argparse
import json
import os
import selectors
import signal
import socket
import struct
import sys
import time
import traceback
from collections import OrderedDict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np
import torch

from zhuyintype_ml.training.reranker.train import CharLM, Config
from zhuyintype_ml.training.v3.models import load_models

HEADER = struct.Struct("<BBHI")
MARKER, VERSION = 0, 1
RERANK, SYLLABLE = 1, 2
OK, ERROR = 0, 1
MAX_REQUEST = 1 << 20
# Prefix-tree nodes per reranker forward (attention over the packed forest grows with its square).
RERANK_NODES = 2048
# Candidate slots (requests × syllables × candidates) per syllable forward.
SYLLABLE_SLOTS = 1 << 18
SYLLABLE_REQUESTS = 128
# Server processes (--workers): one; more share the GPU by time slices.
WORKERS = 1
# Context memories kept on the GPU (fp16), by context ids.
MEMORY_BYTES = 1 << 30


class RequestError(ValueError):
    """A request the server answers with an error (the connection stays open)."""


@dataclass
class Rerank:
    ids: np.ndarray  # (batch, length) int64
    targets: np.ndarray


@dataclass
class Syllable:
    context: tuple[int, ...]
    ids: np.ndarray  # (n,) int64
    tones: np.ndarray
    costs: np.ndarray
    boundary: np.ndarray
    candidates: np.ndarray  # (n, width) int64
    widths: list[int] | None = None  # JSON rows of other widths (answered at their own width)


Request = Rerank | Syllable


@dataclass
class Counters:
    started: float = field(default_factory=time.monotonic)
    busy: float = 0.0
    requests: dict[str, int] = field(default_factory=lambda: {"rerank": 0, "syllable": 0, "context": 0})
    batches: dict[str, int] = field(default_factory=lambda: {"rerank": 0, "syllable": 0, "context": 0})
    seconds: dict[str, float] = field(default_factory=lambda: {"rerank": 0.0, "syllable": 0.0})
    clients: int = 0
    rounds: int = 0
    errors: int = 0

    def summary(self) -> dict[str, Any]:
        wall = time.monotonic() - self.started
        return {
            "wall_s": round(wall, 1),
            "busy_share": round(self.busy / wall, 3) if wall else 0,
            "clients": self.clients,
            "rounds": self.rounds,
            "requests": self.requests,
            "batches": self.batches,
            "mean_batch": {k: round(self.requests[k] / v, 1) for k, v in self.batches.items() if v},
            "seconds": {k: round(v, 1) for k, v in self.seconds.items()},
            "errors": self.errors,
        }


def ints(values: Any, shape: tuple[int, ...], limit: int, name: str) -> np.ndarray:
    """`values` as an int64 array of `shape` with every value in [0, limit)."""
    array = np.asarray(values)
    if array.shape != shape or (array.size and array.dtype.kind not in "iu"):
        raise RequestError(f"invalid {name}")
    array = array.astype(np.int64, copy=False)
    if array.size and (int(array.min()) < 0 or int(array.max()) >= limit):
        raise RequestError(f"{name} out of range")
    return array


class Models:
    def __init__(self, reranker: Path, v3: Path, device: str, memory_gb: float, graphs: bool = True) -> None:
        torch.set_num_threads(1)
        if device == "cuda":
            total = torch.cuda.get_device_properties(0).total_memory
            torch.cuda.set_per_process_memory_fraction(min(1.0, memory_gb * 2**30 / total))
            torch.backends.cuda.matmul.allow_tf32 = False
            torch.backends.cudnn.allow_tf32 = False
        self.device = device
        checkpoint = torch.load(reranker, map_location="cpu", weights_only=False)
        self.reranker = CharLM(Config(**checkpoint["config"]))
        self.reranker.load_state_dict(checkpoint["model"])
        self.reranker = self.reranker.to(device).eval()
        self.context, self.syllable = load_models(v3, device)
        self.vocab = self.reranker.c.vocab
        self.limits = {
            "context": self.context.embed.num_embeddings,
            "ids": self.syllable.syllable.num_embeddings,
            "tones": 6,
            "costs": 4,
            "boundary": 2,
            "candidates": self.syllable.chars.num_embeddings,
        }
        # Context ids -> (k, v), each (layers, length, width) fp16: the exported context encoder's output precision.
        self.memories: OrderedDict[tuple[int, ...], tuple[torch.Tensor, torch.Tensor]] = OrderedDict()
        self.memory_bytes = 0
        self.graphs = Graphs(device == "cuda" and graphs)

    # MARK: - Requests

    def rerank_request(self, batch: Any, length: Any, ids: Any, targets: Any) -> Rerank:
        if type(batch) is not int or type(length) is not int or not 1 <= batch <= 32 or not 2 <= length <= 64:
            raise RequestError("reranker shape outside export limits")
        flat, shape = (batch * length,), (batch, length)
        return Rerank(
            ints(ids, flat, self.vocab, "ids").reshape(shape), ints(targets, flat, self.vocab, "targets").reshape(shape)
        )

    def syllable_request(
        self, context: Any, ids: Any, tones: Any, costs: Any, boundary: Any, candidates: Any, width: int
    ) -> Syllable:
        n = len(ids)
        if not 1 <= n <= 64 or not 1 <= len(context) <= 256 or not 1 <= width <= 64:
            raise RequestError("invalid syllable shape")
        values = {"ids": ids, "tones": tones, "costs": costs, "boundary": boundary}
        arrays = {key: ints(value, (n,), self.limits[key], key) for key, value in values.items()}
        return Syllable(
            tuple(ints(context, (len(context),), self.limits["context"], "context").tolist()),
            candidates=ints(candidates, (n, width), self.limits["candidates"], "candidates"),
            **arrays,
        )

    def json_request(self, r: Any) -> Request:
        if not isinstance(r, dict):
            raise RequestError("request is not an object")
        if r.get("op") == "rerank":
            return self.rerank_request(r.get("batch"), r.get("length"), r.get("ids"), r.get("targets"))
        if r.get("op") == "syllable":
            rows = r.get("candidates")
            if not isinstance(rows, list) or not rows or not all(isinstance(row, list) and row for row in rows):
                raise RequestError("invalid syllable shape")
            widths = [len(row) for row in rows]
            width = max(widths)
            padded = [row + [0] * (width - len(row)) for row in rows]
            keys = ("context", "ids", "tones", "costs", "boundary")
            if not all(isinstance(r.get(key), list) for key in keys):
                raise RequestError("invalid syllable shape")
            context, ids, tones, costs, boundary = (r[key] for key in keys)
            request = self.syllable_request(context, ids, tones, costs, boundary, padded, width)
            request.widths = None if len(set(widths)) == 1 else widths
            return request
        raise RequestError("unknown operation")

    def binary_request(self, kind: int, payload: bytes) -> Request:
        if len(payload) % 4:
            raise RequestError("payload is not int32")
        values = np.frombuffer(payload, dtype="<i4")
        if kind == RERANK and len(values) >= 2:
            batch, length = int(values[0]), int(values[1])
            if len(values) != 2 + 2 * batch * length or batch < 1 or length < 1:
                raise RequestError("invalid reranker frame")
            rows = batch * length
            return self.rerank_request(batch, length, values[2 : 2 + rows], values[2 + rows :])
        if kind == SYLLABLE and len(values) >= 3:
            c, n, w = (int(x) for x in values[:3])
            if c < 1 or n < 1 or w < 1 or len(values) != 3 + c + 4 * n + n * w:
                raise RequestError("invalid syllable frame")
            parts = np.split(values[3:], np.cumsum([c, n, n, n, n]))
            context, ids, tones, costs, boundary, candidates = parts
            return self.syllable_request(context, ids, tones, costs, boundary, candidates.reshape(n, w), w)
        raise RequestError("unknown operation")

    # MARK: - Batches

    def tensor(self, array: np.ndarray) -> torch.Tensor:
        return torch.from_numpy(array).to(self.device, non_blocking=True)

    def score_forest(
        self, ids: torch.Tensor, positions: torch.Tensor, allowed: torch.Tensor, index: torch.Tensor
    ) -> torch.Tensor:
        """The reranker over a packed forest of prefix trees (the exported RerankerTree, with several trees in one
        sequence): a node attends to its ancestors only and takes its depth as its position.  Returns the
        log-probabilities picked by `index` (node × vocabulary + target)."""
        m = self.reranker
        x = m.embed(ids)
        cos = m.cos[0, 0][positions].unsqueeze(1)
        sin = m.sin[0, 0][positions].unsqueeze(1)
        mask = (~allowed).float() * -10000.0
        for block in m.blocks:
            x = block(x, cos, sin, mask)
        logits = (m.norm(x) @ m.embed.weight.T).float()
        return logits.log_softmax(-1).reshape(-1).gather(0, index)

    @staticmethod
    def pack(request: Rerank) -> tuple[list[int], list[int], list[int], np.ndarray]:
        """One request's rows as a prefix tree: node tokens, depths and parents (-1 for a root), and for every row
        position the node whose prefix it continues (-1 after the row's last target)."""
        nodes: dict[tuple[int, int], int] = {}
        tokens: list[int] = []
        depths: list[int] = []
        parents: list[int] = []
        rows, length = request.ids.shape
        state = np.full((rows, length), -1, np.int64)
        for row, (ids, targets) in enumerate(zip(request.ids.tolist(), request.targets.tolist())):
            parent = -1
            for j in range(length):
                if targets[j] == 0:
                    break
                key = (parent, ids[j])
                node = nodes.get(key)
                if node is None:
                    node = nodes[key] = len(tokens)
                    tokens.append(ids[j])
                    depths.append(j)
                    parents.append(parent)
                state[row, j] = node
                parent = node
        return tokens, depths, parents, state

    @torch.inference_mode()
    def run_rerank(self, requests: list[Rerank]) -> list[np.ndarray]:
        """log P(target) at every position of every row.  Each request's rows become a prefix tree (the n-best
        sentences after the same context share most of their tokens), and the trees of a round are packed into one
        sequence with ancestor-only attention, so the model runs about a quarter of the tokens the rows hold.
        Positions after a row's last target (padding) are answered with 0.  The sequence is padded to a multiple of
        64 nodes, so each length's forward is one CUDA graph."""
        packed = [self.pack(r) for r in requests]
        results: list[np.ndarray] = [np.empty(0, np.float32)] * len(requests)
        chunks: list[list[int]] = [[]]
        nodes = 0
        for i, (tokens, _, _, _) in enumerate(packed):
            if chunks[-1] and nodes + len(tokens) > RERANK_NODES:
                chunks.append([])
                nodes = 0
            chunks[-1].append(i)
            nodes += len(tokens)
        for chunk in chunks:
            total = sum(len(packed[i][0]) for i in chunk)
            n = bucket(max(total, 1), 64, step=True)
            ids = np.zeros(n, np.int64)
            positions = np.zeros(n, np.int64)
            allowed = np.zeros((n, n), bool)
            allowed[np.arange(n), np.arange(n)] = True
            keys = []
            offset = 0
            for i in chunk:
                tokens, depths, parents, state = packed[i]
                ids[offset : offset + len(tokens)] = tokens
                positions[offset : offset + len(tokens)] = depths
                for node, parent in enumerate(parents):
                    if parent >= 0:
                        allowed[offset + node, offset : offset + node] = allowed[
                            offset + parent, offset : offset + node
                        ]
                flat = state.reshape(-1)
                valid = flat >= 0
                keys.append((flat[valid] + offset) * self.vocab + requests[i].targets.reshape(-1)[valid])
                offset += len(tokens)
            edges, inverse = np.unique(np.concatenate(keys), return_inverse=True)
            index = np.zeros(bucket(len(edges), 256), np.int64)
            index[: len(edges)] = edges
            inputs = [self.tensor(ids[None]), self.tensor(positions[None]), self.tensor(allowed), self.tensor(index)]
            scores = self.graphs.run("rerank", self.score_forest, inputs).cpu().numpy()[inverse]
            start = 0
            for i in chunk:
                flat = packed[i][3].reshape(-1)
                valid = flat >= 0
                result = np.zeros(len(flat), np.float32)
                result[valid] = scores[start : start + int(valid.sum())]
                start += int(valid.sum())
                results[i] = result
        return results

    def memories_for(self, contexts: list[tuple[int, ...]]) -> int:
        """Runs the context encoder for the contexts not in the cache (one padded batch); returns how many it ran."""
        missing = [c for c in dict.fromkeys(contexts) if c not in self.memories]
        with torch.inference_mode():
            for start in range(0, len(missing), 64):
                part = missing[start : start + 64]
                length = max(map(len, part))
                ids = np.zeros((len(part), length), np.int64)
                for row, context in enumerate(part):
                    ids[row, : len(context)] = context
                tokens = self.tensor(ids)
                k, v = self.syllable.project(self.context.hidden(tokens), tokens != 0)
                for row, context in enumerate(part):
                    # The exported context encoder hands its memory over in fp16.
                    memory = (k[:, row, : len(context)].half(), v[:, row, : len(context)].half())
                    self.memories[context] = memory
                    self.memory_bytes += 2 * memory[0].numel() * 2
        for context in contexts:
            self.memories.move_to_end(context)
        while self.memory_bytes > MEMORY_BYTES and len(self.memories) > len(set(contexts)):
            _, (k, v) = self.memories.popitem(last=False)
            self.memory_bytes -= 2 * k.numel() * 2
        return len(missing)

    @torch.inference_mode()
    def run_syllable(self, requests: list[Syllable]) -> list[np.ndarray]:
        """log P over each syllable's candidates.  Requests are padded to a bucket shape (padded syllables, memory
        rows and candidates are masked), so each shape's forward is one CUDA graph."""

        def shape(r: Syllable) -> tuple[int, int, int]:
            n, w = r.candidates.shape
            return bucket(len(r.context), 1 if len(r.context) == 1 else 16), bucket(n, 4), bucket(w, 8)

        order = sorted(range(len(requests)), key=lambda i: shape(requests[i]))
        results: list[np.ndarray] = [np.empty((0, 0), np.float32)] * len(requests)
        chunks: list[list[int]] = [[]]
        n = w = 0
        for i in order:
            _, rows, width = shape(requests[i])
            n, w = max(n, rows), max(w, width)
            size = len(chunks[-1])
            if size and (size >= SYLLABLE_REQUESTS or bucket(size + 1, 1) * n * w > SYLLABLE_SLOTS):
                chunks.append([])
                n, w = rows, width
            chunks[-1].append(i)
        for chunk in chunks:
            c, n, w = (max(values) for values in zip(*(shape(requests[i]) for i in chunk)))
            b = bucket(len(chunk), 1)
            arrays = {key: np.zeros((b, n), np.int64) for key in ("ids", "tones", "costs", "boundary")}
            candidates = np.zeros((b, n, w), np.int64)
            layers, _, width = self.memories[requests[chunk[0]].context][0].shape
            k = torch.zeros((layers, b, c, width), dtype=torch.float32, device=self.device)
            v = torch.zeros_like(k)
            for row, i in enumerate(chunk):
                r = requests[i]
                for key, array in arrays.items():
                    array[row, : len(r.ids)] = getattr(r, key)
                candidates[row, : len(r.ids), : r.candidates.shape[1]] = r.candidates
                mk, mv = self.memories[r.context]
                k[:, row, : len(r.context)] = mk
                v[:, row, : len(r.context)] = mv
            inputs = [self.tensor(arrays[key]) for key in ("ids", "tones", "costs", "boundary")]
            output = self.graphs.run("syllable", self.syllable, [*inputs, self.tensor(candidates), k, v]).cpu().numpy()
            for row, i in enumerate(chunk):
                r = requests[i]
                results[i] = output[row, : len(r.ids), : r.candidates.shape[1]]
        return results


def bucket(value: int, smallest: int, step: bool = False) -> int:
    """The padded size for `value`: the next power of two from `smallest`, or with `step` the next multiple of
    `smallest`."""
    if step:
        return -(-value // smallest) * smallest
    size = smallest
    while size < value:
        size *= 2
    return size


class Graphs:
    """Forwards as CUDA graphs, one per function and input shape, captured on first use: a graph replays the
    hundreds of small kernels of a forward with one launch, so the server's time goes to the GPU instead of to
    Python launching kernels.  The replayed kernels are the captured eager ones (same numbers).  Every graph shares one
    memory pool; that is safe because replays never overlap and each output is copied out right after its replay."""

    def __init__(self, enabled: bool, limit: int = 512) -> None:
        self.enabled = enabled
        self.limit = limit
        self.graphs: dict[tuple[Any, ...], tuple[torch.cuda.CUDAGraph, list[torch.Tensor], torch.Tensor]] = {}
        self.pool = torch.cuda.graph_pool_handle() if enabled else None
        self.captured = self.replayed = self.eager = 0

    def run(self, name: str, function: Any, inputs: list[torch.Tensor]) -> torch.Tensor:
        key = (name, *((tuple(t.shape), t.dtype) for t in inputs))
        entry = self.graphs.get(key)
        if entry is None:
            if not self.enabled or len(self.graphs) >= self.limit:
                self.eager += 1
                return function(*inputs)
            static = [t.clone() for t in inputs]
            stream = torch.cuda.Stream()
            stream.wait_stream(torch.cuda.current_stream())
            with torch.cuda.stream(stream):
                for _ in range(2):
                    function(*static)
            torch.cuda.current_stream().wait_stream(stream)
            graph = torch.cuda.CUDAGraph()
            with torch.cuda.graph(graph, pool=self.pool):
                output = function(*static)
            entry = self.graphs[key] = (graph, static, output)
            self.captured += 1
        graph, static, output = entry
        for target, value in zip(static, inputs):
            target.copy_(value)
        graph.replay()
        self.replayed += 1
        return output


class Connection:
    def __init__(self, sock: socket.socket) -> None:
        self.sock = sock
        self.buffer = bytearray()
        self.closed = False


class Server:
    """One worker's single-threaded loop: read every connection that has data, answer every complete request in
    batches.  Workers share the listening socket; whichever is free accepts a new client."""

    def __init__(self, models: Models, listener: socket.socket, worker: int = 0, stats_every: float = 60) -> None:
        self.models = models
        self.graphs = models.graphs
        self.worker = worker
        self.counters = Counters()
        self.selector = selectors.DefaultSelector()
        self.listener = listener
        self.selector.register(self.listener, selectors.EVENT_READ, None)
        self.stats_every = stats_every
        self.next_stats = time.monotonic() + stats_every

    def log_stats(self) -> None:
        graphs = {"captured": self.graphs.captured, "replayed": self.graphs.replayed, "eager": self.graphs.eager}
        memory = round(torch.cuda.max_memory_reserved() / 2**30, 2) if self.models.device == "cuda" else 0
        stats = {"worker": self.worker, **self.counters.summary(), "graphs": graphs, "gpu_memory_gb": memory}
        print(json.dumps({"stats": stats}), flush=True)

    def accept(self) -> None:
        try:
            sock, _ = self.listener.accept()
        except BlockingIOError:
            return  # another worker took it
        sock.settimeout(120)
        self.selector.register(sock, selectors.EVENT_READ, Connection(sock))
        self.counters.clients += 1

    def close(self, connection: Connection) -> None:
        if not connection.closed:
            connection.closed = True
            self.selector.unregister(connection.sock)
            connection.sock.close()

    def requests_of(self, connection: Connection) -> list[tuple[Connection, bool, Request | str]]:
        """The complete requests in the connection's buffer: (connection, binary, request or error message)."""
        found: list[tuple[Connection, bool, Request | str]] = []
        buffer = connection.buffer
        while buffer:
            if buffer[0] == MARKER:
                if len(buffer) < HEADER.size:
                    break
                _, kind, version, size = HEADER.unpack_from(buffer)
                if size > MAX_REQUEST or version != VERSION:
                    raise ConnectionError("bad frame")
                if len(buffer) < HEADER.size + size:
                    break
                payload = bytes(buffer[HEADER.size : HEADER.size + size])
                del buffer[: HEADER.size + size]
                try:
                    found.append((connection, True, self.models.binary_request(kind, payload)))
                except RequestError as error:
                    found.append((connection, True, str(error)))
            else:
                end = buffer.find(b"\n")
                if end < 0:
                    if len(buffer) > MAX_REQUEST:
                        raise ConnectionError("request too large")
                    break
                line = bytes(buffer[:end])
                del buffer[: end + 1]
                try:
                    found.append((connection, False, self.models.json_request(json.loads(line))))
                except (RequestError, ValueError) as error:
                    found.append((connection, False, str(error)))
        return found

    def answer(self, connection: Connection, binary: bool, output: np.ndarray | str, request: Request | None) -> None:
        if isinstance(output, np.ndarray) and not np.isfinite(output).all():
            output = "non-finite model output"
        if isinstance(output, str):
            self.counters.errors += 1
        if connection.closed:
            return
        if binary:
            if isinstance(output, str):
                payload, kind = output.encode(), ERROR
            else:
                payload, kind = np.ascontiguousarray(output, dtype="<f4").tobytes(), OK
            message = HEADER.pack(MARKER, kind, VERSION, len(payload)) + payload
        elif isinstance(output, str):
            message = json.dumps({"error": output}).encode() + b"\n"
        else:
            # float32 -> float64 is exact, and JSON writes the shortest text that reads back as the same double.
            if isinstance(request, Syllable) and request.widths:
                widths = request.widths
                values: list[Any] = [output[row, :w].astype(np.float64).tolist() for row, w in enumerate(widths)]
            else:
                values = output.astype(np.float64).tolist()
            message = json.dumps({"output": values}, allow_nan=False, separators=(",", ":")).encode() + b"\n"
        try:
            connection.sock.sendall(message)
        except OSError:
            self.close(connection)

    def round(self, pending: list[tuple[Connection, bool, Request | str]]) -> None:
        """Answers one round of requests: errors at once, then every syllable request in one batch and every
        reranker request in another."""
        start = time.monotonic()
        syllables = [(c, b, r) for c, b, r in pending if isinstance(r, Syllable)]
        reranks = [(c, b, r) for c, b, r in pending if isinstance(r, Rerank)]
        for connection, binary, message in pending:
            if isinstance(message, str):
                self.answer(connection, binary, message, None)
        if syllables:
            syllable_requests = [r for _, _, r in syllables if isinstance(r, Syllable)]
            ran = self.models.memories_for([r.context for r in syllable_requests])
            self.counters.requests["context"] += ran
            self.counters.batches["context"] += -(-ran // 64)
            began = time.monotonic()
            results = self.models.run_syllable(syllable_requests)
            self.counters.seconds["syllable"] += time.monotonic() - began
            for (connection, binary, request), result in zip(syllables, results):
                self.answer(connection, binary, result, request)
            self.counters.requests["syllable"] += len(syllable_requests)
            self.counters.batches["syllable"] += 1
        if reranks:
            rerank_requests = [r for _, _, r in reranks if isinstance(r, Rerank)]
            began = time.monotonic()
            results = self.models.run_rerank(rerank_requests)
            self.counters.seconds["rerank"] += time.monotonic() - began
            for (connection, binary, scored), result in zip(reranks, results):
                self.answer(connection, binary, result, scored)
            self.counters.requests["rerank"] += len(rerank_requests)
            self.counters.batches["rerank"] += 1
        self.counters.busy += time.monotonic() - start
        self.counters.rounds += 1

    def serve(self) -> None:
        while True:
            timeout = max(0.0, self.next_stats - time.monotonic())
            pending: list[tuple[Connection, bool, Request | str]] = []
            for key, _ in self.selector.select(timeout):
                if key.data is None:
                    self.accept()
                    continue
                connection: Connection = key.data
                try:
                    data = connection.sock.recv(1 << 20)
                    if not data:
                        raise ConnectionError("closed")
                    connection.buffer += data
                    pending += self.requests_of(connection)
                except (OSError, ConnectionError):
                    self.close(connection)
            if pending:
                self.round(pending)
            if time.monotonic() >= self.next_stats:
                self.log_stats()
                self.next_stats = time.monotonic() + self.stats_every


def stop(*_: Any) -> None:
    raise SystemExit(0)


def work(args: argparse.Namespace, listener: socket.socket, worker: int) -> None:
    """One worker: loads the models and serves until SIGTERM."""
    signal.signal(signal.SIGTERM, stop)
    models = Models(args.reranker, args.v3, args.device, args.memory_gb, graphs=not args.no_graphs)
    server = Server(models, listener, worker)
    ready = {"ready": args.socket, "worker": worker, "torch": torch.__version__, "device": args.device}
    print(json.dumps({**ready, "precision": "fp32", "memory_gb": args.memory_gb}), flush=True)
    try:
        server.serve()
    finally:
        server.log_stats()


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--socket", required=True)
    p.add_argument("--reranker", type=Path, required=True)
    p.add_argument("--v3", type=Path, required=True)
    p.add_argument("--device", choices=["cuda", "cpu"], default="cuda")
    p.add_argument("--workers", type=int, default=WORKERS, help=f"server processes (default: {WORKERS})")
    p.add_argument("--memory-gb", type=float, default=12, help="cap on each worker's GPU memory (default: 12)")
    p.add_argument("--no-graphs", action="store_true", help="run every forward eagerly (no CUDA graphs)")
    a = p.parse_args(argv)
    if not 0 < a.memory_gb <= 32 or not 1 <= a.workers <= 16:
        p.error("the memory budget must be above 0 and at most 32 GiB, with 1 to 16 workers")
    os.umask(0o077)
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    # Refuse an existing socket: never unlink another evaluator's live server.
    listener.bind(a.socket)
    listener.listen(512)
    listener.setblocking(False)
    if a.workers == 1:
        try:
            work(a, listener, 0)
        finally:
            Path(a.socket).unlink(missing_ok=True)
        return 0
    # Fork before CUDA starts: each worker opens its own CUDA context and loads its own copy of the models.
    signal.signal(signal.SIGTERM, stop)
    children = []
    for worker in range(a.workers):
        pid = os.fork()
        if pid == 0:
            code = 1
            try:
                work(a, listener, worker)
            except SystemExit:
                code = 0
            except BaseException:
                traceback.print_exc()
            finally:
                sys.stdout.flush()
                os._exit(code)
        children.append(pid)
    try:
        # A worker that ends on its own (an error) ends the server, so clients fail instead of waiting.
        return 1 if os.waitpid(-1, 0)[1] else 0
    finally:
        for pid in children:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        for pid in children:
            try:
                os.waitpid(pid, 0)
            except ChildProcessError:
                pass
        Path(a.socket).unlink(missing_ok=True)


if __name__ == "__main__":
    raise SystemExit(main())
