"""One pruning ablation for A1: wiki_expected's posterior counts kept up to six million bigrams (wiki_expected_wide).

Refits MKN from models/wiki_expected/work without recounting the corpus.  Selected in the A1 combination
(REPORT.md).  GPU host only.

    zt --remote train base wide
    python -m nlp.training.base.prune
"""

from __future__ import annotations

import argparse
import json
import shutil
import time

import numpy as np

from nlp.training.lm import mkn
from nlp.training.lm.files import digest, save_json

from .train import OUT, remote_only


def main(argv: list[str] | None = None) -> int:
    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    remote_only()
    started = time.monotonic()
    source = OUT / "models/wiki_expected"
    receipt = json.loads((source / "stats.json").read_text())
    assert receipt["method"] == "expected"
    assert all(digest(source / name) == sha for name, sha in receipt["files"].items())
    words = ["<s>"] + [line.split("\t", 1)[0] for line in (source / "unigram.tsv").open()]
    assert words[:3] == ["<s>", "</s>", "<unk>"] and len(words) == len(set(words))
    keys = np.load(source / "work/expected_keys.npy", mmap_mode="r")
    expected = np.load(source / "work/expected_counts.npy", mmap_mode="r")
    assert len(keys) == len(expected) and np.all(np.isfinite(expected)) and np.all(expected >= 0)
    counts = np.floor(expected + 0.5).astype(np.uint64)
    keep = counts > 0
    model = mkn.fit_mkn(keys[keep], counts[keep], len(words), 6_000_000)
    output = OUT / "models/wiki_expected_wide"
    output.mkdir(parents=True, exist_ok=False)
    mkn.write_tsv(output, words, model)
    validation = mkn.TsvModel(output).verify()
    for name in ("NOTICE.md", "SOURCE-NOTICE.md", "articles.tsv.gz"):
        shutil.copyfile(source / name, output / name)
    save_json(output / "validation.json", validation)
    save_json(
        output / "stats.json",
        {
            "name": "wiki_expected_wide",
            "method": "same posterior counts; wider pruning cap",
            "parent_stats_sha256": digest(source / "stats.json"),
            "max_bigrams": 6_000_000,
            "model": model[4],
            "files": {p.name: digest(p) for p in output.glob("*.tsv")},
            "validation": validation,
            "seconds": time.monotonic() - started,
        },
    )
    print("VALIDATED wiki_expected_wide", model[4], flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
