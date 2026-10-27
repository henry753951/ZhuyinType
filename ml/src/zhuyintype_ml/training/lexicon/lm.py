"""Language models that know the mined words: the Wikipedia and chat bigrams re-segmented with base lexicon + packs.

    python -m zhuyintype_ml.training.lexicon.lm --seed SEED.tsv --out DIR --wiki-work WORK --dump DUMP \\
        --chat-data DATA [--workers 16]

A word the language model has never seen gets only the lexicon's unigram (MODEL §5), and a two-character word with
no counts loses to the characters' bigrams almost always.  So the mined words go into the segmentation vocabulary
and both models are counted again from the same text:

  wiki  `zt train lm`'s training sentences (WORK: extract.json, split.json, extracted/, unique.txt, sentences/ of a
        finished run, linked, not copied), the same three-pass hard EM and modified Kneser-Ney, then prefer-tai;
        written to DIR/lm with the shipped model's test_sentences.txt for its perplexity
  chat  `zt train lm_chat2 real` on the same prepared utterances (DATA/lm_chat2: inputs/, no_generated/work/'s
        prepared.json, sentences/, context_pairs.tsv), with SEED as its seed lexicon; written to DIR/lm_chat

Only the seed vocabulary changes; text, holdouts, smoothing and pruning are the shipped models'.  GPU host (CPU).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path

from zhuyintype_ml.core import paths


def link(source: Path, target: Path) -> None:
    if target.is_symlink() or target.exists():
        return
    target.symlink_to(source.resolve())


def seed(base: Path, packs: list[Path], out: Path) -> Path:
    """base.tsv rows and every pack row, one file."""
    rows = [line for line in base.read_text(encoding="utf-8").splitlines(keepends=True)]
    known = {line.split("\t")[1] for line in rows if not line.startswith("#")}
    for pack in packs:
        for line in pack.read_text(encoding="utf-8").splitlines(keepends=True):
            if not line.startswith("#") and line.split("\t")[1] not in known:
                rows.append(line)
    out.write_text("".join(rows), encoding="utf-8")
    return out


def wiki(seed_path: Path, work_source: Path, dump: Path, out: Path, shipped: Path, workers: int) -> None:
    work = out / "lm_work"
    work.mkdir(parents=True, exist_ok=True)
    for name in ("extracted", "unique.txt", "sentences"):
        link(work_source / name, work / name)
    for name in ("extract.json", "split.json"):
        if not (work / name).exists():
            shutil.copy(work_source / name, work / name)
    model = out / "lm"
    model.mkdir(parents=True, exist_ok=True)
    shutil.copy(shipped / "test_sentences.txt", model / "test_sentences.txt")
    run(
        "zhuyintype_ml.training.lm.train",
        "--dump",
        str(dump),
        "--lexicon",
        str(seed_path),
        "--work",
        str(work),
        "--output",
        str(model),
        "--workers",
        str(workers),
    )
    run("zhuyintype_ml.training.lm.prefer_tai", str(model), str(seed_path))


def chat(seed_path: Path, data: Path, out: Path, workers: int) -> None:
    """lm_chat2's trainer reads data/build/lm_chat2 through ZT_DATA: a private tree with the shared inputs linked."""
    tree = out / "chat_data"
    source = data / "lm_chat2"
    mirror = tree / "lm_chat2"
    (mirror / "inputs").mkdir(parents=True, exist_ok=True)
    for item in (source / "inputs").iterdir():
        if item.name != "lexicon.tsv":
            link(item, mirror / "inputs" / item.name)
    shutil.copy(seed_path, mirror / "inputs" / "lexicon.tsv")
    for name in ("dev_dialogue.txt", "test_dialogue.txt"):
        link(source / name, mirror / name)
    work = mirror / "no_generated" / "work"
    work.mkdir(parents=True, exist_ok=True)
    for name in ("prepared.json", "sentences", "context_pairs.tsv"):
        link(source / "no_generated" / "work" / name, work / name)
    for name in ("corpus", "lm", "lm_chat"):
        link(data / name, tree / name)
    env = dict(paths.python_env(), ZT_DATA=str(tree), OMP_NUM_THREADS="1", OPENBLAS_NUM_THREADS="1")
    run("zhuyintype_ml.training.lm_chat2.train", "--variant", "real", "--workers", str(workers), env=env)
    target = out / "lm_chat"
    target.mkdir(parents=True, exist_ok=True)
    for name in ("unigram.tsv", "bigram.tsv", "backoff.tsv", "stats.json", "validation.json"):
        shutil.copy(mirror / "no_generated" / name, target / name)


def run(module: str, *args: str, env: dict[str, str] | None = None) -> None:
    command = paths.python_command(module, *args, groups=("train",))
    subprocess.run(command, check=True, env=env or dict(paths.python_env(), OMP_NUM_THREADS="1"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--pack", type=Path, action="append", default=[])
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--wiki-work", type=Path, required=True)
    parser.add_argument("--dump", type=Path, required=True)
    parser.add_argument("--shipped", type=Path, default=paths.data("lm"), help="the shipped lm/ (test sentences)")
    parser.add_argument("--chat-data", type=Path, required=True, help="data directory with lm_chat2/")
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--only", choices=("wiki", "chat"))
    args = parser.parse_args(argv)
    args.out.mkdir(parents=True, exist_ok=True)
    seed_path = seed(args.base, args.pack, args.out / "seed.tsv")
    if args.only != "chat":
        wiki(seed_path, args.wiki_work, args.dump, args.out, args.shipped, args.workers)
    if args.only != "wiki":
        chat(seed_path, args.chat_data, args.out, args.workers)
    summary = {"seed_rows": sum(1 for _ in open(seed_path, encoding="utf-8")) - 1, "packs": [str(p) for p in args.pack]}
    (args.out / "lm.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
