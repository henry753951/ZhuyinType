"""Paired comparison of two results on the same items: exact McNemar, paired bootstrap, break-rate bound and MDE.

    zt eval stats base.json new.json                  # two reports of the same evaluation (zt eval ... --json)
    zt eval stats base.json new.json --section wiki-dev --normalization variants --jobs 4 --output stats.json
    zt eval stats BASE.jsonl NEW.jsonl                # per-row results of the former rows_eval.py
    zt eval stats BASE.tsv NEW.tsv                    # reference<TAB>output dumps of the former conversion_eval.py
    zt eval stats --discordant 61 --alpha .05         # minimum detectable effect for a discordant count

Reports pair their items by id within each section (and item group, e.g. one typo rate and mode); every section and
group present in both is compared.  An item is right when its output equals its reference under the normalization
(raw, variants: our interchangeable forms, shanjie: 善解's common-character folds); its errors are 0 when right, else
the Levenshtein distance; rows reports carry these per normalization.  The default normalization is shanjie for row
results and raw otherwise.

Written from the statistical definitions (no external implementation imported).  Differences and fixed/broken are
NEW minus BASE.  The bootstrap resamples item pairs with replacement, reports percentile 95% intervals, and uses the
same draw for top-1 and micro CER.  Correlated items require --clusters (one ID per item, in base order).  MDE
conditions on the observed or assumed discordant count; after an evaluation it is diagnostic, not an independent
prospective power calculation.
"""

from __future__ import annotations

import argparse
import json
import math
import random
from collections import Counter
from collections.abc import Callable
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import Any

from nlp.common.zhuyin import VARIANTS
from nlp.eval.rows import edit_distance, normalizer

# One paired outcome: item id, right (0/1), character errors, reference length.
Outcome = tuple[str, int, int, int]
Difference = tuple[int, int, int, int]
NORMALIZATIONS = ("raw", "shanjie", "variants")


def mcnemar(fixed: int, broken: int) -> float:
    n = fixed + broken
    coefficient = total = 1
    for k in range(1, min(fixed, broken) + 1):
        coefficient = coefficient * (n - k + 1) // k
        total += coefficient
    return min(1.0, (2 * total) / (1 << n)) if n else 1.0


def poisson_upper(events: int) -> float:
    """Solve P(Poisson(lambda) <= events) = .05 in log space (one-sided 95%)."""
    if events == 0:
        return -math.log(0.05)
    factorials = [math.lgamma(i + 1) for i in range(events + 1)]
    low, high = 0.0, float(events + 1)

    def probability(mean: float) -> float:
        return math.fsum(math.exp(i * math.log(mean) - mean - f) for i, f in enumerate(factorials))

    while probability(high) > 0.05:
        high *= 2
    for _ in range(70):
        mid = (low + high) / 2
        if probability(mid) > 0.05:
            low = mid
        else:
            high = mid
    return (low + high) / 2


def minimum_detectable(discordant: int, alpha: float = 0.05, power: float = 0.8) -> int | None:
    """Smallest feasible net with exact two-sided rejection probability >= power."""
    if discordant == 0:
        return None
    critical = -1
    for k in range(discordant // 2 + 1):
        if mcnemar(k, discordant - k) > alpha:
            break
        critical = k
    if critical < 0:
        return None
    reject = list(range(critical + 1)) + list(range(discordant - critical, discordant + 1))
    coefficients = {
        k: math.lgamma(discordant + 1) - math.lgamma(k + 1) - math.lgamma(discordant - k + 1) for k in reject
    }

    def detection(net: int) -> float:
        probability = (discordant + net) / (2 * discordant)
        if probability == 1:
            return 1.0
        return math.fsum(
            math.exp(coefficients[k] + k * math.log(probability) + (discordant - k) * math.log1p(-probability))
            for k in reject
        )

    low, high = 0, discordant // 2
    while low < high:
        mid = (low + high) // 2
        if detection(discordant % 2 + 2 * mid) >= power:
            high = mid
        else:
            low = mid + 1
    return discordant % 2 + 2 * low


def read_results(path: Path | str, normalization: str) -> list[Outcome]:
    """Per-row results of the former rows_eval.py (JSONL, rows numbered from 1)."""
    rows = []
    for i, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), 1):
        r = json.loads(line)
        ok, errors, length = (r["ok_" + normalization], r["errors_" + normalization], r["gold_len"])
        if (
            r["row"] != i
            or not isinstance(r.get("id"), str)
            or not r["id"]
            or ok not in (0, 1)
            or type(errors) is not int
            or errors < 0
            or type(length) is not int
            or length < 1
            or (ok and errors)
        ):
            raise ValueError(f"{path}:{i}: malformed per-row result")
        rows.append((r["id"], ok, errors, length))
    if not rows:
        raise ValueError(f"{path}: no results")
    return rows


def fold(normalization: str) -> Callable[[str], str]:
    if normalization == "variants":
        return lambda s: s.translate(VARIANTS)
    return normalizer() if normalization == "shanjie" else str


def outcome(item: dict[str, Any], normalization: str, fold_text: Callable[[str], str]) -> Outcome:
    """An item's paired outcome: the precomputed one of row results, else from its reference and output."""
    if "ok_" + normalization in item:
        return (str(item["id"]), item["ok_" + normalization], item["errors_" + normalization], item["gold_len"])
    if "reference" not in item or "output" not in item:
        raise ValueError(f"item {item.get('id')!r} has no reference and output to compare")
    want, got = item["reference"], item["output"]
    ok = fold_text(got) == fold_text(want)
    return (str(item["id"]), int(ok), 0 if ok else edit_distance(want, got), len(want))


def has_rows(path: Path) -> bool:
    """Whether a result file holds row results (precomputed outcomes per normalization)."""
    if path.suffix == ".jsonl":
        return True
    if path.suffix == ".json":
        sections = json.loads(path.read_text(encoding="utf-8")).get("sections", [])
        return any("ok_shanjie" in item for s in sections for item in s.get("items", [])[:1])
    return False


def load(path: Path, normalization: str) -> dict[str, list[Outcome]]:
    """Outcomes per comparison (section, or section and item group) of a result file."""
    if path.suffix == ".jsonl":
        return {"": read_results(path, normalization)}
    fold_text = fold(normalization)
    if path.suffix == ".tsv":
        items = []
        for i, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
            reference, output = line.split("\t", 1)
            items.append({"id": i, "reference": reference, "output": output})
        return {"": [outcome(item, normalization, fold_text) for item in items]}
    data = json.loads(path.read_text(encoding="utf-8"))
    if "sections" not in data:
        raise ValueError(f"{path}: not a report (zt eval ... --json), .jsonl or .tsv result")
    found: dict[str, list[Outcome]] = {}
    for section in data["sections"]:
        for item in section.get("items", []):
            name = section["name"] + (f" {item['group']}" if item.get("group") else "")
            found.setdefault(name, []).append(outcome(item, normalization, fold_text))
    return found


def pair(base: list[Outcome], new: list[Outcome], name: str) -> list[Outcome]:
    """`new` in the order of `base`: as it is when the ids already line up (rows may repeat an id), else matched by
    item id."""
    if [o[0] for o in base] == [o[0] for o in new]:
        return new
    by_id = {o[0]: o for o in new}
    if len(by_id) != len(new) or len({o[0] for o in base}) != len(base):
        raise ValueError(f"{name or 'results'}: item ids are not unique")
    missing = [o[0] for o in base if o[0] not in by_id]
    if missing or len(base) != len(new):
        raise ValueError(
            f"{name or 'results'}: items do not pair ({len(base)} and {len(new)} items, {len(missing)} base ids "
            "missing from new); compare runs of the same evaluation with the same sets, -n and seed"
        )
    return [by_id[o[0]] for o in base]


def bootstrap_part(
    task: tuple[list[tuple[Difference, int]], int, int, int, int],
) -> tuple[list[float], list[float]]:
    groups, count, start, stop, seed = task
    top, cer = [], []
    # Sampling multiplicities of identical row differences is exactly the row bootstrap; Python 3.12's binomial
    # sampler avoids n Python draws per replicate.
    weights = [weight for _, weight in groups]
    for replicate in range(start, stop):
        rng = random.Random(seed + replicate)
        total_ok = total_errors = total_length = total_rows = 0
        if hasattr(rng, "binomialvariate"):
            left, mass = count, count
            drawn = []
            for weight in weights[:-1]:
                value = rng.binomialvariate(left, weight / mass)
                drawn.append(value)
                left -= value
                mass -= weight
            drawn.append(left)
        else:
            counts = Counter(rng.choices(range(len(groups)), weights=weights, k=count))
            drawn = [counts[i] for i in range(len(groups))]
        for ((ok, errors, length, rows), _), repetitions in zip(groups, drawn):
            total_ok += ok * repetitions
            total_errors += errors * repetitions
            total_length += length * repetitions
            total_rows += rows * repetitions
        top.append(total_ok / total_rows)
        cer.append(total_errors / total_length)
    return top, cer


def bootstrap(
    differences: list[Difference], resamples: int, seed: int, jobs: int, clusters: list[str] | None = None
) -> tuple[list[float], list[float]]:
    if clusters is not None:
        grouped: dict[str, list[int]] = {}
        for cluster, row in zip(clusters, differences):
            sums = grouped.setdefault(cluster, [0, 0, 0, 0])
            for i, value in enumerate(row):
                sums[i] += value
        differences = [(s[0], s[1], s[2], s[3]) for s in grouped.values()]
    groups = list(Counter(differences).items())
    jobs = min(jobs, resamples)
    size = (resamples + jobs - 1) // jobs
    tasks = [
        (groups, len(differences), start, min(resamples, start + size), seed) for start in range(0, resamples, size)
    ]
    if jobs == 1:
        parts = [bootstrap_part(tasks[0])]
    else:
        with ProcessPoolExecutor(jobs) as pool:
            parts = list(pool.map(bootstrap_part, tasks))

    def interval(values: list[float]) -> list[float]:
        values.sort()
        return [values[int((resamples - 1) * 0.025)], values[math.ceil((resamples - 1) * 0.975)]]

    return (interval([v for top, _ in parts for v in top]), interval([v for _, cer in parts for v in cer]))


def compare(
    base: list[Outcome],
    new: list[Outcome],
    normalization: str = "shanjie",
    resamples: int = 10000,
    seed: int = 7,
    jobs: int = 1,
    alpha: float = 0.05,
    power: float = 0.8,
    clusters: list[str] | None = None,
) -> dict[str, Any]:
    if not base or len(base) != len(new) or any((b[0], b[3]) != (c[0], c[3]) for b, c in zip(base, new)):
        raise ValueError("result rows do not pair: count, row identity/order or gold length differs")
    n, chars = len(base), sum(b[3] for b in base)
    fixed = sum(not b[1] and c[1] for b, c in zip(base, new))
    broken = sum(b[1] and not c[1] for b, c in zip(base, new))
    differences = [(c[1] - b[1], c[2] - b[2], b[3], 1) for b, c in zip(base, new)]
    top_ci, cer_ci = bootstrap(differences, resamples, seed, jobs, clusters)
    upper = poisson_upper(broken)
    mde = minimum_detectable(fixed + broken, alpha, power)
    return {
        "n": n,
        "normalization": normalization,
        "fixed": fixed,
        "broken": broken,
        "net": fixed - broken,
        "mcnemar_p": mcnemar(fixed, broken),
        "top1_base": sum(b[1] for b in base) / n,
        "top1_new": sum(c[1] for c in new) / n,
        "delta_top1": (fixed - broken) / n,
        "delta_top1_ci95": top_ci,
        "cer_base": sum(b[2] for b in base) / chars,
        "cer_new": sum(c[2] for c in new) / chars,
        "delta_cer_ci95": cer_ci,
        "break_rate": broken / n,
        "broken_poisson_upper95": upper,
        "break_rate_upper95": upper / n,
        "discordant": fixed + broken,
        "mde_net": mde,
        "mde_rate": mde / n if mde is not None else None,
        "alpha": alpha,
        "power": power,
        "seed": seed,
        "resamples": resamples,
        "bootstrap_unit": "cluster" if clusters is not None else "row",
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("base", nargs="?", type=Path, help="base result: report .json, rows .jsonl or .tsv dump")
    parser.add_argument("new", nargs="?", type=Path, help="new result of the same kind")
    parser.add_argument(
        "--normalization", choices=NORMALIZATIONS, help="when an item is right (default: shanjie for rows, else raw)"
    )
    parser.add_argument(
        "--section", nargs="+", metavar="NAME", help="compare only these sections or groups (default: all)"
    )
    parser.add_argument("--resamples", type=int, default=10000, help="bootstrap resamples (default: 10000)")
    parser.add_argument("--seed", type=int, default=7, help="bootstrap seed (default: 7)")
    parser.add_argument("--jobs", type=int, default=1, help="bootstrap processes (default: 1)")
    parser.add_argument("--alpha", type=float, default=0.05)
    parser.add_argument("--power", type=float, default=0.8)
    parser.add_argument("--clusters", type=Path, help="one cluster id per item, in base order (one comparison only)")
    parser.add_argument("--discordant", type=int, help="only the minimum detectable effect for this discordant count")
    parser.add_argument("--output", type=Path, help="also write the result JSON here")


def run(args: argparse.Namespace) -> int:
    def error(message: str) -> int:
        raise SystemExit(f"zt eval stats: {message}")

    if args.jobs < 1 or args.resamples < 100 or not 0 < args.alpha < 1 or not 0 < args.power < 1:
        return error("positive --jobs, at least 100 --resamples and --alpha/--power within (0, 1) required")
    result: dict[str, Any]
    if args.discordant is not None:
        if args.discordant < 0 or args.base or args.new:
            return error("use either a nonnegative --discordant or two result files")
        result = {
            "discordant": args.discordant,
            "alpha": args.alpha,
            "power": args.power,
            "mde_net": minimum_detectable(args.discordant, args.alpha, args.power),
        }
    else:
        if not args.base or not args.new:
            return error("two result files required")
        normalization = args.normalization or ("shanjie" if has_rows(args.base) else "raw")
        base, new = load(args.base, normalization), load(args.new, normalization)
        names = [
            name
            for name in base
            if name in new and (not args.section or name.split(" ")[0] in args.section or name in args.section)
        ]
        if not names:
            return error(f"no section in common (base: {', '.join(base) or '—'}; new: {', '.join(new) or '—'})")
        clusters = args.clusters.read_text(encoding="utf-8").splitlines() if args.clusters else None
        if clusters is not None and (len(names) != 1 or len(clusters) != len(base[names[0]]) or not all(clusters)):
            return error("--clusters needs one comparison and one nonempty cluster id per item")
        try:
            results = {
                name: compare(
                    base[name],
                    pair(base[name], new[name], name),
                    normalization,
                    args.resamples,
                    args.seed,
                    args.jobs,
                    args.alpha,
                    args.power,
                    clusters,
                )
                for name in names
            }
        except ValueError as problem:
            return error(str(problem))
        result = results[names[0]] if names == [""] else results
    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0
