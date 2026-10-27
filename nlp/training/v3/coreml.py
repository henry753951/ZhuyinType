"""Compiling Core ML packages with Xcode's coremlcompiler (macOS only).

    from nlp.training.v3.coreml import compile_package
    compile_package(out / "ContextEncoder.mlpackage", out)      # writes out/ContextEncoder.mlmodelc

The command line tools alone lack coremlcompiler; when DEVELOPER_DIR is unset and Xcode is in /Applications, it is
used, as the app build does.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

XCODE = Path("/Applications/Xcode.app/Contents/Developer")


def require_macos(what: str) -> None:
    if sys.platform != "darwin":
        raise SystemExit(f"zt: {what} runs only on macOS (Core ML, xcrun coremlcompiler)")


def compile_package(package: Path, out: Path) -> None:
    """`xcrun coremlcompiler compile package out`: writes out/<name>.mlmodelc."""
    require_macos("compiling Core ML models")
    env = dict(os.environ)
    if "DEVELOPER_DIR" not in env and XCODE.exists():
        env["DEVELOPER_DIR"] = str(XCODE)
    subprocess.run(["xcrun", "coremlcompiler", "compile", str(package), str(out)], check=True, env=env)
