#!/usr/bin/env python3
"""Evaluate context|truth|syllables rows through the actual CLI commit and oracle@16.

Python stdlib only. No engine runs in --self-test. Example (on the remote):
  python training/eval/rows_eval.py --rows SET.txt --output RESULT.jsonl --jobs 4
  python training/eval/stats.py BASE.jsonl NEW.jsonl

The optional --shanjie-variants is an unmodified evaluation-time dictionary, never
training data. Without it, shanjie means its eight common-character folds only.
CER follows its documented rowstats rule: zero for normalized-equal rows,
otherwise raw Unicode Levenshtein distance, divided by raw truth characters.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from conversion_eval import KEYS, VARIANTS, keys_for

COMMON = str.maketrans("她妳它牠嘗周臺裏", "他你他他嚐週台裡")


def normalizer(variants=None):
    table = {}
    if variants:
        for i, line in enumerate(Path(variants).read_text(encoding="utf-8").splitlines(), 1):
            if not line or line.startswith("#"):
                continue
            pair = line.split("\t")
            if len(pair) != 2 or not all(pair) or pair[0] in table:
                raise ValueError(f"{variants}:{i}: invalid variant pair")
            table[pair[0]] = pair[1]
    longest = max(map(len, table), default=0)

    def apply(text):
        text = text.translate(COMMON)
        out, i = [], 0
        while i < len(text):
            for size in range(min(longest, len(text) - i), 0, -1):
                replacement = table.get(text[i:i + size])
                if replacement is not None:
                    out.append(replacement)
                    i += size
                    break
            else:
                out.append(text[i])
                i += 1
        return "".join(out)
    return apply


def row_keys(syllables):
    readings = syllables.split()
    if not readings:
        raise ValueError("empty syllables")
    result = []
    for syllable in readings:
        if syllable.startswith("˙"):
            syllable = syllable[1:] + "˙"
        body = syllable.rstrip("ˊˇˋ˙")
        if (not body or any(c not in KEYS or c in "ˊˇˋ˙" for c in body)
                or len(syllable) - len(body) > 1):
            raise ValueError(f"invalid syllable: {syllable!r}")
        result.append(syllable)
    return keys_for(result)


def load_rows(path):
    rows = []
    for line_number, line in enumerate(Path(path).read_text(encoding="utf-8-sig").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("|")
        if len(parts) != 3 or not parts[1] or any(ord(c) < 32 for c in line):
            raise ValueError(f"{path}:{line_number}: expected context|truth|syllables without controls")
        context, truth, syllables = parts
        keys = row_keys(syllables)
        identity = json.dumps(parts, ensure_ascii=False, separators=(",", ":"))
        rows.append(dict(row=len(rows) + 1, source_line=line_number, context=context, truth=truth,
                         syllables=syllables, keys=keys, id=hashlib.sha256(identity.encode()).hexdigest()))
    if not rows:
        raise ValueError(f"{path}: no rows")
    return rows


def edit_distance(truth, predicted):
    previous = list(range(len(predicted) + 1))
    for i, expected in enumerate(truth, 1):
        current = [i]
        for j, actual in enumerate(predicted, 1):
            current.append(min(current[-1] + 1, previous[j] + 1,
                               previous[j - 1] + (actual != expected)))
        previous = current
    return previous[-1]


def score_row(row, predicted, candidates, normalize):
    result = {**row, "prediction": predicted, "nbest": candidates, "gold_len": len(row["truth"])}
    distance = edit_distance(row["truth"], predicted)
    for name, fold in (("raw", str), ("shanjie", normalize),
                       ("variants", lambda s: s.translate(VARIANTS))):
        gold = fold(row["truth"])
        ok = fold(predicted) == gold
        result["ok_" + name] = int(ok)
        result["errors_" + name] = 0 if ok else distance
        result["oracle16_" + name] = int(any(fold(s) == gold for s in candidates[:16]))
    return result


def measured_run(command, lines):
    """Per-child wait4 avoids mixing RSS/CPU from concurrent workers; files avoid pipe deadlocks."""
    started = time.monotonic()
    with tempfile.TemporaryFile() as inp, tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        inp.write(("\n".join(lines) + "\n").encode())
        inp.seek(0)
        process = subprocess.Popen(command, stdin=inp, stdout=out, stderr=err,
                                   env={**os.environ, "OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1"})
        try:
            _, status, usage = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
        except BaseException:
            process.kill()
            process.wait()
            raise
        wall = time.monotonic() - started
        out.seek(0)
        err.seek(0)
        stdout, stderr = out.read().decode(), err.read().decode()
    if process.returncode:
        raise RuntimeError(f"{command[0]} exited {process.returncode}: {stderr[-2000:]}")
    # Preserve an empty prediction line: str.splitlines() does so for '\n'.
    outputs = stdout.splitlines()
    if len(outputs) != len(lines):
        raise RuntimeError(f"{command[0]} returned {len(outputs)} rows for {len(lines)} inputs")
    return outputs, dict(pid=process.pid, command=command, rows=len(lines), wall_s=wall,
                         wall_ms_per_row=1000 * wall / len(lines),
                         cpu_s=usage.ru_utime + usage.ru_stime,
                         peak_rss_mib=usage.ru_maxrss / (2**20 if sys.platform == "darwin" else 1024),
                         stderr=stderr)


def evaluate(rows, command, jobs, normalize):
    jobs = min(jobs, len(rows))
    size = (len(rows) + jobs - 1) // jobs

    def chunk(part):
        scripts = [r["context"] + "\t" + r["keys"] for r in part]
        predictions, commit = measured_run(command, [s + "<enter>" for s in scripts])
        nbest, oracle = measured_run(command + ["--report-nbest", "16"], scripts)
        commit["pass"], oracle["pass"] = "commit", "oracle16"
        return ([score_row(r, p, n.split("\t") if n else [], normalize)
                 for r, p, n in zip(part, predictions, nbest)], [commit, oracle])

    with ThreadPoolExecutor(jobs) as pool:
        parts = list(pool.map(chunk, [rows[i:i + size] for i in range(0, len(rows), size)]))
    return [r for rr, _ in parts for r in rr], [p for _, pp in parts for p in pp]


def summary(rows):
    n, chars = len(rows), sum(r["gold_len"] for r in rows)
    return {"n": n, "characters": chars, **{
        norm: {"top1": sum(r["ok_" + norm] for r in rows) / n,
               "cer": sum(r["errors_" + norm] for r in rows) / chars,
               "oracle16": sum(r["oracle16_" + norm] for r in rows) / n}
        for norm in ("raw", "shanjie", "variants")}}


def self_test():
    assert row_keys("ㄋㄧˇ ㄏㄠˇ ㄇㄚ˙") == "su3cl3a87"
    assert row_keys("ㄊㄚ ˙ㄉㄜ") == "w8<space>2k7"
    assert edit_distance("𠮷你好", "你好啊") == 2
    assert normalizer()("她妳它牠嘗周臺裏") == "他你他他嚐週台裡"
    r = score_row(dict(truth="她在臺灣"), "他在台灣", ["她在臺灣"], normalizer())
    assert r["ok_shanjie"] == 1 and r["errors_shanjie"] == 0 and r["ok_variants"] == 0
    for invalid in ("", "abc", "˙", "ㄋㄧˇˋ", "ㄋˊㄧ"):
        try:
            row_keys(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError(invalid)
    with tempfile.TemporaryDirectory() as d:
        variants = Path(d) / "variants.tsv"
        variants.write_text("甲乙\t丙\n甲\t丁\n", encoding="utf-8")
        assert normalizer(variants)("甲乙甲") == "丙丁"
        p = Path(d) / "rows.txt"
        p.write_text("# comment\n前文|你好|ㄋㄧˇ ㄏㄠˇ\n|他|ㄊㄚ\n", encoding="utf-8")
        rows = load_rows(p)
        got, processes = evaluate(rows, [sys.executable, "-c",
            "import sys; oracle='--report-nbest' in sys.argv; "
            "[print(('你好\\t你號' if oracle else '你好') if l.startswith('前文\\t') "
            "else ('他' if oracle else '')) for l in sys.stdin]"], 2, normalizer())
        assert len(processes) == 4 and got[0]["ok_raw"] == 1 and got[1]["prediction"] == ""
        p.write_text("bad row\n", encoding="utf-8")
        try:
            load_rows(p)
        except ValueError:
            pass
        else:
            raise AssertionError("malformed rows accepted")
        try:
            measured_run([sys.executable, "-c", "raise SystemExit(3)"], ["one"])
        except RuntimeError:
            pass
        else:
            raise AssertionError("failed child accepted")
    print("rows_eval self-test passed")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--rows", nargs="+", type=Path)
    p.add_argument("--output", type=Path, help="JSONL; multiple --rows files are concatenated in order")
    p.add_argument("--cli", default=".build/release/zhuyin-cli")
    p.add_argument("--lexicon", default="data/build/lexicon.tsv")
    p.add_argument("--jobs", type=int, default=1)
    p.add_argument("--cli-arg", action="append", default=[])
    p.add_argument("--shanjie-variants", type=Path)
    p.add_argument("--self-test", action="store_true")
    a = p.parse_args()
    if a.self_test:
        self_test()
        return
    if not a.rows or not a.output or a.jobs < 1:
        p.error("--rows, --output and positive --jobs are required")
    if any(x in a.cli_arg for x in ("--keep-memory", "--report-nbest", "--report-list")):
        p.error("stateful or report-mode overrides would invalidate independent commit rows")
    command = [a.cli, "--lexicon", a.lexicon, "--typo", "standard", "--batch"] + a.cli_arg
    started = time.monotonic()
    results, processes, sets = [], [], []
    normalize = normalizer(a.shanjie_variants)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = a.output.with_suffix(a.output.suffix + ".tmp")
    temporary.write_text("", encoding="utf-8")
    for path in a.rows:
        rows = [{**r, "source": str(path)} for r in load_rows(path)]
        set_started = time.monotonic()
        scored, workers = evaluate(rows, command, a.jobs, normalize)
        sets.append({"source": str(path), **summary(scored), "wall_s": time.monotonic() - set_started,
                     "processes": workers})
        for row in scored:
            results.append({**row, "row": len(results) + 1})
        # Keep completed sets reviewable if a later CLI batch fails.
        with temporary.open("a", encoding="utf-8") as f:
            f.writelines(json.dumps(r, ensure_ascii=False) + "\n" for r in results[-len(scored):])
        processes.extend(workers)
        print(json.dumps({k: v for k, v in sets[-1].items() if k != "processes"}), flush=True)
    record = {**summary(results), "wall_s": time.monotonic() - started, "jobs": a.jobs,
              "sets": sets,
              "normalization": "common characters + dictionary" if a.shanjie_variants else "common characters only",
              "inputs": {str(f): hashlib.sha256(f.read_bytes()).hexdigest() for f in a.rows},
              "variants_sha256": hashlib.sha256(a.shanjie_variants.read_bytes()).hexdigest() if a.shanjie_variants else None,
              "processes": processes}
    temporary.replace(a.output)
    a.output.with_suffix(".summary.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in record.items() if k not in ("processes", "sets")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
