"""Where things are: the repository, the data directory, the toolchains and the engine command line.

Every tool resolves paths through this module, so a checkout works the same from any working directory, on the Mac and
on a Linux host.  Environment overrides:

    ZT_DATA            data directory (default: data/build in the repository)
    ZT_CLI             zhuyin-cli executable (default: .build/release/zhuyin-cli, built on first use)
    SWIFT_SCRATCH_PATH SwiftPM build directory (default: .build in the repository)
    ZT_BUILD_JOBS      parallel compile jobs for swift build (default: every core), to share a busy host
    ZT_SWIFT           swift executable (default: PATH, then ~/opt/swift/usr/bin/swift)
    ONNXRUNTIME_ROOT   ONNX Runtime C SDK on Linux, with include/ and lib/ (default: ~/opt/onnxruntime)
    ZT_UV              uv executable (default: PATH, then ~/.local/bin/uv and ~/.cargo/bin/uv)

The ~/opt locations are only fallbacks for a toolchain unpacked by hand (zhuyintype_ml/remote/README.md); a toolchain
on PATH always wins.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

# This file is ml/src/zhuyintype_ml/core/paths.py.
ROOT = Path(__file__).resolve().parents[4]
ML = ROOT / "ml"
PACKAGE = ML / "src" / "zhuyintype_ml"
EVAL_SETS = ML / "eval-sets"
RESOURCES = ROOT / "resources"
ENGINE = ROOT / "engine" / "swift"
CLIENT = ROOT / "platforms" / "macos"
DOCS = ROOT / "docs"
DIST = ROOT / "dist"
REMOTE_ENV = ROOT / ".remote.env"


def data_dir() -> Path:
    return Path(os.environ.get("ZT_DATA", ROOT / "data" / "build")).resolve()


def data(*parts: str) -> Path:
    """A path inside the data directory."""
    return data_dir().joinpath(*parts)


def raw(*parts: str) -> Path:
    """Downloaded source material (dumps, API responses, receipts): `raw/` beside the data directory."""
    return data_dir().parent.joinpath("raw", *parts)


def relative(path: Path | str) -> str:
    """`path` relative to the repository when it is inside it, for messages and records."""
    path = Path(path).resolve()
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def scratch() -> Path:
    """The SwiftPM build directory."""
    return Path(os.environ.get("SWIFT_SCRATCH_PATH", ROOT / ".build")).resolve()


def _first(name: str, override: str, fallbacks: list[Path]) -> str | None:
    if os.environ.get(override):
        return os.environ[override]
    return shutil.which(name) or next((str(p) for p in fallbacks if p.exists()), None)


def find_swift() -> str | None:
    """The Swift toolchain, or None: $ZT_SWIFT, PATH, then ~/opt/swift."""
    return _first("swift", "ZT_SWIFT", [Path.home() / "opt" / "swift" / "usr" / "bin" / "swift"])


def swift() -> str:
    found = find_swift()
    if not found:
        sys.exit("zt: Swift not found. Install a Swift 6 toolchain (`zt setup` says how).")
    return found


def find_uv() -> str | None:
    """uv, or None: $ZT_UV (set by tools/zt), PATH, then the installer's default locations."""
    return _first("uv", "ZT_UV", [Path.home() / ".local" / "bin" / "uv", Path.home() / ".cargo" / "bin" / "uv"])


def uv() -> str:
    found = find_uv()
    if not found:
        sys.exit("zt: uv not found. Install it: curl -LsSf https://astral.sh/uv/install.sh | sh")
    return found


def onnxruntime_root() -> Path:
    """The ONNX Runtime C SDK the Linux engine links (engine/swift/Package.swift reads the same variable)."""
    return Path(os.environ.get("ONNXRUNTIME_ROOT", Path.home() / "opt" / "onnxruntime"))


def engine_env() -> dict[str, str]:
    """The environment for building and running the engine on this platform."""
    env = dict(os.environ)
    if sys.platform == "linux":
        env.setdefault("ONNXRUNTIME_ROOT", str(onnxruntime_root()))
    swift_path = find_swift()
    if swift_path:
        env["PATH"] = str(Path(swift_path).parent) + os.pathsep + env.get("PATH", "")
    return env


def swift_command(*args: str) -> list[str]:
    """`swift <args>` for the engine package with the shared scratch path."""
    jobs = ["-j", os.environ["ZT_BUILD_JOBS"]] if os.environ.get("ZT_BUILD_JOBS") else []
    return [swift(), *args, "--package-path", str(ENGINE), "--scratch-path", str(scratch()), *jobs]


def build_cli(quiet: bool = False) -> Path:
    """Builds zhuyin-cli (release) and returns its path."""
    command = swift_command("build", "-c", "release", "--product", "zhuyin-cli")
    subprocess.run(command, check=True, env=engine_env(), stdout=subprocess.DEVNULL if quiet else None)
    return scratch() / "release" / "zhuyin-cli"


def cli() -> Path:
    """zhuyin-cli, building it when it does not exist yet."""
    if "ZT_CLI" in os.environ:
        return Path(os.environ["ZT_CLI"]).resolve()
    path = scratch() / "release" / "zhuyin-cli"
    return path if path.exists() else build_cli()


def python_command(module: str, *args: str, groups: tuple[str, ...] = ()) -> list[str]:
    """`python -m module args` in the project environment with dependency groups, for steps that need third-party
    packages (the zt process itself stays standard-library only)."""
    flags = [flag for group in groups for flag in ("--group", group)]
    return [uv(), "run", "--quiet", "--project", str(ML), *flags, "python", "-m", module, *args]


def python_env() -> dict[str, str]:
    """The environment for python_command: the repository importable as `nlp`."""
    existing = os.environ.get("PYTHONPATH")
    return dict(os.environ, PYTHONPATH=str(ML / "src") + (os.pathsep + existing if existing else ""))


def run_python(module: str, *args: str, groups: tuple[str, ...] = (), check: bool = True) -> int:
    """Runs python_command and returns its exit status."""
    return subprocess.run(python_command(module, *args, groups=groups), env=python_env(), check=check).returncode
