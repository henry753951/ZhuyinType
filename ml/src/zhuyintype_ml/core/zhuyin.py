"""Zhuyin on the standard keyboard: readings for text, keys for readings, and the scoring of converted text.

The evaluation tools type text the way a person would: each Han run gets the lexicon's most likely reading for each
word (longest match), the readings become standard-layout keys, and a first tone is typed as Space.
"""

from __future__ import annotations

import random
import re
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

KEYS = {
    "ㄅ": "1",
    "ㄆ": "q",
    "ㄇ": "a",
    "ㄈ": "z",
    "ㄉ": "2",
    "ㄊ": "w",
    "ㄋ": "s",
    "ㄌ": "x",
    "ㄍ": "e",
    "ㄎ": "d",
    "ㄏ": "c",
    "ㄐ": "r",
    "ㄑ": "f",
    "ㄒ": "v",
    "ㄓ": "5",
    "ㄔ": "t",
    "ㄕ": "g",
    "ㄖ": "b",
    "ㄗ": "y",
    "ㄘ": "h",
    "ㄙ": "n",
    "ㄧ": "u",
    "ㄨ": "j",
    "ㄩ": "m",
    "ㄚ": "8",
    "ㄛ": "i",
    "ㄜ": "k",
    "ㄝ": ",",
    "ㄞ": "9",
    "ㄟ": "o",
    "ㄠ": "l",
    "ㄡ": ".",
    "ㄢ": "0",
    "ㄣ": "p",
    "ㄤ": ";",
    "ㄥ": "/",
    "ㄦ": "-",
    "ˊ": "6",
    "ˇ": "3",
    "ˋ": "4",
    "˙": "7",
}
TONES = "ˊˇˋ˙"

# Interchangeable character forms (both acceptable in Taiwan text): a run differing only in these is still right
# for the variant-equal score.
VARIANTS = str.maketrans("臺妳裏爲衆啓峯綫着麪牀", "台你裡為眾啟峰線著麵床")

# What the evaluation types: runs of 4–20 Han characters.
HAN_RUN = re.compile(r"[一-鿿]{4,20}")


class Lexicon:
    """Each word's most likely reading, from a lexicon TSV (reading, word, log probability)."""

    def __init__(self, path: Path | str | None = None, rows: Iterable[tuple[str, str, float]] = ()) -> None:
        """From a lexicon TSV, or from (reading, word, log probability) rows."""
        best: dict[str, tuple[str, float]] = {}
        if path is not None:
            with open(path, encoding="utf-8") as f:
                rows = [(r, w, float(p)) for r, w, p in (x.rstrip("\n").split("\t") for x in f if x[0] != "#")]
        for reading, word, value in rows:
            if word not in best or best[word][1] < value:
                best[word] = (reading, value)
        self.reading = {word: reading for word, (reading, _) in best.items()}

    def readings(self, text: str, cuts: list[int] | None = None) -> list[str] | None:
        """One reading per character by longest match (words up to 6 characters), or None when a character has none.
        `cuts` collects the word boundaries."""
        out: list[str] = []
        i = 0
        while i < len(text):
            for length in range(min(6, len(text) - i), 0, -1):
                word = text[i : i + length]
                if word in self.reading:
                    out += self.reading[word].split(" ")
                    i += length
                    if cuts is not None:
                        cuts.append(i)
                    break
            else:
                return None
        return out


def keys(readings: list[str]) -> str:
    """Standard-layout keys for readings; a first tone is typed as Space."""
    typed = []
    for syllable in readings:
        text = "".join(KEYS[c] for c in syllable)
        typed.append(text if syllable[-1] in TONES else text + "<space>")
    return "".join(typed)


def syllable_keys(syllables: str) -> str:
    """Keys for space-separated syllables as written in row files (a neutral tone may lead: ˙ㄇㄚ)."""
    readings = syllables.split()
    if not readings:
        raise ValueError("empty syllables")
    result = []
    for syllable in readings:
        if syllable.startswith("˙"):
            syllable = syllable[1:] + "˙"
        body = syllable.rstrip(TONES)
        if not body or any(c not in KEYS or c in TONES for c in body) or len(syllable) - len(body) > 1:
            raise ValueError(f"invalid syllable: {syllable!r}")
        result.append(syllable)
    return keys(result)


@dataclass
class Run:
    """One evaluation item: the text to type, the text before it on its line, its readings and word boundaries."""

    text: str
    before: str
    readings: list[str]
    cuts: list[int]

    @property
    def keys(self) -> str:
        return keys(self.readings)


def sample_runs(path: Path | str, lexicon: Lexicon, n: int, seed: int = 7, whole_readings: bool = False) -> list[Run]:
    """The first `n` Han runs of a text file with readings, after a seeded shuffle.  Every tool picks runs this way,
    so they see the same runs.  `whole_readings` also requires one reading per character."""
    with open(path, encoding="utf-8") as f:
        found = [(m.group(), line[: m.start()][-32:]) for line in f for m in HAN_RUN.finditer(line)]
    random.Random(seed).shuffle(found)
    # A run repeated in a file keeps the context of its last occurrence, as the original tools did.
    before = dict(found)
    runs: list[Run] = []
    for text, _ in found:
        if len(runs) >= n:
            break
        cuts: list[int] = []
        readings = lexicon.readings(text, cuts)
        if not readings or (whole_readings and len(readings) != len(text)):
            continue
        runs.append(Run(text, before[text], readings, cuts))
    return runs


def score(pairs: list[tuple[str, str]]) -> dict:
    """Character accuracy, whole-run accuracy and whole-run accuracy with variants equal, for (reference, output)."""
    total = sum(len(want) for want, _ in pairs)
    correct = sum(sum(a == b for a, b in zip(want, got)) for want, got in pairs)
    count = max(1, len(pairs))
    return {
        "runs": len(pairs),
        "character": correct / max(1, total),
        "whole": sum(want == got for want, got in pairs) / count,
        "variants": sum(want.translate(VARIANTS) == got.translate(VARIANTS) for want, got in pairs) / count,
    }
