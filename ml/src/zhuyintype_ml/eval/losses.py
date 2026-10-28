"""Where conversion loses: every wrong row of a row set, with the stage that lost it and a likely cause.

    zt eval losses --set shanjie-dev302 path/to/rows.txt -k 64 --json data/build/eval/losses.json
    zt eval losses --set shanjie --reference other-lexicon.tsv --whole-words names stores
    python -m zhuyintype_ml.eval.losses REPORT.json --lexicon data/build/lexicon.tsv --reference more.tsv --out new.json
    python -m zhuyintype_ml.eval.losses REPORT.json --lexicon data/build/lexicon.tsv \\
        --channel data/build/lexicon_coverage/channel --out new.json    # reading causes by reading-channel class

Each row (`context|truth|syllables`, as `zt eval rows`) is typed after its context and committed with the app setup
(--config app) and with the lexicon and language model alone (engine).  For each the K best (zhuyin-cli
--report-nbest) are kept: with the app setup they are the engine's sentences with the per-character model's scores,
the pool the sentence reranker picks from.  A row is right under 善解's common-character folds (as `zt eval rows`).

Stage, for a wrong row (where the right sentence was lost):
    reranker   the app's first sentence before the reranker was right; the reranker picked another
    v3         the engine alone was right; the per-character model moved another sentence above it
    pool16     the right sentence is in the reranker's 16 but not chosen
    poolK      it is among the K best but below the 16 the reranker sees
    missing    it is not among the K best (engine_rank says whether the engine alone had it)

Cause, from the lexicon (`classify`; the first that applies):
    length     the output has another length than the truth, or the truth has non-Han characters
    sandhi     a wrong 一 or 不 typed with its changed tone
    neutral    a wrong character typed with a neutral tone the lexicon lacks (or without the neutral tone it has)
    tone       a wrong character typed with a tone the lexicon lacks for it, the same syllable otherwise
    reading    a wrong character typed with a syllable the lexicon lacks for it (a missing reading or a typing error)
    word-reading  a word covering the error is in the lexicon, but not with the typed reading
    oov        a word covering the error is in the reference vocabulary (or is the whole row of a word list) but not
               in the lexicon
    phrase     two or more adjacent wrong characters that no lexicon word covers: an unknown word or name
    form       right once interchangeable forms are equal (著／着, 裡／裏 ...)
    char       one wrong character between right ones, every word in the lexicon: a homophone choice
    word       a lexicon word replaced by another reading-equal choice
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from collections.abc import Callable, Iterable
from pathlib import Path
from typing import Any

from zhuyintype_ml.core.engine import EngineConfig, EnginePool
from zhuyintype_ml.core.results import Section, percent
from zhuyintype_ml.core.sets import EvalSet
from zhuyintype_ml.core.zhuyin import TONES, VARIANTS
from zhuyintype_ml.eval.base import Evaluator
from zhuyintype_ml.eval.rows import FORBIDDEN, load_rows, normalizer

STAGES = ("reranker", "v3", "pool16", "poolK", "missing")
CAUSES = (
    "length", "sandhi", "neutral", "tone", "reading", "word-reading", "oov", "phrase", "form", "char", "word"
)  # fmt: skip
SANDHI = {"一": "ㄧ", "不": "ㄅㄨˋ"}
MAX_WORD = 12
# Function characters: a reference "word" made of one of these and a lexicon word (心的, 再進) is a fragment that
# n-gram style dictionaries list, not a missing word.
FUNCTION = set("的地得了著過在再是也就都和與跟及或而之其這那哪個些有沒不要會能把被給讓")


def is_han(ch: str) -> bool:
    return "㐀" <= ch <= "鿿" or "\U00020000" <= ch <= "\U0003134f" or "豈" <= ch <= "﫿"


def toneless(syllable: str) -> str:
    return syllable.strip(TONES)


def row_syllables(text: str) -> list[str]:
    """Syllables of a row file, a leading neutral tone moved to the end (˙ㄇㄚ → ㄇㄚ˙)."""
    return [s[1:] + "˙" if s.startswith("˙") else s for s in text.split()]


class Vocabulary:
    """Words and their readings, and every syllable each character has in any word.

    Files are lexicon TSVs: reading, word, anything else (# comments skipped); syllables are separated by spaces or
    hyphens, a first tone without a mark."""

    def __init__(self, paths: Iterable[Path | str] = ()) -> None:
        self.words: dict[str, set[tuple[str, ...]]] = {}
        self.chars: dict[str, set[str]] = {}
        for path in paths:
            with open(path, encoding="utf-8") as f:
                for line in f:
                    if line.startswith("#") or "\t" not in line:
                        continue
                    reading, word = line.rstrip("\n").split("\t")[:2]
                    self.add(word, reading.replace("-", " ").split())

    def add(self, word: str, syllables: list[str]) -> None:
        if len(syllables) != len(word) or not word:
            return
        self.words.setdefault(word, set()).add(tuple(syllables))
        for ch, syllable in zip(word, syllables):
            self.chars.setdefault(ch, set()).add(syllable)

    def has(self, word: str, syllables: Iterable[str] | None = None) -> bool:
        readings = self.words.get(word)
        return bool(readings) and (syllables is None or tuple(syllables) in readings)  # type: ignore[operator]


def covering(
    truth: str, syllables: list[str], vocab: Vocabulary, spans: list[tuple[int, int]]
) -> list[tuple[int, int]]:
    """Places (start, end) of words of at least two characters in `vocab`, with the typed readings, that overlap one
    of `spans`."""
    found = []
    for a in range(len(truth)):
        for b in range(a + 2, min(len(truth), a + MAX_WORD) + 1):
            if any(a < end and start < b for start, end in spans) and vocab.has(truth[a:b], syllables[a:b]):
                found.append((a, b))
    return found


def fragment(word: str, lexicon: Vocabulary) -> bool:
    """Whether `word` is a function character joined to a lexicon word or character (see FUNCTION)."""
    return (word[0] in FUNCTION and (len(word) == 2 or lexicon.has(word[1:]))) or (
        word[-1] in FUNCTION and (len(word) == 2 or lexicon.has(word[:-1]))
    )


def error_spans(wrong: list[int]) -> list[tuple[int, int]]:
    spans: list[tuple[int, int]] = []
    for i in wrong:
        if spans and spans[-1][1] == i:
            spans[-1] = (spans[-1][0], i + 1)
        else:
            spans.append((i, i + 1))
    return spans


def classify(
    truth: str,
    syllables: list[str],
    prediction: str,
    lexicon: Vocabulary,
    reference: Vocabulary | None = None,
    fold: Callable[[str], str] = str,
    whole_word: bool = False,
) -> dict[str, Any]:
    """The likely cause of a wrong conversion (see the module docstring), with the evidence: wrong positions and the
    words or readings involved."""
    want, got = fold(truth), fold(prediction)
    if len(want) != len(got) or len(truth) != len(syllables) or not all(map(is_han, truth)):
        return {"cause": "length", "wrong": [], "detail": ""}
    wrong = [i for i in range(len(want)) if want[i] != got[i]]
    spans = error_spans(wrong)
    for i in wrong:
        ch, typed = truth[i], syllables[i]
        known = lexicon.chars.get(ch, set())
        if ch in SANDHI and typed != SANDHI[ch] and toneless(typed) == toneless(SANDHI[ch]):
            return {"cause": "sandhi", "wrong": wrong, "detail": f"{ch} {typed}"}
        if typed in known:
            continue
        same = [k for k in known if toneless(k) == toneless(typed)]
        elsewhere = bool(reference and typed in reference.chars.get(ch, set()))
        detail = f"{ch} {typed} (lexicon: {' '.join(sorted(known)) or '—'})" + (" (in reference)" if elsewhere else "")
        if same and (typed.endswith("˙") or any(k.endswith("˙") for k in same)):
            return {"cause": "neutral", "wrong": wrong, "detail": detail}
        return {"cause": "tone" if same else "reading", "wrong": wrong, "detail": detail}
    ours = covering(truth, syllables, lexicon, spans)
    missing: list[str] = []
    if reference:
        places = covering(truth, syllables, reference, spans)
        missing = [
            truth[a:b]
            for a, b in places
            if not lexicon.has(truth[a:b], syllables[a:b]) and not fragment(truth[a:b], lexicon)
        ]
    if whole_word and len(truth) >= 2 and not lexicon.has(truth, syllables):
        missing.append(truth)
    if missing:
        longest = max(missing, key=len)
        cause = "word-reading" if lexicon.has(longest) else "oov"
        return {"cause": cause, "wrong": wrong, "detail": longest, "words": sorted(set(missing), key=len, reverse=True)}
    if prediction.translate(VARIANTS) == truth.translate(VARIANTS):
        return {"cause": "form", "wrong": wrong, "detail": ""}

    def covered(span: tuple[int, int]) -> bool:
        return any(a <= span[0] and span[1] <= b for a, b in ours)

    uncovered = [s for s in spans if s[1] - s[0] >= 2 and not covered(s)]
    if uncovered:
        a, b = uncovered[0]
        return {"cause": "phrase", "wrong": wrong, "detail": truth[a:b]}
    if len(wrong) == 1 and not covered(spans[0]):
        i = wrong[0]
        return {"cause": "char", "wrong": wrong, "detail": f"{truth[i]}→{prediction[i]}"}
    words = sorted({truth[a:b] for a, b in ours}, key=len, reverse=True)
    return {"cause": "word", "wrong": wrong, "detail": " ".join(words[:3])}


def stage(row: dict[str, Any], fold: Callable[[str], str], k: int) -> str:
    gold = fold(row["truth"])
    nbest = [fold(s) for s in row["nbest"]]
    if nbest and nbest[0] == gold:
        return "reranker"
    if fold(row["engine"]) == gold:
        return "v3"
    if gold in nbest[:16]:
        return "pool16"
    if gold in nbest[:k]:
        return "poolK"
    return "missing"


def rank(candidates: list[str], gold: str, fold: Callable[[str], str]) -> int | None:
    return next((i + 1 for i, s in enumerate(candidates) if fold(s) == gold), None)


def analyse(
    rows: list[dict[str, Any]],
    lexicon: Vocabulary,
    reference: Vocabulary | None,
    fold: Callable[[str], str],
    k: int,
    whole_word: bool = False,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Metrics and items for rows that carry prediction, nbest, engine and engine_nbest."""
    items = []
    for row in rows:
        gold = fold(row["truth"])
        item = {
            **row,
            "ok": int(fold(row["prediction"]) == gold),
            "engine_ok": int(fold(row["engine"]) == gold),
            "rank": rank(row["nbest"], gold, fold),
            "engine_rank": rank(row["engine_nbest"], gold, fold),
        }
        if not item["ok"]:
            item["stage"] = stage(row, fold, k)
            item.update(
                classify(
                    row["truth"],
                    row_syllables(row["syllables"]),
                    row["prediction"],
                    lexicon,
                    reference,
                    fold,
                    whole_word,
                )
            )
        items.append(item)
    wrong = [i for i in items if not i["ok"]]
    n = len(items)
    metrics = {
        "n": n,
        "top1": sum(i["ok"] for i in items) / n,
        "engine_top1": sum(i["engine_ok"] for i in items) / n,
        "oracle16": sum(1 for i in items if i["rank"] and i["rank"] <= 16) / n,
        f"oracle{k}": sum(1 for i in items if i["rank"]) / n,
        "wrong": len(wrong),
        "stages": dict(Counter(i["stage"] for i in wrong)),
        "causes": dict(Counter(i["cause"] for i in wrong)),
        "fixed_by_models": sum(1 for i in items if i["ok"] and not i["engine_ok"]),
        "broken_by_models": sum(1 for i in items if not i["ok"] and i["engine_ok"]),
    }
    return metrics, items


class Losses(Evaluator):
    name = "losses"
    help = "every wrong row with the stage that lost it (reranker, v3, pool, missing) and a likely cause"
    kind = "rows"
    default_sets = ["shanjie"]
    default_n = 0
    n_help = "rows per set, from the start (default: 0, every row)"

    def add_arguments(self, parser: argparse.ArgumentParser) -> None:
        parser.add_argument("-k", type=int, default=64, help="engine sentences to look for the truth in (default: 64)")
        parser.add_argument("--shanjie-variants", metavar="FILE", help="善解's variants.tsv for its full folds")
        parser.add_argument(
            "--reference",
            nargs="+",
            default=[],
            metavar="FILE",
            help="more lexicon TSVs: words found there but not in ours count as out of vocabulary",
        )
        parser.add_argument(
            "--whole-words",
            nargs="+",
            default=[],
            metavar="TEXT",
            help="sets whose name contains one of these are word lists: each row's truth is one word",
        )

    def evaluate(self, eval_set: EvalSet, pool: EnginePool, args: argparse.Namespace) -> Section:
        if any(option in pool.config.options for option in FORBIDDEN):
            raise SystemExit(f"zt eval losses: options must not include {', '.join(FORBIDDEN)}")
        rows = load_rows(eval_set.file)
        if args.n > 0:
            rows = rows[: args.n]
        engine = EnginePool(EngineConfig("engine", pool.config.data, pool.config.options), pool.jobs, pool.cli)
        scripts = [row["context"] + "\t" + row["keys"] for row in rows]
        outputs = {
            "prediction": pool.run([s + "<enter>" for s in scripts]),
            "nbest": pool.run(scripts, "--report-nbest", str(args.k)),
            "engine": engine.run([s + "<enter>" for s in scripts]),
            "engine_nbest": engine.run(scripts, "--report-nbest", str(args.k)),
        }
        collected = [
            {
                "row": row["row"],
                "id": row["id"],
                "context": row["context"],
                "truth": row["truth"],
                "syllables": row["syllables"],
                "prediction": outputs["prediction"][i],
                "nbest": outputs["nbest"][i].split("\t") if outputs["nbest"][i] else [],
                "engine": outputs["engine"][i],
                "engine_nbest": outputs["engine_nbest"][i].split("\t") if outputs["engine_nbest"][i] else [],
            }
            for i, row in enumerate(rows)
        ]
        lexicon = Vocabulary([pool.config.reading_lexicon])
        reference = Vocabulary([pool.config.reading_lexicon, *args.reference]) if args.reference else None
        whole = any(text in eval_set.name for text in args.whole_words)
        metrics, items = analyse(collected, lexicon, reference, normalizer(args.shanjie_variants), args.k, whole)
        for item in items:
            item["reference"], item["output"] = item["truth"], item["prediction"]
        examples = [
            (i["truth"], f"{i['prediction']}  [{i['stage']}/{i['cause']}] {i['detail']}") for i in items if not i["ok"]
        ]
        return Section(eval_set.name, metrics, items=items, examples=examples[:8])

    def describe(self, m: dict[str, Any]) -> str:
        stages = ", ".join(f"{s} {m['stages'][s]}" for s in STAGES if s in m["stages"])
        causes = ", ".join(f"{c} {m['causes'][c]}" for c in CAUSES if c in m["causes"])
        return (
            f"{m['n']} rows: top-1 {percent(m['top1'])} (engine {percent(m['engine_top1'])}), "
            f"oracle@16 {percent(m['oracle16'])}; {m['wrong']} wrong\n  stages: {stages}\n  causes: {causes}"
        )


def reclassify(
    report: Path | str,
    lexicon: list[Path | str],
    reference: list[Path | str],
    variants: Path | str | None = None,
    whole_words: Iterable[str] = (),
    channel: Path | str | None = None,
) -> dict[str, Any]:
    """A `zt eval losses` report classified again offline (another lexicon or reference vocabulary), no engine."""
    data = json.loads(Path(report).read_text(encoding="utf-8"))
    table = reading_classes(channel) if channel else None
    ours = Vocabulary(lexicon)
    ref = Vocabulary([*lexicon, *reference]) if reference else None
    k = int(data["options"].get("k", 64))
    for section in data["sections"]:
        whole = any(text in section["name"] for text in whole_words)
        metrics, items = analyse(section["items"], ours, ref, normalizer(variants), k, whole)
        if table is not None:
            for item in items:
                if not item["ok"] and item["cause"] in READING_CAUSES and item.get("wrong"):
                    i = item["wrong"][0]
                    typed = item["syllables"].split()[i] if isinstance(item["syllables"], str) else item["syllables"][i]
                    item["reading_class"] = table.get((item["truth"][i], typed), "none")
            metrics["reading_classes"] = dict(Counter(i["reading_class"] for i in items if "reading_class" in i))
        section["metrics"], section["items"] = metrics, items
    return data


# Causes where the typed reading is not the lexicon's: grouped by the reading channel's class with --channel.
READING_CAUSES = ("neutral", "tone", "reading", "word-reading")


def reading_classes(directory: Path | str) -> dict[tuple[str, str], str]:
    """(character, reading) -> class, from a reading channel's channel.tsv (resources/formats/reading-channel.md)."""
    table: dict[tuple[str, str], str] = {}
    with open(Path(directory) / "channel.tsv", encoding="utf-8") as lines:
        for line in lines:
            if not line.startswith("#"):
                parts = line.rstrip("\n").split("\t")
                table[(parts[0], parts[1])] = parts[2]
    return table


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="python -m zhuyintype_ml.eval.losses", description=reclassify.__doc__)
    parser.add_argument("report")
    parser.add_argument("--lexicon", nargs="+", required=True)
    parser.add_argument("--reference", nargs="+", default=[])
    parser.add_argument("--shanjie-variants")
    parser.add_argument("--whole-words", nargs="+", default=[])
    parser.add_argument("--channel", help="reading channel directory (channel.tsv): group reading causes by class")
    parser.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    data = reclassify(args.report, args.lexicon, args.reference, args.shanjie_variants, args.whole_words, args.channel)
    Path(args.out).write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    for section in data["sections"]:
        print(section["name"], Losses().describe(section["metrics"]))


if __name__ == "__main__":
    main()
