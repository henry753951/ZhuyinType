"""Generate `src/simplified/table.txt`: the character table for 簡體 output.

Every BMP CJK character is converted on its own with OpenCC's `t2s` configuration (the pinned `opencc` package of
`ml/pyproject.toml`; OpenCC's dictionaries are Apache-2.0, `resources/sources.json` `opencc`).  A pair is kept when
the result is one different BMP character, so conversion never changes a string's length in UTF-16 units and the
cursor and clause offsets of the composition stay valid.  One line per pair: the traditional character, then the
simplified one.

    uv run --project ml python -I engine/crates/zhuyin-core/tools/simplified_table.py
"""

import sys
from importlib import metadata
from pathlib import Path

import opencc

RANGES = [(0x3400, 0x4DBF), (0x4E00, 0x9FFF), (0xF900, 0xFAFF)]
OUT = Path(__file__).resolve().parent.parent / "src" / "simplified" / "table.txt"


def main() -> int:
    converter = opencc.OpenCC("t2s")
    lines = []
    for start, end in RANGES:
        for code in range(start, end + 1):
            source = chr(code)
            target = converter.convert(source)
            if len(target) == 1 and target != source and ord(target) <= 0xFFFF:
                lines.append(source + target)
    header = f"# OpenCC {metadata.version('opencc')} t2s, one character at a time (tools/simplified_table.py)\n"
    OUT.write_text(header + "\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(lines)} pairs -> {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
