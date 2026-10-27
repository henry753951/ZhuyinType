"""Statistical base A1: a McBopomofo-augmented lexicon and posterior-count word bigrams (research; not adopted).

    lexicon   lexicon_source.tsv: every row of the frozen seed lexicon unchanged, plus McBopomofo's Han entries with
              valid syllables, calibrated to the seed scale (new readings of a seed word stay 1 nat below its best)
    prepare   corpus/: Wikipedia, real chat (chat2 without generated text, plus Taiwan Tongues train) and the 0.3.0
              generated corpus, all rescanned against every current dev/test file (exact and 8-character overlap,
              also after s2twp and 臺→台); refuses to run twice
    fit       models/NAME: hard or expected (forward-backward) counts with the incumbent final tokenizer, or with
              --fresh three E/M passes from the lexicon prior; integer MKN after one rounding; refuses an existing
              model directory

Waits for 32 GiB of free host RAM before fitting.  GPU host only (refuses to run elsewhere).

    zt --remote train base
    python -m nlp.training.base.train fit --name chat_fresh_expected --domain chat --method expected --fresh \
        --lexicon data/build/base/lexicon_source.tsv --workers 8
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import multiprocessing as mp
import platform
import re
import shutil
import time
from collections import Counter, defaultdict
from collections.abc import Iterable, Iterator, Mapping
from pathlib import Path
from typing import Any

import numpy as np
from opencc import OpenCC

from nlp.common import paths
from nlp.common.sets import BY_NAME
from nlp.training.lm import mkn
from nlp.training.lm.files import digest, save_json
from nlp.training.lm_chat.train import normalized, read_tt

OUT = paths.data("base")
# (end position, token id, log score) edges leaving each position.
Lattice = list[list[tuple[int, int, float]]]


def remote_only() -> None:
    if platform.system() != "Linux":
        raise SystemExit("Run training and evaluation on the GPU host only.")


def mcbopomofo(path: Path) -> tuple[dict[tuple[str, str], float], Counter[str]]:
    """(reading, word) → natural-log score of McBopomofo's Han rows with one valid syllable per character."""
    rows: dict[tuple[str, str], float] = {}
    rejected: Counter[str] = Counter()
    for line in path.read_text().splitlines():
        if line.startswith("#"):
            continue
        if line.startswith("_"):
            rejected["control_or_punctuation"] += 1
            continue
        fields = line.split()
        if len(fields) != 3:
            raise ValueError(f"malformed McBopomofo row: {line}")
        reading, word, score = fields
        if not re.fullmatch(mkn.HAN.pattern + "+", word):
            rejected["non_han"] += 1
            continue
        reading = reading.replace("-", " ")
        if len(reading.split()) != len(word) or not re.fullmatch(r"[\u3105-\u3129ˊˇˋ˙ ]+", reading):
            rejected["invalid_reading"] += 1
            continue
        p = float(score) * math.log(10)
        if not math.isfinite(p):
            raise ValueError("nonfinite source score")
        rows[reading, word] = max(rows.get((reading, word), -math.inf), p)
    return rows, rejected


def merge(
    seed: list[tuple[str, str, float]], extra: dict[tuple[str, str], float]
) -> tuple[dict[tuple[str, str], float], dict[str, Any]]:
    """Seed rows unchanged plus the source rows, calibrated by the median offset of shared words of each length."""
    best = {(r, w): p for r, w, p in seed}
    tops: defaultdict[str, float] = defaultdict(lambda: -math.inf)
    source_tops: defaultdict[str, float] = defaultdict(lambda: -math.inf)
    for (_, w), p in best.items():
        tops[w] = max(tops[w], p)
    for (_, w), p in extra.items():
        source_tops[w] = max(source_tops[w], p)
    offsets: defaultdict[int, list[float]] = defaultdict(list)
    for w in tops.keys() & source_tops.keys():
        offsets[min(len(w), 5)].append(tops[w] - source_tops[w])
    global_offset = float(np.median([x for v in offsets.values() for x in v]))
    calibration = {n: float(np.median(v)) for n, v in offsets.items()}
    added: Counter[str] = Counter()
    for (r, w), p in sorted(extra.items()):
        if (r, w) in best:
            continue
        if w in tops:
            # New readings cannot change any existing reading's relative prior.
            best[r, w] = tops[w] + min(-1.0, p - source_tops[w])
            added["new_readings_existing_words"] += 1
        else:
            best[r, w] = p + calibration.get(min(len(w), 5), global_offset)
            added["new_word_rows"] += 1
    assert all(best[r, w] == p for r, w, p in seed)
    return best, dict(added, offsets_by_length=calibration, shared_words=sum(map(len, offsets.values())))


def lexicon(a: argparse.Namespace) -> None:
    _, seed, _ = mkn.read_lexicon(OUT / "inputs/lexicon.tsv")
    extra, rejected = mcbopomofo(a.source)
    best, report = merge(seed, extra)
    target = OUT / "lexicon_source.tsv"
    with target.open("w") as f:
        f.write("# ZhuyinType + McBopomofo; CC BY-SA 4.0 and retained MIT/BSD notices. See base/NOTICE.md.\n")
        for (r, w), p in sorted(best.items(), key=lambda x: (x[0][0], -x[1], x[0][1])):
            f.write(f"{r}\t{w}\t{p:.12f}\n")
    save_json(
        OUT / "lexicon-source-merge.json",
        dict(
            report,
            source_rows=len(extra),
            rejected=dict(rejected),
            seed_rows=len(seed),
            merged_rows=len(best),
            source_sha256=digest(a.source),
            seed_sha256=digest(OUT / "inputs/lexicon.tsv"),
            output_sha256=digest(target),
        ),
    )
    print(json.dumps(report), flush=True)


def logadd(a: float, b: float) -> float:
    if a == -math.inf:
        return b
    if b == -math.inf:
        return a
    m = max(a, b)
    return m + math.log1p(math.exp(-abs(a - b)))


def lattice(text: str, trie: mkn.Trie, ids: dict[str, int], scores: Any) -> Lattice:
    """Every lexicon word and single character as an edge, scored by the unigram `scores`."""
    edges: Lattice = [[] for _ in text]
    for i, ch in enumerate(text):
        token = ids.get(ch, 2)
        edges[i].append((i + 1, token, float(scores[token])))
        node: mkn.Trie | None = trie
        for j in range(i, len(text)):
            assert node is not None
            node = node.get(text[j])
            if node is None:
                break
            word: int | None = node.get(None)
            if word is not None and j > i:
                edges[i].append((j + 1, word, float(scores[word])))
    return edges


def expected(
    text: str, trie: mkn.Trie, ids: dict[str, int], scores: Any
) -> tuple[defaultdict[int, float], defaultdict[int, float]]:
    """Exact unigram-lattice posterior; include BOS/EOS and adjacent-edge joints."""
    edges = lattice(text, trie, ids, scores)
    n = len(text)
    forward = [-math.inf] * (n + 1)
    forward[0] = 0.0
    for i, group in enumerate(edges):
        for j, _, p in group:
            forward[j] = logadd(forward[j], forward[i] + p)
    backward = [-math.inf] * (n + 1)
    backward[n] = 0.0
    for i in range(n - 1, -1, -1):
        for j, _, p in edges[i]:
            backward[i] = logadd(backward[i], p + backward[j])
    z = forward[n]
    uni: defaultdict[int, float] = defaultdict(float)
    pairs: defaultdict[int, float] = defaultdict(float)
    if not n:
        pairs[1] = 1.0
        return uni, pairs
    for i, group in enumerate(edges):
        for j, token, p in group:
            probability = math.exp(forward[i] + p + backward[j] - z)
            uni[token] += probability
            if i == 0:
                pairs[token] += probability
            if j == n:
                pairs[(token << 32) | 1] += probability
            else:
                for k, next_token, q in edges[j]:
                    pairs[(token << 32) | next_token] += math.exp(forward[i] + p + q + backward[k] - z)
    return uni, pairs


def heldouts() -> tuple[list[Path], set[str], Any]:
    """Every current dev/test file, its texts (with s2twp and 臺→台 forms), and their 8-character automaton."""
    files = [paths.data("lm", "test_sentences.txt"), BY_NAME["smoke"].file]
    for directory, patterns in [
        ("corpus", ("dev*.txt", "test*.txt")),
        ("lm_chat", ("*_chat_tt.txt",)),
        ("lm_chat2", ("*_dialogue.txt",)),
        ("homophone", ("dev_*", "test_*", "smoke.txt")),
    ]:
        for pattern in patterns:
            files.extend(paths.data(directory).glob(pattern))
    files.extend((paths.ROOT / "nlp/eval/sets").rglob("*.txt"))
    files.extend((paths.ROOT / "nlp/eval/sets").rglob("*.jsonl"))
    files = sorted(set(p for p in files if p.is_file() and p.suffix in (".txt", ".jsonl")))
    texts: set[str] = set()

    def strings(v: Any) -> Iterator[str]:
        if isinstance(v, str):
            yield v
        elif isinstance(v, dict):
            for x in v.values():
                yield from strings(x)
        elif isinstance(v, list):
            for x in v:
                yield from strings(x)

    for path in files:
        for line in path.read_text().splitlines():
            texts.update(strings(json.loads(line)) if path.suffix == ".jsonl" else line.split("\t"))
    converter = OpenCC("s2twp")
    texts.update(converter.convert(s).replace("臺", "台") for s in list(texts))
    texts.update(s.replace("臺", "台") for s in list(texts))
    return files, texts, mkn.automaton(texts)


# The held-out texts and automaton, shared with the forked verification workers.
VERIFY_HELD: set[str] = set()
VERIFY_MACHINE: Any = None


def verify_shard(path: Path) -> tuple[str, int, str]:
    count = 0
    for line in path.open():
        text = line.rstrip("\n")
        for value in (text, text.replace("臺", "台")):
            assert value not in VERIFY_HELD and next(VERIFY_MACHINE.iter(value), None) is None, path
        count += 1
    return path.name, count, digest(path)


def verify_corpus(a: argparse.Namespace) -> None:
    """Rescans the prepared corpus against the holdouts once (corpus/validation.json)."""
    global VERIFY_HELD, VERIFY_MACHINE
    directory = OUT / "corpus"
    if (directory / "validation.json").exists():
        return
    held_files, held, machine = heldouts()
    VERIFY_HELD, VERIFY_MACHINE = held, machine
    assert json.loads((directory / "holdouts.json").read_text()) == {str(p): digest(p) for p in held_files}
    counts: Counter[str] = Counter()
    files = {}
    with mp.get_context("fork").Pool(a.workers) as pool:
        for name, count, sha in pool.imap_unordered(verify_shard, sorted(directory.glob("*.txt"))):
            counts[name.split("-")[0]] += count
            files[name] = sha
    prepared = json.loads((directory / "prepared.json").read_text())
    assert all(counts[name] == prepared[name + "_kept"] for name in ("wiki", "real", "generated"))
    assert prepared["files"] == files
    save_json(
        directory / "validation.json",
        {
            "rescanned": dict(counts),
            "exact_or_eight_character_overlap": 0,
            "prepared_sha256": digest(directory / "prepared.json"),
        },
    )
    print("CORPUS VALIDATED", dict(counts), flush=True)


def prepare(a: argparse.Namespace) -> None:
    """corpus/: wiki-, real- and generated-NN.txt shards, holdouts.json and prepared.json (once)."""
    directory = OUT / "corpus"
    if directory.exists():
        raise ValueError("corpus already exists; use frozen prepared data")
    directory.mkdir()
    held_files, held, machine = heldouts()
    save_json(directory / "holdouts.json", {str(p): digest(p) for p in held_files})
    # Source files remain read-only; freshly rescan all of them against every current holdout.
    wiki = sorted(paths.data("lm", "work", "sentences").glob("*.txt"))
    real = sorted(paths.data("lm_chat2", "no_generated", "work", "sentences").glob("*.txt"))
    # The 0.3.0 v3 colloquial corpus (data/build/v3 when this ran).
    generated = [paths.data("v3_030", "colloquial.txt")]
    tt_train = paths.data("lm_chat", "inputs", "train.tsv")
    if not wiki or not real:
        raise ValueError("missing existing Wikipedia/chat2 training shards")
    counts: Counter[str] = Counter()
    chars: set[str] = set()
    seen: set[bytes] = set()

    def blocked(s: str) -> bool:
        return any(t in held or next(machine.iter(t), None) is not None for t in (s, s.replace("臺", "台")))

    def write_source(name: str, files: list[Path], extra: Iterable[str] = ()) -> None:
        handles = [(directory / f"{name}-{i:02}.txt").open("w") for i in range(a.workers)]

        def add(s: str) -> None:
            s = normalized(s)
            counts[name + "_input"] += 1
            if not s or blocked(s):
                counts[name + "_excluded"] += 1
                return
            key = hashlib.blake2b(s.encode(), digest_size=16).digest()
            if name != "wiki" and key in seen:
                counts[name + "_duplicate"] += 1
                return
            if name != "wiki":
                seen.add(key)
            handles[counts[name + "_kept"] % len(handles)].write(s + "\n")
            counts[name + "_kept"] += 1
            chars.update(s)

        try:
            for path in files:
                with path.open() as f:
                    for line in f:
                        add(line)
            for line in extra:
                add(line)
        finally:
            for f in handles:
                f.close()
        print(name, dict(counts), flush=True)

    # Prepared Wikipedia was globally deduplicated by its original pipeline.
    write_source("wiki", wiki)
    tt, receipt = read_tt(tt_train)
    converter = OpenCC("s2twp")
    write_source("real", real, (converter.convert(s) for s in sorted(tt)))
    write_source("generated", generated)
    save_json(
        directory / "prepared.json",
        dict(
            counts,
            training_characters=sorted(chars),
            tt_train=receipt,
            inputs={str(p): digest(p) for p in wiki + real + generated + [tt_train]},
            files={p.name: digest(p) for p in directory.glob("*.txt")},
        ),
    )


# The segmenter state of the current pass, set by fit() before forking the worker pool.
IDS: dict[str, int] = {}
TRIE: mkn.Trie = {}
SCORES: np.ndarray = np.zeros(0)


def shard(task: tuple[Path, Path, str]) -> dict[str, int]:
    """Unigram and pair counts (hard or posterior) of one corpus shard, saved beside the model."""
    path, directory, method = task
    uni = np.zeros(len(IDS))
    pairs: defaultdict[int, float] = defaultdict(int)
    sentences = 0
    for line in path.open():
        text = line.rstrip("\n")
        u: Mapping[int, float]
        b: Mapping[int, float]
        if method == "expected":
            u, b = expected(text, TRIE, IDS, SCORES)
        else:
            tokens = mkn.segment(text, TRIE, IDS, SCORES, float(SCORES[2]))
            u = Counter(tokens)
            seq = [0, *tokens, 1]
            b = Counter((x << 32) | y for x, y in zip(seq, seq[1:]))
        for token, count in u.items():
            uni[token] += count
        for key, value in b.items():
            pairs[key] += value
        sentences += 1
    keys = np.array(sorted(pairs), dtype=np.uint64)
    target = directory / path.stem
    np.save(str(target) + ".uni.npy", uni)
    np.save(str(target) + ".keys.npy", keys)
    np.save(str(target) + ".counts.npy", np.array([pairs[int(k)] for k in keys]))
    return {"sentences": sentences, "pair_types": len(keys)}


def fit(a: argparse.Namespace) -> None:
    """models/NAME: unigram.tsv, bigram.tsv, backoff.tsv, notices, validation.json, stats.json."""
    global IDS, TRIE, SCORES
    while True:
        found = re.search(r"MemAvailable:\s+(\d+)", Path("/proc/meminfo").read_text())
        assert found is not None
        available = int(found[1]) * 1024
        if available >= 32 * 1024**3:
            break
        print(f"Waiting for host RAM: {available / 1024**3:.1f} GiB available; require 32 GiB", flush=True)
        time.sleep(10)
    started = time.monotonic()
    verify_corpus(a)
    output = OUT / "models" / a.name
    if output.exists():
        raise ValueError("model output exists; refuse stale or partial retraining")
    output.mkdir(parents=True)
    work = output / "work"
    work.mkdir()
    prepared = json.loads((OUT / "corpus/prepared.json").read_text())
    _, _, old = mkn.read_lexicon(a.lexicon)
    words = ["<s>", "</s>", "<unk>"] + sorted(set(old) | set(prepared["training_characters"]))
    IDS = {w: i for i, w in enumerate(words)}
    TRIE = mkn.make_trie(words, old)
    if a.fresh:
        initial = np.array([old.get(w, 1e-10) for w in words])
        SCORES = np.log(initial / initial.sum())
    else:
        # Matched final-E controls retain the incumbent tokenizer.
        tokenizer = paths.data("lm", "work") if a.domain == "wiki" else paths.data("lm_chat", "work")
        prior_words = json.loads((tokenizer / "vocabulary.json").read_text())
        prior_scores = np.load(tokenizer / "segmentation_scores.npy")
        prior = dict(zip(prior_words, map(float, prior_scores)))
        _, _, seed = mkn.read_lexicon(OUT / "inputs/lexicon.tsv")
        shift = float(np.median([prior[w] - math.log(p) for w, p in seed.items() if w in prior]))
        SCORES = np.array([prior.get(w, math.log(old.get(w, 1e-10)) + shift) for w in words])
    sources = ["wiki"] if a.domain == "wiki" else ["real"] + (["generated"] if a.generated_weight else [])
    passes = 3 if a.fresh else 1
    for iteration in range(passes):
        directory = work / f"em{iteration}" if a.fresh else work
        directory.mkdir(exist_ok=True)
        tasks = [
            (p, directory, a.method) for source in sources for p in sorted((OUT / "corpus").glob(source + "-*.txt"))
        ]
        with mp.get_context("fork").Pool(a.workers) as pool:
            for result in pool.imap_unordered(shard, tasks):
                print(a.name, iteration, result, flush=True)
        key_parts = []
        count_parts = []
        uni = np.zeros(len(words))
        for path, _, _ in tasks:
            weight = a.generated_weight if path.name.startswith("generated") else 1.0
            stem = directory / path.stem
            uni += weight * np.load(str(stem) + ".uni.npy")
            key_parts.append(np.load(str(stem) + ".keys.npy"))
            count_parts.append(weight * np.load(str(stem) + ".counts.npy"))
        keys = np.concatenate(key_parts)
        counts = np.concatenate(count_parts)
        order = np.argsort(keys)
        keys = keys[order]
        counts = counts[order]
        starts = np.r_[0, np.flatnonzero(keys[1:] != keys[:-1]) + 1]
        keys = keys[starts]
        counts = np.add.reduceat(counts, starts)
        if iteration < passes - 1:
            SCORES = np.log((uni + 0.1) / (float(uni.sum()) + 0.1 * len(words)))
            assert np.isfinite(SCORES).all() and abs(float(np.exp(SCORES).sum()) - 1) < 1e-12
    np.save(work / "segmentation_scores.npy", SCORES)
    save_json(work / "vocabulary.json", words)
    np.save(work / "expected_keys.npy", keys)
    np.save(work / "expected_counts.npy", counts)
    # shortcut: integer MKN rounds pooled counts once; revisit fractional discounts if paired dev justifies it.
    rounded = np.floor(counts + 0.5).astype(np.uint64)
    keep = rounded > 0
    model = mkn.fit_mkn(keys[keep], rounded[keep], len(words), 3_000_000 if a.domain == "wiki" else 1_600_000)
    mkn.write_tsv(output, words, model)
    shutil.copyfile(OUT / "NOTICE.md", output / "NOTICE.md")
    if a.domain == "wiki":
        for name in ("NOTICE.md", "articles.tsv.gz"):
            shutil.copyfile(paths.data("lm", name), output / ("SOURCE-NOTICE.md" if name == "NOTICE.md" else name))
    else:
        for name in ("NOTICE.md", "source-manifest.json", "attribution.jsonl"):
            shutil.copyfile(
                paths.data("lm_chat2", name),
                output / ("SOURCE-NOTICE.md" if name == "NOTICE.md" else name),
            )
        for name in ("Taiwan-Tongues-LICENSE", "Taiwan-Tongues-README.md", "source_receipts.json"):
            shutil.copyfile(paths.data("lm_chat", name), output / name)
        if a.generated_weight:
            shutil.copyfile(paths.data("lm_chat", "Qwen3-8B-LICENSE"), output / "Qwen3-8B-LICENSE")
    validation = mkn.TsvModel(output).verify()
    save_json(output / "validation.json", validation)
    save_json(
        output / "stats.json",
        {
            "name": a.name,
            "domain": a.domain,
            "method": a.method,
            "generated_weight": a.generated_weight,
            "seed_lexicon_sha256": digest(a.lexicon),
            "posterior_bigram_mass": float(counts.sum()),
            "posterior_word_mass": float(uni.sum()),
            "posterior_pair_types": len(keys),
            "rounded_pair_types": int(keep.sum()),
            "model": model[4],
            "segmentation_passes": passes,
            "segmentation_initialization": "lexicon" if a.fresh else "incumbent final E",
            "tokenizer_scores_sha256": digest(work / "segmentation_scores.npy"),
            "tokenizer_vocabulary_sha256": digest(work / "vocabulary.json"),
            "code": {p.name: digest(p) for p in Path(__file__).parent.glob("*.py")},
            "seconds": time.monotonic() - started,
            "files": {p.name: digest(p) for p in output.glob("*.tsv")},
            "validation": validation,
        },
    )
    print("VALIDATED", a.name, time.monotonic() - started, flush=True)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("phase", choices=["lexicon", "prepare", "fit"])
    p.add_argument(
        "--source",
        type=Path,
        default=OUT / "inputs/mcbpmf-data.txt",
        help="McBopomofo data (MIT/BSD), frozen in base/inputs",
    )
    p.add_argument("--workers", type=int, default=8)
    p.add_argument("--name", default="wiki_expected")
    p.add_argument("--domain", choices=["wiki", "chat"], default="wiki")
    p.add_argument("--method", choices=["hard", "expected"], default="expected")
    p.add_argument("--fresh", action="store_true", help="refit segmentation with three corpus E/M passes")
    p.add_argument("--generated-weight", type=float, default=0.0)
    p.add_argument("--lexicon", type=Path, default=OUT / "lexicon_source.tsv")
    a = p.parse_args(argv)
    if a.workers < 1 or not 0 <= a.generated_weight <= 1:
        p.error("invalid workers/weight")
    remote_only()
    {"lexicon": lexicon, "prepare": prepare, "fit": fit}[a.phase](a)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
