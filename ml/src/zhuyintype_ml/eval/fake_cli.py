"""A stand-in for zhuyin-cli in tests: `fake_cli(directory, body)` writes an executable that runs `body` for each
batch line on standard input, with `line` (the input line) and `argv` (the options) defined, and prints its result
flushed at once, as zhuyin-cli does (the engine pool hands out lines one at a time)."""

from __future__ import annotations

import sys
from pathlib import Path


def fake_cli(directory: Path, body: str) -> Path:
    path = directory / "zhuyin-cli"
    path.write_text(
        f"#!{sys.executable}\n"
        "import sys\n"
        "argv = sys.argv[1:]\n"
        "def answer(line):\n" + "".join(f"    {text}\n" for text in body.splitlines()) + "for line in sys.stdin:\n"
        "    print(answer(line.rstrip('\\n')), flush=True)\n",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path
