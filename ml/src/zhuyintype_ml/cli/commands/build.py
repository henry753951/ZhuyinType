"""zt build: the engine command line and the macOS app (engine/, platforms/macos/scripts).

    zt build cli                      # zhuyin-cli, release (engine/target/release/; with ONNX Runtime on Linux)
    zt build cli --debug
    zt build bench                    # EngineBench: the app's engine path with Core ML (macOS)
    zt build app                      # dist/ZhuyinType.app (macOS, needs Xcode)
    zt build install                  # build and install into ~/Library/Input Methods
    zt build pkg                      # dist/ZhuyinType-<version>.pkg

CARGO_TARGET_DIR moves the engine's build directory (default engine/target), SWIFT_SCRATCH_PATH the app's (default
.build); on Linux ONNXRUNTIME_ROOT names the ONNX Runtime SDK zhuyin-cli links (default ~/opt/onnxruntime).
"""

from __future__ import annotations

import argparse
import subprocess
import sys

from zhuyintype_ml.core import desktop, paths

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

    targets.add_parser("input-method", help="shared Qt host and native Fcitx5/TSF adapter").set_defaults(
        func=lambda _: desktop.build_input_method()
    )

    targets.add_parser("desktop", help="shared Qt client preview (Linux, Windows, macOS)").set_defaults(
        func=lambda _: desktop.build()
    )

    cli = targets.add_parser("cli", help="zhuyin-cli (engine/crates/zhuyin-cli)")
    cli.add_argument("--debug", action="store_true", help="debug build (default: release)")
    cli.set_defaults(func=build_cli)

    bench = targets.add_parser("bench", help="EngineBench, the app's engine path (macOS)")
    bench.set_defaults(func=build_bench)

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
    try:
        print(paths.build_cli(debug=args.debug))
    except subprocess.CalledProcessError as error:
        return error.returncode
    return 0


def build_bench(_args: argparse.Namespace) -> int:
    if sys.platform != "darwin":
        raise SystemExit("zt: EngineBench builds on macOS only")
    try:
        print(paths.build_bench())
    except subprocess.CalledProcessError as error:
        return error.returncode
    return 0


def script(name: str, *args: str) -> int:
    if sys.platform != "darwin":
        raise SystemExit("zt: the app builds on macOS only")
    return subprocess.run([str(SCRIPTS / name), *args], cwd=paths.ROOT).returncode
