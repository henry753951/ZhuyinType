"""A lexicon with mined packs added: every pack row whose word the lexicon lacks, the lexicon itself unchanged.

    python -m zhuyintype_ml.training.lexicon.extend --lexicon data/build/lexicon.tsv \\
        --pack data/build/lexicon_coverage/packs-colloquial/ptt.tsv --out data/build/lexicon_coverage/words/ptt.tsv

For comparing a shipped data directory with the same plus new words (zhuyin-cli --lexicon OUT); `merge` builds the
full data-side lexicon instead.  A word already in the lexicon keeps only its rows there.  The engine opens the
Wikipedia model from the lexicon's own directory (`lm/` next to it), so OUT's directory gets an `lm` link to the
lexicon's `lm/`; without it the engine runs with the chat model alone.  Standard library only.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def extend(lexicon: Path, packs: list[Path], out: Path) -> dict[str, object]:
    rows = lexicon.read_text(encoding="utf-8").splitlines(keepends=True)
    known = {line.split("\t")[1] for line in rows if not line.startswith("#") and "\t" in line}
    added: dict[str, int] = {}
    for pack in packs:
        count = 0
        for line in pack.read_text(encoding="utf-8").splitlines(keepends=True):
            if line.startswith("#") or "\t" not in line:
                continue
            word = line.split("\t")[1]
            if word not in known:
                rows.append(line if line.endswith("\n") else line + "\n")
                known.add(word)
                count += 1
        added[pack.stem] = count
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("".join(rows), encoding="utf-8")
    model, link = lexicon.resolve().parent / "lm", out.parent / "lm"
    if model.is_dir() and not (link.exists() or link.is_symlink()):
        link.symlink_to(model)
    summary: dict[str, object] = {"lexicon_rows": sum(1 for r in rows if not r.startswith("#")), "added": added}
    out.with_suffix(".json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--pack", type=Path, action="append", default=[])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    print(json.dumps(extend(args.lexicon, args.pack, args.out), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
