"""The neural models on Linux: ONNX export from the checkpoints (cached by content), preparing a host for
evaluations, and the optional PyTorch model server for CUDA.

    zt models export                  # Reranker, ContextEncoder, SyllableEncoder .onnx next to their JSON files
    zt models export --tree           # also RerankerTree.onnx (prefix-tree scoring with the same weights)
    zt models serve --device cuda -- eval conversion --set colloquial-dev

The export runs `python -m zhuyintype_ml.remote.export_onnx` with the `export` dependency group (PyTorch, onnx,
onnxruntime); this module itself is standard library only.  An export is skipped when the checkpoints, the export code
and the options hash to the same value as the last export (recorded in <reranker>/onnx-export.sha256).
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

# The code that decides what an export contains: a change in any of these re-exports.
EXPORT_SOURCES = [
    "ml/src/zhuyintype_ml/remote/export_onnx.py",
    "ml/src/zhuyintype_ml/training/reranker/train.py",
    "ml/src/zhuyintype_ml/training/reranker/export_tree.py",
    "ml/src/zhuyintype_ml/training/v3/models.py",
]
STAMP = "onnx-export.sha256"


def checkpoint_files(reranker: Path, v3: Path) -> list[Path]:
    return [reranker / "charlm.pt", v3 / "context.pt", v3 / "syllable.pt"]


def outputs(reranker: Path, v3: Path, tree: bool) -> list[Path]:
    files = [reranker / "Reranker.onnx", v3 / "ContextEncoder.onnx", v3 / "SyllableEncoder.onnx"]
    return files + ([reranker / "RerankerTree.onnx"] if tree else [])


def export_digest(reranker: Path, v3: Path, tree: bool, fp16: bool) -> str:
    digest = hashlib.sha256()
    for path in checkpoint_files(reranker, v3) + [paths.ROOT / s for s in EXPORT_SOURCES]:
        digest.update(path.name.encode() + b"\0")
        with open(path, "rb") as f:
            for block in iter(lambda: f.read(1 << 20), b""):
                digest.update(block)
    digest.update(f"tree={tree} fp16={fp16}".encode())
    return digest.hexdigest()


def export(
    reranker: Path | None = None, v3: Path | None = None, tree: bool = False, fp16: bool = False, force: bool = False
) -> bool:
    """Exports the ONNX models when their inputs changed; returns whether it exported."""
    reranker = reranker or paths.data("reranker")
    v3 = v3 or paths.data("v3")
    missing = [p for p in checkpoint_files(reranker, v3) if not p.exists()]
    if missing:
        raise SystemExit(
            f"zt: missing checkpoints {', '.join(paths.relative(p) for p in missing)}; run `zt data fetch checkpoints`"
        )
    stamp = reranker / STAMP
    digest = export_digest(reranker, v3, tree, fp16)
    current = stamp.exists() and stamp.read_text().strip() == digest
    if current and not force and all(p.exists() for p in outputs(reranker, v3, tree)):
        print(f"ONNX models are current ({paths.relative(reranker)}, {paths.relative(v3)})")
        return False
    args = ["--reranker", str(reranker), "--v3", str(v3), *(["--tree"] if tree else []), *(["--fp16"] if fp16 else [])]
    paths.run_python("zhuyintype_ml.remote.export_onnx", *args, groups=("export",))
    stamp.write_text(digest + "\n")
    return True


def prepare(components: tuple[str, ...] = ("eval",), tree: bool = True, quiet: bool = False) -> None:
    """Everything an evaluation of the app's configuration needs on this machine: the pinned data and models,
    zhuyin-cli (release), the compiled lexicon and language models next to their TSV files, and on Linux the ONNX
    models exported from the checkpoints (macOS uses the Core ML models from the assets)."""
    from zhuyintype_ml.data import assets, compiled

    assets.fetch(assets.expand(list(components)), quiet=quiet)
    paths.build_cli(quiet=quiet)
    compiled.compile_all(quiet=quiet)
    if sys.platform == "linux":
        export(tree=tree)


@contextmanager
def model_server(
    device: str = "cuda",
    memory_gb: float = 12,
    workers: int = 1,
    reranker: Path | None = None,
    v3: Path | None = None,
) -> Iterator[Path]:
    """Starts zhuyintype_ml.remote.model_server (PyTorch, fp32) on a private Unix socket and yields a zhuyin-cli
    wrapper that passes `--model-server SOCKET`; stops the server (by its PID) afterwards and prints its counters.  Set
    ZT_CLI to the wrapper so every engine process uses the server.  The socket exists as soon as the server listens;
    the models load meanwhile (engine processes that connect early wait for their first answer)."""
    reranker = reranker or paths.data("reranker")
    v3 = v3 or paths.data("v3")
    cli = paths.cli()
    with tempfile.TemporaryDirectory(prefix="zt-model-server-") as temporary:
        directory = Path(temporary)
        socket = directory / "models.sock"
        log = paths.data("remote_eval", "model-server.log")
        log.parent.mkdir(parents=True, exist_ok=True)
        command = paths.python_command(
            "zhuyintype_ml.remote.model_server",
            "--socket",
            str(socket),
            "--reranker",
            str(reranker / "charlm.pt"),
            "--v3",
            str(v3 / "syllable.pt"),
            "--device",
            device,
            "--memory-gb",
            str(memory_gb),
            "--workers",
            str(workers),
            groups=("export",),
        )
        with open(log, "w") as out:
            server = subprocess.Popen(command, env=paths.python_env(), stdout=out, stderr=subprocess.STDOUT)
        try:
            for _ in range(480):
                if socket.exists() or server.poll() is not None:
                    break
                time.sleep(0.25)
            if not socket.exists():
                raise SystemExit(f"zt: the model server did not start:\n{log.read_text()[-2000:]}")
            wrapper = directory / "zhuyin-cli"
            wrapper.write_text(f'#!/bin/sh\nexec "{cli}" --model-server "{socket}" "$@"\n')
            wrapper.chmod(0o755)
            print(f"model server: {device}, pid {server.pid}, log {paths.relative(log)}", flush=True)
            yield wrapper
        finally:
            server.terminate()
            try:
                server.wait(timeout=30)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            for line in log.read_text().splitlines():
                if line.startswith('{"stats"'):
                    print(f"model server: {summary(json.loads(line)['stats'])}", flush=True)


def summary(stats: dict[str, Any]) -> str:
    """One line of a worker's counters for people."""
    requests = stats["requests"]
    batches = stats.get("mean_batch", {})
    return (
        f"worker {stats.get('worker', 0)}: {requests['rerank']:,} reranker and {requests['syllable']:,} "
        f"character-model requests in {stats['rounds']:,} rounds (mean batch {batches.get('rerank', 0)} and "
        f"{batches.get('syllable', 0)}), busy {stats['busy_share']:.0%} of {stats['wall_s']} s, "
        f"GPU memory {stats.get('gpu_memory_gb', 0)} GiB"
    )


def run_with_server(
    argv: list[str],
    device: str = "cuda",
    memory_gb: float = 12,
    workers: int = 1,
    reranker: Path | None = None,
    v3: Path | None = None,
) -> int:
    """Runs a zt command with every zhuyin-cli process using the model server (with these model directories, by
    default the data directory's reranker/ and v3/)."""
    with model_server(device, memory_gb, workers, reranker=reranker, v3=v3) as wrapper:
        env = dict(os.environ, ZT_CLI=str(wrapper))
        return subprocess.run([str(paths.ROOT / "tools" / "zt"), *argv], env=env).returncode
