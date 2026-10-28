"""Typing variants for syllable training: valid-syllable keyboard noise, tone sandhi and Taiwan readings.

`Noise(lexicon.candidates)` knows every reading's standard-layout keys.  `variants` gives each character of a
training example the reading as typed (maybe corrupted by a nearby key, a swapped tone, a transposed, extra or
missing key, when the result is still a valid syllable), a cost bucket and an implicit-boundary flag.  The candidate
pool of a reading also takes in its three nearest valid neighbours.

    from zhuyintype_ml.training.v3.noise import Noise
    noise = Noise(lexicon.candidates)
    typed = noise.variants(text, readings, lexicon, random.Random(7))

`variants` also takes a reading channel (`reading`, ml/src/zhuyintype_ml/training/reading/noise.py): readings typed
another way, each with its own cost bucket; those positions take no further augmentation or key noise.
"""

from __future__ import annotations

import math
import random
from collections.abc import Callable, Iterable, Sequence
from typing import TYPE_CHECKING

from zhuyintype_ml.core import zhuyin

if TYPE_CHECKING:
    from .readings import Lexicon, Reading

    # (text, readings, lexicon, rng) -> {position: (typed reading, cost bucket)}
    ReadingChannel = Callable[[str, Sequence[Reading], Lexicon, random.Random], dict[int, tuple[Reading, int]]]

# Standard-layout key → Zhuyin symbol, in keyboard order 1qaz2wsx… (the order `corrupt` draws extra keys from).
KEYS = {key: symbol for symbol, key in zhuyin.KEYS.items() if symbol not in zhuyin.TONES}
TONES = {1: " ", 2: "6", 3: "3", 4: "4", 5: "7"}
CENTRES = {
    key: (x + shift, y)
    for y, (row, shift) in enumerate(
        [("1234567890-", 0.0), ("qwertyuiop", 0.25), ("asdfghjkl;", 0.5), ("zxcvbnm,./", 1.0)]
    )
    for x, key in enumerate(row)
}


class Noise:
    def __init__(self, readings: Iterable[Reading]) -> None:
        inv = {v: k for k, v in KEYS.items()}
        self.encode = {r: "".join(inv[c] for c in r[0]) + TONES[r[1]] for r in readings if r[0] != "<literal>"}
        self.decode = {keys: r for r, keys in self.encode.items()}
        self.substitutions: dict[str, tuple[list[str], list[float]]] = {}
        for key, (x, y) in CENTRES.items():
            choices: list[str] = []
            weights: list[float] = []
            for other, (a, b) in CENTRES.items():
                if other != key:
                    choices.append(other)
                    weights.append(math.exp(-0.5 * (((a - x) / 0.9) ** 2 + ((b - y) / 0.45) ** 2)))
            self.substitutions[key] = choices, weights
        self.neighbours: dict[Reading, list[Reading]] = {}
        for r, keys in self.encode.items():
            scores: dict[Reading, float] = {}
            for i, key in enumerate(keys):
                if key in self.substitutions:
                    choices, weights = self.substitutions[key]
                    for other, weight in zip(choices, weights):
                        candidate = self.decode.get(keys[:i] + other + keys[i + 1 :])
                        if candidate and weight > scores.get(candidate, 0):
                            scores[candidate] = weight
                for changed in [keys[:i] + keys[i + 1 :]]:
                    candidate = self.decode.get(changed)
                    if candidate:
                        scores[candidate] = max(scores.get(candidate, 0), 0.05)
            for a, b in [(3, 4), (4, 3), (2, 5), (5, 2)]:
                if r[1] == a and (r[0], b) in self.encode:
                    scores[(r[0], b)] = 1.0
            self.neighbours[r] = sorted(scores, key=lambda s: (-scores[s], s))[:3]

    def corrupt(self, reading: Reading, rng: random.Random) -> tuple[Reading, int]:
        keys = self.encode.get(reading)
        if not keys:
            return reading, 0
        kind = rng.choices(["sub", "tone", "transpose", "extra", "missing"], [45, 20, 15, 10, 10])[0]
        i = rng.randrange(len(keys))
        if kind == "tone":
            target = {3: 4, 4: 3, 2: 5, 5: 2}.get(reading[1])
            changed = keys[:-1] + TONES[target] if target else keys
        elif kind == "sub" and keys[i] in self.substitutions:
            choices, weights = self.substitutions[keys[i]]
            changed = keys[:i] + rng.choices(choices, weights)[0] + keys[i + 1 :]
        elif kind == "transpose" and len(keys) >= 2:
            i = rng.randrange(len(keys) - 1)
            changed = keys[:i] + keys[i + 1] + keys[i] + keys[i + 2 :]
        elif kind == "extra":
            changed = keys[:i] + rng.choice(list(KEYS)) + keys[i:]
        elif kind == "missing":
            changed = keys[:i] + keys[i + 1 :]
        else:
            changed = keys
        output = self.decode.get(changed, reading)
        return output, (
            0 if output == reading else {"tone": 1, "sub": 2, "transpose": 2, "extra": 3, "missing": 3}[kind]
        )

    def variants(
        self,
        text: str,
        readings: Sequence[Reading],
        lexicon: Lexicon,
        rng: random.Random,
        rate: float | None = None,
        augment: bool = True,
        reading: ReadingChannel | None = None,
    ) -> list[tuple[Reading, int, int]]:
        channel = reading(text, readings, lexicon, rng) if reading else {}
        readings = list(readings)
        for i, (char, r) in enumerate(zip(text, readings)):
            if i in channel:
                continue
            if augment and char in "一不" and rng.random() < 0.5:
                following = readings[i + 1][1] if i + 1 < len(readings) else 0
                if char == "一" and following:
                    proposed = (r[0], 2 if following in (4, 5) else 4)
                    if proposed in self.encode:
                        readings[i] = proposed
                elif char == "不" and following == 4 and (r[0], 2) in self.encode:
                    readings[i] = (r[0], 2)
            if augment and r[1] == 5 and rng.random() < 0.3 and (r[0], 1) in self.encode:
                readings[i] = (r[0], 1)
            if augment and char in lexicon.single and rng.random() < 0.1:
                rows = lexicon.single[char]
                # Taiwan sense rows retain at least 25% of the leading reading's prior.
                choices = [s for p, s in rows if p >= rows[0][0] + math.log(0.24)]
                readings[i] = rng.choice(choices)
        rate = rng.uniform(0, 0.15) if rate is None else rate
        result = []
        for i, item in enumerate(readings):
            if i in channel:
                typed, cost = channel[i]
            else:
                typed, cost = self.corrupt(item, rng) if rng.random() < rate else (item, 0)
            boundary = int(augment and typed[1] == 1 and rng.random() < 0.2)
            result.append((typed, cost, boundary))
        return result
