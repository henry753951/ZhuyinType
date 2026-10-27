"""zt models: the neural models on Linux — ONNX export, parity checks and the CUDA model server (nlp/remote).

    zt models export                  # ONNX from data/build/{reranker,v3} checkpoints (skipped when current)
    zt models export --tree --fp16    # also RerankerTree.onnx and *.fp16.onnx experiments
    zt models export --reranker reranker_v2c --v3 v3_new     # other model directories in the data directory
    zt models parity --coreml         # Mac: Core ML reference outputs
    zt models parity                  # Linux: ONNX against PyTorch and Core ML
    zt models serve --device cuda -- eval conversion --set colloquial-dev    # every model call on the GPU

Export and parity run with the `export` dependency group (PyTorch, onnx, onnxruntime), installed on first use.
"""

from __future__ import annotations

import argparse

from nlp.common import paths
from nlp.remote import models


def model_dir(value: str) -> paths.Path:
    """A model directory: a name inside the data directory, or a path."""
    path = paths.Path(value)
    return path.resolve() if path.is_absolute() or path.exists() else paths.data(value)


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "models",
        description="ONNX export, parity checks and the CUDA model server.",
        help="export and check the neural models (Linux)",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    actions = parser.add_subparsers(dest="action", metavar="ACTION")

    def directories(action: argparse.ArgumentParser) -> argparse.ArgumentParser:
        action.add_argument("--reranker", type=model_dir, default=None, help="reranker directory (default: reranker)")
        action.add_argument("--v3", type=model_dir, default=None, help="v3 directory (default: v3)")
        return action

    export = directories(actions.add_parser("export", help="ONNX models from the checkpoints (cached by content)"))
    export.add_argument("--tree", action="store_true", help="also RerankerTree.onnx")
    export.add_argument("--fp16", action="store_true", help="also *.fp16.onnx experiments")
    export.add_argument("--force", action="store_true", help="export even when current")
    export.set_defaults(func=lambda a: models.export(a.reranker, a.v3, a.tree, a.fp16, a.force) and 0)

    parity = directories(actions.add_parser("parity", help="ONNX against PyTorch and Core ML"))
    parity.add_argument("--coreml", action="store_true", help="write the Core ML reference outputs (macOS)")
    parity.add_argument("--output", help="report file")
    parity.set_defaults(func=run_parity)

    serve = actions.add_parser("serve", help="run a zt command with every model call on one PyTorch server")
    serve.add_argument("--device", choices=["cuda", "cpu"], default="cuda", help="default: cuda")
    serve.add_argument("--memory-gb", type=float, default=2, help="GPU memory budget in GiB (default: 2)")
    serve.add_argument("command", nargs=argparse.REMAINDER, help="-- then the zt command")
    serve.set_defaults(func=serve_command)

    parser.set_defaults(func=lambda a: parser.print_help())


def run_parity(args: argparse.Namespace) -> int:
    options = [
        *(["--coreml"] if args.coreml else []),
        *(["--output", args.output] if args.output else []),
        *(["--reranker", str(args.reranker)] if args.reranker else []),
        *(["--v3", str(args.v3)] if args.v3 else []),
    ]
    return paths.run_python("nlp.remote.parity", *options, groups=("export",), check=False)


def serve_command(args: argparse.Namespace) -> int:
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        raise SystemExit("zt: give the command to run, e.g. `zt models serve -- eval conversion`")
    return models.run_with_server(command, args.device, args.memory_gb)
