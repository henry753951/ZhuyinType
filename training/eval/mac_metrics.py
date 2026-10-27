#!/usr/bin/env python3
"""Per-key latency, CPU, energy and memory of the app configuration on this Mac, for every release.

    python3 training/eval/mac_metrics.py --cli .build/release/zhuyin-cli --out metrics-mac.json

Types each sentence of training/eval/latency_sentences.txt from start to end in one composition (zhuyin-cli --timing)
and aggregates the summary lines.  Uses the models in data/build when present (the app configuration).
"""
import argparse
import json
import os
import platform
import re
import statistics
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from conversion_eval import keys_for, load_lexicon, readings_for  # noqa: E402

APP_ARGS = [("data/build/reranker", ["--reranker", "data/build/reranker", "--rerank-count", "16"]),
            ("data/build/v3", ["--v3", "data/build/v3", "--v3-weight", "0.5"]),
            ("data/build/lm_chat", ["--chat-lm", "data/build/lm_chat", "--chat-prior", "0.3"])]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", default=".build/release/zhuyin-cli")
    parser.add_argument("--lexicon", default="data/build/lexicon.tsv")
    parser.add_argument("--sentences", default=os.path.join(os.path.dirname(__file__), "latency_sentences.txt"))
    parser.add_argument("--engine-data", help="the app's Resources folder: the engine loads its compiled lexicon "
                        "and language models, as the app does (the readings still come from --lexicon)")
    parser.add_argument("--out")
    args = parser.parse_args()
    lexicon = load_lexicon(args.lexicon)
    extra = [a for path, flags in APP_ARGS if os.path.exists(path) for a in flags]
    engine_lexicon = args.lexicon
    if args.engine_data:
        engine_lexicon = os.path.join(args.engine_data, "lexicon.tsv")
        extra = [os.path.join(args.engine_data, os.path.basename(a)) if a.startswith("data/build/") else a for a in extra]
    rows = []
    for sentence in open(args.sentences, encoding="utf-8").read().split():
        readings = readings_for(sentence, lexicon)
        if not readings:
            continue
        output = subprocess.run([args.cli, "--lexicon", engine_lexicon, *extra, "--timing", keys_for(readings) + "<enter>"],
                                capture_output=True, text=True, check=True).stdout
        def number(pattern):
            match = re.search(pattern, output)
            return float(match.group(1)) if match else None
        rows.append({"sentence": sentence, "syllables": len(readings),
                     "p50_ms": number(r"keys \d+: p50 ([\d.]+) ms"), "p95_ms": number(r"p95 ([\d.]+) ms"),
                     "max_ms": number(r"max ([\d.]+) ms\]"), "cpu_ms_per_key": number(r"([\d.]+) ms per key"),
                     "energy_mj_per_key": number(r"([\d.]+) mJ per key"),
                     "memory_mb": number(r"memory (\d+) MB now")})
    def summary(key):
        values = [r[key] for r in rows if r[key] is not None]
        return round(statistics.median(values), 2) if values else None
    result = {"machine": platform.machine(), "macos": platform.mac_ver()[0], "configuration": extra,
              "median": {k: summary(k) for k in ["p50_ms", "p95_ms", "max_ms", "cpu_ms_per_key", "energy_mj_per_key",
                                                  "memory_mb"]},
              "sentences": rows}
    text = json.dumps(result, ensure_ascii=False, indent=1)
    if args.out:
        open(args.out, "w", encoding="utf-8").write(text + "\n")
    print(json.dumps(result["median"], ensure_ascii=False))


if __name__ == "__main__":
    main()
