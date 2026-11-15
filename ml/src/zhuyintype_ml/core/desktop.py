"""Build and check ZhuyinType for Linux and Windows (platforms/desktop): zt build desktop / zt check desktop.

Both run platforms/desktop/scripts/build.sh, which builds on Linux only (the GPU host or CI, never the Mac):

    zt build desktop                         # Linux debug build: the app (zhuyintype) and the Fcitx 5 addon
    zt build desktop --release
    zt build desktop --package               # dist/desktop/*.deb and *.rpm, compiled in the Debian 13 builder image
    zt build desktop --target windows        # the Windows installer (platforms/desktop/packaging/windows/build.sh)
    zt check desktop                         # the Linux build, cargo test and the addon's tests
    zt --remote --pull dist/desktop build desktop --package    # from the Mac: on the GPU host, packages copied back
"""

from __future__ import annotations

import subprocess
import sys

from zhuyintype_ml.core import paths

SCRIPT = paths.ROOT / "platforms" / "desktop" / "scripts" / "build.sh"
TARGETS = ("linux", "windows", "all")


def arguments(
    target: str = "linux", *, release: bool = False, package: bool = False, native: bool = False, test: bool = False
) -> list[str]:
    """build.sh's arguments for these options."""
    if target not in TARGETS:
        raise ValueError(f"unknown desktop target {target!r}")
    flags = {"--release": release, "--package": package, "--native": native, "--test": test}
    return [target, *(flag for flag, on in flags.items() if on)]


def run(args: list[str]) -> int:
    """Runs build.sh with `args`; its exit status."""
    if sys.platform != "linux":
        raise SystemExit(
            "zt: ZhuyinType for Linux and Windows builds on Linux only (the GPU host or CI); "
            "from this machine run `tools/zt --remote build desktop` (add `--pull dist/desktop` for the packages)"
        )
    return subprocess.run(["bash", str(SCRIPT), *args], cwd=paths.ROOT).returncode


def build(target: str = "linux", *, release: bool = False, package: bool = False, native: bool = False) -> int:
    return run(arguments(target, release=release, package=package, native=native))


def check() -> int:
    """The Linux debug build, cargo test over the workspace and the addon's CTest."""
    return run(arguments("linux", test=True))
