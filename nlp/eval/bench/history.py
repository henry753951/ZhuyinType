#!/usr/bin/env python3
"""Rebuilds docs/benchmarks/README.md (every version side by side) and one page per version from the JSON records
written by suite.py and the Mac metrics attached to each release (metrics-mac.json, optional).

    python3 nlp/eval/bench/history.py
"""
import glob
import json
import os
import re

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
DIR = os.path.join(ROOT, "docs", "benchmarks")
SETS = [("wikipedia", "test_sentences.txt", "維基"), ("colloquial", "test_colloquial.txt", "口語"),
        ("smoke", "colloquial_smoke.txt", "日常短句"), ("chat_tt", "test_chat_tt.txt", "Taiwan Tongues")]


def version_key(record):
    return [int(x) for x in re.findall(r"\d+", record["version"])]


def pct(value):
    return "—" if value is None else f"{value:.1f}%"


def cell(results, group, file, field="whole"):
    row = results.get(group, {}).get(file)
    return pct(row.get(field)) if row else "—"


def page(record, mac):
    r = record["results"]
    lines = [f"# ZhuyinBench {record['version']}", "",
             f"套件 `{record['suite']}`，commit `{record['commit']}`，{record['date']}，{record['host']}，每組最多 {record['n']} 段。"
             "整句全對率；括號內為「台／臺」等寫法視為相同。方法見 [EVALUATION.md](../EVALUATION.md)。", "",
             "## 整句轉換（沒有前文）", "", "| 測試集 | app | 只用引擎 |", "|---|---|---|"]
    for _, file, label in SETS:
        app = r.get("conversion/app", {}).get(file, {})
        variant = f"（{pct(app.get('variant_equal'))}）" if app.get("variant_equal") is not None else ""
        lines.append(f"| {label} | {cell(r, 'conversion/app', file)}{variant} | {cell(r, 'conversion/engine', file)} |")
    lines += ["", "## 前文與接續", "", "| 項目 | 整句全對率 |", "|---|---|",
              f"| 維基，以同一行前面的文字為前文 | {cell(r, 'context/app', 'test_sentences.txt')} |",
              f"| 口語，分兩次送出 | {cell(r, 'split/app', 'test_colloquial.txt')} |",
              f"| 維基，分兩次送出 | {cell(r, 'split/app', 'test_sentences.txt')} |",
              "", "## 打字習慣（顯示正確／前 16 句命中）", "", "| 習慣 | 口語 | 維基 |", "|---|---|---|"]
    habits = r.get("habits/app", {})
    for habit in ["clean", "sandhi", "neutral", "nospace", "typo5"]:
        def h(file):
            row = habits.get(file, {}).get(habit)
            return f"{row['shown']:.1f}%／{row['recall16']:.1f}%" if row else "—"
        lines.append(f"| {habit} | {h('test_colloquial.txt')} | {h('test_sentences.txt')} |")
    lines += ["", "## 候選字清單（轉錯的字，正確答案的位置）", "", "| 測試集 | 第一個 | 第一頁 | 平均名次 |", "|---|---|---|---|"]
    for _, file, label in SETS:
        row = r.get("candidates/app", {}).get(file)
        if row:
            lines.append(f"| {label} | {pct(row['first'])} | {pct(row['first_page'])} | {row['mean_rank']:.2f} |")
    external = r.get("external/app", {})
    if external:
        lines += ["", "## 善解的評測集（善解的正規化，整句全對率）", "", "| 集合 | 段數 | app | 只用引擎 |", "|---|---|---|---|"]
        for name, row in external.items():
            engine = r.get("external/engine", {}).get(name, {})
            lines.append(f"| {name} | {row['n']} | {pct(row['top1'])} | {pct(engine.get('top1'))} |")
    lines += ["", "## 免選字（模擬使用者：錯了就到第一個錯字選正確的候選）", "",
              "| 測試集 | 不用選字 | 最多選 1 次 | 最多選 2 次 | 平均選字次數 | 選字改壞其他字 |", "|---|---|---|---|---|---|"]
    for _, file, label in SETS:
        row = r.get("repick/app", {}).get(file)
        if row:
            lines.append(f"| {label} | {pct(row['picks0'])} | {pct(row['picks1'])} | {pct(row['picks2'])} | "
                         f"{row['mean_picks']:.2f} | {pct(row['broke'])} |")
    if mac:
        m = mac["median"]
        lines += ["", "## 速度、能耗、記憶體（macOS，app 設定，固定 10 句的中位數）", "",
                  f"每鍵 p50 {m['p50_ms']} ms、p95 {m['p95_ms']} ms；每鍵 CPU {m['cpu_ms_per_key']} ms、"
                  f"能耗 {m['energy_mj_per_key']} mJ；記憶體 {m['memory_mb']} MB（{mac.get('machine')}，macOS {mac.get('macos')}）。"]
    return "\n".join(lines) + "\n"


def main():
    records = sorted((json.load(open(p, encoding="utf-8")) for p in glob.glob(os.path.join(DIR, "v*.json"))
                      if not p.endswith("-mac.json")), key=version_key)
    rows = ["# ZhuyinBench 歷代成績", "",
            "每次發布都在 GPU 主機上跑同一套評測（`nlp/eval/bench/suite.py`），這裡列出每一版的整句全對率（app 設定，沒有前文），"
            "點版本看完整結果。測試集只用來追蹤，不用來調參數；方法見 [EVALUATION.md](../EVALUATION.md)。", "",
            "| 版本 | 日期 | " + " | ".join(label for _, _, label in SETS) + " | 維基有前文 | 一聲不按空白 | 每鍵 p95 |",
            "|---" * (len(SETS) + 5) + "|"]
    for record in records:
        mac_path = os.path.join(DIR, f"{record['version']}-mac.json")
        mac = json.load(open(mac_path, encoding="utf-8")) if os.path.exists(mac_path) else None
        with open(os.path.join(DIR, f"{record['version']}.md"), "w", encoding="utf-8") as f:
            f.write(page(record, mac))
        r = record["results"]
        nospace = r.get("habits/app", {}).get("test_colloquial.txt", {}).get("nospace")
        rows.append(f"| [{record['version']}]({record['version']}.md) | {record['date']} | "
                    + " | ".join(cell(r, "conversion/app", file) for _, file, _ in SETS)
                    + f" | {cell(r, 'context/app', 'test_sentences.txt')} | {pct(nospace['shown']) if nospace else '—'}"
                    + f" | {str(mac['median']['p95_ms']) + ' ms' if mac else '—'} |")
    competitors = os.path.join(DIR, "competitors.md")
    if os.path.exists(competitors):
        rows += ["", "其他輸入法在同一批句子上的結果見 [competitors.md](competitors.md)。"]
    with open(os.path.join(DIR, "README.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(rows) + "\n")
    print(f"{len(records)} versions")


if __name__ == "__main__":
    main()
