"""Readings of training text: the lexicon's longest match, with polyphonic single characters left to a classifier.

A reading is a (toneless syllable, tone) pair, tone 1–5 (5 is the neutral tone); characters without a reading are
("<literal>", 0).  `Lexicon.label` gives one reading per character and says whether any of them was uncertain.  The
classifier (nlp/training/v3/polyphone.py) decides a polyphonic character only at p >= 0.9.

    from nlp.training.v3.readings import Lexicon
    lexicon = Lexicon("data/build/v3_030/lexicon.tsv")
    readings, uncertain = lexicon.label("今天不要一直工作")

This module needs no PyTorch unless a classifier is passed to `label`.
"""

from __future__ import annotations

import math
import re
from collections import defaultdict
from collections.abc import Iterator
from pathlib import Path
from typing import Any

TONES = {"ˊ": 2, "ˇ": 3, "ˋ": 4, "˙": 5}

Reading = tuple[str, int]


def split_reading(text: str) -> Reading:
    """ "ㄓㄨㄥ" → ("ㄓㄨㄥ", 1); "ㄉㄜ˙" → ("ㄉㄜ", 5)."""
    tone = next((v for k, v in TONES.items() if k in text), 1)
    return re.sub("[ˊˇˋ˙]", "", text), tone


class Lexicon:
    """A lexicon TSV (reading, word, log prior) indexed for labelling and candidate lists.

    words: word → [(prior, readings)], best first.  single: character → [(prior, reading)] for one-character words.
    candidates: reading → {character: best prior}.  poly: single characters with more than one reading.
    """

    def __init__(self, path: Path | str) -> None:
        self.words: defaultdict[str, list[tuple[float, tuple[Reading, ...]]]] = defaultdict(list)
        self.single: defaultdict[str, list[tuple[float, Reading]]] = defaultdict(list)
        self.candidates: defaultdict[Reading, dict[str, float]] = defaultdict(dict)
        self.trie: dict[Any, Any] = {}
        with open(path, encoding="utf-8") as lines:
            for line in lines:
                if line.startswith("#"):
                    continue
                raw, word, value = line.rstrip().split("\t")
                reading = tuple(map(split_reading, raw.split()))
                if len(reading) != len(word):
                    continue
                prior = float(value)
                self.words[word].append((prior, reading))
                if len(word) == 1:
                    self.single[word].append((prior, reading[0]))
                for char, item in zip(word, reading):
                    self.candidates[item][char] = max(prior, self.candidates[item].get(char, -math.inf))
        for word, rows in self.words.items():
            rows.sort(reverse=True)
            node = self.trie
            for char in word:
                node = node.setdefault(char, {})
            node[None] = word
        for single in self.single.values():
            single.sort(reverse=True)
        self.poly = {c for c, single in self.single.items() if len({r for _, r in single}) > 1}

    def segments(self, text: str) -> Iterator[tuple[int, int, str, tuple[Reading, ...]]]:
        """Longest-match words: (start, end, word, its best readings); an unknown character is a literal."""
        i = 0
        while i < len(text):
            node, found = self.trie, None
            j = i
            while j < len(text) and text[j] in node:
                node = node[text[j]]
                j += 1
                if None in node:
                    found = (j, node[None])
            if found:
                j, word = found
                yield i, j, word, self.words[word][0][1]
                i = j
            else:
                yield i, i + 1, text[i], (("<literal>", 0),)
                i += 1

    def label(
        self, text: str, classifier: Any = None, chars: dict[str, int] | None = None, labels: list[Any] | None = None
    ) -> tuple[list[Reading], bool]:
        """One reading per character, and whether a polyphonic single character stayed uncertain.  Without a
        classifier every polyphonic single character keeps its most likely reading and counts as uncertain."""
        readings: list[Reading] = []
        low = False
        for start, _end, word, reading in self.segments(text):
            if len(word) == 1 and word in self.poly:
                chosen = reading[0]
                if classifier is not None:
                    import torch

                    assert chars is not None and labels is not None
                    ids = window(text, start, chars)
                    with torch.no_grad():
                        logits = classifier(torch.tensor([ids]))[0]
                        allowed = [labels.index(r) for _, r in self.single[word] if r in labels]
                        probs = logits[sorted(set(allowed))].softmax(0)
                        p, index = probs.max(0)
                    if p.item() >= 0.9:
                        chosen = tuple(labels[sorted(set(allowed))[index.item()]])
                    else:
                        low = True
                else:
                    low = True
                readings.append(chosen)
            else:
                readings.extend(reading)
        assert len(readings) == len(text)
        return readings, low


def window(text: str, position: int, chars: dict[str, int]) -> list[int]:
    """Character ids of the 9 characters centred on `position` (0 outside the text, 2 for unknown characters)."""
    return [chars.get(text[i], 2) if 0 <= i < len(text) else 0 for i in range(position - 4, position + 5)]
