"""Readings as the lexicon writes them, and the two conversions the reading channel needs.

- `bopomofo`: a Wiktionary Bopomofo string ("ㄕㄣˊ ˙ㄇㄜ") in the lexicon's form ("ㄕㄣˊ ㄇㄜ˙").
- `tailo_to_zhuyin`: a Taiwanese Hokkien syllable in Tâi-lô ("á", "tsian", "ô") as the Mandarin syllable a Mandarin
  speaker types for it ("ㄚˇ", "ㄐㄧㄢ", "ㄜˊ"), or None when Mandarin has no such syllable.

The Hokkien mapping is a fixed phonological correspondence, not a list of words: initials and finals map to their
nearest Zhuyin symbols (only those with a direct counterpart: no b, g, ng, j initials, no checked or syllabic-nasal
finals), and tones map by their Middle Chinese category (陰平 1 → first tone, 陽平 5 → second, 上 2 → third,
去 3/7 → fourth), the categories the two languages inherited from the same source.  A syllable after "--" is
neutral.  The result must be a syllable Mandarin has (`SYLLABLES`, from the lexicon).
"""

from __future__ import annotations

import re
import unicodedata

TONE_MARKS = "ˊˇˋ˙"


def split(syllable: str) -> tuple[str, str]:
    """'ㄏㄠˇ' -> ('ㄏㄠ', 'ˇ'); 'ㄇㄚ' -> ('ㄇㄚ', '')."""
    if syllable and syllable[-1] in TONE_MARKS:
        return syllable[:-1], syllable[-1]
    return syllable, ""


def bopomofo(text: str) -> str | None:
    """Wiktionary Bopomofo -> lexicon form, or None when it holds anything but Zhuyin syllables."""
    out = []
    for token in text.split():
        neutral = token.startswith("˙") or token.endswith("˙")
        token = token.strip("˙")
        if not token or not re.fullmatch(r"[ㄅ-ㄩ]+[ˊˇˋ]?", token):
            return None
        out.append(token + "˙" if neutral and token[-1] not in "ˊˇˋ" else token)
    return " ".join(out) if out else None


# MARK: - Tâi-lô

INITIALS = [
    ("tsh", "c"),
    ("ts", "z"),
    ("ph", "ㄆ"),
    ("th", "ㄊ"),
    ("kh", "ㄎ"),
    ("p", "ㄅ"),
    ("m", "ㄇ"),
    ("t", "ㄉ"),
    ("n", "ㄋ"),
    ("l", "ㄌ"),
    ("k", "ㄍ"),
    ("h", "ㄏ"),
    ("s", "s"),
]
UNMAPPED_INITIALS = ("ng", "b", "g", "j")
FINALS = {
    "a": "ㄚ",
    "e": "ㄝ",
    "o": "ㄜ",
    "oo": "ㄛ",
    "i": "ㄧ",
    "u": "ㄨ",
    "ai": "ㄞ",
    "au": "ㄠ",
    "ia": "ㄧㄚ",
    "iau": "ㄧㄠ",
    "io": "ㄧㄛ",
    "iu": "ㄧㄡ",
    "ua": "ㄨㄚ",
    "uai": "ㄨㄞ",
    "ue": "ㄨㄝ",
    "ui": "ㄨㄟ",
    "an": "ㄢ",
    "ang": "ㄤ",
    "in": "ㄧㄣ",
    "ing": "ㄧㄥ",
    "ian": "ㄧㄢ",
    "iang": "ㄧㄤ",
    "un": "ㄨㄣ",
    "uan": "ㄨㄢ",
    "ong": "ㄨㄥ",
    "iong": "ㄩㄥ",
    "am": "ㄢ",
    "im": "ㄧㄣ",
    "iam": "ㄧㄢ",
}
# Tâi-lô tone diacritics (combining marks after NFD) -> tone number.
DIACRITIC_TONES = {"́": 2, "̀": 3, "̂": 5, "̄": 7, "̍": 8, "̋": 9}
# Middle Chinese tone category -> Mandarin tone mark (陰平, 上, 陰去, 陽平, 陽去); entering tones have no counterpart.
CATEGORY = {1: "", 2: "ˇ", 3: "ˋ", 5: "ˊ", 7: "ˋ"}


def tailo_syllables(text: str) -> list[tuple[str, bool]]:
    """'ô-á-tsian' -> [('ô', False), ('á', False), ('tsian', False)]; 'o--á' -> [('o', False), ('á', True)]
    (True: neutral, after a double hyphen).  Words are split on spaces and hyphens."""
    out: list[tuple[str, bool]] = []
    for word in text.lower().split():
        neutral = False
        for part in re.split(r"(--|-)", word):
            if part == "--":
                neutral = True
            elif part and part != "-":
                out.append((part, neutral))
    return out


def tailo_to_zhuyin(syllable: str, neutral: bool, syllables: set[str]) -> str | None:
    decomposed = unicodedata.normalize("NFD", syllable)
    tone = 0
    letters = []
    for ch in decomposed:
        if ch in DIACRITIC_TONES:
            tone = DIACRITIC_TONES[ch]
        elif ch.isalpha() and ch.isascii():
            letters.append(ch)
        else:
            return None
    base = "".join(letters)
    if base.endswith("nn"):
        base = base[:-2]  # nasalised vowels: the oral vowel is the nearest Mandarin sound
    if not base or base[-1] in "ptkh" and base not in ("h",):
        return None  # checked syllables (entering tone) have no Mandarin counterpart
    if tone == 0:
        tone = 1
    if tone not in CATEGORY and not neutral:
        return None
    if base.startswith(UNMAPPED_INITIALS) or base in ("m", "ng"):
        return None
    initial, rest = "", base
    for latin, zhuyin in INITIALS:
        if base.startswith(latin):
            initial, rest = zhuyin, base[len(latin) :]
            break
    if rest not in FINALS:
        return None
    final = FINALS[rest]
    palatal = final.startswith("ㄧ") or final.startswith("ㄩ")
    if initial in ("z", "c", "s"):
        initial = {"z": "ㄐ", "c": "ㄑ", "s": "ㄒ"}[initial] if palatal else {"z": "ㄗ", "c": "ㄘ", "s": "ㄙ"}[initial]
    elif palatal and initial in ("ㄍ", "ㄎ", "ㄏ"):
        return None  # Mandarin has no velar before i
    mark = "˙" if neutral else CATEGORY[tone]
    result = initial + final + mark
    return result if split(result)[0] in syllables else None
