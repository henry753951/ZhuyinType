"""The lexicon with the reading channel and vocabulary packs: base rows unchanged, variant rows added below them.

    python -m zhuyintype_ml.training.lexicon.merge --base BASE.tsv --channel DIR --wiktionary W.tsv \\
        [--pack PACK.tsv ...] [--offset taiwan=0 --offset misread=2 ...] --out lexicon.tsv

Every base row (reading, word, log P) is kept as it is, so an exact dictionary reading scores as before.  The engine
splits a row into log P(word) (the word's best row) and log P(reading | word) (the rest), so a variant row is a
reading the lattice uses at a cost; every cost is below zero, so a variant never ties with the standard reading.
Rows are added for (the class names are what --mechanisms and --offset take):

  standard    a word whose character has a Taiwan reading strong enough to be the standard (channel.py: CNS11643
              lists it and two independent groups agree, 期 ㄑㄧˊ, 危 ㄨㄟˊ) takes it as its reading at the word's
              best score; the replaced reading stays at log max(1 - strength, m).  Typing the old reading costs
              that much, so this class changes the standard typing the dev sets assume (re-label them to compare)

  char        each reading the channel gives a character (channel.py: Taiwan, dictionary words, Wiktionary, Unihan,
              phonetic series), at the character's best row plus log of the reading's share relative to the best
              reading (蚵 ㄎㄜ, 熟 ㄕㄡˊ)
  taiwan      a word with a character whose CC-CEDICT reading Wiktionary's Taiwan readings change (期 ㄑㄧ -> ㄑㄧˊ):
              the word with the Taiwan reading, at log strength (期中 ㄑㄧˊ ㄓㄨㄥ); every subset of such characters
  neutral     a neutral-tone syllable typed with another reading of its character (什麼 ㄕㄣˊ ㄇㄛˊ): the neutral rate
              times that reading's share among the character's other readings (the same syllable with a tone is the
              engine's neutral-tone tolerance already)
  mcbopomofo  a reading McBopomofo's data.txt (MIT) gives a word of ours (危險 ㄨㄟˊ ㄒㄧㄢˇ, 熟悉 ㄕㄡˊ ㄒㄧ):
              its share among McBopomofo's readings of the word (McBopomofo's own scores); only readings, never its
              words or frequencies
  sandhi      third-tone sandhi: before a third tone, a second or third tone typed as the other (你好 ㄋㄧˊ ㄏㄠˇ,
              危險 ㄨㄟˇ ㄒㄧㄢˇ), one syllable at a time, at log m: no source records how often people type the
              pronounced tone, so the cost starts at the general misreading rate and is set by --offset
  variants    Wiktionary's variant, colloquial and misreading readings of the word, at log m per changed character
  loan        a word Wiktionary gives a Taiwanese Hokkien reading for, with one character typed as the Mandarin
              syllable nearest its Hokkien syllable (phonology.tailo_to_zhuyin).  No source gives the Mandarin tone
              (仔 á is typed ㄚˇ by some, ㄚ or ㄚ˙ by others), so every tone gets log m − log 5
  misread     an unfamiliar character (channel.py) typed with another of its readings, its phonetic series included:
              log m + log P(r | char, not the word's reading), kept down to --max-cost (蚵仔煎 ㄎㄜ ㄗˇ ㄐㄧㄢ)
  packs       vocabulary.py's mined words with their readings and frequencies, given the same variants

--offset CLASS=VALUE adds VALUE nats to the cost of a class (tuned on the development sets: standard typing must not
change, misread-dev must convert); --mechanisms keeps only some classes, to measure each on its own.  The row counts
per class go to the .json beside the output.
"""

from __future__ import annotations

import argparse
import collections
import itertools
import json
import math
from collections.abc import Iterable
from pathlib import Path

from zhuyintype_ml.data.lexicon import HAN

from . import sources, wiktionary
from .channel import Channel, hokkien_readings, load, mandarin_readings, read_rows, variants
from .phonology import TONE_MARKS, bopomofo, split, tailo_syllables, tailo_to_zhuyin

MECHANISMS = ("standard", "char", "taiwan", "mcbopomofo", "neutral", "sandhi", "variants", "loan", "misread")
# The classes the engine's reading channel is to take over (resources/formats/reading-channel.md); a data-only build
# keeps the others.
ENGINE_CLASSES = ("neutral", "sandhi", "loan", "misread")
ENGINE_CHARACTER_MECHANISMS = ("series", "interjection")
TIE = 0.01  # a variant never ties with the word's best reading, even when a source gives it the whole share
HEADER = (
    "# ZhuyinType lexicon with reading variants and vocabulary packs. Sources: CC-CEDICT (CC BY-SA 4.0), "
    "Unicode Unihan (Unicode License v3), wordfreq (CC BY-SA 4.0), English Wiktionary via Wiktextract "
    "(CC BY-SA 4.0), BabelStone IDS{packs}. See resources/SOURCES.md.\n"
)


class Builder:
    def __init__(
        self,
        base: list[tuple[str, str, float]],
        channel: Channel,
        max_cost: float = 8.0,
        mechanisms: Iterable[str] = MECHANISMS,
        offsets: dict[str, float] | None = None,
        mcbopomofo: dict[str, dict[str, float]] | None = None,
    ) -> None:
        self.mcbopomofo = mcbopomofo or {}
        self.engine_character: tuple[str, ...] = ()
        self.channel = channel
        self.max_cost = max_cost
        self.mechanisms = set(mechanisms)
        self.offsets = offsets or {}
        self.rows: dict[tuple[str, str], float] = {}
        self.standard: dict[str, list[str]] = {}  # each word's Taiwan and McBopomofo readings (sandhi applies to them)
        self.kind: dict[tuple[str, str], str] = {}
        self.syllables = {split(s)[0] for reading, _, _ in base for s in reading.split()}
        for reading, word, logp in base:
            self.put(reading, word, logp, "base")
        self.best: dict[str, tuple[str, float]] = {}
        for (reading, word), logp in self.rows.items():
            if word not in self.best or self.best[word][1] < logp:
                self.best[word] = (reading, logp)

    def put(self, reading: str, word: str, logp: float, kind: str) -> None:
        """Adds a row unless the word already has this reading at least as high (a base row is never replaced)."""
        key = (reading, word)
        if key in self.rows and (self.kind[key] == "base" or self.rows[key] >= logp):
            return
        self.rows[key] = logp
        self.kind[key] = kind
        if kind in ("taiwan", "mcbopomofo"):
            self.standard.setdefault(word, []).append(reading)

    def cost(self, kind: str, value: float) -> float:
        return value - self.offsets.get(kind, 0.0)

    def standardize(self) -> int:
        """Taiwan readings strong enough to be the standard (channel.py `standard`) become the word's reading at its
        best score; the reading it replaces stays as a variant at log max(1 - strength, m).  Returns the words
        changed."""
        if "standard" not in self.mechanisms:
            return 0
        changed = 0
        for word, (reading, logp) in list(self.best.items()):
            syls = reading.split()
            new = list(syls)
            strengths = []
            for i, (c, syl) in enumerate(zip(word, syls)):
                if (c, syl) in self.channel.standard:
                    new[i], strength = self.channel.taiwan[(c, syl)]
                    strengths.append(strength)
            if not strengths:
                continue
            taiwan = " ".join(new)
            demoted = logp + self.cost("standard", math.log(max(1 - min(strengths), self.channel.misread)))
            self.rows[(reading, word)] = demoted
            self.kind[(reading, word)] = "mainland"
            self.rows[(taiwan, word)] = logp
            self.kind[(taiwan, word)] = "standard"
            self.best[word] = (taiwan, logp)
            changed += 1
        return changed

    def characters(self) -> None:
        if "char" not in self.mechanisms:
            return
        for char, shares in self.channel.share.items():
            if char not in self.best:
                continue
            reading, logp = self.best[char]
            top = shares.get(reading)
            if not top:
                continue
            for r, p in shares.items():
                official = not self.engine_character or "cns" in self.channel.evidence.get((char, r), "")
                if p < top and self.channel.mechanism.get((char, r)) not in self.engine_character and official:
                    self.put(r, char, logp + self.cost("char", math.log(p / top)), "char")

    def word(
        self,
        word: str,
        reading: str,
        logp: float,
        attested: Iterable[str] = (),
        hokkien: Iterable[str] = (),
    ) -> None:
        """The variants of one word (every class the builder uses)."""
        syls = reading.split()
        rate = math.log(self.channel.misread)

        def replace(i: int, r: str) -> str:
            return " ".join(syls[:i] + [r] + syls[i + 1 :])

        if "taiwan" in self.mechanisms:
            changes = [
                (i, *self.channel.taiwan[(c, s)])
                for i, (c, s) in enumerate(zip(word, syls))
                if (c, s) in self.channel.taiwan
            ]
            for size in range(1, len(changes) + 1):
                for subset in itertools.combinations(changes, size):
                    new = list(syls)
                    value = 0.0
                    for i, taiwan, strength in subset:
                        new[i] = taiwan
                        value += math.log(strength) - TIE
                    self.put(" ".join(new), word, logp + self.cost("taiwan", value), "taiwan")
        for i, (char, syl) in enumerate(zip(word, syls)):
            if syl.endswith("˙") and "neutral" in self.mechanisms:
                shares = self.channel.share.get(char, {})
                others = {r: p for r, p in shares.items() if not r.endswith("˙") and split(r)[0] != split(syl)[0]}
                total = sum(others.values())
                for r, p in others.items():
                    value = math.log(self.channel.neutral) + math.log(p / total)
                    self.put(replace(i, r), word, logp + self.cost("neutral", value), "neutral")
            if "misread" in self.mechanisms:
                for r, value in self.channel.alternatives(char, syl):
                    # A different tone alone is the key channel's tone correction (and neutral-tone tolerance).
                    if value >= -self.max_cost and split(r)[0] != split(syl)[0]:
                        self.put(replace(i, r), word, logp + self.cost("misread", value), "misread")
        if "mcbopomofo" in self.mechanisms and word in self.mcbopomofo:
            for r, share in self.mcbopomofo[word].items():
                if len(r.split()) == len(syls):
                    self.put(r, word, logp + self.cost("mcbopomofo", math.log(share) - TIE), "mcbopomofo")
        if "sandhi" in self.mechanisms:
            # On the word's reading and on its Taiwan and McBopomofo readings (危險 ㄨㄟˊ ㄒㄧㄢˇ -> ㄨㄟˇ ㄒㄧㄢˇ).
            sources = [(reading, logp)] + [(r, self.rows[(r, word)]) for r in self.standard.get(word, [])]
            for source, value in sources:
                parts = source.split()
                for i in range(len(parts) - 1):
                    (bare, tone), (_, following) = split(parts[i]), split(parts[i + 1])
                    if following == "ˇ" and tone in ("ˊ", "ˇ"):
                        sandhi = " ".join(parts[:i] + [bare + ("ˇ" if tone == "ˊ" else "ˊ")] + parts[i + 1 :])
                        self.put(sandhi, word, value + self.cost("sandhi", rate), "sandhi")
        if "variants" in self.mechanisms:
            for variant in attested:
                changed = sum(a != b for a, b in zip(syls, variant.split()))
                if changed and len(variant.split()) == len(syls):
                    self.put(variant, word, logp + self.cost("variants", rate * changed), "variants")
        if "loan" in self.mechanisms:
            for i, base, tones in self.loan_syllables(word, syls, hokkien):
                for tone in tones:
                    value = rate - math.log(len(tones))
                    self.put(replace(i, base + tone), word, logp + self.cost("loan", value), "loan")

    def loan_syllables(self, word: str, syls: list[str], values: Iterable[str]) -> list[tuple[int, str, list[str]]]:
        """(position, Mandarin syllable without tone, its tones) for each character whose Hokkien syllable maps to
        another Mandarin syllable than the word's reading."""
        out = []
        for value in values:
            tailo = tailo_syllables(value)
            if len(tailo) != len(word):
                continue
            for i, (syl, neutral) in enumerate(tailo):
                zy = tailo_to_zhuyin(syl, neutral, self.syllables)
                if not zy or split(zy)[0] == split(syls[i])[0]:
                    continue
                out.append((i, split(zy)[0], ["", *TONE_MARKS]))
        return out

    def words(self, facts: list[tuple[str, str, str, list[str]]]) -> None:
        mandarin = mandarin_readings(facts)
        hokkien = hokkien_readings(facts)
        for word, (reading, logp) in list(self.best.items()):
            if len(word) < 2:
                continue
            attested = [v for _, v in variants(mandarin.get(word, {}))]
            self.word(word, reading, logp, attested, hokkien.get(word, []))

    def pack(self, path: Path) -> int:
        """Adds a pack (reading, word, log P rows) and its variants; returns how many words it brought."""
        added = 0
        for reading, word, logp in read_rows(path):
            if word in self.best:
                continue
            self.put(reading, word, logp, "pack")
            self.best[word] = (reading, logp)
            self.word(word, reading, logp)
            added += 1
        return added

    def write(self, out: Path, packs: list[str]) -> dict[str, int]:
        rows = sorted(((r, w, p) for (r, w), p in self.rows.items()), key=lambda x: (x[0], -x[2], x[1]))
        out.parent.mkdir(parents=True, exist_ok=True)
        names = "".join(f", {name}" for name in packs)
        with open(out, "w", encoding="utf-8") as handle:
            handle.write(HEADER.format(packs=names))
            for reading, word, logp in rows:
                handle.write(f"{reading}\t{word}\t{logp:.3f}\n")
        return dict(collections.Counter(self.kind.values()))


def read_mcbopomofo(path: Path) -> dict[str, dict[str, float]]:
    """McBopomofo's data.txt: word -> {reading: share among its readings}, from its scores (log10)."""
    scores: dict[str, dict[str, float]] = collections.defaultdict(dict)
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            parts = line.split()
            if len(parts) != 3 or line.startswith(("#", "_")) or not HAN.match(parts[1]):
                continue
            reading = bopomofo(parts[0].replace("-", " "))
            if reading and len(reading.split()) == len(parts[1]):
                scores[parts[1]][reading] = max(scores[parts[1]].get(reading, -math.inf), float(parts[2]))
    shares = {}
    for word, readings in scores.items():
        total = sum(10**v for v in readings.values())
        shares[word] = {r: 10**v / total for r, v in readings.items()}
    return shares


def parse_offsets(items: list[str]) -> dict[str, float]:
    offsets = {}
    for item in items:
        name, value = item.split("=", 1)
        if name not in MECHANISMS:
            raise SystemExit(f"unknown class {name!r}; classes: {', '.join(MECHANISMS)}")
        offsets[name] = float(value)
    return offsets


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--channel", type=Path, required=True, help="channel.py's output directory")
    parser.add_argument("--wiktionary", type=Path, required=True)
    parser.add_argument("--pack", type=Path, action="append", default=[], help="pack TSV (repeatable)")
    parser.add_argument("--max-cost", type=float, default=8.0, help="lowest log P(typed | word) of a misread variant")
    parser.add_argument(
        "--mechanisms", default=",".join(MECHANISMS), help=f"classes to use (default: {','.join(MECHANISMS)})"
    )
    parser.add_argument("--offset", action="append", default=[], metavar="CLASS=NATS", help="extra cost of a class")
    parser.add_argument(
        "--data-only",
        action="store_true",
        help="the merge candidate without the engine's classes: no neutral, sandhi, loan or misread rows, and "
        "character rows only for readings CNS11643 lists (Taiwan's official readings); the rest of the character "
        "table goes to the engine's reading channel",
    )
    parser.add_argument("--mcbopomofo", type=Path, help="McBopomofo's data.txt (default: data/raw/lexicon)")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    mechanisms = [m for m in args.mechanisms.split(",") if m]
    if args.data_only:
        mechanisms = [m for m in mechanisms if m not in ENGINE_CLASSES]
    offsets = parse_offsets(args.offset)
    mcb = read_mcbopomofo(args.mcbopomofo or sources.path("mcbpmf-data.txt"))
    builder = Builder(read_rows(args.base), load(args.channel), args.max_cost, mechanisms, offsets, mcb)
    if args.data_only:
        builder.engine_character = ENGINE_CHARACTER_MECHANISMS
    standardized = builder.standardize()
    builder.characters()
    builder.words(wiktionary.read(args.wiktionary))
    packs = {path.stem: builder.pack(path) for path in args.pack}
    counts = builder.write(args.out, list(packs))
    summary = {
        "rows": sum(counts.values()),
        "by_class": counts,
        "pack_words": packs,
        "max_cost": args.max_cost,
        "mechanisms": mechanisms,
        "offsets": offsets,
        "standardized_words": standardized,
        "data_only": args.data_only,
    }
    args.out.with_suffix(".json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
