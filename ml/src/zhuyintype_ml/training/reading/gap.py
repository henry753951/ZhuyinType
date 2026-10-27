"""The same sentences typed two ways: misread-dev's variant readings and the lexicon's standard readings.

    python -m zhuyintype_ml.training.reading.gap --lexicon data/build/lexcov-s14/lexicon.tsv \\
        --misread data/build/lexicon_coverage/misread --out data/build/v3_reading/gap

For each misread-dev subset (`<subset>.rows.txt`, `context|truth|syllables`), writes `<subset>.variant.rows.txt`
(the rows as they are) and `<subset>.standard.rows.txt` (the same truth typed with the lexicon's most likely readings,
longest match), keeping only rows the lexicon can read and whose two typings differ.  Line i of both files is the same
sentence, so `zt eval stats STANDARD.json VARIANT.json` pairs them: the difference is the gap in first-choice accuracy
between typing the standard and the variant readings.  Writes gap.json with the row counts.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from zhuyintype_ml.core.zhuyin import Lexicon

SUBSETS = ("taiwan", "mainland", "neutral", "colloquial", "misread", "interjection", "examples")


def pairs(rows: list[str], lexicon: Lexicon) -> list[tuple[str, str]]:
    """(variant row, standard row) for every row with a standard typing that differs from the variant."""
    found = []
    for row in rows:
        context, truth, typed = row.split("|")
        standard = lexicon.readings(truth)
        if standard and " ".join(standard) != typed:
            found.append((row, f"{context}|{truth}|{' '.join(standard)}"))
    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lexicon", type=Path, required=True, help="the lexicon whose readings are the standard")
    parser.add_argument("--misread", type=Path, required=True, help="misread-dev's directory")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    lexicon = Lexicon(args.lexicon)
    args.out.mkdir(parents=True, exist_ok=True)
    summary = {}
    for subset in SUBSETS:
        source = args.misread / f"{subset}.rows.txt"
        if not source.exists():
            continue
        found = pairs([r for r in source.read_text(encoding="utf-8").splitlines() if r], lexicon)
        (args.out / f"{subset}.variant.rows.txt").write_text("".join(v + "\n" for v, _ in found), encoding="utf-8")
        (args.out / f"{subset}.standard.rows.txt").write_text("".join(s + "\n" for _, s in found), encoding="utf-8")
        summary[subset] = len(found)
    (args.out / "gap.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
