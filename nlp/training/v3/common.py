"""Shared v3 paths and immutable input provenance; no writes outside v3."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "data/build/v3"
HELD = ["data/build/lm/test_sentences.txt", "data/build/corpus/test_colloquial.txt",
        "data/build/corpus/dev_colloquial.txt", "data/build/corpus/dev_wikipedia.txt"]


def save(path, value):
    path = Path(path)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    tmp.replace(path)


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(8 << 20), b""):
            h.update(block)
    return h.hexdigest()


def holdouts():
    import ahocorasick
    exact = set()
    machine = ahocorasick.Automaton()
    for relative in HELD:
        for line in (ROOT / relative).read_text().splitlines():
            line = line.strip()
            if line:
                exact.add(line)
            for i in range(len(line) - 7):
                machine.add_word(line[i:i + 8], True)
    machine.make_automaton()
    return exact, machine


def excluded(text, held):
    exact, machine = held
    return text in exact or next(machine.iter(text), None) is not None
