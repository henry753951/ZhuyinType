"""What this machine has for building and running ZhuyinType, and how to get what is missing.

`zt setup` prints these checks and syncs the uv environment; `zt --remote` runs `zt setup --quiet` on the host before
every command.  Each check says what it found or how to install the tool; nothing here installs system packages.
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from . import paths

# The ONNX Runtime C SDK the Linux engine is tested with (also in .github/workflows/ci.yml).
ONNXRUNTIME_VERSION = "1.24.4"


@dataclass(frozen=True)
class Check:
    name: str
    ok: bool
    detail: str
    # Without it nothing builds or runs on this platform (as opposed to a feature that needs it).
    required: bool = True
    hint: str = ""


def _version(command: list[str]) -> str:
    try:
        done = subprocess.run(command, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    return (done.stdout or done.stderr).strip().splitlines()[0] if done.returncode == 0 else ""


def check_uv() -> Check:
    found = paths.find_uv()
    version = _version([found, "--version"]) if found else ""
    return Check(
        "uv",
        bool(version),
        version or "not found",
        hint="curl -LsSf https://astral.sh/uv/install.sh | sh   (or: brew install uv)",
    )


def check_cargo() -> Check:
    """The engine is Rust; engine/rust-toolchain.toml pins the toolchain rustup installs."""
    found = paths.find_cargo()
    version = _version([found, "--version"]) if found else ""
    return Check(
        "cargo",
        bool(version),
        f"{version} ({found})" if version else "not found",
        hint="install Rust with rustup: https://rustup.rs (user space; the engine pins its toolchain)",
    )


def check_swift() -> Check:
    """Swift builds the macOS app."""
    found = paths.find_swift()
    version = _version([found, "--version"]) if found else ""
    hint = "install Xcode from the App Store (Swift 6 ships with it)"
    return Check("swift", bool(version), f"{version} ({found})" if version else "not found", hint=hint)


def check_xcode() -> Check:
    """The app needs full Xcode: SwiftUI's macros are not in the Command Line Tools."""
    selected = _version(["xcode-select", "-p"])
    has_xcode = "Xcode" in selected or Path("/Applications/Xcode.app").exists()
    return Check(
        "xcode",
        has_xcode,
        selected or "not found",
        required=False,
        hint="install Xcode (needed for `zt build app`)",
    )


def check_onnxruntime() -> Check:
    root = paths.onnxruntime_root()
    library = next(iter(sorted((root / "lib").glob("libonnxruntime.so*"))), None) if (root / "lib").exists() else None
    ok = bool(library) and (root / "include").exists()
    return Check(
        "onnxruntime",
        ok,
        str(root) if ok else f"no SDK at {root}",
        hint=f"`zt setup --install-onnxruntime` (ONNX Runtime {ONNXRUNTIME_VERSION} into ~/opt), "
        "or set ONNXRUNTIME_ROOT to an SDK with include/ and lib/",
    )


def check_tool(name: str, hint: str) -> Check:
    found = shutil.which(name)
    return Check(name, bool(found), found or "not found", required=False, hint=hint)


def checks() -> list[Check]:
    """Everything this platform needs, then optional tools."""
    result = [check_uv(), check_cargo(), check_tool("git", "install git")]
    if sys.platform == "darwin":
        result += [check_swift(), check_xcode()]
    elif sys.platform == "linux":
        result.append(check_onnxruntime())
    result += [
        check_tool("rsync", "needed for `zt --remote` (brew/apt install rsync)"),
        check_tool("ssh", "needed for `zt --remote`"),
        check_tool("gh", "needed for `zt data publish` and releases (https://cli.github.com)"),
    ]
    return result


def install_onnxruntime(version: str = ONNXRUNTIME_VERSION) -> Path:
    """Downloads Microsoft's ONNX Runtime C SDK for Linux into the parent of ONNXRUNTIME_ROOT (default ~/opt) and
    points ONNXRUNTIME_ROOT's path at it with a symlink.  User space only; no sudo."""
    if sys.platform != "linux":
        raise SystemExit("zt: the ONNX Runtime SDK is only used on Linux (macOS uses Core ML)")
    arch = {"x86_64": "x64", "aarch64": "aarch64"}.get(platform.machine())
    if not arch:
        raise SystemExit(f"zt: no ONNX Runtime SDK for {platform.machine()}")
    root = paths.onnxruntime_root()
    name = f"onnxruntime-linux-{arch}-{version}"
    target = root.parent / name
    if not target.exists():
        url = f"https://github.com/microsoft/onnxruntime/releases/download/v{version}/{name}.tgz"
        print(f"downloading {url}", flush=True)
        root.parent.mkdir(parents=True, exist_ok=True)
        archive = root.parent / f"{name}.tgz"
        urllib.request.urlretrieve(url, archive)
        with tarfile.open(archive, "r:gz") as tar:
            tar.extractall(root.parent, filter="data")
        archive.unlink()
    if root.is_symlink() or not root.exists():
        if root.is_symlink():
            root.unlink()
        root.symlink_to(target.name if target.parent == root.parent else target)
    return target


def sync(groups: list[str], quiet: bool = False) -> int:
    """`uv sync` of the project with these dependency groups, keeping packages other groups installed."""
    command = [
        paths.uv(),
        "sync",
        "--inexact",
        "--project",
        str(paths.ML),
        *[flag for group in groups for flag in ("--group", group)],
    ]
    if quiet:
        command.append("--quiet")
    return subprocess.run(command, env=dict(os.environ)).returncode
