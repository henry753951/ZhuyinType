#!/usr/bin/env python3
"""Build the basic lexicon (data/build/lexicon.tsv) from open data.

Inputs (downloaded by data/tools/fetch.sh into data/raw/):
  - CC-CEDICT (CC BY-SA 4.0): words, Traditional forms, readings (Taiwan readings preferred when noted)
  - Unicode Unihan (Unicode License v3): readings for Big5 characters missing from CC-CEDICT
  - wordfreq (CC BY-SA 4.0 data): word frequencies, looked up by Simplified form

Output: one line per (reading, word): `reading<TAB>word<TAB>logp`, where reading is the Zhuyin syllables separated by
spaces (tone marks ˊˇˋ˙ after the syllable, first tone unmarked) and logp is a natural-log probability of the word
written with that reading.  A character's frequency is shared among its readings by Unihan's kHanyuPinlu counts, so
the engine can tell a common reading (說 ㄕㄨㄛ) from a rare one (說 ㄕㄨㄟˋ).  Words with 一 or 不 also get the
readings people type after tone sandhi (一直 ㄧˋ ㄓˊ, 一定 ㄧˊ ㄉㄧㄥˋ, 不要 ㄅㄨˊ ㄧㄠˋ).
"""
import collections
import math
import os
import re
import sys
import unicodedata

import wordfreq

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RAW = os.path.join(ROOT, "raw")
OUT = os.path.join(ROOT, "build", "lexicon.tsv")

# MARK: - Pinyin -> Zhuyin

INITIALS = [("zh", "ㄓ"), ("ch", "ㄔ"), ("sh", "ㄕ"), ("b", "ㄅ"), ("p", "ㄆ"), ("m", "ㄇ"), ("f", "ㄈ"), ("d", "ㄉ"),
            ("t", "ㄊ"), ("n", "ㄋ"), ("l", "ㄌ"), ("g", "ㄍ"), ("k", "ㄎ"), ("h", "ㄏ"), ("j", "ㄐ"), ("q", "ㄑ"),
            ("x", "ㄒ"), ("r", "ㄖ"), ("z", "ㄗ"), ("c", "ㄘ"), ("s", "ㄙ")]
FINALS = {
    "a": "ㄚ", "o": "ㄛ", "e": "ㄜ", "ê": "ㄝ", "ai": "ㄞ", "ei": "ㄟ", "ao": "ㄠ", "ou": "ㄡ", "an": "ㄢ", "en": "ㄣ",
    "ang": "ㄤ", "eng": "ㄥ", "er": "ㄦ", "ong": "ㄨㄥ",
    "i": "ㄧ", "ia": "ㄧㄚ", "io": "ㄧㄛ", "ie": "ㄧㄝ", "iai": "ㄧㄞ", "iao": "ㄧㄠ", "iu": "ㄧㄡ", "iou": "ㄧㄡ",
    "ian": "ㄧㄢ", "in": "ㄧㄣ", "iang": "ㄧㄤ", "ing": "ㄧㄥ", "iong": "ㄩㄥ",
    "u": "ㄨ", "ua": "ㄨㄚ", "uo": "ㄨㄛ", "uai": "ㄨㄞ", "ui": "ㄨㄟ", "uei": "ㄨㄟ", "uan": "ㄨㄢ", "un": "ㄨㄣ",
    "uen": "ㄨㄣ", "uang": "ㄨㄤ", "ueng": "ㄨㄥ",
    "ü": "ㄩ", "üe": "ㄩㄝ", "üan": "ㄩㄢ", "ün": "ㄩㄣ",
}
Y_W = {
    "yi": "i", "ya": "ia", "yo": "io", "ye": "ie", "yai": "iai", "yao": "iao", "you": "iou", "yan": "ian", "yin": "in",
    "yang": "iang", "ying": "ing", "yong": "iong", "yu": "ü", "yue": "üe", "yuan": "üan", "yun": "ün",
    "wu": "u", "wa": "ua", "wo": "uo", "wai": "uai", "wei": "uei", "wan": "uan", "wen": "uen", "wang": "uang",
    "weng": "ueng",
}
TONE_MARKS = {1: "", 2: "ˊ", 3: "ˇ", 4: "ˋ", 5: "˙"}
DIACRITICS = {"̄": 1, "́": 2, "̌": 3, "̀": 4}


def numbered_from_diacritic(syl):
    """'hǎo' -> ('hao', 3); 'lǜ' -> ('lü', 4)"""
    decomposed = unicodedata.normalize("NFD", syl)
    tone = 5
    base = []
    for ch in decomposed:
        if ch in DIACRITICS:
            tone = DIACRITICS[ch]
        else:
            base.append(ch)
    return unicodedata.normalize("NFC", "".join(base)), tone


def to_zhuyin(base, tone):
    """Pinyin syllable without tone ('zhong', 'lü', 'yuan') -> Zhuyin with tone mark, or None."""
    base = base.lower().replace("u:", "ü").replace("v", "ü")
    if base in ("zhi", "chi", "shi", "ri", "zi", "ci", "si"):
        initial = dict(INITIALS)[base[:-1]]
        return initial + TONE_MARKS[tone]
    if base in Y_W:
        rest, initial = Y_W[base], ""
    else:
        initial = ""
        rest = base
        for py, zy in INITIALS:
            if base.startswith(py):
                initial, rest = zy, base[len(py):]
                break
        if initial in ("ㄐ", "ㄑ", "ㄒ") and rest.startswith("u"):
            rest = "ü" + rest[1:]
    if rest not in FINALS:
        return None
    if initial == "" and rest in ("ong",):
        return None
    return initial + FINALS[rest] + TONE_MARKS[tone]


def cedict_reading(pinyin):
    """'ni3 hao3' -> 'ㄋㄧˇ ㄏㄠˇ'; None when any syllable is not a standard one."""
    out = []
    for token in pinyin.split():
        m = re.fullmatch(r"([A-Za-z:]+)([1-5])", token)
        if not m:
            return None
        zy = to_zhuyin(m.group(1), int(m.group(2)))
        if zy is None:
            return None
        out.append(zy)
    return " ".join(out)


# MARK: - Sources

HAN = re.compile(r"^[㐀-鿿豈-﫿\U00020000-\U0003134f]+$")
VARIANT = re.compile(r"^(old |archaic |Japanese |ancient |erhua )?variant of ")


COMMON = set()  # (trad, reading) with a lower-case (non-name) entry: surnames alone do not make a usual reading
SENSE = set()  # (trad, reading) given as the Taiwan reading of one sense: "和 (and) (Taiwan pr. [han4])"
SENSE_SHARE = 0.25  # the least share of a character's frequency such a reading gets; kHanyuPinlu counts mainland usage


def read_cedict():
    entries = []  # (trad, simp, reading, is_taiwan, is_variant)
    with open(os.path.join(RAW, "cedict.txt"), encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            m = re.match(r"^(\S+) (\S+) \[([^\]]*)\] /(.*)/\s*$", line)
            if not m:
                continue
            trad, simp, pinyin, defs = m.groups()
            if not HAN.match(trad) or len(trad) > 8:
                continue
            definitions = defs.split("/")
            variant = all(VARIANT.match(d) or d.startswith("see ") for d in definitions)
            # "/Taiwan pr. [wei2]/" as its own item is the reading of the whole entry; "(Taiwan pr. [xing4])" inside
            # an item belongs to that sense only (行 conduct), and the entry's reading stays the main one.
            taiwan, sense_readings = None, []
            for d in definitions:
                whole = re.fullmatch(r"Taiwan pr\. \[([^\]]*)\]", d.strip())
                if whole:
                    taiwan = cedict_reading(whole.group(1))
                    continue
                for tm in re.finditer(r"Taiwan pr\. \[([^\]]*)\]", d):
                    sense_readings.append(cedict_reading(tm.group(1)))
            reading = cedict_reading(pinyin)
            if taiwan and len(taiwan.split()) == len(trad):
                if not pinyin[:1].isupper():
                    COMMON.add((trad, taiwan))
                entries.append((trad, simp, taiwan, True, variant))
                if reading:
                    entries.append((trad, simp, reading, False, True))  # mainland reading, kept but demoted
            elif reading and len(reading.split()) == len(trad):
                if not pinyin[:1].isupper():
                    COMMON.add((trad, reading))
                entries.append((trad, simp, reading, False, variant))
                for sense in sense_readings:
                    if sense and sense != reading and len(sense.split()) == len(trad):
                        SENSE.add((trad, sense))
                        entries.append((trad, simp, sense, True, False))
    return entries


def read_unihan():
    """Big5 characters and their Mandarin readings (Taiwan value first when Unihan gives two)."""
    big5 = set()
    with open(os.path.join(RAW, "Unihan_OtherMappings.txt"), encoding="utf-8") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3 and parts[1] == "kBigFive":
                big5.add(chr(int(parts[0][2:], 16)))
    readings = {}
    with open(os.path.join(RAW, "Unihan_Readings.txt"), encoding="utf-8") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3 and parts[1] == "kMandarin":
                ch = chr(int(parts[0][2:], 16))
                values = parts[2].split()
                values = values[::-1] if len(values) == 2 else values  # [zh-Hans, zh-Hant] -> Taiwan first
                zys = []
                for v in values:
                    base, tone = numbered_from_diacritic(v)
                    zy = to_zhuyin(base, tone)
                    if zy:
                        zys.append(zy)
                if zys:
                    readings[ch] = zys
    return big5, readings


def read_pinlu():
    """kHanyuPinlu: character -> {Zhuyin reading: count} from a frequency dictionary of spoken and written Mandarin."""
    counts = {}
    with open(os.path.join(RAW, "Unihan_Readings.txt"), encoding="utf-8") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) != 3 or parts[1] != "kHanyuPinlu":
                continue
            ch = chr(int(parts[0][2:], 16))
            for value in parts[2].split():
                m = re.fullmatch(r"(\S+)\((\d+)\)", value)
                if not m:
                    continue
                base, tone = numbered_from_diacritic(m.group(1))
                zy = to_zhuyin(base, tone)
                if zy:
                    counts.setdefault(ch, collections.Counter())[zy] += int(m.group(2))
    return counts


def reading_shares(char_readings, pinlu, taiwan_of):
    """char -> {reading: share of the character's frequency}, from kHanyuPinlu counts with add-one smoothing.

    `taiwan_of[(char, mainland reading)]` is CC-CEDICT's Taiwan reading for the same character: its count moves there.
    Characters without counts, or whose counts cover none of their readings, share equally."""
    shares = {}
    for ch, readings in char_readings.items():
        if len(readings) < 2:
            continue
        counts = collections.Counter()
        for reading, count in pinlu.get(ch, {}).items():
            counts[taiwan_of.get((ch, reading), reading)] += count
        known = {r: counts[r] for r in readings}
        if sum(known.values()) == 0:
            continue
        floor = SENSE_SHARE * sum(known.values())
        for r in readings:
            if (ch, r) in SENSE:
                known[r] = max(known[r], floor)
        total = sum(known.values()) + len(readings)
        shares[ch] = {r: (known[r] + 1) / total for r in readings}
    return shares


SANDHI_PENALTY = 2.5  # a lone 一 or 不 typed with its sandhi tone, where the next character is another word


def sandhi_readings(word, reading):
    """Readings of `word` with 一 or 不 changed the way they are pronounced before the next syllable of the word."""
    syllables = reading.split()
    changed = list(syllables)
    for i, ch in enumerate(word[:-1]):
        tone = syllables[i + 1][-1] if syllables[i + 1][-1] in "ˊˇˋ˙" else ""
        if ch == "一" and syllables[i] == "ㄧ":
            changed[i] = "ㄧˊ" if tone in ("ˋ", "˙") else "ㄧˋ"
        elif ch == "不" and syllables[i] == "ㄅㄨˋ" and tone in ("ˋ", "˙"):
            changed[i] = "ㄅㄨˊ"
    variant = " ".join(changed)
    return [variant] if variant != reading else []


# MARK: - Build


def main():
    freq = wordfreq.get_frequency_dict("zh")
    cedict = read_cedict()
    big5, unihan = read_unihan()
    pinlu = read_pinlu()

    # A Simplified form can stand for several Traditional ones (后: 後/后, 发: 發/髮). The frequency is shared out by how
    # much each Traditional character is used inside other words, which separates 後 (以後, 然後) from 后 (皇后).
    contribution = collections.Counter()
    for trad, simp, reading, taiwan, variant in cedict:
        if len(trad) > 1 and not variant:
            f = freq.get(simp, 0.0)
            for i, ch in enumerate(trad):
                contribution[(ch, reading.split()[i])] += f

    groups = collections.defaultdict(list)
    for e in cedict:
        trad, simp, reading = e[0], e[1], e[2]
        groups[(simp, reading)].append(e)

    MISSING_CHAR, MISSING_WORD = 2e-9, 3e-9
    best = {}  # (reading, trad) -> logp

    def put(reading, word, logp):
        key = (reading, word)
        if key not in best or best[key] < logp:
            best[key] = logp

    for (simp, reading), es in groups.items():
        base = freq.get(simp, 0.0)
        trads = sorted(set(e[0] for e in es))
        weights = {}
        for t in trads:
            w = 1.0
            for i, ch in enumerate(t):
                w *= contribution[(ch, reading.split()[i])] + 1e-7
            weights[t] = w
        total = sum(weights.values())
        for e in es:
            trad, _, _, taiwan, variant = e
            share = weights[trad] / total if total > 0 else 1.0 / len(trads)
            f = base * share
            if f <= 0:
                f = MISSING_CHAR if len(trad) == 1 else MISSING_WORD
            logp = math.log(f)
            if variant:
                logp -= 4.0
            put(reading, trad, logp)

    # Characters CC-CEDICT does not have: Big5 characters with an Unihan reading, at the floor.
    have = set(w for (_, w) in best)
    for ch in big5:
        if ch in have or ch not in unihan:
            continue
        for i, zy in enumerate(unihan[ch]):
            put(zy, ch, math.log(MISSING_CHAR) - 1.0 - i)

    # A character's frequency is shared among its readings.  CC-CEDICT gives every reading of a character the full
    # frequency of its form; kHanyuPinlu says how often each reading is used.
    char_readings = collections.defaultdict(set)
    for (reading, word) in best:
        if len(word) == 1:
            char_readings[word].add(reading)
    taiwan_of = {}
    by_char = collections.defaultdict(list)
    for trad, simp, reading, taiwan, variant in cedict:
        if len(trad) == 1:
            by_char[trad].append((reading, taiwan, variant))
    for ch, items in by_char.items():
        tw = sorted({r for r, t, v in items if t and not v and (ch, r) not in SENSE})
        main = {r for r, t, v in items if not v and (ch, r) in COMMON}
        if len(tw) == 1:
            for r, t, v in items:
                if not t and r != tw[0] and r not in main:
                    taiwan_of[(ch, r)] = tw[0]
    for ch, share in reading_shares(char_readings, pinlu, taiwan_of).items():
        for reading, value in share.items():
            best[(reading, ch)] += math.log(value)

    # Tone sandhi of 一 and 不 inside words, and at a cost for the lone characters.
    for (reading, word), logp in list(best.items()):
        if len(word) > 1 and ("一" in word or "不" in word):
            for variant in sandhi_readings(word, reading):
                put(variant, word, logp - 0.01)
    for word, plain, tones in [("一", "ㄧ", ["ㄧˊ", "ㄧˋ"]), ("不", "ㄅㄨˋ", ["ㄅㄨˊ"])]:
        if (plain, word) in best:
            for tone in tones:
                put(tone, word, best[(plain, word)] - SANDHI_PENALTY)

    # Traditional-only lexicon: drop entries whose characters are all outside Big5 and Unihan's Traditional set
    # only when they are pure Simplified forms (the same string also appears as some entry's Simplified side only).
    trad_forms = set(e[0] for e in cedict)
    rows = [(r, w, p) for (r, w), p in best.items() if w in trad_forms or len(w) == 1]

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    rows.sort(key=lambda x: (x[0], -x[2], x[1]))
    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# ZhuyinType basic lexicon. Sources: CC-CEDICT (CC BY-SA 4.0), Unicode Unihan (Unicode License v3), "
                "wordfreq (CC BY-SA 4.0). See data/SOURCES.md.\n")
        for r, w, p in rows:
            f.write(f"{r}\t{w}\t{p:.3f}\n")
    print(f"wrote {len(rows)} entries to {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
