"""Where v3 training writes, the held-out text it must never train on, and two small file helpers.

Everything the v3 pipeline makes goes to `v3_030/` in the data directory (the 0.3.0 model, its corpus shards and
checkpoints).  The 0.4.0 app model continues from it in nlp/training/homophone.

    from nlp.training.v3.common import OUT, holdouts, excluded
    held = holdouts()
    excluded("某一段文字", held)
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from nlp.common import paths

# The directory name in the data directory (data/build/v3 at the time of training; renamed when 0.4.0 shipped).
NAME = "v3_030"
OUT = paths.data(NAME)
# Evaluation text, relative to the data directory: excluded from training, scored only as dev.
HELD = [
    "lm/test_sentences.txt",
    "corpus/test_colloquial.txt",
    "corpus/dev_colloquial.txt",
    "corpus/dev_wikipedia.txt",
]

# (exact lines, Aho-Corasick automaton of their 8-character substrings)
Held = tuple[set[str], Any]


def save(path: Path | str, value: object) -> None:
    """Writes JSON atomically (a temporary file, then a rename)."""
    path = Path(path)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    tmp.replace(path)


def digest(path: Path | str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(8 << 20), b""):
            h.update(block)
    return h.hexdigest()


def holdouts() -> Held:
    """The held-out lines and every 8-character substring of them."""
    import ahocorasick

    exact: set[str] = set()
    machine = ahocorasick.Automaton()
    for relative in HELD:
        for line in paths.data(relative).read_text().splitlines():
            line = line.strip()
            if line:
                exact.add(line)
            for i in range(len(line) - 7):
                machine.add_word(line[i : i + 8], True)
    machine.make_automaton()
    return exact, machine


def excluded(text: str, held: Held) -> bool:
    """True when `text` is a held-out line or shares an 8-character substring with one."""
    exact, machine = held
    return text in exact or next(machine.iter(text), None) is not None
