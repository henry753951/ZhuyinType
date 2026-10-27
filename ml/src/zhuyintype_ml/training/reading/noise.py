"""Reading noise for v3: characters typed with another reading, drawn from the reading channel the engine uses.

The channel directory (`zt train lexicon channel`, resources/formats/reading-channel.md) gives `v3_noise.tsv`: for a
character and its reading in the text (`*` for any), a reading typed instead, its probability, its state and v3's
cost bucket (0 for an accepted reading, 1–3 for a correction, on the key channel's bounds).  `channel.json` gives the
rule-level rates: third-tone sandhi (m) and the neutral tone typed with another reading of its character.

Which rows apply where follows the engine's channel (reading-channel.md, 引擎該用的成本):

  a single-character word   every `*` row of the character (熟 ㄕㄡˊ, 蚵 ㄎㄜ, 呃 ㄜ)
  any word                  the rows for the character's reading in the text: Taiwan readings of words (期 ㄑㄧ →
                            ㄑㄧˊ) and, for unfamiliar characters, their other readings at m (`misread`, the phonetic
                            series among them, which is a correction)
  a neutral tone            another reading of the character at `neutral_full_tone` (什麼 ㄇㄛˊ), accepted
  2nd or 3rd before a 3rd   the other tone at m (你好 ㄋㄧˊ ㄏㄠˇ), accepted

A row for the reading in the text overrides a `*` row typing the same reading.  At each position the readings that
apply have probabilities p; the position is typed another way with probability min(cap, Σ w·p), the reading drawn in
proportion to w·p, where w is `scale` for the classes in `scaled` and 1 for the others.  `scale` is the noise rate
chosen on the dev sets (1 is the channel's own rate).  `scaled` is every class by default; RARE are the classes whose
probability is the misreading rate m or the neutral rate (a few per thousand), the rest are shares of a character's
or a word's readings and already as frequent as typists use them.  `cap` keeps the text's reading at least as likely
as the variants, so a Taiwan reading with strength 0.99 does not erase the reading the standard dev sets type.

    from zhuyintype_ml.training.reading.noise import ReadingNoise
    noise = ReadingNoise(paths.data("lexicon_coverage", "channel"), scale=10)
    builder.reading = noise                  # zhuyintype_ml.training.v3.dataset.Builder
    noise.one(text, readings, lexicon, rng)  # one variant per text, for the dev checks
"""

from __future__ import annotations

import json
import math
import random
from collections import defaultdict
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

from zhuyintype_ml.training.v3.readings import Lexicon, Reading, split_reading

# Classes whose probability is a small rate (m, the neutral rate), not a share of the character's readings.
RARE = frozenset({"misread", "series", "neutral", "sandhi"})
HEADER = "# char\treading in the text\treading typed\tclass\tprobability\tstate\tcost bucket"
STATES = ("accepted", "correction")


@dataclass(frozen=True)
class Option:
    typed: Reading
    probability: float
    bucket: int
    kind: str  # the row's class, or "neutral" / "sandhi" for the rule-level variants
    state: str


class ReadingNoise:
    def __init__(
        self,
        directory: Path,
        scale: float = 1.0,
        cap: float = 0.5,
        valid: Callable[[Reading], bool] | None = None,
        scaled: frozenset[str] | None = None,
    ) -> None:
        """`valid` keeps only readings v3 knows (the training vocabulary's syllables); `scaled` the classes `scale`
        applies to (default: all)."""
        self.scale, self.cap, self.scaled = scale, cap, scaled
        ok = valid or (lambda _r: True)
        self.any: dict[str, dict[Reading, Option]] = defaultdict(dict)
        self.specific: dict[tuple[str, Reading], dict[Reading, Option]] = defaultdict(dict)
        with open(directory / "v3_noise.tsv", encoding="utf-8") as lines:
            header = next(lines).rstrip("\n")
            if header != HEADER:
                raise ValueError(f"{directory / 'v3_noise.tsv'}: expected the 7-column header {HEADER!r}")
            for line in lines:
                char, text, typed_text, kind, p, state, bucket = line.rstrip("\n").split("\t")
                if state not in STATES or (state == "accepted") != (bucket == "0"):
                    raise ValueError(f"v3_noise.tsv: state and bucket disagree: {line!r}")
                typed = split_reading(typed_text)
                if not ok(typed):
                    continue
                option = Option(typed, float(p), int(bucket), kind, state)
                table = self.any[char] if text == "*" else self.specific[(char, split_reading(text))]
                if option.probability > 0 and option.probability > getattr(table.get(typed), "probability", 0.0):
                    table[typed] = option
        summary = json.loads((directory / "channel.json").read_text(encoding="utf-8"))
        self.misread = float(summary["misread"])
        self.neutral = float(summary["neutral_full_tone"])
        # Toned readings of each character and their shares (channel.tsv), for the neutral tone typed another way.
        self.toned: dict[str, list[tuple[Reading, float]]] = defaultdict(list)
        with open(directory / "channel.tsv", encoding="utf-8") as lines:
            for line in lines:
                if line.startswith("#"):
                    continue
                char, reading, _kind, logp, *_ = line.rstrip("\n").split("\t")
                r = split_reading(reading)
                if r[1] != 5 and ok(r):
                    self.toned[char].append((r, math.exp(float(logp))))
        self.valid = ok

    def options(self, text: str, readings: Sequence[Reading], i: int, single: bool) -> list[Option]:
        """The readings a typist may enter at position `i` instead of `readings[i]`."""
        char, reading = text[i], readings[i]
        if reading[0] == "<literal>":
            return []
        found: dict[Reading, Option] = {}
        if single:
            found.update(self.any.get(char, {}))
        found.update(self.specific.get((char, reading), {}))
        if reading[1] == 5:
            toned = self.toned.get(char, [])
            total = sum(p for _, p in toned)
            for r, p in toned:
                if r not in found and total > 0:
                    found[r] = Option(r, self.neutral * p / total, 0, "neutral", "accepted")
        if reading[1] in (2, 3) and i + 1 < len(readings) and readings[i + 1][1] == 3:
            other = (reading[0], 5 - reading[1])
            if other not in found and self.valid(other):
                found[other] = Option(other, self.misread, 0, "sandhi", "accepted")
        found.pop(reading, None)
        return list(found.values())

    def weight(self, option: Option) -> float:
        return self.scale if self.scaled is None or option.kind in self.scaled else 1.0

    @staticmethod
    def singles(text: str, lexicon: Lexicon) -> list[bool]:
        """Whether each character is a word of its own in the lexicon's longest-match segmentation."""
        flags = [False] * len(text)
        for start, end, _word, _r in lexicon.segments(text):
            if end - start == 1:
                flags[start] = True
        return flags

    def __call__(
        self, text: str, readings: Sequence[Reading], lexicon: Lexicon, rng: random.Random
    ) -> dict[int, tuple[Reading, int]]:
        """Training noise: {position: (typed reading, cost bucket)} for the positions typed another way."""
        single = self.singles(text, lexicon)
        changed: dict[int, tuple[Reading, int]] = {}
        for i in range(len(text)):
            options = self.options(text, readings, i, single[i])
            weights = [o.probability * self.weight(o) for o in options]
            if options and rng.random() < min(self.cap, sum(weights)):
                o = rng.choices(options, weights)[0]
                changed[i] = (o.typed, o.bucket)
        return changed

    def one(
        self, text: str, readings: Sequence[Reading], lexicon: Lexicon, rng: random.Random
    ) -> tuple[int, Option] | None:
        """One position typed another way (drawn uniformly among the positions that have a variant), for the dev
        checks; None when no position has one."""
        single = self.singles(text, lexicon)
        positions = [(i, o) for i in range(len(text)) if (o := self.options(text, readings, i, single[i]))]
        if not positions:
            return None
        i, options = rng.choice(positions)
        return i, rng.choices(options, [o.probability for o in options])[0]
