"""Where things are: the repository, the data directory, the toolchains and the engine command line.

Every tool resolves paths through this module, so a checkout works the same from any working directory, on the Mac and
on a Linux host.  Environment overrides:

    ZT_DATA            data directory (default: data/build in the repository)
    ZT_CLI             zhuyin-cli executable (default: engine/target/release/zhuyin-cli, built from the current sources
                       on first use)
    ZT_CARGO           cargo executable (default: PATH, then ~/.cargo/bin/cargo); CARGO_TARGET_DIR moves its output
    SWIFT_SCRATCH_PATH SwiftPM build directory of the macOS app (default: .build in the repository)
    ZT_BUILD_JOBS      parallel compile jobs for cargo and swift builds (default: every core), to share a busy host
    ZT_SWIFT           swift executable, for the macOS app (default: PATH, then ~/opt/swift/usr/bin/swift)
    ONNXRUNTIME_ROOT   ONNX Runtime C SDK on Linux, with include/ and lib/ (default: ~/opt/onnxruntime); zhuyin-cli
                       links it there (the `onnx` feature) for the neural models
    ZT_UV              uv executable (default: PATH, then ~/.local/bin/uv and ~/.cargo/bin/uv)

The ~/opt locations are only fallbacks for a toolchain unpacked by hand (zhuyintype_ml/remote/README.md); a toolchain
on PATH always wins.
"""

from __future__ import annotations

import functools
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
ENGINE = ROOT / "engine"
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
    """The SwiftPM build directory of the macOS app."""
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


def find_cargo() -> str | None:
    """cargo, or None: $ZT_CARGO, PATH, then rustup's default location."""
    return _first("cargo", "ZT_CARGO", [Path.home() / ".cargo" / "bin" / "cargo"])


def cargo() -> str:
    found = find_cargo()
    if not found:
        sys.exit("zt: cargo not found. Install Rust (`zt setup` says how).")
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
    """The ONNX Runtime C SDK zhuyin-cli links on Linux (its `onnx` feature, through ORT_LIB_PATH)."""
    return Path(os.environ.get("ONNXRUNTIME_ROOT", Path.home() / "opt" / "onnxruntime"))


def engine_env() -> dict[str, str]:
    """The environment for building and running the engine on this platform."""
    env = dict(os.environ)
    if sys.platform == "linux":
        env.setdefault("ONNXRUNTIME_ROOT", str(onnxruntime_root()))
        env.setdefault("ORT_LIB_PATH", str(onnxruntime_root() / "lib"))
    for tool in (find_swift(), find_cargo()):
        if tool:
            env["PATH"] = str(Path(tool).parent) + os.pathsep + env.get("PATH", "")
    return env


def jobs() -> list[str]:
    return ["-j", os.environ["ZT_BUILD_JOBS"]] if os.environ.get("ZT_BUILD_JOBS") else []


def engine_target() -> Path:
    """cargo's output directory for the engine."""
    return Path(os.environ.get("CARGO_TARGET_DIR", ENGINE / "target")).resolve()


def cargo_command(*args: str) -> list[str]:
    """`cargo <args>` for the engine workspace; run it in ENGINE, where cargo and rustup find engine/.cargo/config.toml
    and rust-toolchain.toml."""
    return [cargo(), *args, "--manifest-path", str(ENGINE / "Cargo.toml"), *jobs()]


def cli_features() -> list[str]:
    """zhuyin-cli's features here: the neural models with ONNX Runtime on Linux (macOS evaluates them in the app's
    engine path, EngineBench, with Core ML)."""
    return ["--features", "onnx"] if sys.platform == "linux" else []


def _build(command: list[str], quiet: bool, cwd: Path | None = None) -> None:
    """Runs a build; when quiet, its output is shown only if it fails."""
    done = subprocess.run(command, env=engine_env(), cwd=cwd, capture_output=quiet, text=True)
    if done.returncode:
        if quiet:
            sys.stderr.write(done.stdout + done.stderr)
        raise subprocess.CalledProcessError(done.returncode, command)


def build_cli(quiet: bool = False, debug: bool = False) -> Path:
    """Builds zhuyin-cli and returns its path."""
    profile = [] if debug else ["--release"]
    command = cargo_command("build", *profile, "--locked", "-p", "zhuyin-cli", *cli_features())
    _build(command, quiet, cwd=ENGINE)
    return engine_target() / ("debug" if debug else "release") / "zhuyin-cli"


def swift_command(*args: str) -> list[str]:
    """`swift <args>` for the macOS package with the shared scratch path."""
    return [swift(), *args, "--package-path", str(CLIENT), "--scratch-path", str(scratch()), *jobs()]


def build_bench(quiet: bool = False) -> Path:
    """Builds EngineBench (macOS: the app's engine path with Core ML, platforms/macos) and returns its path."""
    _build(cargo_command("build", "--release", "--locked", "-p", "zhuyin-ffi"), quiet, cwd=ENGINE)
    _build(swift_command("build", "-c", "release", "--product", "EngineBench"), quiet)
    return scratch() / "release" / "EngineBench"


def cli() -> Path:
    """zhuyin-cli, built from the current sources: cargo rebuilds what changed (under a second when nothing did),
    once per process.  Using an existing binary as is let evaluations run old engine code after a source change."""
    if "ZT_CLI" in os.environ:
        return Path(os.environ["ZT_CLI"]).resolve()
    return _current_cli()


@functools.cache
def _current_cli() -> Path:
    return build_cli(quiet=True)


def bench() -> Path:
    """EngineBench built from the current sources (macOS), once per process; $ZT_CLI overrides it as for cli()."""
    if "ZT_CLI" in os.environ:
        return Path(os.environ["ZT_CLI"]).resolve()
    return _current_bench()


@functools.cache
def _current_bench() -> Path:
    return build_bench(quiet=True)


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
