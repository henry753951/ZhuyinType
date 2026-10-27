"""The reading channel: how likely a typist enters reading r for character c, P(r | c), estimated from data.

    python -m zhuyintype_ml.training.lexicon.channel --base BASE.tsv --wiktionary W.tsv --out DIR

The base lexicon gives each character the readings CC-CEDICT has, shared by kHanyuPinlu.  The channel reaches a
character from every plausible reading, each with a probability from evidence:

  taiwan    a reading Wiktionary marks as Taiwan's for the character (期 ㄑㄧˊ, 熟 ㄕㄡˊ)
  word      a reading the character has inside dictionary words (CC-CEDICT, Wiktionary), weighted by how often those
            words are used (蚵 ㄎㄜ from 屎蚵螂); a neutral tone inside a word is the word's, not the character's
  attested  another reading Wiktionary gives the character itself (蚵 ㄎㄜ)
  unihan    a reading only Unihan lists (kHanyuPinyin, kXHC1983, kTGHZ2013, kMandarin, both values); many are
            archaic, so their share is the small pooled rate below
  series    有邊讀邊: the reading of the phonetic component and the same-syllable readings of the other characters
            that contain it (BabelStone IDS, first level, kept when at least 20% of the series reads like the
            component; Wiktionary's glyph origin when it names the component), only for unfamiliar characters
  interjection  every tone of an interjection's or particle's syllables (呃 ㄜ for ㄜˋ): characters a dictionary
            tags as interjection, particle, filler or onomatopoeia and that stand alone in the spoken corpus
            (--spoken: at least 80% of their occurrences touch an utterance boundary; function words such as 了 那
            do about half the time); their share is the rate at which dictionaries attest another tone for such a
            syllable

`taiwan`, `attested` and `unihan` readings get the character's own kHanyuPinlu share when it has counts (add-half
smoothing), otherwise a pooled rate: the share of spoken usage that falls on readings only Wiktionary, or only
Unihan, lists, over the characters that have counts.  `series` readings get the misreading rate m times the series
share.  A character is familiar when it is among the most frequent characters that cover 99% of the lexicon's
character occurrences (word frequencies); readers guess an unfamiliar character from its component, and they are the
characters whose other readings get word-level variants (merge.py).

Two word-level tables come with it:

  taiwan.tsv  (char, the reading CC-CEDICT's words use, Taiwan's reading, log strength): of the words with the
              character that Wiktionary gives a reading for, the share whose Taiwan reading changes the first to the
              second (期 ㄑㄧ -> ㄑㄧˊ in 期中); plus CC-CEDICT's own Taiwan readings of single characters (危 ㄨㄟ ->
              ㄨㄟˊ) at the median strength of the pairs both sources have; merge.py puts the Taiwan reading into
              every word with the character
  rates       m: of the character positions in Wiktionary's multi-character words, how many carry a recorded variant,
              colloquial or misreading reading; the neutral rate: of Wiktionary's neutral-tone syllables, how many
              have a recorded full-tone variant

All rates and counts are in channel.json; the app's correction log can later replace them.  Taiwanese Hokkien
readings are word-level only (merge.py): they apply to the words Wiktionary gives a Hokkien reading for.
"""

from __future__ import annotations

import argparse
import collections
import json
import math
import re
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

from zhuyintype_ml.data import lexicon as base_lexicon

from . import sources, wiktionary
from .corpora import corpus_lines
from .phonology import bopomofo, split
from .wiktionary import INTERJECTION

UNIHAN_FIELDS = ("kHanyuPinyin", "kXHC1983", "kTGHZ2013", "kMandarin")
MECHANISMS = ("lexicon", "taiwan", "word", "attested", "cns", "unihan", "series", "interjection")
# Source groups: sources that copy each other count once (resources/formats/reading-channel.md).
GROUPS = ("cedict-unihan", "wiktionary", "cns", "ids", "rule")
# The table formats (resources/formats/reading-channel.md); the engine reads the same files.
CHANNEL_HEADER = "# ZTRC/1 char\treading\tclass\tlog P(reading | char)\tsource groups\n"
TAIWAN_HEADER = "# ZTRT/1 char\tword reading\tTaiwan reading\tlog strength\tstandard\n"
COVERAGE = 0.99  # familiar characters: the most frequent ones covering this share of character occurrences
SERIES_AGREEMENT = 0.2  # a component is phonetic when at least this share of its series reads like it
# An interjection touches an utterance boundary in at least this share of its spoken occurrences.
STANDALONE = 0.8
VARIANT_TAGS = {"variant", "colloquial", "common misreading", "nonstandard", "dialectal", "common"}
FLOOR = 3e-9  # the base lexicon's frequency for a word wordfreq does not know


Rows = list[tuple[str, str, float]]


def read_rows(path: Path) -> Rows:
    rows = []
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("#"):
                continue
            reading, word, logp = line.rstrip("\n").split("\t")
            rows.append((reading, word, float(logp)))
    return rows


def read_unihan(archive: Path) -> dict[str, set[str]]:
    """Every Mandarin reading Unihan gives a character, in Zhuyin."""
    readings: dict[str, set[str]] = collections.defaultdict(set)
    with zipfile.ZipFile(archive) as z, z.open("Unihan_Readings.txt") as raw:
        for line in raw.read().decode("utf-8").splitlines():
            parts = line.split("\t")
            if len(parts) != 3 or parts[1] not in UNIHAN_FIELDS:
                continue
            ch = chr(int(parts[0][2:], 16))
            values = parts[2].split() if parts[1] == "kMandarin" else re.findall(r":([^\s]+)", parts[2])
            for value in values:
                for item in value.split(","):
                    base, tone = base_lexicon.numbered_from_diacritic(item)
                    zy = base_lexicon.to_zhuyin(base, tone)
                    if zy:
                        readings[ch].add(zy)
    return readings


def read_unigrams(path: Path) -> dict[str, float]:
    """A language model's single-character unigrams (word, log P), from its unigram.tsv."""
    result = {}
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            parts = line.split()
            if len(parts) == 2 and len(parts[0]) == 1:
                result[parts[0]] = float(parts[1])
    return result


def read_cns(properties: Path, mapping: Path) -> dict[str, set[str]]:
    """CNS11643 全字庫: every Zhuyin reading Taiwan's official character set gives a character (CNS_phonetic.txt,
    mapped to Unicode with the CNS2UNICODE tables)."""
    unicode_of: dict[str, str] = {}
    with zipfile.ZipFile(mapping) as z:
        for name in z.namelist():
            if name.startswith("Unicode/CNS2UNICODE") and name.endswith(".txt"):
                for line in z.read(name).decode("utf-8", errors="replace").splitlines():
                    parts = line.split("\t")
                    if len(parts) >= 2 and re.fullmatch(r"[0-9A-F]{4,6}", parts[1].strip()):
                        unicode_of[parts[0].strip()] = chr(int(parts[1].strip(), 16))
    readings: dict[str, set[str]] = collections.defaultdict(set)
    with zipfile.ZipFile(properties) as z, z.open("CNS_phonetic.txt") as raw:
        for line in raw.read().decode("utf-8-sig").splitlines():
            parts = line.split("\t")
            if len(parts) == 2 and parts[0] in unicode_of:
                reading = bopomofo(parts[1].strip())
                if reading and " " not in reading:
                    readings[unicode_of[parts[0]]].add(reading)
    return readings


def cedict_interjections() -> set[str]:
    """Single characters CC-CEDICT defines as an interjection, particle, filler or onomatopoeia."""
    found = set()
    with open(base_lexicon.RAW / "cedict.txt", encoding="utf-8") as lines:
        for line in lines:
            m = re.match(r"^(\S) \S \[[^\]]*\] /(.*)/\s*$", line)
            if m and INTERJECTION.search(m.group(2)):
                found.add(m.group(1))
    return found


def read_ids(path: Path) -> dict[str, list[str]]:
    """BabelStone IDS: each character's first-level components that are single characters."""
    components: dict[str, list[str]] = {}
    with open(path, encoding="utf-8-sig") as lines:
        for line in lines:
            if line.startswith("#"):
                continue
            parts = line.rstrip("\r\n").split("\t")
            if len(parts) < 3:
                continue
            match = re.match(r"\^([⿰-⿿])(.*)\$", parts[2])
            if not match:
                continue
            children = first_level(match.group(2))
            components[parts[1]] = [c for c in children if len(c) == 1 and HAN_CHAR.match(c)]
    return components


HAN_CHAR = re.compile(r"[㐀-䶿一-鿿\U00020000-\U0003134f]")
ARITY = {chr(c): 2 for c in range(0x2FF0, 0x2FFC)} | {"⿲": 3, "⿳": 3}


def first_level(sequence: str) -> list[str]:
    """'虫可' -> ['虫', '可']; '⿱艹口可' -> ['⿱艹口', '可'] (a nested sequence is one child)."""

    def unit(i: int) -> int:
        if i < len(sequence) and sequence[i] in ARITY:
            j = i + 1
            for _ in range(ARITY[sequence[i]]):
                j = unit(j)
            return j
        return i + 1

    children, i = [], 0
    while i < len(sequence):
        j = unit(i)
        children.append(sequence[i:j])
        i = j
    return children


def mandarin_readings(facts: list[tuple[str, str, str, list[str]]]) -> dict[str, dict[str, set[str]]]:
    """word -> {reading: Wiktionary's labels}, without erhua and without readings only used in Mainland China
    (Wiktionary repeats every reading without labels, so the labels of all copies are joined first)."""
    joined: dict[str, dict[str, set[str]]] = collections.defaultdict(lambda: collections.defaultdict(set))
    for word, kind, value, tags in facts:
        if kind == "mandarin":
            joined[word][value].update(tags)
    result: dict[str, dict[str, set[str]]] = {}
    for word, readings in joined.items():
        kept = {
            r: t for r, t in readings.items() if "Erhua" not in t and not ("Mainland-China" in t and "Taiwan" not in t)
        }
        if kept:
            result[word] = kept
    return result


def variants(readings: dict[str, set[str]]) -> list[tuple[str, str]]:
    """(standard, variant) pairs: the Taiwan or standard reading and each reading labelled variant, colloquial,
    nonstandard, dialectal or common misreading."""
    if not readings:
        return []
    standard = next((r for r, t in readings.items() if "Taiwan" in t), None)
    standard = standard or next((r for r, t in readings.items() if "standard" in t), None) or min(readings)
    return [(standard, r) for r, t in readings.items() if r != standard and VARIANT_TAGS & t]


def hokkien_readings(facts: list[tuple[str, str, str, list[str]]]) -> dict[str, list[str]]:
    """Taiwanese Mandarin words -> their Taiwanese Hokkien readings in Tâi-lô.  Wiktionary gives a Hokkien reading for
    almost every common word (成熟 sîng-sio̍k); a typist uses the Hokkien sound only in words Taiwan Mandarin took
    from Hokkien, so only words in Wiktionary's Taiwanese Mandarin category or marked (Tw)/Taiwanese by CC-CEDICT
    count."""
    taiwanese = {w for w, kind, value, _ in facts if kind == "class" and value == "taiwanese-mandarin"}
    taiwanese |= cedict_taiwanese()
    result: dict[str, list[str]] = collections.defaultdict(list)
    for word, kind, value, tags in facts:
        if kind == "hokkien" and "Taiwan" in tags and word in taiwanese and value not in result[word]:
            result[word].append(value)
    return result


def cedict_taiwanese() -> set[str]:
    """Words CC-CEDICT marks as Taiwan usage or as taken from Taiwanese."""
    found: set[str] = set()
    path = base_lexicon.RAW / "cedict.txt"
    if not path.exists():
        return found
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            m = re.match(r"^(\S+) \S+ \[[^\]]*\] /(.*)/\s*$", line)
            if m and ("(Tw)" in m.group(2) or "Taiwanese" in m.group(2)):
                found.add(m.group(1))
    return found


@dataclass
class Channel:
    """P(reading | character) for every character, with the mechanism that brought each reading in."""

    share: dict[str, dict[str, float]] = field(default_factory=dict)
    mechanism: dict[tuple[str, str], str] = field(default_factory=dict)
    familiar: set[str] = field(default_factory=set)
    taiwan: dict[tuple[str, str], tuple[str, float]] = field(default_factory=dict)
    evidence: dict[tuple[str, str], str] = field(default_factory=dict)  # (char, reading) -> source groups
    standard: set[tuple[str, str]] = field(default_factory=set)  # Taiwan pairs strong enough to be the standard
    misread: float = 0.0  # m: the share of character positions typed with another reading of the character
    neutral: float = 0.0
    rates: dict[str, object] = field(default_factory=dict)

    def alternatives(self, char: str, reading: str) -> list[tuple[str, float]]:
        """Readings a typist may enter for an unfamiliar `char` where the word reads it `reading`, each with
        log P(typed | char in the word) = log m + log P(r | char, r is not the word's reading)."""
        shares = self.share.get(char)
        if not shares or char in self.familiar:
            return []
        rest = 1.0 - shares.get(reading, 0.0)
        if rest <= 0:
            return []
        rate = math.log(self.misread)
        return [(r, rate + math.log(p / rest)) for r, p in shares.items() if r != reading and p > 0]

    def write_noise(self, path: Path) -> None:
        """The variants as reading noise for training the per-character model (v3): `char<TAB>reading in the
        text (* for any)<TAB>reading typed<TAB>class<TAB>probability`.  A training example replaces a character's
        reading by a row's typed reading with that probability, so the model learns from context which character
        a variant reading stands for.  Rule-level variants (third-tone sandhi, the neutral tone typed with its own
        syllable) are in channel.json (misread, neutral_full_tone)."""
        with open(path, "w", encoding="utf-8") as handle:
            handle.write("# char\treading in the text\treading typed\tclass\tprobability\n")
            for char in sorted(self.share):
                shares = self.share[char]
                for reading, p in sorted(shares.items()):
                    mechanism = self.mechanism[(char, reading)]
                    if mechanism != "lexicon":
                        rate = p if char not in self.familiar or mechanism != "series" else 0.0
                        if rate > 0:
                            handle.write(f"{char}\t*\t{reading}\t{mechanism}\t{rate:.6g}\n")
            for (char, mainland), (taiwan, strength) in sorted(self.taiwan.items()):
                handle.write(f"{char}\t{mainland}\t{taiwan}\ttaiwan\t{strength:.6g}\n")
            for char in sorted(self.share):
                if char in self.familiar:
                    continue
                for reading in sorted(self.share[char]):
                    for typed, cost in self.alternatives(char, reading):
                        handle.write(f"{char}\t{reading}\t{typed}\tmisread\t{math.exp(cost):.6g}\n")

    def write(self, out: Path) -> None:
        out.mkdir(parents=True, exist_ok=True)
        with open(out / "channel.tsv", "w", encoding="utf-8") as handle:
            handle.write(CHANNEL_HEADER)
            for char in sorted(self.share):
                for reading, p in sorted(self.share[char].items(), key=lambda x: (-x[1], x[0])):
                    sources = self.evidence.get((char, reading), "")
                    handle.write(
                        f"{char}\t{reading}\t{self.mechanism[(char, reading)]}\t{math.log(p):.4f}\t{sources}\n"
                    )
        with open(out / "taiwan.tsv", "w", encoding="utf-8") as handle:
            handle.write(TAIWAN_HEADER)
            for (char, mainland), (taiwan, strength) in sorted(self.taiwan.items()):
                standard = int((char, mainland) in self.standard)
                handle.write(f"{char}\t{mainland}\t{taiwan}\t{math.log(strength):.4f}\t{standard}\n")
        (out / "familiar.txt").write_text("".join(sorted(self.familiar)) + "\n", encoding="utf-8")
        self.write_noise(out / "v3_noise.tsv")
        counts = collections.Counter(m for m in self.mechanism.values())
        summary = dict(self.rates)
        summary.update(
            {
                "misread": self.misread,
                "neutral_full_tone": self.neutral,
                "readings_by_mechanism": {m: counts.get(m, 0) for m in MECHANISMS},
                "characters": len(self.share),
                "familiar_characters": len(self.familiar),
                "taiwan_pairs": len(self.taiwan),
            }
        )
        (out / "channel.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


def estimate(
    base: Rows,
    facts: list[tuple[str, str, str, list[str]]],
    unihan: dict[str, set[str]],
    ids: dict[str, list[str]],
    cns: dict[str, set[str]] | None = None,
    cedict: list[base_lexicon.CedictEntry] | None = None,
    tagged: set[str] | None = None,
    spoken_lines: list[str] | None = None,
    unigrams: tuple[dict[str, float], dict[str, float]] | None = None,
) -> Channel:
    top: dict[str, float] = {}
    best_reading: dict[str, str] = {}
    for reading, word, logp in base:
        if logp > top.get(word, -math.inf):
            top[word], best_reading[word] = logp, reading
    singles: dict[str, dict[str, float]] = collections.defaultdict(dict)
    for reading, word, logp in base:
        if len(word) == 1:
            singles[word][reading] = math.exp(logp)
    main = {c: max(r, key=lambda k: r[k]) for c, r in singles.items()}

    # Frequency mass of words with each character read each way (CC-CEDICT's words in the base lexicon).
    evidence: dict[str, dict[str, float]] = collections.defaultdict(lambda: collections.defaultdict(float))
    char_freq: dict[str, float] = collections.defaultdict(float)
    known = {(reading, word) for reading, word, _ in base}
    for reading, word, logp in base:
        if len(word) >= 2:
            for ch, syl in zip(word, reading.split()):
                evidence[ch][syl] += math.exp(logp)
    for word, best_logp in top.items():
        for ch in word:
            char_freq[ch] += math.exp(best_logp)

    # Familiar characters: the most frequent ones that cover COVERAGE of the character occurrences.
    ranked = sorted(char_freq, key=lambda c: (-char_freq[c], c))
    total_mass, running = sum(char_freq.values()), 0.0
    familiar: set[str] = set()
    for c in ranked:
        if running >= COVERAGE * total_mass:
            break
        familiar.add(c)
        running += char_freq[c]

    base_readings: dict[str, set[str]] = collections.defaultdict(set)
    for ch, syls in evidence.items():
        base_readings[ch] |= set(syls)
    mandarin = mandarin_readings(facts)
    wikt_readings: dict[str, set[str]] = collections.defaultdict(set)
    for word, readings in mandarin.items():
        if len(word) > 1:
            for text in readings:
                for ch, syl in zip(word, text.split()):
                    wikt_readings[ch].add(syl)
    taiwan_single: dict[str, set[str]] = collections.defaultdict(set)
    attested_single: dict[str, set[str]] = collections.defaultdict(set)
    new_word_readings: list[tuple[str, str]] = []
    for word, readings in mandarin.items():
        for text, tags in readings.items():
            if len(word) == 1:
                (taiwan_single if "Taiwan" in tags else attested_single)[word].add(text)
            elif (text, word) not in known:
                new_word_readings.append((word, text))

    # Pooled rates from kHanyuPinlu: the share of spoken usage on readings only Wiktionary or only Unihan lists.
    pinlu = base_lexicon.read_pinlu()
    pooled = {"attested": [0.0, 0.0], "unihan": [0.0, 0.0]}
    for ch, counts in pinlu.items():
        if ch not in singles:
            continue  # kHanyuPinlu counts Simplified forms; only characters of the Traditional lexicon count here
        spoken = sum(counts.values())
        listed = set(singles[ch]) | set(evidence.get(ch, {}))
        extra_w = (attested_single.get(ch, set()) | taiwan_single.get(ch, set())) - listed
        extra_u = unihan.get(ch, set()) - listed - extra_w
        for name, extra in (("attested", extra_w), ("unihan", extra_u)):
            pooled[name][0] += sum(counts.get(r, 0) for r in extra)
            pooled[name][1] += spoken * len(extra)
    alpha = {name: (hits + 0.5) / (n + 1.0) for name, (hits, n) in pooled.items()}
    for word, text in new_word_readings:
        weight = math.exp(top[word]) * alpha["attested"] if word in top else FLOOR
        for ch, syl in zip(word, text.split()):
            evidence[ch][syl] += weight

    # Taiwan readings of characters inside words: Wiktionary's Taiwan reading of a word against the base's reading.
    changed: collections.Counter[tuple[str, str, str]] = collections.Counter()
    for word, readings in mandarin.items():
        if len(word) < 2 or word not in best_reading:
            continue
        taiwan_word = [r for r, t in readings.items() if "Taiwan" in t]
        if not taiwan_word:
            continue
        ours = best_reading[word].split()
        theirs = taiwan_word[0].split()
        for ch, a, b in zip(word, ours, theirs):
            if a != b and not b.endswith("˙") and not a.endswith("˙"):
                changed[(ch, a, b)] += 1
    # Wiktionary labels a reading Taiwan only where Taiwan differs, so every word with a Wiktionary reading counts in
    # the denominator: a word without a Taiwan label reads the character the same way.
    seen: collections.Counter[tuple[str, str]] = collections.Counter()
    for word in mandarin:
        if len(word) >= 2 and word in best_reading:
            seen.update(zip(word, best_reading[word].split()))
    taiwan_pairs: dict[tuple[str, str], tuple[str, float]] = {}
    for (ch, a, b), n in changed.items():
        strength = (n + 0.5) / (seen[(ch, a)] + 1.0)
        if (ch, a) not in taiwan_pairs or taiwan_pairs[(ch, a)][1] < strength:
            taiwan_pairs[(ch, a)] = (b, strength)

    # CC-CEDICT's Taiwan readings of single characters (危 wei1, Taiwan pr. wei2) go into every word with the
    # character too, at the median strength the Wiktionary pairs have (CC-CEDICT gives no per-word evidence).
    cedict = cedict or []
    cedict_pairs: set[tuple[str, str, str]] = set()
    by_char: dict[str, list[tuple[str, bool, bool]]] = collections.defaultdict(list)
    for trad, _simp, reading, is_taiwan, is_variant in cedict:
        if len(trad) == 1:
            by_char[trad].append((reading, is_taiwan, is_variant))
    for c, entries in by_char.items():
        taiwan_readings = {r for r, t, v in entries if t and not v}
        mainland_readings = {r for r, t, v in entries if not t and v}
        if len(taiwan_readings) == 1:
            for r in mainland_readings - taiwan_readings:
                cedict_pairs.add((c, r, next(iter(taiwan_readings))))
    both = [taiwan_pairs[(c, a)][1] for c, a, b in cedict_pairs if taiwan_pairs.get((c, a), ("", 0.0))[0] == b]
    cedict_strength = sorted(both)[len(both) // 2] if both else 0.5
    for c, a, b in sorted(cedict_pairs):
        if (c, a) not in taiwan_pairs:
            taiwan_pairs[(c, a)] = (b, cedict_strength)

    # Phonetic series: Wiktionary's named component, else an IDS first-level component that most series read like.
    syllable = {c: split(r)[0] for c, r in main.items()}
    members: dict[str, list[str]] = collections.defaultdict(list)
    for c, parts in ids.items():
        if c in main:  # only characters the lexicon can read take part in a series
            for part in set(parts):
                members[part].append(c)
    agreement = {
        p: sum(syllable.get(m) == syllable[p] for m in ms) / len(ms)
        for p, ms in members.items()
        if p in syllable and len(ms) >= 2
    }
    phonetic: dict[str, str] = {}
    for c, parts in ids.items():
        good = [p for p in parts if agreement.get(p, 0.0) >= SERIES_AGREEMENT and p != c]
        if good:
            phonetic[c] = max(good, key=lambda p: (agreement[p], p))
    for word, kind, value, _ in facts:
        if kind == "phonetic" and value in syllable:
            phonetic[word] = value

    def series(c: str) -> dict[str, float]:
        p = phonetic.get(c)
        if not p or c in familiar or p not in main:
            return {}
        same = [main[m] for m in members.get(p, []) if m != c and m in main and syllable[m] == syllable[p]]
        counts = collections.Counter([main[p], *same])
        n = sum(counts.values())
        return {r: k / n for r, k in counts.items()}

    # Interjections and particles: tagged by a dictionary (Wiktionary's part of speech or glosses, CC-CEDICT's
    # definitions) and standing at an utterance boundary in spoken text more often than characters do on average.
    tagged = (tagged or set()) | {w for w, kind, _, _ in facts if kind == "class" and len(w) == 1}
    boundary: collections.Counter[str] = collections.Counter()
    occurrences: collections.Counter[str] = collections.Counter()
    for line in spoken_lines or []:
        for run in re.findall(r"[㐀-䶿一-鿿]+", line):
            occurrences.update(run)
            boundary[run[0]] += 1
            if len(run) > 1:
                boundary[run[-1]] += 1
    average = sum(boundary.values()) / max(1, sum(occurrences.values()))
    # (and, with the language models given, is at least as likely in the chat model as in the encyclopedic one).
    # An interjection stands alone: at least STANDALONE of its occurrences touch an utterance or clause boundary
    # (function words such as 了 那 好 do so about half the time).  A neutral-tone particle (吧 ㄅㄚ˙) already
    # matches every typed tone through the engine's neutral-tone tolerance.
    interjections = {
        c
        for c in tagged
        if c in main
        and not main[c].endswith("˙")
        and occurrences[c] >= 3
        and boundary[c] / occurrences[c] >= STANDALONE
        and (unigrams is None or unigrams[0].get(c, -math.inf) >= unigrams[1].get(c, math.inf))
    }
    # How freely their tones vary: of the tones their syllables could take, the share dictionaries attest.
    bases_tones: dict[str, set[str]] = collections.defaultdict(set)
    for c in interjections:
        for r in set(singles[c]) | taiwan_single.get(c, set()) | attested_single.get(c, set()) | unihan.get(c, set()):
            bases_tones[c + split(r)[0]].add(split(r)[1])
    extra_tones = sum(len(t) - 1 for t in bases_tones.values())
    possible = sum(4 for _ in bases_tones)
    interjection_rate = (extra_tones + 0.5) / (possible + 1.0)

    # The misreading rate and the neutral rate, from Wiktionary's recorded word variants.
    positions = sum(len(w) for w in mandarin if len(w) >= 2)
    misreads = 0
    neutral_counts = [0, 0]
    for word, readings in mandarin.items():
        if len(word) < 2:
            continue
        for standard, variant in variants(readings):
            s, v = standard.split(), variant.split()
            diff = [i for i in range(len(s)) if s[i] != v[i]]
            for i in diff:
                if s[i].endswith("˙") and not v[i].endswith("˙"):
                    neutral_counts[0] += 1
                elif len(diff) == 1:
                    misreads += 1
        standard_reading = next((r for r, t in readings.items() if "Taiwan" in t), None) or min(readings)
        neutral_counts[1] += sum(x.endswith("˙") for x in standard_reading.split())
    misread = (misreads + 0.5) / (positions + 1.0)

    # A Taiwan reading is the standard (merge.py makes it the word's reading, the other one a variant) when CNS11643
    # lists it and two independent groups agree: Wiktionary's words (strength >= 0.9), CC-CEDICT's single character,
    # CNS11643 not listing the other reading.  Sources that copy each other count once (see GROUPS).
    cns = cns or {}
    wikt_pairs = {(ch, a) for (ch, a), (b, strength) in taiwan_pairs.items() if strength >= 0.9}
    cedict_keys = {(c, a) for c, a, _ in cedict_pairs}
    standard_pairs = set()
    for (ch, a), (b, _) in taiwan_pairs.items():
        listed = cns.get(ch, set())
        votes = ((ch, a) in wikt_pairs) + ((ch, a) in cedict_keys) + (bool(listed) and a not in listed)
        if b in listed and votes >= 2:
            standard_pairs.add((ch, a))

    channel = Channel(
        familiar=familiar,
        taiwan=taiwan_pairs,
        standard=standard_pairs,
        misread=misread,
        neutral=(neutral_counts[0] + 0.5) / (neutral_counts[1] + 1.0),
    )
    channel.rates = {
        "alpha_attested": alpha["attested"],
        "alpha_unihan": alpha["unihan"],
        "pinlu_pooled": pooled,
        "wiktionary_positions": positions,
        "wiktionary_misreads": misreads,
        "neutral_counts": neutral_counts,
        "coverage": COVERAGE,
        "series_agreement": SERIES_AGREEMENT,
        "phonetic_components": len(phonetic),
        "series_characters": sum(1 for c in phonetic if c not in familiar),
        "interjections": "".join(sorted(interjections)),
        "cedict_taiwan_pairs": len(cedict_pairs),
        "standard_taiwan_pairs": len(standard_pairs),
        "cns_characters": len(cns),
        "cedict_taiwan_strength": cedict_strength,
        "interjection_tone_rate": interjection_rate,
        "spoken_boundary_rate": average,
    }

    for c in set(singles) | (set(evidence) & set(char_freq)):
        mass: dict[str, float] = {}
        label: dict[str, str] = {}
        char_total = sum(singles.get(c, {}).values()) + sum(evidence.get(c, {}).values()) or FLOOR
        for r, single_mass in singles.get(c, {}).items():
            mass[r], label[r] = single_mass, "lexicon"

        def add(r: str, value: float, name: str, mass: dict[str, float] = mass, label: dict[str, str] = label) -> None:
            if value <= 0:
                return
            if r not in mass:
                mass[r], label[r] = value, name
            else:
                mass[r] += value

        # A character with kHanyuPinlu counts uses its own (add-half smoothed); others the pooled rates.
        spoken_c: dict[str, int] = dict(pinlu.get(c, {})) if c in singles else {}
        spoken_total = sum(spoken_c.values())

        def rate(
            r: str, pooled_rate: float, spoken_c: dict[str, int] = spoken_c, spoken_total: int = spoken_total
        ) -> float:
            return (spoken_c.get(r, 0) + 0.5) / (spoken_total + 1.0) if spoken_total else pooled_rate

        for r in sorted(taiwan_single.get(c, set())):
            add(r, max(rate(r, alpha["attested"]), base_lexicon.SENSE_SHARE) * char_total, "taiwan")
        for r, word_mass in evidence.get(c, {}).items():
            # A neutral tone belongs to the word (認識 ㄕ˙), not to the character; the engine already accepts a
            # typed neutral tone for a toned reading.
            if not r.endswith("˙") or r in mass:
                add(r, word_mass, "word")
        for r in sorted(attested_single.get(c, set())):
            add(r, rate(r, alpha["attested"]) * char_total, "attested")
        for r in sorted(cns.get(c, set())):
            add(r, rate(r, alpha["attested"]) * char_total, "cns")
        for r in sorted(unihan.get(c, set())):
            add(r, rate(r, alpha["unihan"]) * char_total, "unihan")
        for r, member_share in series(c).items():
            add(r, misread * member_share * char_total, "series")
        if c in interjections:
            for r, reading_mass in list(mass.items()):
                bare = split(r)[0]
                for tone in ("", "ˊ", "ˇ", "ˋ", "˙"):
                    add(bare + tone, interjection_rate * reading_mass, "interjection")
        norm = sum(mass.values())
        channel.share[c] = {r: v / norm for r, v in mass.items()}
        series_readings = series(c)
        for r in mass:
            channel.mechanism[(c, r)] = label[r]
            groups = []
            if r in singles.get(c, {}) or r in base_readings.get(c, set()) or r in unihan.get(c, set()):
                groups.append("cedict-unihan")
            if (
                r in attested_single.get(c, set())
                or r in taiwan_single.get(c, set())
                or r in wikt_readings.get(c, set())
            ):
                groups.append("wiktionary")
            if r in cns.get(c, set()):
                groups.append("cns")
            if r in series_readings:
                groups.append("ids")
            if label[r] == "interjection":
                groups.append("rule")
            channel.evidence[(c, r)] = ",".join(groups)
    return channel


SYLLABLE = re.compile(r"[ㄅ-ㄩ]{1,3}[ˊˇˋ˙]?")


def validate(directory: Path) -> list[str]:
    """Problems with channel.tsv and taiwan.tsv against resources/formats/reading-channel.md (empty when valid).
    The engine's reader checks the same rules."""
    problems: list[str] = []
    lines = (directory / "channel.tsv").read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != CHANNEL_HEADER.rstrip("\n"):
        problems.append("channel.tsv: missing ZTRC/1 header")
    seen: set[tuple[str, str]] = set()
    totals: dict[str, float] = collections.defaultdict(float)
    previous: tuple[str, float, bytes] | None = None
    for number, line in enumerate(lines[1:], 2):
        parts = line.split("\t")
        if len(parts) != 5:
            problems.append(f"channel.tsv:{number}: {len(parts)} fields")
            continue
        char, reading, kind, logp, groups = parts
        if len(char) != 1 or not SYLLABLE.fullmatch(reading) or kind not in MECHANISMS:
            problems.append(f"channel.tsv:{number}: bad character, reading or class")
        value = float(logp)
        if not math.isfinite(value) or value > 1e-9:
            problems.append(f"channel.tsv:{number}: log P must be finite and at most 0")
        if (char, reading) in seen:
            problems.append(f"channel.tsv:{number}: duplicate")
        seen.add((char, reading))
        if groups and not set(groups.split(",")) <= set(GROUPS):
            problems.append(f"channel.tsv:{number}: unknown source group")
        if previous and previous[0] == char and (previous[1], previous[2]) > (-value, reading.encode()):
            problems.append(f"channel.tsv:{number}: rows of a character out of order")
        previous = (char, -value, reading.encode())
        totals[char] += math.exp(value)
    problems += [f"channel.tsv: {c} sums to {t:.4f}" for c, t in totals.items() if abs(t - 1) > 1e-3]
    lines = (directory / "taiwan.tsv").read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != TAIWAN_HEADER.rstrip("\n"):
        problems.append("taiwan.tsv: missing ZTRT/1 header")
    for number, line in enumerate(lines[1:], 2):
        parts = line.split("\t")
        if len(parts) != 5 or not SYLLABLE.fullmatch(parts[1]) or not SYLLABLE.fullmatch(parts[2]):
            problems.append(f"taiwan.tsv:{number}: bad row")
        elif float(parts[3]) > 1e-9 or parts[4] not in ("0", "1"):
            problems.append(f"taiwan.tsv:{number}: bad strength or standard flag")
    return problems


def load(directory: Path) -> Channel:
    """Reads channel.tsv, taiwan.tsv, familiar.txt and channel.json back."""
    channel = Channel()
    with open(directory / "channel.tsv", encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("#"):
                continue
            char, reading, mechanism, logq, *rest = line.rstrip("\n").split("\t")
            channel.share.setdefault(char, {})[reading] = math.exp(float(logq))
            channel.mechanism[(char, reading)] = mechanism
            channel.evidence[(char, reading)] = rest[0] if rest else ""
    with open(directory / "taiwan.tsv", encoding="utf-8") as lines:
        for line in lines:
            if not line.startswith("#"):
                char, mainland, taiwan, strength, *rest = line.rstrip("\n").split("\t")
                channel.taiwan[(char, mainland)] = (taiwan, math.exp(float(strength)))
                if rest and rest[0] == "1":
                    channel.standard.add((char, mainland))
    channel.familiar = set((directory / "familiar.txt").read_text(encoding="utf-8").strip())
    summary = json.loads((directory / "channel.json").read_text(encoding="utf-8"))
    channel.misread = summary["misread"]
    channel.neutral = summary["neutral_full_tone"]
    return channel


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", type=Path, required=True, help="the base lexicon (zt data lexicon)")
    parser.add_argument("--wiktionary", type=Path, required=True, help="wiktionary.py's facts")
    parser.add_argument("--spoken", type=Path, nargs="*", default=[], help="spoken text (real_dialogues.jsonl, ...)")
    parser.add_argument("--chat-lm", type=Path, help="the chat model's unigram.tsv")
    parser.add_argument("--wiki-lm", type=Path, help="the encyclopedic model's unigram.tsv")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    unigrams = (read_unigrams(args.chat_lm), read_unigrams(args.wiki_lm)) if args.chat_lm and args.wiki_lm else None
    channel = estimate(
        read_rows(args.base),
        wiktionary.read(args.wiktionary),
        read_unihan(sources.path("Unihan-18.0.0.zip")),
        read_ids(sources.path("IDS.TXT")),
        read_cns(sources.path("CNS11643-Properties.zip"), sources.path("CNS11643-MapingTables.zip")),
        base_lexicon.read_cedict(),
        cedict_interjections(),
        corpus_lines("chat", args.spoken, "name"),
        unigrams,
    )
    channel.write(args.out)
    print(json.dumps(channel.rates, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
