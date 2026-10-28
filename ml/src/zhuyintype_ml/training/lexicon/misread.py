"""misread-dev: development sentences typed the way people in Taiwan actually read them, one variant per row.

    python -m zhuyintype_ml.training.lexicon.misread --base BASE.tsv --channel DIR --wiktionary W.tsv --out DIR \\
        TEXT_DEV_FILES...

The standard dev sets are typed with the lexicon's own readings, so a reading the lexicon lacks can never appear in
them.  This set takes the same dev texts (never the test sets) and types each run with one realistic variant, by
mechanism, so each subset answers "if a typist reads it this way here, is the sentence right at top-1?":

  taiwan        every syllable of the run whose character has a Taiwan reading (channel.py taiwan.tsv, strength >= 0.5)
                is typed with it: a Taiwan typist reads 成熟 ㄕㄡˊ wherever it occurs (rate 1).  With --standard the
                Taiwan readings strong enough to be the standard (期中 ㄑㄧˊ) are the readings every set is typed with
  mainland      with --standard: every syllable whose standard Taiwan reading replaced another is typed with the
                reading it replaced (期中 ㄑㄧ), the typing of someone who learned the other reading
  neutral       one neutral-tone syllable of a word typed with its character's citation reading (陣子 ㄗˇ, 什麼 ㄇㄛˊ)
  colloquial    one word typed with a reading Wiktionary labels variant, colloquial or common misreading (什麼 ㄕㄜˊ)
  misread       one unfamiliar character typed with the most common reading of its phonetic series (有邊讀邊)
  interjection  one interjection typed with its first tone or neutral tone (呃 ㄜ), in the spoken dev texts
  examples      the users' reported cases, fixed rows

Every subset has the reading state the engine gives its variant (resources/formats/reading-channel.md, 三種狀態):
`misread` is a correction, the others are accepted readings, and `examples` mixes both; misread.json records them.

Each run is a Han run of 4–20 characters (as `zt eval conversion` picks them) with one variant (all Taiwan readings
for `taiwan`); a run is used once per subset; at most --per-subset rows per subset, seeded.  The rates above are
conditional: the set measures each variant where it occurs, not how often it occurs.  There is no Hokkien-reading
subset: no public source says which words people type with Hokkien sounds or with which tone (仔 ㄚˇ is only in
`examples`); the app's correction log is the source for that.  `taiwan` and `colloquial` use the same sources as
the lexicon's variants, so they measure whether the variants decode right, not
whether the sources are right; `misread` is likewise only as true as the series model (its known-misreading recall
is in docs/research/log.md).  The 善解 neutral-tone and dictionary-example sets carry independent readings.

Writes `<subset>.rows.txt` (`context|truth|syllables`, as `zt eval rows`) and `misread-dev.rows.txt` (all of them),
with a summary in misread.json.
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path

from zhuyintype_ml.core.zhuyin import HAN_RUN, Lexicon

from . import wiktionary
from .channel import load, mandarin_readings, read_rows, variants
from .merge import Builder
from .phonology import split

SUBSETS = ("taiwan", "mainland", "neutral", "colloquial", "misread", "interjection", "examples")
STATES = {
    "taiwan": "accepted",
    "mainland": "accepted",
    "neutral": "accepted",
    "colloquial": "accepted",
    "misread": "correction",
    "interjection": "accepted",
    "examples": "mixed",
}
EXAMPLES = [
    ("鮮蚵", "ㄒㄧㄢ ㄎㄜ"),
    ("鮮蚵", "ㄒㄧㄢ ㄜˊ"),
    ("蚵仔煎", "ㄎㄜ ㄗˇ ㄐㄧㄢ"),
    ("蚵仔煎", "ㄜˊ ㄚˇ ㄐㄧㄢ"),
    ("蚵仔煎", "ㄜˊ ㄚ˙ ㄐㄧㄢ"),
    ("蚵仔煎", "ㄜˊ ㄗˇ ㄐㄧㄢ"),
    ("我想吃鮮蚵", "ㄨㄛˇ ㄒㄧㄤˇ ㄔ ㄒㄧㄢ ㄎㄜ"),
    ("夜市的蚵仔煎", "ㄧㄝˋ ㄕˋ ㄉㄜ˙ ㄎㄜ ㄗˇ ㄐㄧㄢ"),
    ("夜市的蚵仔煎", "ㄧㄝˋ ㄕˋ ㄉㄜ˙ ㄜˊ ㄚˇ ㄐㄧㄢ"),
    ("欸你在幹嘛", "ㄝˋ ㄋㄧˇ ㄗㄞˋ ㄍㄢˋ ㄇㄚˊ"),
    ("欸", "ㄝˋ"),
    ("好噁心", "ㄏㄠˇ ㄜˇ ㄒㄧㄣ"),
    ("噁", "ㄜˇ"),
    ("呃我想一下", "ㄜ ㄨㄛˇ ㄒㄧㄤˇ ㄧˊ ㄒㄧㄚˋ"),
    ("呃", "ㄜ"),
    ("期中考", "ㄑㄧˊ ㄓㄨㄥ ㄎㄠˇ"),
    ("他的意識", "ㄊㄚ ㄉㄜ˙ ㄧˋ ㄕˋ"),
    ("攜手合作", "ㄒㄧ ㄕㄡˇ ㄏㄜˊ ㄗㄨㄛˋ"),
    ("很成熟", "ㄏㄣˇ ㄔㄥˊ ㄕㄡˊ"),
]


def runs(files: list[Path], lexicon: Lexicon) -> list[tuple[str, list[str], list[int]]]:
    """Every Han run of 4–20 characters with its standard readings and word ends, seeded order."""
    found = []
    for path in files:
        for line in path.read_text(encoding="utf-8").splitlines():
            for match in HAN_RUN.finditer(line):
                cuts: list[int] = []
                readings = lexicon.readings(match.group(), cuts)
                if readings and len(readings) == len(match.group()):
                    found.append((match.group(), readings, cuts))
    random.Random(7).shuffle(found)
    return found


def words(text: str, cuts: list[int]) -> list[tuple[int, int]]:
    starts = [0, *cuts[:-1]]
    return list(zip(starts, cuts))


def build(
    base: Path,
    channel_dir: Path,
    facts_path: Path,
    texts: list[Path],
    spoken: list[Path],
    per: int,
    standard: bool = False,
) -> dict[str, list[tuple[str, str]]]:
    channel = load(channel_dir)
    rows = read_rows(base)
    original = Lexicon(rows=rows)  # the same words, so the same longest-match cuts
    if standard:
        standardizer = Builder(rows, channel, mechanisms=["standard"])
        standardizer.standardize()
        rows = [(r, w, p) for (r, w), p in standardizer.rows.items()]
    lexicon = Lexicon(rows=rows)
    facts = wiktionary.read(facts_path)
    mandarin = mandarin_readings(facts)
    summary = json.loads((channel_dir / "channel.json").read_text(encoding="utf-8"))
    interjections = set(summary.get("interjections", ""))
    out: dict[str, list[tuple[str, str]]] = {s: [] for s in SUBSETS}
    out["examples"] = list(EXAMPLES)

    def series_top(char: str, standard: str) -> str | None:
        shares = channel.share.get(char, {})
        options = [(p, r) for r, p in shares.items() if channel.mechanism[(char, r)] == "series" and r != standard]
        return max(options)[1] if options else None

    for subset, files in (
        ("taiwan", texts),
        ("mainland", texts if standard else []),
        ("neutral", texts),
        ("colloquial", texts),
        ("misread", texts),
        ("interjection", spoken),
    ):
        for text, readings, cuts in runs(files, lexicon):
            if len(out[subset]) >= per:
                break
            typed = list(readings)
            changed = False
            if subset == "taiwan":
                for i, (c, r) in enumerate(zip(text, readings)):
                    pair = channel.taiwan.get((c, r))
                    if pair and pair[1] >= 0.5:
                        typed[i], changed = pair[0], True
            elif subset == "mainland":
                typed = original.readings(text) or readings
                changed = typed != readings
            else:
                for start, end in words(text, cuts):
                    word = text[start:end]
                    syls = readings[start:end]
                    if subset == "neutral" and len(word) > 1:
                        for i, s in enumerate(syls):
                            shares = channel.share.get(word[i], {})
                            toned = [(p, r) for r, p in shares.items() if not r.endswith("˙")]
                            if s.endswith("˙") and toned:
                                typed[start + i], changed = max(toned)[1], True
                                break
                    elif subset == "colloquial" and len(word) > 1:
                        options = [v for _, v in variants(mandarin.get(word, {})) if v != " ".join(syls)]
                        if options:
                            typed[start:end], changed = options[0].split(), True
                    elif subset == "misread":
                        for i, c in enumerate(word):
                            guess = series_top(c, syls[i]) if c not in channel.familiar else None
                            if guess and split(guess)[0] != split(syls[i])[0]:
                                typed[start + i], changed = guess, True
                                break
                    elif subset == "interjection" and len(word) == 1 and word in interjections:
                        bare = split(syls[0])[0]
                        for tone in ("", "˙"):
                            if bare + tone != syls[0]:
                                typed[start], changed = bare + tone, True
                                break
                    if changed:
                        break
            if changed:
                out[subset].append((text, " ".join(typed)))
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("texts", nargs="+", type=Path, help="dev text files (one sentence per line)")
    parser.add_argument("--spoken", nargs="+", type=Path, required=True, help="spoken dev texts (interjections)")
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--channel", type=Path, required=True)
    parser.add_argument("--wiktionary", type=Path, required=True)
    parser.add_argument("--per-subset", type=int, default=200)
    parser.add_argument(
        "--standard", action="store_true", help="type with the Taiwan standard readings (merge.py standard class)"
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    subsets = build(args.base, args.channel, args.wiktionary, args.texts, args.spoken, args.per_subset, args.standard)
    args.out.mkdir(parents=True, exist_ok=True)
    every = []
    for name, rows in subsets.items():
        lines = [f"|{text}|{typed}\n" for text, typed in rows]
        (args.out / f"{name}.rows.txt").write_text("".join(lines), encoding="utf-8")
        every += lines
    (args.out / "misread-dev.rows.txt").write_text("".join(every), encoding="utf-8")
    summary = {
        "rows": {name: len(rows) for name, rows in subsets.items()},
        "states": {name: STATES[name] for name in subsets},
        "standard": args.standard,
    }
    (args.out / "misread.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
