"""Keep only the Taiwan-like sentences of the simplified-Chinese corpora (cross-entropy difference selection).

Each sentence of zh-meme, the LCCC eighth and the 知乎 eighth (the same sentences prepare would add, after OpenCC
s2twp) is segmented with the Taiwan model's unigram and scored with two word bigrams: the Taiwan one (ptt/: the
shipped chat corpus plus PTT Gossiping) and one trained on those three corpora alone (cn/).  The score is the
difference of per-token cross-entropies, H_taiwan − H_cn (Moore and Lewis 2010): negative means the Taiwan model
finds the sentence at least as natural as the mainland model does.  Writes data/build/lm_chat2/cn-select/:
kept-0.txt (difference ≤ 0), kept-q20.txt (the lowest-difference fifth), kept-strict.txt (that fifth without any
sentence holding a word 20 times or more likelier in the mainland model than in the Taiwan one, so that no
mainland-specific word gains mass) and select.json (counts, thresholds and
review samples of kept and dropped sentences).  GPU host (24 workers).

    python -m zhuyintype_ml.training.lm_chat2.select --workers 24
"""

from __future__ import annotations

import argparse
import math
import random
from multiprocessing import Pool

import numpy as np

from zhuyintype_ml.training.lm import mkn
from zhuyintype_ml.training.lm.files import save_json

from .prepare import OUT, other_dialogues, valid

TAIWAN: mkn.TsvModel
CN: mkn.TsvModel
TRIE: mkn.Trie = {}
IDS: dict[str, int] = {}
WORDS: list[str] = []
SCORES: np.ndarray = np.zeros(0)


def sentences() -> list[str]:
    """The sentences prepare adds for meme, lccc and zhihu, in a fixed order, without duplicates."""
    seen: set[str] = set()
    out: list[str] = []
    for source in ("meme", "lccc", "zhihu"):
        for turns in other_dialogues(source):
            for turn in turns:
                for sentence in mkn.SPLIT.split(turn):
                    sentence = sentence.strip()
                    if 4 <= len(sentence) <= 200 and valid(sentence) and sentence not in seen:
                        seen.add(sentence)
                        out.append(sentence)
    return out


def entropy(model: mkn.TsvModel, words: list[str]) -> float:
    previous, total = "<s>", 0.0
    for word in [*words, "</s>"]:
        total += model.score(previous, word)
        previous = word
    return -total / (len(words) + 1)


MAINLAND_RATIO = math.log(20)


def score(sentence: str) -> tuple[float, bool]:
    """(cross-entropy difference, whether the sentence holds a mainland-specific word)."""
    words = [WORDS[i] for i in mkn.segment(sentence, TRIE, IDS, SCORES, float(SCORES[IDS["<unk>"]]))]
    mainland = any(
        CN.uni.get(w, CN.uni["<unk>"]) - TAIWAN.uni.get(w, TAIWAN.uni["<unk>"]) >= MAINLAND_RATIO for w in words
    )
    return entropy(TAIWAN, words) - entropy(CN, words), mainland


def main(argv: list[str] | None = None) -> int:
    global TAIWAN, CN, TRIE, IDS, WORDS, SCORES
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--workers", type=int, default=24)
    a = p.parse_args(argv)
    out = OUT / "cn-select"
    out.mkdir(parents=True, exist_ok=True)
    TAIWAN, CN = mkn.TsvModel(OUT / "ptt"), mkn.TsvModel(OUT / "cn")
    _, _, lexicon = mkn.read_lexicon(OUT / "inputs/lexicon.tsv")
    WORDS = list(TAIWAN.uni)
    IDS = {w: i for i, w in enumerate(WORDS)}
    SCORES = np.array([TAIWAN.uni[w] for w in WORDS])
    TRIE = mkn.make_trie(WORDS, lexicon)
    texts = sentences()
    with Pool(a.workers) as pool:
        scored = pool.map(score, texts, chunksize=2000)
    diffs = [d for d, _ in scored]
    flagged = [m for _, m in scored]
    q20 = float(np.quantile(diffs, 0.2))
    kept0 = [t for t, d in zip(texts, diffs) if d <= 0]
    kept20 = [t for t, d in zip(texts, diffs) if d <= q20]
    strict = [t for t, d, m in zip(texts, diffs, flagged) if d <= q20 and not m]
    (out / "kept-0.txt").write_text("".join(t + "\n" for t in kept0), encoding="utf-8")
    (out / "kept-q20.txt").write_text("".join(t + "\n" for t in kept20), encoding="utf-8")
    (out / "kept-strict.txt").write_text("".join(t + "\n" for t in strict), encoding="utf-8")
    rng = random.Random(20261117)
    order = np.argsort(diffs)
    save_json(
        out / "select.json",
        {
            "sentences": len(texts),
            "kept_0": len(kept0),
            "kept_q20": len(kept20),
            "kept_strict": len(strict),
            "with_mainland_word": sum(flagged),
            "q20_threshold": q20,
            "difference_quantiles": {str(q): float(np.quantile(diffs, q)) for q in (0.05, 0.2, 0.5, 0.8, 0.95)},
            "most_taiwan_like": [texts[i] for i in order[:30]],
            "least_taiwan_like": [texts[i] for i in order[-30:]],
            "kept_0_sample": rng.sample(kept0, min(40, len(kept0))),
            "dropped_sample": rng.sample([t for t, d in zip(texts, diffs) if d > 0], 40),
            "mean_difference": float(np.mean(diffs)) if not math.isnan(float(np.mean(diffs))) else None,
        },
    )
    print("sentences", len(texts), "kept<=0", len(kept0), "kept q20", len(kept20), "strict", len(strict), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
