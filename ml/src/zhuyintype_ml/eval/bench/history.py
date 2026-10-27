"""The ZhuyinBench pages: docs/benchmarks/README.md (every version side by side) and one page per version, from the
JSON records `zt bench run` writes and the Mac figures attached to each release (VERSION-mac.json, optional).

    zt bench history

A Mac file is either a report of `zt eval latency --json` or the older metrics-mac.json format (machine, macos and
the medians).
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

DIR = paths.DOCS / "benchmarks"
SETS = [
    ("wikipedia", "test_sentences.txt", "維基"),
    ("colloquial", "test_colloquial.txt", "口語"),
    ("smoke", "colloquial_smoke.txt", "日常短句"),
    ("chat_tt", "test_chat_tt.txt", "Taiwan Tongues"),
]


def version_key(record: dict[str, Any]) -> list[int]:
    return [int(x) for x in re.findall(r"\d+", record["version"])]


def pct(value: float | None) -> str:
    return "—" if value is None else f"{value:.1f}%"


def cell(results: dict[str, Any], group: str, file: str, field: str = "whole") -> str:
    row = results.get(group, {}).get(file)
    return pct(row.get(field)) if row else "—"


def load_mac(path: Path) -> dict[str, Any] | None:
    """Mac figures as {machine, macos, median}, from a latency report or the older format."""
    if not path.exists():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    if "sections" in data:
        metrics = data["sections"][0]["metrics"]
        return {"machine": metrics.get("machine"), "macos": metrics.get("macos"), "median": metrics["median"]}
    mac: dict[str, Any] = data
    return mac


def page(record: dict[str, Any], mac: dict[str, Any] | None) -> str:
    r = record["results"]
    lines = [
        f"# ZhuyinBench {record['version']}",
        "",
        f"套件 `{record['suite']}`，commit `{record['commit']}`，{record['date']}，{record['host']}，"
        f"每組最多 {record['n']} 段。"
        "整句全對率；括號內為「台／臺」等寫法視為相同。方法見 [EVALUATION.md](../ml/EVALUATION.md)。",
        "",
        "## 整句轉換（沒有前文）",
        "",
        "| 測試集 | app | 只用引擎 |",
        "|---|---|---|",
    ]
    for _, file, label in SETS:
        app = r.get("conversion/app", {}).get(file, {})
        variant = f"（{pct(app.get('variant_equal'))}）" if app.get("variant_equal") is not None else ""
        lines.append(f"| {label} | {cell(r, 'conversion/app', file)}{variant} | {cell(r, 'conversion/engine', file)} |")
    lines += [
        "",
        "## 前文與接續",
        "",
        "| 項目 | 整句全對率 |",
        "|---|---|",
        f"| 維基，以同一行前面的文字為前文 | {cell(r, 'context/app', 'test_sentences.txt')} |",
        f"| 口語，分兩次送出 | {cell(r, 'split/app', 'test_colloquial.txt')} |",
        f"| 維基，分兩次送出 | {cell(r, 'split/app', 'test_sentences.txt')} |",
        "",
        "## 打字習慣（顯示正確／前 16 句命中）",
        "",
        "| 習慣 | 口語 | 維基 |",
        "|---|---|---|",
    ]
    habits = r.get("habits/app", {})
    for habit in ["clean", "sandhi", "neutral", "nospace", "typo5"]:

        def h(file: str, habit: str = habit) -> str:
            row = habits.get(file, {}).get(habit)
            return f"{row['shown']:.1f}%／{row['recall16']:.1f}%" if row else "—"

        lines.append(f"| {habit} | {h('test_colloquial.txt')} | {h('test_sentences.txt')} |")
    lines += [
        "",
        "## 候選字清單（轉錯的字，正確答案的位置）",
        "",
        "| 測試集 | 第一個 | 第一頁 | 平均名次 |",
        "|---|---|---|---|",
    ]
    for _, file, label in SETS:
        row = r.get("candidates/app", {}).get(file)
        if row and row.get("wrong") == 0:
            lines.append(f"| {label} | — | — | —（沒有轉錯的字） |")
        elif row:
            lines.append(f"| {label} | {pct(row['first'])} | {pct(row['first_page'])} | {row['mean_rank']:.2f} |")
    external = r.get("external/app", {})
    if external:
        lines += [
            "",
            "## 善解的評測集（善解的正規化，整句全對率）",
            "",
            "| 集合 | 段數 | app | 只用引擎 |",
            "|---|---|---|---|",
        ]
        for name, row in external.items():
            engine = r.get("external/engine", {}).get(name, {})
            lines.append(f"| {name} | {row['n']} | {pct(row['top1'])} | {pct(engine.get('top1'))} |")
    lines += [
        "",
        "## 免選字（模擬使用者：錯了就到第一個錯字選正確的候選）",
        "",
        "| 測試集 | 不用選字 | 最多選 1 次 | 最多選 2 次 | 平均選字次數 | 選字改壞其他字 |",
        "|---|---|---|---|---|---|",
    ]
    for _, file, label in SETS:
        row = r.get("repick/app", {}).get(file)
        if row:
            lines.append(
                f"| {label} | {pct(row['picks0'])} | {pct(row['picks1'])} | {pct(row['picks2'])} | "
                f"{row['mean_picks']:.2f} | {pct(row['broke'])} |"
            )
    if mac:
        m = mac["median"]
        # The release runner has no powermetrics, so it reports zero energy.
        energy = f"{m['energy_mj_per_key']} mJ" if m.get("energy_mj_per_key") else "量不到"
        lines += [
            "",
            "## 速度、能耗、記憶體（macOS，app 設定，固定 10 句的中位數）",
            "",
            f"每鍵 p50 {m['p50_ms']} ms、p95 {m['p95_ms']} ms；每鍵 CPU {m['cpu_ms_per_key']} ms、"
            f"能耗{energy if energy == '量不到' else ' ' + energy}；記憶體 {m['memory_mb']} MB"
            f"（{mac.get('machine')}，macOS {mac.get('macos')}）。",
            "",
            "這是發布流程在 GitHub Actions 的 macOS 虛擬機上量的，只適合版本之間互相比較，比實機慢。",
        ]
    return "\n".join(lines) + "\n"


def records(directory: Path = DIR) -> list[dict[str, Any]]:
    found = [
        json.loads(p.read_text(encoding="utf-8")) for p in directory.glob("v*.json") if not p.name.endswith("-mac.json")
    ]
    return sorted(found, key=version_key)


def readme(entries: list[tuple[dict[str, Any], dict[str, Any] | None]]) -> str:
    rows = [
        "# ZhuyinBench 歷代成績",
        "",
        "每次發布都在 GPU 主機上跑同一套評測（`zt bench run`），這裡列出每一版的整句全對率（app 設定，沒有前文），"
        "點版本看完整結果。測試集只用來追蹤，不用來調參數；方法見 [EVALUATION.md](../ml/EVALUATION.md)。",
        "",
        "| 版本 | 日期 | " + " | ".join(label for _, _, label in SETS) + " | 維基有前文 | 一聲不按空白 | 每鍵 p95 |",
        "|---" * (len(SETS) + 5) + "|",
    ]
    for record, mac in entries:
        r = record["results"]
        nospace = r.get("habits/app", {}).get("test_colloquial.txt", {}).get("nospace")
        rows.append(
            f"| [{record['version']}]({record['version']}.md) | {record['date']} | "
            + " | ".join(cell(r, "conversion/app", file) for _, file, _ in SETS)
            + f" | {cell(r, 'context/app', 'test_sentences.txt')} | {pct(nospace['shown']) if nospace else '—'}"
            + f" | {str(mac['median']['p95_ms']) + ' ms' if mac else '—'} |"
        )
    rows += [
        "",
        "和其他注音輸入法在同一批句子上的比較（含條件與限制）見 [competition.md](competition.md)，"
        "各專案的做法見 [相關工作](../research/related-work.md)。",
    ]
    return "\n".join(rows) + "\n"


def main(directory: Path = DIR) -> int:
    """Rewrites every version page and the README from the records in `directory`."""
    entries = []
    for record in records(directory):
        mac = load_mac(directory / f"{record['version']}-mac.json")
        (directory / f"{record['version']}.md").write_text(page(record, mac), encoding="utf-8")
        entries.append((record, mac))
    (directory / "README.md").write_text(readme(entries), encoding="utf-8")
    print(f"{len(entries)} versions")
    return 0
