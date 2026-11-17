"""Screened colloquial packs: fragments, mainland usage and wrong readings out, vulgar words kept but demoted.

    python -m zhuyintype_ml.training.lexicon.screen --packs DIR --corpus ptt generated --labels REVIEW_DIR \\
        --tocp data/raw/lm_chat2/ptt/TOCP_v1.0.zip --out DIR [--vulgar-penalty 4.6]

Mining from PTT finds real words (魯蛇, 慣老闆, 限動) next to pieces of frequent phrases (叫他, 後再, 讓她) and
vulgar words.  A judgement per candidate (REVIEW_DIR/labels-*.tsv: word, label, reading; labels word, fragment,
vulgar, mainland; reading ok or wrong; written by Claude Haiku 5.5, which only judges: every word comes from the
corpus) decides:

  word      kept
  fragment  left out: its parts are words already and the bigram counts carry the phrase
  mainland  left out
  vulgar    kept in the lexicon with --vulgar-penalty nats less, but not in the chat model's vocabulary, so it can
            be typed and never outranks a common word on its own counts; a word holding any expression TOCP v1.0
            marks as profane (two characters or more) counts as vulgar whatever its label
  reading   wrong: left out (the v3 labeller's reading is not the Taiwan one)

A candidate without a label is left out.  Writes OUT/<corpus>.tsv (lexicon rows), OUT/<corpus>.seed.tsv (the rows
whose words may join the chat model's vocabulary) and OUT/screen.json.  Standard library only.
"""

from __future__ import annotations

import argparse
import collections
import json
import zipfile
from pathlib import Path

LABELS = ("word", "fragment", "vulgar", "mainland")


def profane_expressions(tocp: Path) -> set[str]:
    with zipfile.ZipFile(tocp) as archive:
        name = next(n for n in archive.namelist() if n.lower().endswith(".json"))
        rows = json.loads(archive.read(name).decode("utf-8-sig"))
    found: set[str] = set()
    for row in rows if isinstance(rows, list) else rows.values():
        for span in row.get("profane_expression") or []:
            text = (span.get("orginal_expression") or span.get("original_expression") or "").strip()
            if len(text) >= 2:
                found.add(text)
    return found


def read_labels(folder: Path) -> dict[str, tuple[str, str]]:
    labels: dict[str, tuple[str, str]] = {}
    for path in sorted(folder.glob("labels-*.tsv")):
        for line in path.read_text(encoding="utf-8").splitlines():
            parts = line.split("\t")
            if len(parts) >= 3 and parts[1].strip() in LABELS:
                labels[parts[0].strip()] = (parts[1].strip(), parts[2].strip())
    return labels


def screen(
    packs: Path, corpora: list[str], labels: dict[str, tuple[str, str]], profane: set[str], out: Path, penalty: float
) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    summary: dict = {"vulgar_penalty": penalty, "tocp_expressions": len(profane), "corpora": {}}
    for corpus in corpora:
        counts: collections.Counter[str] = collections.Counter()
        lexicon_rows: list[str] = []
        seed_rows: list[str] = []
        for line in (packs / f"{corpus}.tsv").read_text(encoding="utf-8").splitlines():
            if line.startswith("#"):
                continue
            reading, word, logp = line.split("\t")
            label, reading_ok = labels.get(word, ("unlabelled", "ok"))
            if any(expression in word for expression in profane):
                label = "vulgar"
            if label in ("fragment", "mainland", "unlabelled") or reading_ok == "wrong":
                counts["left_out_" + (label if reading_ok != "wrong" else "wrong_reading")] += 1
                continue
            if label == "vulgar":
                counts["vulgar_demoted"] += 1
                lexicon_rows.append(f"{reading}\t{word}\t{float(logp) - penalty:.3f}\n")
                continue
            counts["kept"] += 1
            lexicon_rows.append(line + "\n")
            seed_rows.append(line + "\n")
        header = f"# ZhuyinType screened colloquial pack: {corpus}. See resources/SOURCES.md.\n"
        (out / f"{corpus}.tsv").write_text(header + "".join(lexicon_rows), encoding="utf-8")
        (out / f"{corpus}.seed.tsv").write_text(header + "".join(seed_rows), encoding="utf-8")
        summary["corpora"][corpus] = dict(counts)
    (out / "screen.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--packs", type=Path, required=True)
    parser.add_argument("--corpus", nargs="+", required=True)
    parser.add_argument("--labels", type=Path, required=True)
    parser.add_argument("--tocp", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--vulgar-penalty", type=float, default=4.6)
    args = parser.parse_args(argv)
    summary = screen(
        args.packs,
        args.corpus,
        read_labels(args.labels),
        profane_expressions(args.tocp),
        args.out,
        args.vulgar_penalty,
    )
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
