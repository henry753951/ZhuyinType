"""Readings and glyph origins from English Wiktionary's Chinese entries (Wiktextract's extraction on kaikki.org).

    python -m zhuyintype_ml.training.lexicon.wiktionary OUT.tsv

Writes one row per fact, `word<TAB>kind<TAB>value<TAB>tags`:

  mandarin   a Mandarin reading in the lexicon's Zhuyin form; tags such as Taiwan, Mainland-China, variant,
             colloquial, "common misreading" (Wiktionary's own labels)
  hokkien    a Taiwanese Hokkien reading in Tâi-lô; tag Taiwan when Wiktionary lists it for a Taiwanese locale
  phonetic   for a character: the phonetic component of a phono-semantic compound ("semantic 虫 + phonetic 可")
  class      "taiwanese-mandarin" for a word in Wiktionary's Taiwanese Mandarin category; "interjection" for a one-
             or two-character word whose part of speech is an interjection or particle, or
             a sense of which is glossed as one (interjection, particle, filler, onomatopoeia, "huh", "oh" ...)

Only words written entirely in Han characters are kept.  The text is CC BY-SA 4.0 (Wiktionary contributors); the
rows are facts taken from it (readings, components), recorded with the source in resources/SOURCES.md.
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
from pathlib import Path

from zhuyintype_ml.data.lexicon import HAN

from . import sources
from .phonology import bopomofo

TAIWAN_PLACES = {"Taiwan", "Taipei", "Kaohsiung", "Tainan", "Taichung", "Yilan", "Lukang", "Hsinchu", "Magong"}
# Glosses of interjections, modal particles, fillers and onomatopoeia (also used for CC-CEDICT's definitions).
INTERJECTION = re.compile(
    r"interjection|particle|exclamation|onomatop|\bonom\.|filler|grunt|groan|\b(hey|wow|eh|huh|oh|hmm|uh|um|ah)\b",
    re.IGNORECASE,
)
PHONETIC = re.compile(r"\+\s*phonetic\s+(\S)")
KEPT_TAGS = (
    "Taiwan",
    "Mainland-China",
    "standard",
    "variant",
    "colloquial",
    "common",
    "literary",
    "dialectal",
    "nonstandard",
    "toneless-final-syllable-variant",
    "Erhua",
)


def facts(entry: dict) -> list[tuple[str, str, str, str]]:
    word = entry.get("word", "")
    if not word or not HAN.match(word) or entry.get("pos") == "soft-redirect":
        return []
    rows: set[tuple[str, str, str, str]] = set()
    for sound in entry.get("sounds", []):
        tags = set(sound.get("tags", []))
        raw = set(sound.get("raw_tags", []))
        value = sound.get("zh_pron")
        if not value:
            continue
        if "Mandarin" in tags and "Bopomofo" in tags:
            reading = bopomofo(value)
            if reading and len(reading.split()) == len(word):
                labels = sorted(t for t in tags if t in KEPT_TAGS) + sorted(r for r in raw if " " in r)
                rows.add((word, "mandarin", reading, ",".join(labels)))
        elif "Hokkien" in tags and "Tai-lo" in tags:
            taiwan = bool(tags & TAIWAN_PLACES) or "General Taiwanese" in raw or any("Taiwan" in r for r in raw)
            rows.add((word, "hokkien", value, "Taiwan" if taiwan else ""))
    if len(word) <= 2 and (
        entry.get("pos") in ("intj", "particle")
        or any(INTERJECTION.search(g) for sense in entry.get("senses", []) for g in sense.get("glosses", []))
    ):
        rows.add((word, "class", "interjection", ""))
    categories = {c["name"] if isinstance(c, dict) else c for c in entry.get("categories", [])}
    for sense in entry.get("senses", []):
        categories |= {c["name"] if isinstance(c, dict) else c for c in sense.get("categories", [])}
    if "Taiwanese Mandarin" in categories:
        rows.add((word, "class", "taiwanese-mandarin", ""))
    if len(word) == 1 and entry.get("pos") == "character":
        match = PHONETIC.search(entry.get("etymology_text", ""))
        if match and HAN.match(match.group(1)) and match.group(1) != word:
            rows.add((word, "phonetic", match.group(1), ""))
    return sorted(rows)


def extract(source: Path, out: Path) -> int:
    seen: set[tuple[str, str, str, str]] = set()
    with gzip.open(source, "rt", encoding="utf-8") as lines:
        for line in lines:
            seen.update(facts(json.loads(line)))
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "w", encoding="utf-8") as handle:
        handle.write("# Facts from English Wiktionary (CC BY-SA 4.0, Wiktionary contributors) via Wiktextract.\n")
        for row in sorted(seen):
            handle.write("\t".join(row) + "\n")
    return len(seen)


def read(path: Path) -> list[tuple[str, str, str, list[str]]]:
    rows = []
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("#"):
                continue
            word, kind, value, tags = line.rstrip("\n").split("\t")
            rows.append((word, kind, value, [t for t in tags.split(",") if t]))
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("output", type=Path)
    parser.add_argument("--source", type=Path, default=None, help="default: data/raw/lexicon/kaikki-Chinese.jsonl.gz")
    args = parser.parse_args(argv)
    count = extract(args.source or sources.path("kaikki-Chinese.jsonl.gz"), args.output)
    print(f"{count} facts -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
