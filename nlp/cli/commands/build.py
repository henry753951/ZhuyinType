"""zt build: the engine command line and the macOS app (nlp/engine, clients/macos/scripts).

    zt build cli                      # zhuyin-cli and the engine checks, release (.build/release/)
    zt build cli --debug
    zt build app                      # dist/ZhuyinType.app (macOS, needs Xcode)
    zt build install                  # build and install into ~/Library/Input Methods
    zt build pkg                      # dist/ZhuyinType-<version>.pkg

SWIFT_SCRATCH_PATH moves the SwiftPM build directory (default .build); on Linux ONNXRUNTIME_ROOT names the ONNX
Runtime SDK (default ~/opt/onnxruntime).
"""

from __future__ import annotations

import argparse
import subprocess
import sys

from nlp.common import paths

SCRIPTS = paths.CLIENT / "scripts"


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "build",
        description="Build the engine command line and the macOS app.",
        help="build zhuyin-cli, the app, its installer",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    targets = parser.add_subparsers(dest="target", metavar="TARGET")

    cli = targets.add_parser("cli", help="zhuyin-cli and the engine checks (nlp/engine)")
    cli.add_argument("--debug", action="store_true", help="debug build (default: release)")
    cli.add_argument("--product", help="one product only, e.g. zhuyin-cli")
    cli.set_defaults(func=build_cli)

    app = targets.add_parser("app", help="dist/ZhuyinType.app (macOS)")
    app.add_argument("--debug", action="store_true", help="debug build (default: release)")
    app.set_defaults(func=lambda a: script("build-app.sh", "debug" if a.debug else "release"))

    install = targets.add_parser("install", help="build and install into ~/Library/Input Methods (macOS)")
    install.add_argument("--debug", action="store_true", help="debug build (default: release)")
    install.set_defaults(func=lambda a: script("install-dev.sh", "debug" if a.debug else "release"))

    pkg = targets.add_parser("pkg", help="dist/ZhuyinType-<version>.pkg (macOS)")
    pkg.set_defaults(func=lambda a: script("build-pkg.sh"))

    parser.set_defaults(func=lambda a: parser.print_help())


def build_cli(args: argparse.Namespace) -> int:
    command = paths.swift_command(
        "build", "-c", "debug" if args.debug else "release", *(["--product", args.product] if args.product else [])
    )
    return subprocess.run(command, env=paths.engine_env()).returncode


def script(name: str, *args: str) -> int:
    if sys.platform != "darwin":
        raise SystemExit("zt: the app builds on macOS only")
    return subprocess.run([str(SCRIPTS / name), *args], cwd=paths.ROOT).returncode
