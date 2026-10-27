"""Valid-syllable keyboard noise, sandhi and Taiwan reading augmentation."""
import math
import random

from readings import split_reading

KEYS = dict(zip("1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-", "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ"))
TONES = {1: " ", 2: "6", 3: "3", 4: "4", 5: "7"}
CENTRES = {key: (x + shift, y) for y, (row, shift) in enumerate([
    ("1234567890-", 0.), ("qwertyuiop", .25), ("asdfghjkl;", .5), ("zxcvbnm,./", 1.)]) for x, key in enumerate(row)}


class Noise:
    def __init__(self, readings):
        inv = {v: k for k, v in KEYS.items()}
        self.encode = {r: "".join(inv[c] for c in r[0]) + TONES[r[1]] for r in readings if r[0] != "<literal>"}
        self.decode = {keys: r for r, keys in self.encode.items()}
        self.substitutions = {}
        for key, (x, y) in CENTRES.items():
            choices, weights = [], []
            for other, (a, b) in CENTRES.items():
                if other != key:
                    choices.append(other)
                    weights.append(math.exp(-.5 * (((a - x) / .9) ** 2 + ((b - y) / .45) ** 2)))
            self.substitutions[key] = choices, weights
        self.neighbours = {}
        for r, keys in self.encode.items():
            scores = {}
            for i, key in enumerate(keys):
                if key in self.substitutions:
                    choices, weights = self.substitutions[key]
                    for other, weight in zip(choices, weights):
                        candidate = self.decode.get(keys[:i] + other + keys[i + 1:])
                        if candidate and weight > scores.get(candidate, 0):
                            scores[candidate] = weight
                for changed in [keys[:i] + keys[i + 1:]]:
                    candidate = self.decode.get(changed)
                    if candidate:
                        scores[candidate] = max(scores.get(candidate, 0), .05)
            for a, b in [(3, 4), (4, 3), (2, 5), (5, 2)]:
                if r[1] == a and (r[0], b) in self.encode:
                    scores[(r[0], b)] = 1.
            self.neighbours[r] = sorted(scores, key=lambda s: (-scores[s], s))[:3]

    def corrupt(self, reading, rng):
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
            changed = keys[:i] + rng.choices(choices, weights)[0] + keys[i + 1:]
        elif kind == "transpose" and len(keys) >= 2:
            i = rng.randrange(len(keys) - 1)
            changed = keys[:i] + keys[i + 1] + keys[i] + keys[i + 2:]
        elif kind == "extra":
            changed = keys[:i] + rng.choice(list(KEYS)) + keys[i:]
        elif kind == "missing":
            changed = keys[:i] + keys[i + 1:]
        else:
            changed = keys
        output = self.decode.get(changed, reading)
        return output, (0 if output == reading else {"tone": 1, "sub": 2, "transpose": 2, "extra": 3, "missing": 3}[kind])

    def variants(self, text, readings, lexicon, rng, rate=None, augment=True):
        readings = list(readings)
        for i, (char, r) in enumerate(zip(text, readings)):
            if augment and char in "一不" and rng.random() < .5:
                following = readings[i + 1][1] if i + 1 < len(readings) else 0
                if char == "一" and following:
                    proposed = (r[0], 2 if following in (4, 5) else 4)
                    if proposed in self.encode:
                        readings[i] = proposed
                elif char == "不" and following == 4 and (r[0], 2) in self.encode:
                    readings[i] = (r[0], 2)
            if augment and r[1] == 5 and rng.random() < .3 and (r[0], 1) in self.encode:
                readings[i] = (r[0], 1)
            if augment and char in lexicon.single and rng.random() < .1:
                rows = lexicon.single[char]
                # Taiwan sense rows retain at least 25% of the leading reading's prior.
                choices = [s for p, s in rows if p >= rows[0][0] + math.log(.24)]
                readings[i] = rng.choice(choices)
        rate = rng.uniform(0, .15) if rate is None else rate
        result = []
        for reading in readings:
            typed, cost = self.corrupt(reading, rng) if rng.random() < rate else (reading, 0)
            boundary = int(augment and typed[1] == 1 and rng.random() < .2)
            result.append((typed, cost, boundary))
        return result
