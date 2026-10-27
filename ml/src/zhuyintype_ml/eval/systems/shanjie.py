"""善解（shanjie）in two unmodified forms, each through its own documented interface (no 善解 code is copied or
imported):

- its evaluation CLI, shanjie-eval, for everything typed exactly: rows `context|truth|syllables` decoded with its
  bigram model, Kneser-Ney side file, ACG and IT packs and its small reranking model (plus the colloquial table in the
  chat profile), the setup its own benchmark tool used.  The committed text is the candidate its small model picked:
  the `c` index of `--tinylm-scores` into the `--dump` list, checked against the SHA-256 of the chosen outputs its
  summary line prints.  A row whose readings have no path in its lexicon makes the CLI stop; such rows are found from
  where the dump ends, recorded as no-path (counted wrong) and the rest run again.
- its core library through the C interface its macOS app uses (core/include/shanjie.h), for keys typed with errors and
  per-key time: one engine per worker, set up as its app sets it up (the same packs and models as above, demotion
  and reranking on at the standard level, prediction row, abbreviations and English off, learning off), each key sent
  as the app sends it, the small model's job run and applied after every key (its replay tool's `each` mode: the model
  always finishes before the next key).

Split commits: the second part is typed after the first was committed, with the committed text as its context (the
CLI's row context; set_left_context through the C interface).  Profiles: chat and formal.
"""

from __future__ import annotations

import ctypes
import hashlib
import json
import os
import re
import subprocess
import tempfile
import threading
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

from zhuyintype_ml.eval.rows import normalizer
from zhuyintype_ml.eval.systems.base import Case, Key, Outcome, Segment, System, Task, key_time_summary, parse_script
from zhuyintype_ml.eval.systems.references import SHANJIE

PROFILES = {"chat": 0, "formal": 1}
PACKS = "acg,it"
PACK_BITS = 0b101  # bit 0 ACG, bit 2 IT (shanjie.h)
CHUNK = 64
KEY_KINDS = {"space": 2, "enter": 3, "left": 7}
NO_PATH = "no decode path"


# MARK: - The CLI's files


def row_line(context: str, truth: str, syllables: tuple[str, ...]) -> str:
    """One `context|truth|syllables` row; the separators cannot appear inside a field."""

    def clean(text: str) -> str:
        return text.replace("|", " ").replace("\n", " ").replace("\r", " ")

    return f"{clean(context)}|{clean(truth)}|{' '.join(syllables)}"


def parse_dump(text: str) -> dict[int, list[str]]:
    """Candidates per row (rows counted from 1) of a `--dump` file: `row\\trank\\tsurface\\tscore` lines."""
    found: dict[int, list[str]] = {}
    for line in text.splitlines():
        row, rank, surface, _ = line.split("\t")
        candidates = found.setdefault(int(row), [])
        if int(rank) != len(candidates) + 1:
            raise ValueError(f"dump row {row}: rank {rank} out of order")
        candidates.append(surface)
    return found


def parse_scores(text: str) -> dict[int, int]:
    """The small model's pick per row (index into the dump's list) from `--tinylm-scores`."""
    return {int(record["i"]): int(record["c"]) for record in map(json.loads, text.splitlines())}


SUMMARY = re.compile(
    r"^## (\S+)\s+(\S+)\s+\{'n': (\d+), 'top1': (\d+), 'oracle@64': (\d+), 'top1_sha256': '([0-9a-f]+)'\}"
)


def parse_summary(stdout: str) -> dict[str, Any]:
    for line in stdout.splitlines():
        match = SUMMARY.match(line)
        if match:
            name, tag, n, top1, oracle, digest = match.groups()
            return {"name": name, "tag": tag, "n": int(n), "top1": int(top1), "oracle64": int(oracle), "sha256": digest}
    raise ValueError("no summary line in shanjie-eval's output")


class Cli:
    """shanjie-eval over row files, `chunk` rows per process, several processes at once."""

    def __init__(self, profile: str, workers: int, binary: Path | None = None, root: Path | None = None) -> None:
        self.profile = profile
        self.workers = workers
        self.root = root or SHANJIE.checkout
        self.binary = binary or self.root / "target" / "release" / "shanjie-eval"
        self.fold = normalizer(self.root / "eval" / "variants.tsv")
        self.runs = 0
        self.mismatches = 0
        self.lock = threading.Lock()

    def command(self, rows: Path, directory: Path) -> list[str]:
        lm = self.root / "data" / "lm"
        command = [
            str(self.binary),
            "--lm",
            str(lm / "bigram.sjlm"),
            "--profile",
            self.profile,
            "--rows",
            str(rows),
            "--dump",
            str(directory / "dump.tsv"),
            "--rowstats",
            str(directory / "rowstats.tsv"),
            "--context",
            "--packs",
            PACKS,
            "--tinylm",
            str(lm / "tinylm.sjtl"),
        ]
        if self.profile == "chat":
            command += ["--colloquial-core", str(lm / "colloquial.sjlm")]
        return command + ["--tinylm-scores", str(directory / "scores.jsonl"), "--threads", "1"]

    def chunk(self, rows: list[str]) -> list[str | None]:
        """The committed text of each row, None where the readings have no path."""
        results: list[str | None] = []
        pending = list(rows)
        with tempfile.TemporaryDirectory(prefix="zt-sj-") as name:
            directory = Path(name)
            while pending:
                path = directory / "rows.txt"
                path.write_text("".join(r + "\n" for r in pending), encoding="utf-8")
                done = subprocess.run(
                    self.command(path, directory),
                    capture_output=True,
                    text=True,
                    env=dict(os.environ, OMP_NUM_THREADS="1"),
                    cwd=self.root,
                )
                dump = parse_dump((directory / "dump.tsv").read_text(encoding="utf-8"))
                picks = parse_scores((directory / "scores.jsonl").read_text(encoding="utf-8"))
                complete = len(pending) if done.returncode == 0 else len(dump)
                chosen = [dump[i][picks[i]] for i in range(1, complete + 1)]
                if done.returncode == 0:
                    self.check(done.stdout, pending, chosen, directory)
                    results += chosen
                    break
                if NO_PATH not in done.stderr or len(dump) >= len(pending):
                    raise RuntimeError(f"shanjie-eval failed: {done.stderr[-500:]}")
                results += [*chosen, None]
                pending = pending[complete + 1 :]
        return results

    def check(self, stdout: str, rows: list[str], chosen: list[str], directory: Path) -> None:
        """The chosen outputs against the CLI's own summary (hash of the chosen texts, top-1 count) and rowstats."""
        summary = parse_summary(stdout)
        digest = hashlib.sha256("\n".join(chosen).encode()).hexdigest()
        stats = [line.split("\t") for line in (directory / "rowstats.tsv").read_text(encoding="utf-8").splitlines()]
        ours = [int(self.fold(c) == self.fold(r.split("|")[1])) for c, r in zip(chosen, rows)]
        theirs = [int(s[1]) for s in stats]
        if summary["n"] != len(rows) or digest != summary["sha256"]:
            raise RuntimeError("shanjie-eval: recovered outputs do not match its summary")
        with self.lock:
            self.runs += 1
            self.mismatches += sum(a != b for a, b in zip(ours, theirs)) + abs(summary["top1"] - sum(theirs))

    def run(self, rows: list[str]) -> list[str | None]:
        chunks = [rows[i : i + CHUNK] for i in range(0, len(rows), CHUNK)]
        with ThreadPoolExecutor(self.workers) as pool:
            return [out for part in pool.map(self.chunk, chunks) for out in part]


# MARK: - The C interface


class Output(ctypes.Structure):
    _fields_ = [
        ("handled", ctypes.c_int32),
        ("commit", ctypes.c_char_p),
        ("preedit", ctypes.c_char_p),
        ("cursor_utf16", ctypes.c_uint32),
        ("candidate_count", ctypes.c_uint32),
        ("candidates", ctypes.c_void_p),
        ("candidate_selected", ctypes.c_int32),
        ("candidate_columns", ctypes.c_uint32),
        ("candidate_first", ctypes.c_uint32),
        ("candidate_total", ctypes.c_uint32),
        ("candidate_vertical", ctypes.c_uint32),
        ("english_run_utf16_start", ctypes.c_uint32),
        ("english_run_utf16_len", ctypes.c_uint32),
    ]


class KeyEvent(ctypes.Structure):
    _fields_ = [("kind", ctypes.c_uint32), ("ch", ctypes.c_uint32), ("modifiers", ctypes.c_uint32)]


OUT = ctypes.POINTER(ctypes.POINTER(Output))
# Setters that take (engine, value, ShanjieOutput **out) and return a snapshot.
SNAPSHOT_SETTERS = (
    "set_profile",
    "set_demote",
    "set_prediction",
    "set_candidate_vertical",
    "set_abbreviation",
    "set_english",
    "set_rerank",
    "set_rerank_level",
)


def library(path: Path) -> ctypes.CDLL:
    lib = ctypes.CDLL(str(path))
    p, u32, i32, s = ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int32, ctypes.c_char_p
    signatures: dict[str, tuple[list[Any], Any]] = {
        "shanjie_engine_new_packs": ([s, u32, s, u32, ctypes.POINTER(p)], i32),
        "shanjie_engine_free": ([p], None),
        "shanjie_engine_key": ([p, KeyEvent, OUT], i32),
        "shanjie_engine_reset": ([p, u32, OUT], i32),
        "shanjie_output_free": ([ctypes.POINTER(Output)], None),
        "shanjie_engine_load_lm": ([p, s], i32),
        "shanjie_engine_load_tinylm": ([p, s], i32),
        "shanjie_engine_load_colloquial": ([p, s], i32),
        "shanjie_engine_set_left_context": ([p, s], i32),
        "shanjie_engine_set_learning": ([p, u32], i32),
        "shanjie_engine_rerank_take": ([p, ctypes.POINTER(p)], i32),
        "shanjie_rerank_run": ([p], i32),
        "shanjie_engine_rerank_apply": ([p, p, OUT], i32),
        "shanjie_rerank_free": ([p], None),
        **{f"shanjie_engine_{name}": ([p, u32, OUT], i32) for name in SNAPSHOT_SETTERS},
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(lib, name)
        function.argtypes = arguments
        function.restype = result
    return lib


class Engine:
    """One 善解 engine, used from one thread only (its rule), set up like its app."""

    def __init__(self, lib: ctypes.CDLL, profile: str, root: Path) -> None:
        self.lib = lib
        handle = ctypes.c_void_p()
        lexicon, packs, lm = root / "data" / "lexicon", root / "data" / "packs", root / "data" / "lm"
        self.ok(lib.shanjie_engine_new_packs(str(lexicon).encode(), 0, str(packs).encode(), PACK_BITS, handle), "new")
        self.handle = handle
        self.ok(lib.shanjie_engine_load_lm(handle, str(lm / "bigram.sjlm").encode()), "load_lm")
        self.ok(lib.shanjie_engine_load_tinylm(handle, str(lm / "tinylm.sjtl").encode()), "load_tinylm")
        self.ok(lib.shanjie_engine_load_colloquial(handle, str(lm / "colloquial.sjlm").encode()), "load_colloquial")
        for name, value in (
            ("set_demote", 1),
            ("set_prediction", 0),
            ("set_abbreviation", 0),
            ("set_english", 0),
            ("set_candidate_vertical", 0),
            ("set_rerank", 1),
            ("set_rerank_level", 1),
            ("set_profile", PROFILES[profile]),
        ):
            self.snapshot(getattr(lib, f"shanjie_engine_{name}"), value)
        self.ok(lib.shanjie_engine_set_learning(handle, 0), "set_learning")

    @staticmethod
    def ok(code: int, what: str) -> None:
        if code != 0:
            raise RuntimeError(f"善解 {what}: code {code}")

    def snapshot(self, function: Callable[..., int], *arguments: Any) -> tuple[str, str, bool]:
        """Calls a function that returns a snapshot; (commit, preedit, handled), the snapshot freed."""
        out = ctypes.POINTER(Output)()
        self.ok(function(self.handle, *arguments, ctypes.byref(out)), function.__name__)
        try:
            o = out.contents
            return (o.commit or b"").decode(), (o.preedit or b"").decode(), bool(o.handled)
        finally:
            self.lib.shanjie_output_free(out)

    def rerank(self) -> None:
        """Takes the small model's job of the last change, runs it and applies it, as the app does off the key path."""
        job = ctypes.c_void_p()
        self.ok(self.lib.shanjie_engine_rerank_take(self.handle, ctypes.byref(job)), "rerank_take")
        if not job.value:
            return
        try:
            self.ok(self.lib.shanjie_rerank_run(job), "rerank_run")
            out = ctypes.POINTER(Output)()
            code = self.lib.shanjie_engine_rerank_apply(self.handle, job, ctypes.byref(out))
            if code == 0:
                self.lib.shanjie_output_free(out)
            elif code != 5:
                raise RuntimeError(f"善解 rerank_apply: code {code}")
        finally:
            self.lib.shanjie_rerank_free(job)

    def key(self, key: Key) -> tuple[str, str, bool]:
        if key.is_char:
            event = KeyEvent(1, ord(key.name), 0)
        else:
            event = KeyEvent(KEY_KINDS[key.name], 0, 0)
        return self.snapshot(self.lib.shanjie_engine_key, event)

    def reset(self, context: str) -> None:
        self.snapshot(self.lib.shanjie_engine_reset, 1)
        self.ok(self.lib.shanjie_engine_set_left_context(self.handle, context.encode()), "set_left_context")

    def type_case(self, case: Case, with_context: bool) -> Outcome:
        self.reset(case.context if with_context else "")
        typed: list[str] = []
        key_ms: list[float] = []
        total_ms: list[float] = []
        for index, segment in enumerate(case.segments):
            if index:
                before = (case.context if with_context else "") + "".join(typed)
                self.ok(self.lib.shanjie_engine_set_left_context(self.handle, before.encode()), "context")
            for key in [*parse_script(segment.keys), Key("enter")]:
                started = time.perf_counter()
                commit, _, handled = self.key(key)
                middle = time.perf_counter()
                if key.name != "enter":
                    self.rerank()
                typed.append(commit)
                if not handled and key.is_char:
                    typed.append(key.name)  # passed through: the app inserts the character
                key_ms.append((middle - started) * 1000)
                total_ms.append((time.perf_counter() - started) * 1000)
        return Outcome("".join(typed), info={"key_ms": key_ms[:-1], "with_rerank_ms": total_ms[:-1]})

    def close(self) -> None:
        if self.handle:
            self.lib.shanjie_engine_free(self.handle)
            self.handle = ctypes.c_void_p()


# MARK: - The system


class Shanjie(System):
    family = "shanjie"
    capabilities = frozenset({"context"})

    def __init__(self, profile: str, workers: int = 8, root: Path | None = None) -> None:
        if profile not in PROFILES:
            raise ValueError(profile)
        self.profile = profile
        self.name = f"shanjie-{profile}"
        self.label = "善解（聊天）" if profile == "chat" else "善解（正式）"
        self.workers = workers
        self.root = root or SHANJIE.checkout
        self.cli = Cli(profile, workers, root=self.root)
        self.lib: ctypes.CDLL | None = None

    def resize(self, processes: int) -> None:
        self.workers = self.cli.workers = max(1, processes)

    def start(self) -> None:
        if root_is_default(self.root):
            SHANJIE.setup_record()

    def engines(self, count: int) -> list[Engine]:
        if self.lib is None:
            self.lib = library(self.root / "target" / "release" / "libcore.so")
        lib = self.lib
        with ThreadPoolExecutor(count) as pool:
            return list(pool.map(lambda _: Engine(lib, self.profile, self.root), range(count)))

    def convert(self, task: Task, cases: list[Case]) -> list[Outcome]:
        if task.keys_only or not all(case.exact for case in cases):
            return self.convert_keys(task, cases)
        return self.convert_rows(task, cases)

    def convert_rows(self, task: Task, cases: list[Case]) -> list[Outcome]:
        """Through the CLI, one pass per segment: later segments get the text committed so far as their context."""
        committed = ["" for _ in cases]
        failed = [False for _ in cases]
        for index in range(max(len(case.segments) for case in cases)):
            rows, owners = [], []
            for i, case in enumerate(cases):
                if index >= len(case.segments) or failed[i]:
                    continue
                segment = case.segments[index]
                assert segment.readings is not None
                start = sum(len(s.readings or ()) for s in case.segments[:index])
                part = case.reference[start : start + len(segment.readings)] or "-"
                before = (case.context if task.uses_context else "") + committed[i]
                rows.append(row_line(before, part, segment.readings))
                owners.append(i)
            for i, text in zip(owners, self.cli.run(rows)):
                if text is None:
                    failed[i] = True
                else:
                    committed[i] += text
        return [Outcome("", "no-path") if failed[i] else Outcome(committed[i]) for i in range(len(cases))]

    def convert_keys(self, task: Task, cases: list[Case]) -> list[Outcome]:
        """Through the C interface, one engine per worker."""
        engines = self.engines(min(self.workers, len(cases)) or 1)
        results: list[Outcome | None] = [None] * len(cases)

        def work(number: int) -> None:
            engine = engines[number]
            for i in range(number, len(cases), len(engines)):
                try:
                    results[i] = engine.type_case(cases[i], task.uses_context)
                except RuntimeError as problem:
                    results[i] = Outcome("", "error", {"error": str(problem)[:200]})
            engine.close()

        with ThreadPoolExecutor(len(engines)) as pool:
            list(pool.map(work, range(len(engines))))
        outcomes = [r for r in results if r is not None]
        for outcome in outcomes:
            outcome.info = {k: v for k, v in outcome.info.items() if not k.endswith("_ms")}
        return outcomes

    def latency(self, scripts: list[str]) -> dict[str, Any] | None:
        (engine,) = self.engines(1)
        cases = [Case(str(i), "", (Segment(s.removesuffix("<enter>")),)) for i, s in enumerate(scripts)]
        try:
            for case in cases[:3]:
                engine.type_case(case, False)
            outcomes = [engine.type_case(case, False) for case in cases]
        finally:
            engine.close()
        keys = key_time_summary([o.info["key_ms"] for o in outcomes])
        with_rerank = key_time_summary([o.info["with_rerank_ms"] for o in outcomes])
        return {
            **keys,
            "with_rerank": with_rerank["median"],
            "measured": "the key call through 善解's C interface; with_rerank: plus running and applying the small "
            "model's job, which its app does on another thread",
        }

    def describe(self) -> dict[str, Any]:
        record = SHANJIE.setup_record() if root_is_default(self.root) else {}
        return {
            "repository": SHANJIE.url,
            "commit": SHANJIE.commit,
            "profile": self.profile,
            "cli_options": [
                "--context",
                "--packs",
                PACKS,
                "--tinylm",
                "data/lm/tinylm.sjtl",
                *(["--colloquial-core", "data/lm/colloquial.sjlm"] if self.profile == "chat" else []),
                "--threads",
                "1",
            ],
            "c_interface": "packs acg+it, demote 1, rerank 1 (standard), prediction 0, abbreviation 0, english 0, "
            "learning 0, colloquial table loaded, rerank job applied after every key",
            "files": record.get("files", {}),
            "cli_runs": self.cli.runs,
            "cli_rowstats_mismatches": self.cli.mismatches,
        }


def root_is_default(root: Path) -> bool:
    return root == SHANJIE.checkout
