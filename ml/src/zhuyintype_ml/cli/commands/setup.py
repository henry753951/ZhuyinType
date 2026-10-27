"""zt setup: check this machine's tools and prepare the Python environment (ml/src/zhuyintype_ml/core/environment.py).

zt setup                          # check uv, Swift, Xcode (macOS) or ONNX Runtime (Linux); uv sync
zt setup --group export           # also the ONNX export packages (PyTorch, onnx, onnxruntime)
zt setup --group train --group dev
zt setup --install-onnxruntime    # Linux: ONNX Runtime C SDK into ~/opt (or ONNXRUNTIME_ROOT)
zt setup --quiet                  # print only problems (zt --remote runs this on the host)
"""

from __future__ import annotations

import argparse
import sys

from zhuyintype_ml.core import environment

GROUPS = ("dev", "data", "export", "train")


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "setup",
        description="Check this machine's tools and prepare the Python environment.",
        help="check tools and sync the uv environment",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--group",
        action="append",
        default=[],
        choices=GROUPS,
        help="also install a dependency group from pyproject.toml (repeatable)",
    )
    parser.add_argument("--no-sync", action="store_true", help="only check; do not run uv sync")
    parser.add_argument(
        "--install-onnxruntime",
        action="store_true",
        help=f"Linux: download ONNX Runtime {environment.ONNXRUNTIME_VERSION} into ~/opt",
    )
    parser.add_argument("--quiet", action="store_true", help="print only what is missing")
    parser.set_defaults(func=run)


def run(args: argparse.Namespace) -> int:
    if args.install_onnxruntime:
        print(f"ONNX Runtime: {environment.install_onnxruntime()}")
    missing = 0
    for check in environment.checks():
        if check.ok and args.quiet:
            continue
        mark = "✓" if check.ok else ("✗" if check.required else "·")
        print(f"{mark} {check.name:12} {check.detail}")
        if not check.ok:
            print(f"  {'' if check.required else '(optional) '}{check.hint}")
            missing += check.required
    if not args.no_sync:
        sys.stdout.flush()
        status = environment.sync(args.group, quiet=args.quiet)
        if status:
            return status
        if not args.quiet:
            print(f"✓ {'python env':12} .venv synced" + (f" with {', '.join(args.group)}" if args.group else ""))
    if missing:
        print(f"{missing} required tool(s) missing; see above.")
    return 1 if missing else 0
