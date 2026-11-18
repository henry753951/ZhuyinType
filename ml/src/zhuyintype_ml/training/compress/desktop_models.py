"""The Linux and Windows apps' neural models (the `onnx` data component): ONNX Runtime's dynamic int8 of the shipped
weights, in ONNX Runtime's own format so the app memory-maps them.

    zt models export --tree                                                     # fp32 ONNX of the checkpoints
    uv run --group export python -m zhuyintype_ml.training.compress.desktop_models
    zt data pack data-N onnx

1. The fp32 exports of the shipped checkpoints (`checkpoints` component: the int8-channel weights of data-5) are
   quantized with onnxruntime.quantization.quantize_dynamic, int8 weights per channel: the weights stay int8 in
   memory and the matrix products run in int8 (activations quantized at run time).
2. Each model is converted to the ORT format with its graph optimizations applied ahead of time (`Fixed`).  The app
   maps the file and runs from it (`session.use_ort_model_bytes_for_initializers`, prepacking off), so the weights
   stay file-backed pages the system can drop, as Core ML's do on macOS.
3. manifest.json records the checkpoints' SHA-256, ONNX Runtime's version and each file's SHA-256.
4. RerankerTree.ort is checked on the CPU against PyTorch on 600 engine-shaped trees (trees.py, as `parity` checks
   the macOS tree); the result goes into manifest.json and a tree off by more than `--limit` nats, or not finite,
   fails the build.  Dynamic int8 quantizes each call's activations, so every tree is somewhat off: the 0.7.1 model
   by 0.75 nats (median of each tree's largest error; at most 2.2), the 0.7.2 one by 0.91 (at most 3.1), the fp32
   export by 0.00003 (research log 2026-11-18, `chat-reranker`).  The limit, 5 nats, sits above that noise; it
   catches NaN and the larger errors of the kind fp16 Core ML showed (up to 13 nats), not every smaller one.

Measured (docs/research/log.md, 2026-11-17): the same outputs as the dynamic int8 ONNX files, 0.2–0.8 points below
the macOS app's models on the development sets; the app's private memory grows by about 21 MB (folding the macOS
weights into fp32 instead: about 490 MB).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from zhuyintype_ml.core import paths

from .trees import check_trees

MODELS = {"reranker": ["RerankerTree"], "v3": ["ContextEncoder", "SyllableEncoder"]}
CHECKPOINTS = ["reranker/charlm.pt", "v3/context.pt", "v3/syllable.pt"]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--data", type=Path, default=paths.data_dir(), help="data directory with the fp32 exports")
    parser.add_argument("--out", type=Path, default=None, help="output directory (default: <data>/onnx)")
    parser.add_argument("--limit", type=float, default=5.0, help="largest error of the tree check, nats (default 5)")
    args = parser.parse_args(argv)
    import onnxruntime
    from onnxruntime.quantization import QuantType, quantize_dynamic

    data: Path = args.data
    out: Path = args.out or data / "onnx"
    sources = {name: data / directory / f"{name}.onnx" for directory, names in MODELS.items() for name in names}
    missing = [str(path) for path in sources.values() if not path.is_file()]
    if missing:
        raise SystemExit(f"missing fp32 exports ({', '.join(missing)}); run `zt models export --tree` first")
    out.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, object] = {
        "method": "onnxruntime.quantization.quantize_dynamic (per channel, QInt8), ORT format, Fixed optimizations",
        "onnxruntime": onnxruntime.__version__,
        "checkpoints": {name: sha256(data / name) for name in CHECKPOINTS if (data / name).is_file()},
        "files": {},
    }
    with tempfile.TemporaryDirectory() as work:
        for name, source in sources.items():
            target = Path(work) / f"{name}.onnx"
            quantize_dynamic(str(source), str(target), per_channel=True, weight_type=QuantType.QInt8)
        subprocess.run(
            [sys.executable, "-m", "onnxruntime.tools.convert_onnx_models_to_ort", "--optimization_style=Fixed", work],
            check=True,
        )
        for name in sources:
            target = out / f"{name}.ort"
            shutil.copyfile(Path(work) / f"{name}.ort", target)
            files = manifest["files"]
            assert isinstance(files, dict)
            files[target.name] = {"sha256": sha256(target), "size": target.stat().st_size}
            print(f"{paths.relative(target)} {target.stat().st_size / 1e6:.1f} MB", flush=True)
    session = onnxruntime.InferenceSession(str(out / "RerankerTree.ort"), providers=["CPUExecutionProvider"])
    check = check_trees(data / "reranker" / "charlm.pt", lambda inputs: session.run(None, inputs)[0], args.limit)
    manifest["tree_check"] = check
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("RerankerTree.ort against PyTorch:", json.dumps(check), flush=True)
    if check["failed_node_counts"]:
        print(f"zt: RerankerTree.ort is off by more than {args.limit} nats on some trees", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
