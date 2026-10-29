"""Build and check the shared Qt client: zt build desktop / zt check desktop.

Qt is supplied by the workstation; CMAKE_PREFIX_PATH may point to a Qt SDK.
"""

from __future__ import annotations

import shutil
import subprocess
import sys

from zhuyintype_ml.core import paths


def build(*, check: bool = False) -> int:
    """Configure the same sources on Windows, Linux or the macOS preview host."""
    cmake = shutil.which("cmake")
    ctest = shutil.which("ctest")
    if not cmake or (check and not ctest):
        raise SystemExit("zt: CMake/CTest is required for the Qt client; install the development tools first")
    output = paths.ROOT / ".build-qt"
    engine_build = paths.cargo_command("build", "--release", "--locked", "-p", "zhuyin-ffi")
    status = subprocess.run(engine_build, cwd=paths.ENGINE, env=paths.engine_env()).returncode
    if status:
        return status
    library_name = (
        "zhuyin.dll" if sys.platform == "win32" else "libzhuyin.dylib" if sys.platform == "darwin" else "libzhuyin.so"
    )
    library = paths.engine_target() / "release" / library_name
    commands = [
        [
            cmake,
            "-S",
            str(paths.ROOT / "platforms" / "qt"),
            "-B",
            str(output),
            "-DCMAKE_BUILD_TYPE=Release",
            f"-DZT_ENGINE_LIBRARY={library}",
        ],
        [cmake, "--build", str(output), "--config", "Release", "--parallel", "4"],
    ]
    if check and ctest:
        commands.append([ctest, "--test-dir", str(output), "-C", "Release", "--output-on-failure"])
    for command in commands:
        status = subprocess.run(command, cwd=paths.ROOT).returncode
        if status:
            return status
    return 0


def build_input_method(*, check: bool = False) -> int:
    """Build the shared host and this operating system's native input adapter."""
    platform = {"linux": "linux", "win32": "windows"}.get(sys.platform)
    if platform is None:
        raise SystemExit("zt: the native input adapter builds on Linux or Windows only")
    status = build(check=check)
    if status:
        return status
    cmake = shutil.which("cmake")
    ctest = shutil.which("ctest")
    if not cmake or (check and not ctest):
        raise SystemExit("zt: CMake/CTest is required for the native input adapter")
    output = paths.ROOT / ".build-input-method"
    commands = [
        [cmake, "-S", str(paths.ROOT / "platforms" / platform), "-B", str(output), "-DCMAKE_BUILD_TYPE=Release"],
        [cmake, "--build", str(output), "--config", "Release", "--parallel", "4"],
    ]
    if check and ctest:
        commands.append([ctest, "--test-dir", str(output), "-C", "Release", "--output-on-failure"])
    for command in commands:
        status = subprocess.run(command, cwd=paths.ROOT).returncode
        if status:
            return status
    return 0
