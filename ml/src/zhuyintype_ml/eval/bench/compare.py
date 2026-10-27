"""The comparison with other input methods: every system on every set in every situation, one record, one page.

    zt systems setup                                    # once per host: the other systems, pinned and built
    zt --remote --export --detach bench compare         # on the GPU host: run (resumable), write the record
    zt bench compare --render                           # docs/benchmarks/competition.md from the record

The matrix (tasks × systems):

- Tasks are sets in situations.  Text sets (ours) are sampled as every evaluation samples them (Han runs of 4–20
  characters, seed 7, the first n with readings from our lexicon): plain; with the text before the run on its line
  (`context`); committed in two parts cut at the word boundary nearest the middle (`split`); keyboard slips at 10% per
  syllable (`typo`, zt eval typo's generator); timed typing errors per condition (`typing/<condition>`, zt eval
  typing-errors' generator).  Row sets (善解's eval/ at its pinned commit, every set, plus the tracked subsets with
  rows overlapping our training text removed) are typed as given, after their context.  0c's public regression cases
  (testdata/cases) are typed with their own readings where they give one.
- Systems (zhuyintype_ml/eval/systems): ZhuyinType (app setup, engine only), 0c (statistical, full with its D512
  model), 善解 (chat, formal).  Every system receives the same physical keys; 善解's CLI receives the same readings
  for everything typed exactly.  A system that takes no arbitrary text before the run gets none: the cell is N/A for
  `context`, and for row sets it counts the contexts it did not get.

Each (system, task) cell is saved as soon as it is done under data/build/bench/compare/<label>/cells/, so an
interrupted run continues where it stopped; identical inputs within a system are converted once (cache/).  The record
(data/build/bench/compare/<label>/record.json) holds every task's references and every system's output for every
item; the tracked record (default docs/benchmarks/competition.json) keeps only what the page is made from: the
metrics with both normalizations (S: 善解's, V: ours), paired exact McNemar tests against ZhuyinType's app setup,
latency summaries and versions.
"""

from __future__ import annotations

import csv
import datetime
import hashlib
import json
import os
import platform
import random
import re
import subprocess
import threading
import time
from collections import Counter
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.core.engine import EngineConfig
from zhuyintype_ml.core.sets import BY_NAME, EvalSet
from zhuyintype_ml.core.zhuyin import KEYS, VARIANTS, Lexicon, keys, sample_runs
from zhuyintype_ml.eval import systems as registry
from zhuyintype_ml.eval.rows import edit_distance, load_rows, normalizer
from zhuyintype_ml.eval.stats import mcnemar
from zhuyintype_ml.eval.systems.base import Case, Segment, System, Task
from zhuyintype_ml.eval.systems.references import SHANJIE, ZEROC
from zhuyintype_ml.eval.typing_errors import CONDITIONS, condition, seed_key, seeded, timed_script
from zhuyintype_ml.eval.typo import noisy_script

SUITE = "compare-1"
SEED = 7
TEXT_SETS = [
    "wiki-test",
    "wiki-dev",
    "colloquial-test",
    "colloquial-dev",
    "chat-test",
    "chat-dev",
    "smoke",
    "zai-test",
    "zai-dev",
]
CONTEXT_SETS = [s for s in TEXT_SETS if s != "smoke"]
SPLIT_SETS = ["wiki-test", "colloquial-test", "chat-test"]
SLIP_SETS = ["colloquial-test", "wiki-test"]
TYPO_RATE = 0.1
CLEAN_ROWS = paths.EVAL_SETS / "external" / "shanjie" / "eval"
BASE = "zhuyintype-app"
# Cases typed for a capability a system does not declare: enough to show the behaviour, not a full measurement.
SAMPLE = 50
NORMALIZATIONS = ("S", "V")


def default_dir(label: str) -> Path:
    return paths.data("bench", "compare", label)


# MARK: - Tasks


def run_case(i: int, text: str, readings: list[str], before: str = "") -> Case:
    return Case(str(i), text, (Segment(keys(readings), tuple(readings)),), before.replace("\t", " ").replace("\n", " "))


def text_tasks(eval_set: EvalSet, lexicon: Lexicon, n: int, typing_n: int) -> list[Task]:
    name = eval_set.name
    runs = sample_runs(eval_set.file, lexicon, n, SEED)
    plain = [run_case(i, r.text, r.readings, r.before) for i, r in enumerate(runs)]
    tasks = [Task(name, "plain", "zhuyintype", plain)]
    if name in CONTEXT_SETS:
        tasks.append(Task(name, "context", "zhuyintype", plain, uses_context=True))
    if name in SPLIT_SETS:
        cases: list[Case] = []
        for run in sample_runs(eval_set.file, lexicon, 10**9, SEED):
            if len(cases) >= n:
                break
            cuts = [c for c in run.cuts[:-1] if 2 <= c <= len(run.text) - 2]
            if not cuts:
                continue
            cut = min(cuts, key=lambda c: abs(c - len(run.text) / 2))
            first, second = run.readings[:cut], run.readings[cut:]
            segments = (Segment(keys(first), tuple(first)), Segment(keys(second), tuple(second)))
            cases.append(Case(str(len(cases)), run.text, segments))
        tasks.append(Task(name, "split", "zhuyintype", cases))
    if name in SLIP_SETS:
        rng = random.Random(SEED)
        typo = [
            Case(str(i), r.text, (Segment(noisy_script(r.readings, TYPO_RATE, rng).removesuffix("<enter>")),))
            for i, r in enumerate(runs)
        ]
        tasks.append(Task(name, f"typo/{TYPO_RATE:g}", "zhuyintype", typo))
        whole = sample_runs(eval_set.file, lexicon, typing_n, SEED, whole_readings=True)
        key = seed_key(eval_set)
        for kind in CONDITIONS:
            cases = []
            for index, run in enumerate(whole):
                result = condition(run.readings, kind, seeded(SEED, key, index, kind, "errors"))
                if result is None:
                    continue
                strokes, fast, _ = result
                timing = "exact" if kind == "exact-fast" else kind
                script = timed_script(strokes, seeded(SEED, key, index, timing, "timing"), fast)
                cases.append(Case(str(index), run.text, (Segment(script),)))
            tasks.append(Task(name, f"typing/{kind}", "zhuyintype", cases))
    return tasks


def row_task(name: str, path: Path, description: str = "") -> Task:
    cases = [
        Case(str(row["row"]), row["truth"], (Segment(row["keys"], tuple(row["syllables"].split())),), row["context"])
        for row in load_rows(path)
    ]
    task = Task(name, "rows", "shanjie", cases, description, uses_context=True)
    task.info["source"] = description
    return task


def shanjie_tasks() -> list[Task]:
    """善解's sets at its pinned commit (from `zt systems setup`), then the tracked subsets without the rows that
    overlap our training text (named NAME.clean)."""
    rows = SHANJIE.root / "rows"
    manifest = json.loads((rows / "manifest.json").read_text(encoding="utf-8"))
    tasks = [
        row_task(entry["name"], rows / f"{entry['name'].replace('/', '--')}.txt", entry["source"])
        for entry in manifest["sets"]
    ]
    for path in sorted(CLEAN_ROWS.rglob("*.clean.rows.txt")):
        relative = path.relative_to(CLEAN_ROWS).as_posix().removesuffix(".clean.rows.txt")
        tasks.append(row_task(f"sj/{relative}.clean", path, paths.relative(path)))
    return tasks


def zeroc_task(lexicon: Lexicon) -> Task:
    """0c's public regression cases (testdata/cases at its pinned commit): the wanted text with the reading the case
    gives (first-choice, context and field cases; field cases also with their alternative typing), or our lexicon's
    reading (confusion pairs, which give the sentence only).  Cases with readings that do not align are left out."""
    cases: list[Case] = []
    skipped: list[str] = []
    directory = ZEROC.checkout / "testdata" / "cases"
    for file in ("first_choice_cases.tsv", "context_cases.tsv", "field_cases.tsv", "confusion_pairs.tsv"):
        with open(directory / file, encoding="utf-8") as f:
            for row in csv.DictReader(f, delimiter="\t"):
                entries: list[tuple[str, list[str] | None, str]]
                if file == "confusion_pairs.tsv":
                    entries = [(row[f"sentence_{s}"].replace("＿", row[f"answer_{s}"]), None, s) for s in "ab"]
                elif file == "field_cases.tsv":
                    entries = [(row["want"], row["typed"].split(), "typed")]
                    if row.get("alt_typed"):
                        entries.append((row["want"], row["alt_typed"].split(), "alt"))
                else:
                    entries = [(row["want"], row["reading"].split(), "native")]
                for text, readings, kind in entries:
                    text = "".join(re.findall(r"[一-鿿]", text))
                    readings = readings or lexicon.readings(text)
                    ident = f"{file.removesuffix('.tsv')}/{row['id']}/{kind}"
                    valid = bool(readings) and readings is not None and len(readings) == len(text)
                    if not valid or not all(c in KEYS for r in readings or [] for c in r):
                        skipped.append(ident)
                        continue
                    assert readings is not None
                    cases.append(Case(ident, text, (Segment(keys(readings), tuple(readings)),)))
    task = Task("0c-public", "plain", "0c", cases, "testdata/cases")
    task.info["skipped"] = skipped
    return task


def build_tasks(n: int = 400, typing_n: int = 300, data: Path | None = None) -> list[Task]:
    lexicon = Lexicon(EngineConfig("engine", data or paths.data_dir()).reading_lexicon)
    tasks: list[Task] = []
    for name in TEXT_SETS:
        tasks += text_tasks(BY_NAME[name], lexicon, n, typing_n)
    tasks.append(zeroc_task(lexicon))
    tasks += shanjie_tasks()
    return tasks


def latency_scripts(lexicon: Lexicon) -> list[str]:
    sentences = BY_NAME["latency"].file.read_text(encoding="utf-8").split()
    return [keys(r) + "<enter>" for r in map(lexicon.readings, sentences) if r]


# MARK: - Running


def slug(task_id: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]", "_", task_id)


def request_key(system: System, task: Task, case: Case) -> str:
    """Identical requests to one system give one outcome: what the system is actually given."""
    context = case.context if task.uses_context and system.context else ""
    body = [context, [s.keys for s in case.segments], [s.readings for s in case.segments], task.keys_only]
    return hashlib.sha256(json.dumps(body, ensure_ascii=False).encode()).hexdigest()


class Store:
    """The run's files: a cell per (system, task), a cache of outcomes per system, latency and descriptions."""

    def __init__(self, root: Path) -> None:
        self.root = root

    def cell(self, system: str, task_id: str) -> Path:
        return self.root / "cells" / system / f"{slug(task_id)}.json"

    def read(self, path: Path) -> Any:
        return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None

    def write(self, path: Path, value: Any) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(".tmp")
        temporary.write_text(json.dumps(value, ensure_ascii=False) + "\n", encoding="utf-8")
        temporary.replace(path)

    def cache(self, system: str) -> dict[str, dict[str, Any]]:
        path = self.root / "cache" / f"{system}.jsonl"
        found: dict[str, dict[str, Any]] = {}
        if path.exists():
            for line in path.read_text(encoding="utf-8").splitlines():
                if line.strip():
                    entry = json.loads(line)
                    found[entry["key"]] = entry
        return found

    def remember(self, system: str, entries: list[dict[str, Any]]) -> None:
        path = self.root / "cache" / f"{system}.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "a", encoding="utf-8") as f:
            for entry in entries:
                f.write(json.dumps(entry, ensure_ascii=False) + "\n")


def run_cell(system: System, task: Task, cache: dict[str, dict[str, Any]], store: Store) -> dict[str, Any]:
    """One (system, task) cell.  A task that needs a capability the system does not declare (apart from context,
    which is N/A) is typed on its first SAMPLE cases only and marked unsupported."""
    reason = system.unsupported(task)
    if reason:
        return {"status": "n/a", "reason": reason}
    missing = system.missing(task)
    cases = task.cases[:SAMPLE] if missing else task.cases
    keys_ = [request_key(system, task, case) for case in cases]
    todo: dict[str, Case] = {}
    for key, case in zip(keys_, cases):
        if key not in cache and key not in todo:
            todo[key] = case
    started = time.monotonic()
    if todo:
        outcomes = system.convert(task, list(todo.values()))
        entries: list[dict[str, Any]] = [
            {"key": key, "output": o.output, "status": o.status, **({"info": o.info} if o.info else {})}
            for key, o in zip(todo, outcomes)
        ]
        store.remember(system.name, entries)
        cache.update({e["key"]: e for e in entries})
    found = [cache[key] for key in keys_]
    cell: dict[str, Any] = {
        "status": "done",
        "outputs": [e["output"] for e in found],
        "statuses": {str(i): e["status"] for i, e in enumerate(found) if e["status"] != "ok"},
        "seconds": round(time.monotonic() - started, 1),
        "converted": len(todo),
    }
    if missing:
        cell["unsupported"] = missing
    if task.uses_context and not system.context:
        cell["context_dropped"] = sum(bool(case.context) for case in cases)
    states = Counter(e.get("info", {}).get("selection_state") for e in found)
    states.pop(None, None)
    if states:
        cell["selection_states"] = dict(states)
    changed = sum(e.get("info", {}).get("enter_changed_display", 0) for e in found)
    if changed:
        cell["enter_changed_display"] = changed
    return cell


# Systems whose processes start once and stay (0c's servers); the others take a new share before every task.
FIXED = ("0c-statistical", "0c-full")
# Relative amount of work per case: ZhuyinType's app setup runs its neural models in every engine process.
WEIGHTS = {"zhuyintype-app": 3}


class Budget:
    """Processes shared by the systems that run at once.  0c's servers count from when they are running (two
    processes each with the model: the server and its model worker; a 0c run waiting for a quiet host takes
    nothing); the other systems divide the rest by WEIGHTS, and take more as others finish."""

    def __init__(self, total: int, names: list[str], overrides: dict[str, int] | None = None) -> None:
        self.total = total
        self.lock = threading.Lock()
        self.overrides = overrides or {}
        even = max(1, total // max(1, len(names)))
        self.servers = {
            n: self.overrides.get(n, max(1, even // 2) if n == "0c-full" else even) for n in names if n in FIXED
        }
        self.running: set[str] = set()
        self.active = {n for n in names if n not in FIXED}

    def begin(self, name: str) -> None:
        with self.lock:
            if name in self.servers:
                self.running.add(name)

    def cores(self, name: str) -> int:
        return self.servers[name] * (2 if name == "0c-full" else 1)

    def share(self, name: str) -> int:
        with self.lock:
            if name in self.servers:
                return self.servers[name]
            if name in self.overrides:
                return self.overrides[name]
            spare = self.total - sum(self.cores(n) for n in self.running)
            weights = sum(WEIGHTS.get(n, 1) for n in self.active)
            return max(1, spare * WEIGHTS.get(name, 1) // max(1, weights))

    def release(self, name: str) -> None:
        with self.lock:
            self.running.discard(name)
            self.servers.pop(name, None)
            self.active.discard(name)


def convert_system(
    label: str, system: System, tasks: list[Task], store: Store, only: Callable[[Task], bool], budget: Budget
) -> None:
    pending = [t for t in tasks if only(t) and not store.cell(system.name, t.id).exists()]
    system.start()
    budget.begin(system.name)
    try:
        cache = store.cache(system.name)
        for task in pending:
            system.resize(budget.share(system.name))
            cell = run_cell(system, task, cache, store)
            store.write(store.cell(system.name, task.id), cell)
            print(f"[{label}] {system.name} {task.id}: {summarize(task, cell)}", flush=True)
        describe = {"label": system.label, "family": system.family, "capabilities": sorted(system.capabilities)}
        store.write(store.root / "systems" / f"{system.name}.json", {**describe, **system.describe()})
    finally:
        system.close()
        budget.release(system.name)


def run_matrix(
    label: str,
    names: list[str],
    tasks: list[Task],
    store: Store,
    workers: dict[str, int],
    total: int,
    only: Callable[[Task], bool] = lambda _task: True,
) -> None:
    """The systems with cells to do run at the same time, each in its own thread with its own processes, `total`
    processes in all; then per-key latency, one system at a time so they do not compete."""
    pending = [
        name
        for name in names
        if any(only(t) and not store.cell(name, t.id).exists() for t in tasks)
        or not (store.root / "systems" / f"{name}.json").exists()
    ]
    budget = Budget(total, pending, workers)
    print(f"[{label}] converting with {', '.join(f'{n} × {budget.share(n)}' for n in pending) or 'nothing to do'}")
    # Created here, one after another: building zhuyin-cli and loading what the adapters need is not thread-safe.
    systems = [registry.create(name, workers=budget.share(name), jobs=budget.share(name)) for name in pending]
    with ThreadPoolExecutor(max(1, len(systems))) as pool:
        for future in [pool.submit(convert_system, label, s, tasks, store, only, budget) for s in systems]:
            future.result()
    lexicon = Lexicon(EngineConfig("engine").reading_lexicon)
    for name in names:
        path = store.root / "latency" / f"{name}.json"
        if path.exists():
            continue
        system = registry.create(name, workers=1, jobs=1)
        system.start()
        try:
            load = os.getloadavg()
            result = system.latency(latency_scripts(lexicon))
            store.write(path, {"result": result, "load_before": load, "load_after": os.getloadavg()})
            print(f"[{label}] {name} latency: {result and result['median']} (load {load[0]:.0f})", flush=True)
        finally:
            system.close()


def summarize(task: Task, cell: dict[str, Any]) -> str:
    if cell["status"] != "done":
        return cell["reason"]
    right = sum(a == b for a, b in zip(cell["outputs"], (c.reference for c in task.cases)))
    return f"{right}/{len(cell['outputs'])} raw, {cell['converted']} converted in {cell['seconds']} s"


# MARK: - The record


def folds(variants: Path | None) -> dict[str, Callable[[str], str]]:
    return {"S": normalizer(variants), "V": lambda s: s.translate(VARIANTS), "raw": str}


def metrics(
    references: list[str], outputs: list[str], statuses: dict[str, str], fold: dict[str, Callable[[str], str]]
) -> dict[str, Any]:
    n = len(references)
    if not n:
        return {"n": 0}
    result: dict[str, Any] = {"n": n}
    for name, f in fold.items():
        result[name] = sum(f(a) == f(b) for a, b in zip(references, outputs)) / n
    errors = sum(edit_distance(a, b) for a, b in zip(references, outputs))
    result["character"] = max(0.0, 1 - errors / max(1, sum(map(len, references))))
    for status, count in Counter(statuses.values()).items():
        result[status] = count
    return result


def paired(
    references: list[str], base: list[str], other: list[str], fold: dict[str, Callable[[str], str]]
) -> dict[str, dict[str, Any]]:
    result = {}
    for name in NORMALIZATIONS:
        f = fold[name]
        right_base = [f(a) == f(b) for a, b in zip(references, base)]
        right_other = [f(a) == f(b) for a, b in zip(references, other)]
        only_base = sum(a and not b for a, b in zip(right_base, right_other))
        only_other = sum(b and not a for a, b in zip(right_base, right_other))
        result[name] = {"only_base": only_base, "only_other": only_other, "p": mcnemar(only_base, only_other)}
    return result


def app_version() -> str:
    """The app version in Info.plist (the version this checkout would release as)."""
    plist = (paths.CLIENT / "Resources" / "Info.plist").read_text(encoding="utf-8")
    found = re.search(r"<key>CFBundleShortVersionString</key>\s*<string>([^<]+)</string>", plist)
    return found.group(1) if found else ""


def commit() -> str:
    """This checkout's commit; $ZT_COMMIT where the checkout has no git metadata (a work directory `zt --remote`
    synced), with a `+` when the comparison ran with changes not yet committed."""
    done = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=paths.ROOT, capture_output=True, text=True)
    return done.stdout.strip() or os.environ.get("ZT_COMMIT", "")


def assemble(label: str, names: list[str], tasks: list[Task], store: Store, n: int, typing_n: int) -> dict[str, Any]:
    variants = SHANJIE.checkout / "eval" / "variants.tsv"
    fold = folds(variants if variants.exists() else None)
    record: dict[str, Any] = {
        "suite": SUITE,
        "label": label,
        "date": datetime.date.today().isoformat(),
        "commit": commit(),
        "zhuyintype_version": app_version(),
        "host": f"{platform.system()} {platform.machine()}, {os.cpu_count()} cores",
        "n": n,
        "typing_n": typing_n,
        "seed": SEED,
        "normalizations": {
            "S": "善解: 她妳它牠嘗周臺裏 → 他你他他嚐週台裡, then its eval/variants.tsv (longest match)"
            + ("" if variants.exists() else " (variants.tsv missing: common characters only)"),
            "V": "ZhuyinType: 臺妳裏爲衆啓峯綫着麪牀 → 台你裡為眾啟峰線著麵床",
        },
        "systems": {},
        "references": {},
        "tasks": {},
        "latency": {},
    }
    for name in names:
        describe = store.read(store.root / "systems" / f"{name}.json")
        if describe:
            describe["capabilities"] = describe.get("capabilities") or sorted(registry.capabilities(name))
            record["systems"][name] = describe
        latency = store.read(store.root / "latency" / f"{name}.json")
        if latency:
            record["latency"][name] = latency
    for reference in (ZEROC, SHANJIE):
        if reference.setup_file.exists():
            record["references"][reference.name] = reference.setup_record()
    for task in tasks:
        references = [c.reference for c in task.cases]
        entry: dict[str, Any] = {
            "set": task.set,
            "situation": task.situation,
            "builder": task.builder,
            "n": len(task.cases),
            "with_context": sum(bool(c.context) for c in task.cases) if task.uses_context else 0,
            "ids": [c.id for c in task.cases],
            "references": references,
            "cells": {},
        }
        if task.info:
            entry["info"] = task.info
        if task.keys_only:
            entry["keys_sha256"] = hashlib.sha256("\n".join(c.keys for c in task.cases).encode()).hexdigest()
        for name in names:
            cell = store.read(store.cell(name, task.id))
            if cell is None:
                entry["cells"][name] = {"status": "missing"}
                continue
            capabilities = record["systems"].get(name, {}).get("capabilities") or registry.capabilities(name)
            missing = [c for c in task.requires if c not in capabilities]
            if cell["status"] == "done" and missing and "context" not in missing:
                # Not something the system is designed to do: reported on the sample only, however much was typed.
                cell["outputs"] = cell["outputs"][:SAMPLE]
                cell["statuses"] = {k: v for k, v in cell["statuses"].items() if int(k) < SAMPLE}
                cell["unsupported"] = missing
            if cell["status"] == "done":
                size = len(cell["outputs"])
                cell["metrics"] = metrics(references[:size], cell["outputs"], cell["statuses"], fold)
            entry["cells"][name] = cell
        base = entry["cells"].get(BASE, {})
        if base.get("status") == "done":
            entry["paired"] = {
                name: paired(
                    references[: len(cell["outputs"])], base["outputs"][: len(cell["outputs"])], cell["outputs"], fold
                )
                for name, cell in entry["cells"].items()
                if name != BASE and cell.get("status") == "done"
            }
            for name, found in entry["paired"].items():
                if entry["cells"][name].get("unsupported"):
                    for result in found.values():
                        result["sample"] = len(entry["cells"][name]["outputs"])  # descriptive only
        record["tasks"][task.id] = entry
    return record


def run(
    label: str,
    names: list[str],
    out: Path,
    n: int = 400,
    typing_n: int = 300,
    budget: int = 16,
    workers: dict[str, int] | None = None,
    tasks_filter: str | None = None,
) -> dict[str, Any]:
    if not platform.system() == "Linux":
        raise SystemExit("zt bench compare: evaluations run on the GPU host (zt --remote bench compare)")
    store = Store(default_dir(label))
    tasks = build_tasks(n, typing_n)
    pattern = re.compile(tasks_filter) if tasks_filter else None
    only: Callable[[Task], bool] = (lambda t: bool(pattern.search(t.id))) if pattern else (lambda _t: True)
    print(f"[{label}] {len(tasks)} tasks, {sum(len(t.cases) for t in tasks)} cases, systems: {', '.join(names)}")
    run_matrix(label, names, tasks, store, workers or {}, budget, only)
    record = assemble(label, names, tasks, store, n, typing_n)
    record["process_budget"] = budget
    full = store.root / "record.json"
    full.write_text(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"wrote {paths.relative(full)} ({full.stat().st_size / 1e6:.1f} MB, with every output)")
    write_slim(record, out)
    return record


def write_slim(record: dict[str, Any], out: Path) -> None:
    """The tracked record: numbers only (competition.slim), which the page is generated from."""
    from zhuyintype_ml.eval.bench.competition import slim

    out.parent.mkdir(parents=True, exist_ok=True)
    # One task per line: small, and a rerun's diff shows which tasks changed.
    body = slim(record)
    tasks = body.pop("tasks")
    head = json.dumps(body, ensure_ascii=False, indent=1)[:-2]
    lines = [
        f"  {json.dumps(k, ensure_ascii=False)}: {json.dumps(v, ensure_ascii=False, separators=(',', ':'))}"
        for k, v in tasks.items()
    ]
    out.write_text(head + ',\n "tasks": {\n' + ",\n".join(lines) + "\n }\n}\n", encoding="utf-8")
    print(f"wrote {paths.relative(out)} ({out.stat().st_size / 1e3:.0f} KB)")
