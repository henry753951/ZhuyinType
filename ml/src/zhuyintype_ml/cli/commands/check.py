"""zt check: engine behaviour, the app, the Python tools, lint, types and privacy.

zt check engine                   # the engine's tests, release (ZHUYIN_CHECKS_TIMING=warn on slow or busy machines)
zt check app                      # the built app's self-test and packaging check (after `zt build app`)
zt check python                   # unittest over ml/src/**/test_*.py
zt check python --group train     # with PyTorch, for the training tests
zt check lint [--fix]             # ruff check (and `ruff format --check` with --format)
zt check types                    # mypy over the typed modules (pyproject.toml lists them)
zt check public                   # no private hosts, keys or addresses in tracked files (tools/check-public.sh)
zt check all                      # python, lint, types and public: what CI runs besides the builds
"""

from __future__ import annotations

import argparse
import subprocess
import sys

from zhuyintype_ml.core import paths


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "check",
        description="Engine behaviour, the app, the Python tools, lint, types, privacy.",
        help="run checks and tests",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    kinds = parser.add_subparsers(dest="kind", metavar="KIND")

    kinds.add_parser("engine", help="engine behaviour and latency checks (cargo test, release)").set_defaults(
        func=engine
    )
    kinds.add_parser("app", help="self-test and packaging check of dist/ZhuyinType.app").set_defaults(func=app)

    python = kinds.add_parser("python", help="unittest over ml/src/**/test_*.py")
    python.add_argument("--group", action="append", default=[], help="dependency group to run with (repeatable)")
    python.add_argument(
        "--start",
        default="src/zhuyintype_ml",
        help="directory to discover tests in, relative to ml/ or the repository (default: src/zhuyintype_ml)",
    )
    python.add_argument("-p", "--pattern", default="test_*.py", help="test file pattern (default: test_*.py)")
    python.set_defaults(func=python_tests)

    lint = kinds.add_parser("lint", help="ruff check")
    lint.add_argument("--fix", action="store_true", help="apply safe fixes")
    lint.add_argument("--format", action="store_true", help="also check formatting (ruff format --check)")
    lint.add_argument("paths", nargs="*", help="files or directories, relative to ml/ (default: all of ml/)")
    lint.set_defaults(func=ruff)

    types = kinds.add_parser("types", help="mypy over the typed modules")
    types.add_argument("paths", nargs="*", help="files or directories (default: [tool.mypy] files)")
    types.set_defaults(func=mypy)

    kinds.add_parser("public", help="no private hosts, keys or addresses (tools/check-public.sh)").set_defaults(
        func=public
    )
    kinds.add_parser("all", help="python, lint, types and public").set_defaults(func=everything)
    parser.set_defaults(func=lambda a: parser.print_help())


def run(command: list[str]) -> int:
    print("$ " + " ".join(command[command.index("python") :] if "python" in command else command), flush=True)
    return subprocess.run(command, cwd=paths.ML, env=paths.python_env()).returncode


def engine(_args: argparse.Namespace) -> int:
    """The Rust workspace's tests in release: behaviour (the former ZhuyinCoreChecks), golden digests and the latency
    targets on the real data in data/build."""
    command = paths.cargo_command("test", "--release", "--locked", "--workspace")
    return subprocess.run(command, env=paths.engine_env(), cwd=paths.ENGINE).returncode


def app(_args: argparse.Namespace) -> int:
    bundle = paths.DIST / "ZhuyinType.app"
    if sys.platform != "darwin" or not bundle.exists():
        raise SystemExit("zt: no dist/ZhuyinType.app; run `zt build app` (macOS) first")
    lexicon = paths.data("lexicon.tsv")
    status = subprocess.run(
        [str(bundle / "Contents" / "MacOS" / "ZhuyinType"), "--self-test", "--lexicon", str(lexicon)]
    ).returncode
    return (
        status
        or subprocess.run(["swift", str(paths.CLIENT / "scripts" / "check-packaging.swift"), str(bundle)]).returncode
    )


def test_start(start: str) -> str:
    """The test directory relative to ml/, whether it was given relative to ml/, to the repository, or to the
    package (`src/zhuyintype_ml/remote`, `ml/src/zhuyintype_ml/remote` and `zhuyintype_ml/remote` are the same)."""
    start = start.rstrip("/")
    if start.startswith("ml/"):
        start = start[len("ml/") :]
    if start.startswith("zhuyintype_ml"):
        start = "src/" + start
    return start


def python_tests(args: argparse.Namespace) -> int:
    start = test_start(args.start)
    return run(
        paths.python_command(
            "unittest", "discover", "-s", start, "-t", "src", "-p", args.pattern, groups=tuple(args.group)
        )
    )


def ruff(args: argparse.Namespace) -> int:
    status = run(paths.python_command("ruff", "check", *(["--fix"] if args.fix else []), *args.paths, groups=("dev",)))
    if args.format:
        status = run(paths.python_command("ruff", "format", "--check", *args.paths, groups=("dev",))) or status
    return status


def mypy(args: argparse.Namespace) -> int:
    return run(paths.python_command("mypy", *args.paths, groups=("dev",)))


def public(_args: argparse.Namespace) -> int:
    return subprocess.run([str(paths.ROOT / "tools" / "check-public.sh")]).returncode


def everything(_args: argparse.Namespace) -> int:
    empty = argparse.Namespace(
        group=[], start="src/zhuyintype_ml", pattern="test_*.py", fix=False, format=False, paths=[]
    )
    results = {
        name: step(empty)
        for name, step in [("python", python_tests), ("lint", ruff), ("types", mypy), ("public", public)]
    }
    print("  ".join(f"{name}: {'ok' if status == 0 else 'FAILED'}" for name, status in results.items()))
    return 1 if any(results.values()) else 0
