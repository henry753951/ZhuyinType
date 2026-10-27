"""Mined words as lexicon packs: a reading and a frequency for each, grouped by the corpus they came from.

    python -m zhuyintype_ml.training.lexicon.vocabulary --mined DIR --lexicon BASE.tsv --corpus wiki chat ... --out DIR
        [--polyphone data/build/v3_030]

Reading: the v3 labeller (training/v3/readings.py): the lexicon's longest match gives each known part its reading,
and a polyphonic single character is decided by the v3 polyphone classifier when it is given (p >= 0.9), otherwise
it keeps its most likely reading.  Words with a character the lexicon cannot read are left out.

Frequency: log(count / Han characters in the corpus) plus a per-corpus offset, the median over the base lexicon's
words of length 2–6 found in the same corpus of (their lexicon log P − their log relative count).  So a mined word
gets the probability a dictionary word with the same count has.  A word found in several corpora goes to the pack
where that estimate is highest.

With --check DATA, each word is typed alone with that data directory's lexicon and language model (zhuyin-cli,
no neural models), and a word the engine already writes is dropped: it would only add a competitor.

Writes `<corpus>.tsv` packs (reading, word, log P) and vocabulary.json (offsets and counts).
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path
from typing import Any

from zhuyintype_ml.training.v3.readings import Lexicon

from .mine import read_ngrams

TONE = {1: "", 2: "ˊ", 3: "ˇ", 4: "ˋ", 5: "˙"}


def read_mined(path: Path) -> tuple[int, dict[str, int]]:
    total, words = 0, {}
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            if line.startswith("# total"):
                total = int(line.split("\t")[1])
                continue
            word, count, *_ = line.rstrip("\n").split("\t")
            words[word] = int(count)
    return total, words


def offset(ngrams: Path, best: dict[str, float]) -> tuple[float, int]:
    """Median of (lexicon log P - log relative count) over the lexicon's words found in the corpus."""
    total, counts, _ = read_ngrams(ngrams)
    gaps = [best[w] - math.log(c / total) for w, c in counts.items() if 2 <= len(w) <= 6 and w in best]
    return (statistics.median(gaps), len(gaps)) if gaps else (0.0, 0)


def labeller(lexicon: Path, polyphone: Path | None) -> Any:
    lex = Lexicon(lexicon)
    if polyphone is None:
        return lambda word: lex.label(word)
    classifier, labels = _load(polyphone)
    chars = {c: i for i, c in enumerate(json.loads((polyphone / "chars.json").read_text(encoding="utf-8")))}
    return lambda word: lex.label(word, classifier, chars, labels)


def _load(directory: Path) -> tuple[Any, list[Any]]:
    import torch

    from zhuyintype_ml.training.v3.polyphone import Polyphone

    checkpoint = torch.load(directory / "polyphone.pt", map_location="cpu", weights_only=False)
    classifier = Polyphone(checkpoint["vocab"], len(checkpoint["labels"]))
    classifier.load_state_dict(checkpoint["model"])
    classifier.eval()
    return classifier, checkpoint["labels"]


def build(
    mined: Path,
    lexicon: Path,
    corpora: list[str],
    out: Path,
    polyphone: Path | None,
    check: Path | None = None,
    jobs: int | None = None,
) -> dict[str, Any]:
    best: dict[str, float] = {}
    with open(lexicon, encoding="utf-8") as lines:
        for line in lines:
            if not line.startswith("#"):
                _, word, value = line.rstrip("\n").split("\t")
                best[word] = max(best.get(word, -math.inf), float(value))
    label = labeller(lexicon, polyphone)
    chosen: dict[str, tuple[float, str]] = {}
    summary: dict[str, Any] = {"corpora": {}}
    for corpus in corpora:
        total, words = read_mined(mined / f"{corpus}.mined.tsv")
        shift, reference = offset(mined / f"{corpus}.ngrams.tsv", best)
        summary["corpora"][corpus] = {"mined": len(words), "offset": shift, "reference_words": reference}
        for word, count in words.items():
            logp = math.log(count / total) + shift
            if word not in chosen or chosen[word][0] < logp:
                chosen[word] = (logp, corpus)
    packs: dict[str, list[tuple[str, str, float]]] = {c: [] for c in corpora}
    unreadable = uncertain = 0
    for word, (logp, corpus) in chosen.items():
        readings, low = label(word)
        if any(tone == 0 for _, tone in readings):
            unreadable += 1
            continue
        uncertain += low
        reading = " ".join(base + TONE[tone] for base, tone in readings)
        packs[corpus].append((reading, word, logp))
    if check is not None:
        # Keep only the words the current engine (lexicon and language model) gets wrong when typed alone: a word
        # it already writes adds nothing but a competitor.
        from zhuyintype_ml.core.engine import EngineConfig, EnginePool
        from zhuyintype_ml.core.zhuyin import keys

        pool = EnginePool(EngineConfig("engine", check), jobs=jobs)
        every = [(corpus, row) for corpus, rows in packs.items() for row in rows]
        outputs = pool.run([keys(row[0].split()) + "<enter>" for _, row in every])
        known_right = {row[1] for (_, row), output in zip(every, outputs) if output.strip() == row[1]}
        summary["engine_already_right"] = len(known_right)
        packs = {c: [row for row in rows if row[1] not in known_right] for c, rows in packs.items()}
    out.mkdir(parents=True, exist_ok=True)
    for corpus, rows in packs.items():
        rows.sort(key=lambda x: (x[0], -x[2], x[1]))
        with open(out / f"{corpus}.tsv", "w", encoding="utf-8") as handle:
            handle.write(f"# ZhuyinType mined vocabulary pack: {corpus}. See resources/SOURCES.md.\n")
            for reading, word, logp in rows:
                handle.write(f"{reading}\t{word}\t{logp:.3f}\n")
        summary["corpora"][corpus]["pack_words"] = len(rows)
    summary.update({"unreadable": unreadable, "polyphone_uncertain": uncertain, "classifier": polyphone is not None})
    (out / "vocabulary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mined", type=Path, required=True)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--corpus", nargs="+", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--polyphone", type=Path, help="directory with polyphone.pt and chars.json (v3_030)")
    parser.add_argument("--check", type=Path, help="data directory: drop words its engine already converts")
    parser.add_argument("--jobs", type=int, default=16)
    args = parser.parse_args(argv)
    summary = build(args.mined, args.lexicon, args.corpus, args.out, args.polyphone, args.check, args.jobs)
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
