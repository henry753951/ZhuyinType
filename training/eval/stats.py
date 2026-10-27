#!/usr/bin/env python3
"""Paired JSONL results: exact McNemar, paired bootstrap, break-rate bound and MDE.

  python training/eval/stats.py BASE.jsonl NEW.jsonl --jobs 4 --output stats.json
  python training/eval/stats.py --discordant 61 --alpha .05

Written from the statistical definitions (no external implementation imported).
Differences and fixed/broken are NEW minus BASE. Bootstrap resamples row pairs
with replacement, reports percentile 95% intervals, and uses the same draw for
top-1 and micro CER. Correlated rows require --clusters (one ID per result row).
MDE conditions on the observed/assumed discordant count; it is diagnostic, not
an independent prospective power calculation when calculated after evaluation.
"""
import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import json
import math
from pathlib import Path
import random
import sys


def mcnemar(fixed, broken):
    n = fixed + broken
    coefficient = total = 1
    for k in range(1, min(fixed, broken) + 1):
        coefficient = coefficient * (n - k + 1) // k
        total += coefficient
    return min(1.0, (2 * total) / (1 << n)) if n else 1.0


def poisson_upper(events):
    """Solve P(Poisson(lambda) <= events) = .05 in log space (one-sided 95%)."""
    if events == 0:
        return -math.log(.05)
    factorials = [math.lgamma(i + 1) for i in range(events + 1)]
    low, high = 0.0, float(events + 1)

    def probability(mean):
        return math.fsum(math.exp(i * math.log(mean) - mean - f) for i, f in enumerate(factorials))

    while probability(high) > .05:
        high *= 2
    for _ in range(70):
        mid = (low + high) / 2
        if probability(mid) > .05:
            low = mid
        else:
            high = mid
    return (low + high) / 2


def minimum_detectable(discordant, alpha=.05, power=.8):
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
    coefficients = {k: math.lgamma(discordant + 1) - math.lgamma(k + 1)
                     - math.lgamma(discordant - k + 1) for k in reject}

    def detection(net):
        probability = (discordant + net) / (2 * discordant)
        if probability == 1:
            return 1.0
        return math.fsum(math.exp(coefficients[k] + k * math.log(probability)
                                 + (discordant - k) * math.log1p(-probability)) for k in reject)

    low, high = 0, discordant // 2
    while low < high:
        mid = (low + high) // 2
        if detection(discordant % 2 + 2 * mid) >= power:
            high = mid
        else:
            low = mid + 1
    return discordant % 2 + 2 * low


def read_results(path, normalization):
    rows = []
    for i, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), 1):
        r = json.loads(line)
        ok, errors, length = (r["ok_" + normalization], r["errors_" + normalization], r["gold_len"])
        if (r["row"] != i or not isinstance(r.get("id"), str) or not r["id"]
                or ok not in (0, 1) or type(errors) is not int or errors < 0
                or type(length) is not int or length < 1 or (ok and errors)):
            raise ValueError(f"{path}:{i}: malformed per-row result")
        rows.append((r["id"], ok, errors, length))
    if not rows:
        raise ValueError(f"{path}: no results")
    return rows


def bootstrap_part(task):
    groups, count, start, stop, seed = task
    top, cer = [], []
    # Sampling multiplicities of identical row differences is exactly the row
    # bootstrap; Python 3.12's binomial sampler avoids n Python draws per replicate.
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


def bootstrap(differences, resamples, seed, jobs, clusters=None):
    if clusters is not None:
        grouped = {}
        for cluster, row in zip(clusters, differences):
            sums = grouped.setdefault(cluster, [0, 0, 0, 0])
            for i, value in enumerate(row):
                sums[i] += value
        differences = [tuple(sums) for sums in grouped.values()]
    groups = list(Counter(differences).items())
    jobs = min(jobs, resamples)
    size = (resamples + jobs - 1) // jobs
    tasks = [(groups, len(differences), start, min(resamples, start + size), seed)
             for start in range(0, resamples, size)]
    if jobs == 1:
        parts = [bootstrap_part(tasks[0])]
    else:
        with ProcessPoolExecutor(jobs) as pool:
            parts = list(pool.map(bootstrap_part, tasks))

    def interval(values):
        values.sort()
        return [values[int((resamples - 1) * .025)], values[math.ceil((resamples - 1) * .975)]]
    return (interval([v for top, _ in parts for v in top]),
            interval([v for _, cer in parts for v in cer]))


def compare(base, new, normalization="shanjie", resamples=10000, seed=7, jobs=1,
            alpha=.05, power=.8, clusters=None):
    if not base or len(base) != len(new) or any((b[0], b[3]) != (c[0], c[3]) for b, c in zip(base, new)):
        raise ValueError("result rows do not pair: count, row identity/order or gold length differs")
    n, chars = len(base), sum(b[3] for b in base)
    fixed = sum(not b[1] and c[1] for b, c in zip(base, new))
    broken = sum(b[1] and not c[1] for b, c in zip(base, new))
    differences = [(c[1] - b[1], c[2] - b[2], b[3], 1) for b, c in zip(base, new)]
    top_ci, cer_ci = bootstrap(differences, resamples, seed, jobs, clusters)
    upper = poisson_upper(broken)
    mde = minimum_detectable(fixed + broken, alpha, power)
    return dict(n=n, normalization=normalization, fixed=fixed, broken=broken, net=fixed - broken,
                mcnemar_p=mcnemar(fixed, broken), top1_base=sum(b[1] for b in base) / n,
                top1_new=sum(c[1] for c in new) / n, delta_top1=(fixed - broken) / n,
                delta_top1_ci95=top_ci, cer_base=sum(b[2] for b in base) / chars,
                cer_new=sum(c[2] for c in new) / chars, delta_cer_ci95=cer_ci,
                break_rate=broken / n, broken_poisson_upper95=upper, break_rate_upper95=upper / n,
                discordant=fixed + broken, mde_net=mde, mde_rate=mde / n if mde is not None else None,
                alpha=alpha, power=power, seed=seed, resamples=resamples,
                bootstrap_unit="cluster" if clusters is not None else "row")


def self_test():
    assert mcnemar(6, 0) == .03125 and mcnemar(9, 1) == .021484375
    assert mcnemar(0, 0) == mcnemar(3, 3) == 1
    assert abs(poisson_upper(0) - 2.99573227355) < 1e-9
    assert abs(poisson_upper(1) - 4.74386451839) < 1e-9
    assert minimum_detectable(61, .05) == 23 and minimum_detectable(61, .01) == 27
    assert minimum_detectable(0) is None and minimum_detectable(5) is None
    base = [(str(i), 1, 0, 4) for i in range(10)]
    r = compare(base, base, resamples=100, jobs=1)
    assert r["delta_top1_ci95"] == r["delta_cer_ci95"] == [0, 0]
    changed = [(str(i), int(i >= 3), int(i < 3), 4) for i in range(10)]
    differences = [(c[1] - b[1], c[2] - b[2], 4, 1) for b, c in zip(base, changed)]
    assert bootstrap(differences, 100, 7, 1) == bootstrap(differences, 100, 7, 2)
    assert bootstrap(differences, 100, 7, 1) == bootstrap(differences, 100, 7, 1, list(range(10)))
    try:
        compare(base, list(reversed(base)), resamples=10)
    except ValueError:
        pass
    else:
        raise AssertionError("misordered pairs accepted")
    print("stats self-test passed")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("base", nargs="?", type=Path)
    p.add_argument("new", nargs="?", type=Path)
    p.add_argument("--normalization", choices=["raw", "shanjie", "variants"], default="shanjie")
    p.add_argument("--resamples", type=int, default=10000)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--jobs", type=int, default=1)
    p.add_argument("--alpha", type=float, default=.05)
    p.add_argument("--power", type=float, default=.8)
    p.add_argument("--clusters", type=Path)
    p.add_argument("--discordant", type=int)
    p.add_argument("--output", type=Path)
    p.add_argument("--self-test", action="store_true")
    a = p.parse_args()
    if a.self_test:
        self_test()
        return
    if a.jobs < 1 or a.resamples < 100 or not 0 < a.alpha < 1 or not 0 < a.power < 1:
        p.error("positive jobs, >=100 resamples and alpha/power within (0,1) required")
    if a.discordant is not None:
        if a.discordant < 0 or a.base or a.new:
            p.error("use either nonnegative --discordant or two result files")
        result = dict(discordant=a.discordant, alpha=a.alpha, power=a.power,
                      mde_net=minimum_detectable(a.discordant, a.alpha, a.power))
    else:
        if not a.base or not a.new:
            p.error("two result files required")
        base, new = [read_results(path, a.normalization) for path in (a.base, a.new)]
        clusters = a.clusters.read_text(encoding="utf-8").splitlines() if a.clusters else None
        if clusters is not None and (len(clusters) != len(base) or not all(clusters)):
            p.error("one nonempty cluster ID required per result row")
        result = compare(base, new, a.normalization, a.resamples, a.seed, a.jobs, a.alpha, a.power, clusters)
    text = json.dumps(result, indent=2) + "\n"
    if a.output:
        a.output.parent.mkdir(parents=True, exist_ok=True)
        a.output.write_text(text, encoding="utf-8")
    print(text, end="")


if __name__ == "__main__":
    main()
