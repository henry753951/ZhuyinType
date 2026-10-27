"""Serve the pinned Qwen3-8B checkpoint with vLLM on loopback for the generator (GPU host only).

Waits until the last weight shard is present and no snapshot download of it is still running, then replaces this
process with `vllm serve` (BF16, 4,096-token context, 0.92 GPU memory; FlashInfer sampling off so the host needs
no CUDA toolkit for JIT).  The checkpoint is Qwen/Qwen3-8B revision b968826d9c46dd6066d109eabc6255188de91218,
Apache-2.0 (raw/Qwen3-8B-LICENSE in the corpus directory).

    python -m nlp.training.corpus.serve --model data/raw/Qwen3-8B --port 8102
"""

from __future__ import annotations

import argparse
import os
import subprocess
import time
from pathlib import Path

from nlp.training.lm.files import raw

REVISION = "b968826d9c46dd6066d109eabc6255188de91218"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--model", type=Path, default=raw("Qwen3-8B"), help="checkpoint directory")
    parser.add_argument("--port", type=int, default=8102)
    parser.add_argument("--gpu-memory-utilization", default="0.92")
    parser.add_argument("--max-num-seqs", default="192")
    parser.add_argument("--omp-threads", default="8")
    args = parser.parse_args(argv)
    while (
        not (args.model / "model-00005-of-00005.safetensors").exists()
        or subprocess.run(["pgrep", "-f", "snapshot_download.*Qwen3-8B"], stdout=subprocess.DEVNULL).returncode == 0
    ):
        time.sleep(10)
    os.environ.update(OMP_NUM_THREADS=args.omp_threads, VLLM_USE_FLASHINFER_SAMPLER="0")
    command = ["vllm", "serve", str(args.model), "--served-model-name", "Qwen3-8B", "--host", "127.0.0.1"]
    command += ["--port", str(args.port), "--dtype", "bfloat16", "--max-model-len", "4096"]
    command += ["--gpu-memory-utilization", args.gpu_memory_utilization, "--max-num-seqs", args.max_num_seqs]
    command += ["--max-num-batched-tokens", "8192", "--generation-config", "vllm"]
    os.execvp(command[0], command)


if __name__ == "__main__":
    raise SystemExit(main())
