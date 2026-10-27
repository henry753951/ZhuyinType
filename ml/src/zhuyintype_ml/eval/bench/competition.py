"""docs/benchmarks/competition.md, written from the comparison record (zhuyintype_ml/eval/bench/compare.py).

    zt bench compare --render                            # from docs/benchmarks/competition.json
    zt bench compare --render --record other.json --page /tmp/page.md

The page is generated, never edited by hand: the summary sentences, every table and the counts in the limits come
from the record.  Wording follows docs/product/WRITING.md (related work: conditions and limits, no ranking words);
the best number of a row is only set in bold.
"""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

PAGE = paths.DOCS / "benchmarks" / "competition.md"
RECORD = paths.DOCS / "benchmarks" / "competition.json"
ORDER = ["zhuyintype-app", "zhuyintype-engine", "0c-statistical", "0c-full", "shanjie-chat", "shanjie-formal"]
SHORT = {
    "zhuyintype-app": "ZhuyinType",
    "zhuyintype-engine": "只用引擎",
    "0c-statistical": "靈犀（統計）",
    "0c-full": "靈犀（完整）",
    "shanjie-chat": "善解（聊天）",
    "shanjie-formal": "善解（正式）",
}
BUILDERS = {"zhuyintype": "ZhuyinType 建立", "0c": "靈犀注音建立", "shanjie": "善解建立"}
SET_LABELS = {
    "wiki-test": "維基，測試",
    "wiki-dev": "維基，開發",
    "colloquial-test": "口語，測試",
    "colloquial-dev": "口語，開發",
    "chat-test": "對話（Taiwan Tongues），測試",
    "chat-dev": "對話（Taiwan Tongues），開發",
    "smoke": "日常短句",
    "zai-test": "在／再，測試",
    "zai-dev": "在／再，開發",
    "0c-public": "靈犀注音公開的回歸案例",
    "sj/bench/suite-v1/dev302": "dev302",
    "sj/bench/suite-v1/probe": "probe（一、不的變調）",
    "sj/bench/suite-v1/typing76": "typing76",
    "sj/bench/suite-v2/fw": "虛詞（fw）",
    "sj/bench/suite-v2/user-reported": "使用者回報的錯誤句",
    "sj/dev/acg/character-parts": "ACG 角色名的一部分",
    "sj/dev/acg/characters": "ACG 角色名",
    "sj/dev/acg/interpunct": "ACG 含間隔號的名稱",
    "sj/dev/acg/recent": "ACG 近期作品",
    "sj/dev/acg/works": "ACG 作品名",
    "sj/dev/existing": "既有詞",
    "sj/dev/holdout-2026-10-03/holdout": "保留集（2026-10-03）",
    "sj/dev/homophones": "同音詞",
    "sj/dev/it/words": "IT 詞彙",
    "sj/dev/names/creators": "創作者名",
    "sj/dev/names/names": "人名",
    "sj/dev/names/stores": "店名",
    "sj/dev/net-slang/round2": "網路用語（第二輪）",
    "sj/dev/net-slang/words": "網路用語",
    "sj/dev/neutral/words": "輕聲詞",
    "sj/dev/oov": "詞庫外的詞",
    "sj/dev/user-reported": "使用者回報的錯誤句（開發）",
    "sj/holdout/news": "保留集：新聞",
    "sj/holdout/talk": "保留集：對話",
    "sj/holdout/next/shard-2/news": "保留集第 2 批：新聞",
    "sj/holdout/next/shard-2/talk": "保留集第 2 批：對話",
    "sj/holdout/next/shard-3/news": "保留集第 3 批：新聞",
    "sj/holdout/next/shard-3/talk": "保留集第 3 批：對話",
    "sj/learn/newword-boundary-teach": "新詞（教學句）",
    "sj/learn/newword-probe-boundary": "新詞邊界",
    "sj/learn/newword-probe": "新詞",
    "sj/learn/cases": "學習案例（不學習）",
    "sj/sets/daily": "日常句（daily）",
    "sj/sets/moedict": "教育部辭典例句",
    "sj/sets/trap": "陷阱句（trap）",
}
CONDITION_LABELS = {
    "exact": "沒有打錯",
    "exact-fast": "沒有打錯，打得快",
    "swap-in-syllable": "音節內兩鍵對調",
    "swap-across": "跨音節兩鍵對調",
    "swap-across-2": "跨音節對調兩處",
    "rollover": "幾乎同時按下而順序顛倒",
    "rollover-2": "順序顛倒兩處",
    "hotspot": "按到相鄰的鍵",
    "hotspot-2": "按到相鄰的鍵兩處",
    "extra-key": "多按一個相鄰的鍵",
    "missing-key": "漏按一鍵",
    "tone": "聲調 3／4、6／7 按錯",
    "nospace-all": "一聲都不按空白",
    "nospace-30": "三成的一聲不按空白",
    "mixed": "以上混合，低比例",
}


def spaced(label: str) -> str:
    """A label followed by Chinese text: a half-width space after Latin letters and digits (WRITING.md)."""
    return label + " " if label[-1:].isascii() and label[-1:].isalnum() else label


def set_label(name: str) -> str:
    if name.endswith(".clean"):
        return set_label(name.removesuffix(".clean")) + "，去除重疊"
    return SET_LABELS.get(name, name)


class View:
    """Read access to the record."""

    def __init__(self, record: dict[str, Any]) -> None:
        self.record = record
        self.tasks: dict[str, Any] = record["tasks"]
        self.systems = [s for s in ORDER if s in record["systems"]]

    def cell(self, task_id: str, system: str) -> dict[str, Any] | None:
        task = self.tasks.get(task_id)
        return task["cells"].get(system) if task else None

    def value(self, task_id: str, system: str, norm: str) -> float | None:
        cell = self.cell(task_id, system)
        if not cell or cell.get("status") != "done":
            return None
        value: float = cell["metrics"][norm]
        return value

    def pair(self, task_id: str, system: str, norm: str) -> dict[str, Any] | None:
        found: dict[str, Any] | None = self.tasks.get(task_id, {}).get("paired", {}).get(system, {}).get(norm)
        return found

    def ids(self, situation: str, builder: str | None = None) -> list[str]:
        return [
            i
            for i, t in self.tasks.items()
            if t["situation"] == situation and (builder is None or t["builder"] == builder)
        ]


def pct(value: float | None) -> str:
    return "—" if value is None else f"{100 * value:.1f}"


def p_text(p: float) -> str:
    if p < 0.001:
        return "< 0.001"
    return f"{p:.3f}".rstrip("0").rstrip(".") if p < 1 else "1"


def pair_text(found: dict[str, Any] | None) -> str:
    if not found or found.get("sample"):
        return "—"
    return f"{found['only_base']}／{found['only_other']}；{p_text(found['p'])}"


def bold_row(values: list[float | None]) -> list[str]:
    """Each value with one decimal; the largest (at that precision) in bold."""
    shown = [pct(v) for v in values]
    numbers = [round(100 * v, 1) for v in values if v is not None]
    best = max(numbers) if numbers else None
    return [
        f"**{text}**" if v is not None and best is not None and round(100 * v, 1) == best else text
        for v, text in zip(values, shown)
    ]


def sv_cells(view: View, task_id: str, systems: list[str]) -> list[str]:
    """`S／V` per system, the best S and the best V of the row in bold; N/A where the system cannot run the task;
    「不支援」 with the result on its sample where the task needs something the system is not designed to do (never
    in bold: a sample is not a measurement of the whole set)."""

    def supported(name: str) -> bool:
        return not (view.cell(task_id, name) or {}).get("unsupported")

    s = bold_row([view.value(task_id, name, "S") if supported(name) else None for name in systems])
    v = bold_row([view.value(task_id, name, "V") if supported(name) else None for name in systems])
    result = []
    for name, a, b in zip(systems, s, v):
        cell = view.cell(task_id, name) or {}
        if cell.get("status") == "n/a":
            result.append("N/A")
        elif cell.get("unsupported") and cell.get("status") == "done":
            sample = f"{pct(view.value(task_id, name, 'S'))}／{pct(view.value(task_id, name, 'V'))}"
            result.append(f"不支援（{cell['metrics']['n']} 句：{sample}）")
        elif a == "—":
            result.append("—")
        else:
            result.append(f"{a}／{b}")
    return result


def table(header: list[str], rows: list[list[str]], align: list[str] | None = None) -> list[str]:
    align = align or ["---"] + ["---:"] * (len(header) - 1)
    return ["| " + " | ".join(header) + " |", "|" + "|".join(align) + "|", *("| " + " | ".join(r) + " |" for r in rows)]


def tally(view: View, ids: list[str], other: str, norm: str) -> tuple[int, int, int]:
    """Sets where ZhuyinType (app) is significantly higher, significantly lower, or not significantly different."""
    higher = lower = same = 0
    for task_id in ids:
        found = view.pair(task_id, other, norm)
        if not found:
            continue
        if found["p"] < 0.05:
            if found["only_base"] > found["only_other"]:
                higher += 1
            else:
                lower += 1
        else:
            same += 1
    return higher, lower, same


def spread(view: View, ids: list[str], system: str, norm: str) -> str:
    values = [v for v in (view.value(i, system, norm) for i in ids) if v is not None]
    if not values:
        return "—"
    low, high = min(values), max(values)
    return f"{100 * low:.1f}%" if round(low, 3) == round(high, 3) else f"{100 * low:.1f}–{100 * high:.1f}%"


def tally_text(view: View, ids: list[str], other: str, norm: str) -> str:
    higher, lower, same = tally(view, ids, other, norm)
    parts = []
    if higher:
        parts.append(f"ZhuyinType 顯著較高的 {higher} 組")
    if lower:
        parts.append(f"{SHORT[other]}顯著較高的 {lower} 組")
    if same:
        parts.append(f"差異不顯著的 {same} 組")
    return "、".join(parts) or "沒有可比較的集合"


def differences(view: View, ids: list[str], other: str, norm: str, count: int) -> list[tuple[str, float, float, float]]:
    """The significant differences, largest first: (set, ZhuyinType, other, p)."""
    found = []
    for task_id in ids:
        pair = view.pair(task_id, other, norm)
        base, value = view.value(task_id, "zhuyintype-app", norm), view.value(task_id, other, norm)
        if pair and pair["p"] < 0.05 and base is not None and value is not None:
            found.append((view.tasks[task_id]["set"], base, value, pair["p"]))
    found.sort(key=lambda x: -abs(x[1] - x[2]))
    return found[:count]


def example_text(items: list[tuple[str, float, float, float]], other: str) -> str:
    return "、".join(
        f"{set_label(name)} {100 * base:.1f}% 對 {100 * value:.1f}%（p {p_text(p)}）" for name, base, value, p in items
    )


# MARK: - Sections


def summary(view: View) -> list[str]:
    ours_plain = [i for i in view.ids("plain", "zhuyintype")]
    shanjie_rows = [i for i in view.ids("rows", "shanjie") if not i.endswith(".clean")]
    lines = ["## 摘要", ""]
    systems = [s for s in view.systems if s.startswith(("0c", "shanjie"))]
    if ours_plain:
        lines.append(
            f"- ZhuyinType 建立的 {len(ours_plain)} 組句子（沒有前文，V）：ZhuyinType 為 "
            f"{spread(view, ours_plain, 'zhuyintype-app', 'V')}。"
            + "；".join(f"和{SHORT[s]}相比，{tally_text(view, ours_plain, s, 'V')}" for s in systems)
            + "。"
        )
    for other in ("shanjie-chat", "shanjie-formal"):
        if other not in view.systems or not shanjie_rows:
            continue
        lower = [d for d in differences(view, shanjie_rows, other, "S", 40) if d[2] > d[1]][:3]
        higher = [d for d in differences(view, shanjie_rows, other, "S", 40) if d[1] > d[2]][:3]
        counted = tally_text(view, shanjie_rows, other, "S")
        line = f"- 善解建立的 {len(shanjie_rows)} 組句子（S）：和{SHORT[other]}相比，{counted}。"
        if lower:
            line += f"{SHORT[other]}較高、差距最大的是{example_text(lower, other)}。"
        if higher:
            line += f"ZhuyinType 較高、差距最大的是{example_text(higher, other)}。"
        lines.append(line)
    public = "plain/0c-public"
    if public in view.tasks:
        parts = [
            f"{SHORT[s]} {pct(view.value(public, s, 'V'))}%"
            for s in view.systems
            if view.value(public, s, "V") is not None
        ]
        lines.append(f"- 靈犀注音公開的 {view.tasks[public]['n']} 個回歸案例（V）：" + "、".join(parts) + "。")
    context = view.ids("context")
    gains = []
    for system in ("zhuyintype-app", "shanjie-chat", "shanjie-formal"):
        before = [view.value(i.replace("context/", "plain/", 1), system, "V") for i in context]
        after = [view.value(i, system, "V") for i in context]
        pairs = [(a, b) for a, b in zip(before, after) if a is not None and b is not None]
        if pairs:
            delta = sum(b - a for a, b in pairs) / len(pairs)
            gains.append(f"{spaced(SHORT[system])}平均 {100 * delta:+.1f} 個百分點")
    if gains:
        lines.append(
            f"- 加上同一行前面的文字（{len(context)} 組，V）："
            + "、".join(gains)
            + "。靈犀注音的協定不接受任意的前文（N/A）。"
        )
    typo = view.ids("typo/0.1")
    if typo:
        values = "、".join(
            f"{set_label(view.tasks[i]['set'])} {pct(view.value(i, 'zhuyintype-app', 'V'))}%"
            f"（沒有打錯時 {pct(view.value(i.replace('typo/0.1/', 'plain/'), 'zhuyintype-app', 'V'))}%）"
            for i in typo
        )
        lines.append(
            f"- 每音節 10% 的機率打錯一個鍵時（V），ZhuyinType 為{values}。靈犀注音與善解沒有按鍵容錯的設計，"
            "這些情境標為「不支援」，只輸入 50 句作為樣本；這反映的是功能範圍，不是同一功能做得好壞。"
        )
    lines.append("")
    return lines


def overview(view: View) -> list[str]:
    systems = view.systems
    lines = [
        "## 總覽",
        "",
        "整句全對率（%），每格是 S／V（兩種寫法正規化，"
        "見[方法](#方法)）。依建立者分組；我們的集合與靈犀的案例沒有前文，"
        "善解的集合照原樣帶前文輸入。每列 S 與 V 各自最高的數字以粗體標示。",
        "",
    ]
    header = ["測試集", "句數", *[SHORT[s] for s in systems]]
    for builder, title in BUILDERS.items():
        ids = [i for i, t in view.tasks.items() if t["builder"] == builder and t["situation"] in ("plain", "rows")]
        if not ids:
            continue
        rows = [[set_label(view.tasks[i]["set"]), str(view.tasks[i]["n"]), *sv_cells(view, i, systems)] for i in ids]
        lines += [f"### {title}", "", *table(header, rows), ""]
    return lines


def setups(view: View) -> list[str]:
    record = view.record
    zeroc = record.get("references", {}).get("0c", {})
    shanjie = record.get("references", {}).get("shanjie", {})
    rows = [
        [
            "ZhuyinType",
            f"引擎 commit `{record['commit']}`，App 版本號 {record.get('zhuyintype_version', '')}",
            "和 App 相同：`--config app`（`zhuyin-cli --models`）。"
            "另列「只用引擎」：只有詞庫與語言模型（`--config engine`）",
        ],
        [
            "靈犀注音（0c）",
            f"commit `{zeroc.get('commit', '')[:7]}`，{zeroc.get('release', '')} 發布的詞庫與模型",
            "統計引擎；完整設定（加上 D512 選字模型，預設 100 ms 時限，"
            "每個程序一個推論執行緒）。標準鍵盤，混打、補標點、"
            "不打聲調關閉",
        ],
        [
            "善解（shanjie）",
            f"commit `{shanjie.get('commit', '')[:7]}`，發布的模型與詞包（ACG、IT）",
            "聊天配置（含口語項）；正式配置。小模型重排開啟（標準強度）。讀音經由它的評測程式 `shanjie-eval`；"
            "含打錯的按鍵經由它 App 使用的 C 介面，以未修改的原始碼建成共享程式庫（cdylib）呼叫",
        ],
    ]
    lines = ["## 比較的對象與設定", "", *table(["", "版本", "設定"], rows, ["---", "---", "---"]), ""]
    lines += [
        f"- 所有系統都在同一台主機（{record['host']}）上以 CPU 執行，"
        "每句從新的組字開始，不學習（學習不會影響下一句）。",
        "- 三方都只執行對方未修改的程式：ZhuyinType 用 `zhuyin-cli`；"
        "靈犀注音用它的 `0c-server`，經由它文件記載的 socket 協定"
        "逐鍵送出；善解用它的評測程式 `shanjie-eval`，"
        "以及它的 App 使用的 C 介面（`core/include/shanjie.h`，原始碼未修改，"
        "建置成共享程式庫）。`zt systems setup` 依固定的 commit 取得並建置，所有檔案都不放進本專案。",
        "- 靈犀注音的程式是 AGPL-3.0，只建置、執行，沒有複製任何程式碼。"
        "善解的評測集依各檔的授權（多為 CC0）讀取，模型檔不放進本專案。",
    ]
    if record.get("process_budget"):
        lines.append(
            f"- 各系統同時執行，共用 {record['process_budget']} 個單執行緒程序（靈犀的選字模型另有一個推論程序），"
            "以較低的優先權（nice 10）和主機上的其他工作共用 CPU；每鍵延遲在轉換結束後逐一量測。"
        )
    lines.append("")
    return lines


def method(view: View) -> list[str]:
    record = view.record
    return [
        "## 方法",
        "",
        "- 指標是整句全對率：一段輸出和參考答案完全相同才算對。",
        "- 寫法正規化，兩種都套用在所有系統上：",
        "  - S：善解的正規化（她妳它牠嘗周臺裏的合併，加上它的異體字表 `eval/variants.tsv`）。",
        "  - V：ZhuyinType 的 11 組寫法（臺／台、裏／裡等）。",
        f"- 我們的集合：每組依固定的種子（{record['seed']}）抽出 4–20 字的漢字段落，"
        f"最多 {record['n']} 段，讀音取 ZhuyinType "
        "詞庫最可能的讀音，換成標準鍵盤的按鍵，一聲按空白，最後按 Enter。善解的集合用各列給的讀音；其中沒有讀音的"
        "（trap、daily、教育部辭典例句）由善解自己的讀音工具產生。"
        "靈犀的案例用案例給的讀音，沒有讀音的由 ZhuyinType 的詞庫產生。",
        "- 每個系統收到同樣的按鍵：ZhuyinType 與靈犀注音逐鍵輸入；善解的評測程式收到同樣的讀音。含打錯的按鍵沒有讀音，"
        "善解改由它的 C 介面逐鍵輸入，設定比照它的 App，每鍵之後讓小模型的重排完成再按下一鍵。",
        "- 情境：",
        "  - 沒有前文；",
        "  - 前文：同一行在這段之前的文字（最多 32 字）。靈犀注音的協定只接受它自己送出的文字作為前文，標為 N/A；"
        "善解的集合照原樣帶前文時，靈犀注音收到的是沒有前文的同一列（表中註明筆數）；",
        "  - 分兩次送出：在最接近中間的詞界切開，先送出前半，再接著輸入後半；後半的前文是已送出的前半。"
        "靈犀注音只對可以學習的送出保留前文，所以這個情境每一句都重新啟動一個空的靈犀程序；",
        f"  - 每音節 {100 * 0.1:.0f}% 的機率打錯一個鍵（`zt eval typo` 的產生方式）；",
        "  - 14 種有時間的打錯情境（`zt eval typing-errors`，"
        f"每組最多 {record['typing_n']} 段）。按鍵間隔只有 ZhuyinType 使用，"
        "其他系統收到同樣順序的按鍵。",
        "- 成對比較用精確 McNemar 檢定，對象都是 ZhuyinType（App 設定）：只看兩邊一對一錯的句子。表中「成對」一欄是"
        "「只有 ZhuyinType 對／只有對方對」的句數與 p 值；p < 0.05 才視為差異顯著。p 值沒有做多重比較校正，只供參考。",
        "",
    ]


def family_section(view: View, title: str, others: list[str], paired_with: str, intro: str) -> list[str]:
    systems = ["zhuyintype-app", "zhuyintype-engine", *others]
    systems = [s for s in systems if s in view.systems]
    lines = [f"## {title}", "", intro, ""]
    header = ["測試集", "句數", *[SHORT[s] for s in systems], f"成對 S（對{SHORT[paired_with]}）", "成對 V"]
    groups: list[tuple[str, list[str]]] = [
        ("沒有前文", [i for i in view.ids("plain")]),
        ("照原樣（帶前文）", [i for i in view.ids("rows")]),
        ("有前文", view.ids("context")),
        ("分兩次送出", view.ids("split")),
    ]
    for name, ids in groups:
        if not ids:
            continue
        rows = []
        for task_id in ids:
            task = view.tasks[task_id]
            n = str(task["n"])
            dropped = (view.cell(task_id, "0c-full") or {}).get("context_dropped")
            if dropped and paired_with.startswith("0c"):
                n += f"（其中 {dropped} 列的前文沒有給靈犀）"
            rows.append(
                [
                    set_label(task["set"]),
                    n,
                    *sv_cells(view, task_id, systems),
                    pair_text(view.pair(task_id, paired_with, "S")),
                    pair_text(view.pair(task_id, paired_with, "V")),
                ]
            )
        lines += [f"### {name}", "", *table(header, rows), ""]
    return lines


def slips_section(view: View) -> list[str]:
    systems = view.systems
    ids = view.ids("typo/0.1") + [i for i in view.tasks if i.startswith("typing/")]
    if not ids:
        return []
    lines = [
        "## 按錯鍵",
        "",
        "同一批按鍵（含打錯）送給每個系統；整句全對率（%），S／V。成對比較對靈犀（完整）與善解（聊天），V。"
        "靈犀注音與善解沒有按鍵容錯的設計，這些數字反映的是功能範圍，不是同一功能做得好壞。",
        "",
    ]
    header = ["情境", "測試集", "句數", *[SHORT[s] for s in systems], "成對（對靈犀完整）", "成對（對善解聊天）"]
    rows = []
    for task_id in ids:
        task = view.tasks[task_id]
        situation = task["situation"]
        name = (
            "每音節 10% 打錯"
            if situation.startswith("typo/")
            else CONDITION_LABELS.get(situation.split("/", 1)[1], situation)
        )
        rows.append(
            [
                name,
                set_label(task["set"]),
                str(task["n"]),
                *sv_cells(view, task_id, systems),
                pair_text(view.pair(task_id, "0c-full", "V")),
                pair_text(view.pair(task_id, "shanjie-chat", "V")),
            ]
        )
    return [*lines, *table(header, rows, ["---", "---", *["---:"] * (len(header) - 2)]), ""]


def latency_section(view: View) -> list[str]:
    latency = view.record.get("latency", {})
    if not latency:
        return []
    rows = []
    loads: list[float | None] = []
    for system in view.systems:
        found = (latency.get(system) or {}).get("result")
        if not found:
            continue
        median = found["median"]
        extra = ""
        if "engine" in found:
            extra = f"引擎內 p50 {found['engine']['p50_ms']}、p95 {found['engine']['p95_ms']} ms"
        if "with_rerank" in found:
            extra = f"加上重排 p50 {found['with_rerank']['p50_ms']}、p95 {found['with_rerank']['p95_ms']} ms"
        load = (latency[system].get("load_before") or [None])[0]
        extra = "；".join(x for x in (extra, f"負載 {load:.0f}" if load is not None else "") if x)
        rows.append([SHORT[system], str(median["p50_ms"]), str(median["p95_ms"]), str(median["max_ms"]), extra])
        loads.append(latency[system].get("load_before", [None])[0])
    measured = [
        "ZhuyinType：`zhuyin-cli --timing`，引擎程序內處理每一鍵的時間（含神經模型）",
        "靈犀注音：經由 socket 一問一答的來回時間；備註是 0c 自己回報的引擎時間（engine_us，含選字模型的等待）",
        "善解：經由 C 介面呼叫按鍵的時間；它的 App 在另一個執行緒跑小模型的重排，備註是加上重排的時間",
    ]
    known = [x for x in loads if x is not None]
    load = f"量測時主機的 1 分鐘負載為 {min(known):.0f}–{max(known):.0f}（其他工作同時執行）。" if known else ""
    return [
        "## 每鍵延遲",
        "",
        "**這些數字是在共用、忙碌的主機上量的，不能互相比較，只是暫時的紀錄；之後會在空閒的主機上另外量測，取代這一節。**",
        "",
        "延遲集的 10 句，一個程序、一句一句逐鍵輸入，取每句每鍵時間的 p50、p95 與最大值，再取各句的中位數（毫秒）。"
        f"各系統的量法也不相同。{load}",
        "",
        *[f"- {text}" for text in measured],
        "",
        *table(["", "p50", "p95", "最大", "備註"], rows, ["---", "---:", "---:", "---:", "---"]),
        "",
    ]


def limits(view: View) -> list[str]:
    no_path: dict[str, int] = {}
    for task in view.tasks.values():
        for system, cell in task["cells"].items():
            if cell.get("status") == "done":
                no_path[system] = no_path.get(system, 0) + int(cell["metrics"].get("no-path", 0))
    states: dict[str, int] = {}
    for task in view.tasks.values():
        for state, count in (task["cells"].get("0c-full") or {}).get("selection_states", {}).items():
            states[state] = states.get(state, 0) + count
    lines = [
        "## 限制",
        "",
        "- 每一方在自己建立的集合上都有優勢：我們的集合和我們的訓練資料分布相近；"
        "善解的集合反映它的詞包與它在開發時遇到的錯誤；"
        "靈犀的案例是它的回歸測試。",
        "- 讀音由各集合的建立者產生（我們的集合用 ZhuyinType 的詞庫，善解的集合用善解的），可能偏向建立者支援的讀音。",
    ]
    if any(no_path.values()):
        parts = "、".join(f"{SHORT[s]} {n} 筆" for s, n in no_path.items() if n)
        lines.append(
            f"- 讀音不在詞庫裡、找不到解碼路徑的句子計為錯誤：{parts}（所有情境合計，同一句在不同情境重複計算）。"
        )
    if states:
        parts = "、".join(f"{k} {v}" for k, v in sorted(states.items(), key=lambda x: -x[1]))
        lines.append(
            "- 靈犀的完整設定在每句結束時的選字模型狀態（所有情境合計）："
            f"{parts}。少於一半的句子由模型選字的格子，等主機負載下降後重跑（最多兩次）；"
            "其餘退回統計引擎的句子照實計入。"
        )
    lines += [
        "- 善解的評測集中，有些列和我們的訓練資料重疊；"
        "去除重疊的子集（「去除重疊」）沿用先前的重疊檢查，這次沒有對最新的訓練資料重新檢查。",
        "- 善解有兩種輸入方式：讀音經由評測程式，按鍵經由 C 介面。兩者的設定一致，但不是同一段程式；「沒有打錯」情境"
        "（C 介面）和「沒有前文」（評測程式）可以對照兩者的差異。",
        "- 「不支援」的格子是對方沒有這項設計的情境，只輸入 50 句作為樣本，不做成對檢定，也不標粗體。"
        "注音按鍵依聲母、介音、韻母的位置組成音節，所以音節內的對調在任何注音輸入法多半仍能打對；"
        "其他的打錯則需要容錯的設計。",
        "- 按鍵間隔只有 ZhuyinType 使用；其他系統沒有這項輸入。",
        "- 靈犀注音有選用的「不打聲調」，這次維持它的預設（關閉），所以一聲不按空白的情境也標為不支援。",
        "- 這是一次量測，各方的版本與設定都會更新。靈犀注音用的是 v0.2.6 的發布資料，不代表它最新的版本。",
        "- 只比整句全對率與每鍵延遲。記憶體、補全、補標點、不打聲調等功能沒有比較；各專案公布的數字見 "
        "[相關工作](../research/related-work.md#設計上的差異)。",
        "",
    ]
    return lines


def reproduction(view: View) -> list[str]:
    return [
        "## 重現",
        "",
        "```sh",
        "tools/zt --remote systems setup                     # 依固定的 commit 取得、建置靈犀注音與善解",
        "tools/zt --remote --nice 10 --export --detach \\",
        "  --push data/build/homophone2/test_zai.txt --push data/build/homophone2/dev_zai.txt \\",
        "  bench compare                                     # 全部系統 × 全部集合；中斷後再執行會接著做",
        "tools/zt remote pull docs/benchmarks/competition.json",
        "tools/zt bench compare --render                     # 由紀錄檔產生這一頁",
        "```",
        "",
        "紀錄檔 [competition.json](competition.json) 保存這一頁的數字：每格兩種正規化的整句全對率、句數、成對檢定、"
        "延遲摘要與各系統的版本。每個系統對每一句的輸出保存在完整的紀錄檔（`data/build/bench/compare/main/record.json`，"
        "不放進版本庫）。做法見 `ml/src/zhuyintype_ml/eval/systems/README.md`。",
        "",
    ]


def render(record: dict[str, Any]) -> str:
    view = View(record)
    lines = [
        "# 與其他注音輸入法的比較",
        "",
        "這一頁把 ZhuyinType 和兩個公開研究過程的注音輸入法——靈犀注音（0c）與善解（shanjie）——放在同一批句子、"
        "同一批按鍵上比較。"
        "目的是看清楚各自的取捨與我們自己的不足，不是排名。三個專案的設計目標不同，一個數字不能代表一個輸入法的好壞；"
        "各專案的做法與我們從中學到的，見 [相關工作](../research/related-work.md)。",
        "",
        f"量測日期：{record['date']}。這一頁由紀錄檔產生（`zt bench compare --render`），不手動修改。",
        "",
    ]
    sections: list[Callable[[View], list[str]]] = [summary, overview, setups, method]
    for section in sections:
        lines += section(view)
    lines += family_section(
        view,
        "和靈犀注音",
        ["0c-statistical", "0c-full"],
        "0c-full",
        "我們的集合、靈犀的案例與善解的集合，加上前文與分兩次送出。整句全對率（%），S／V；成對比較對靈犀（完整）。",
    )
    lines += family_section(
        view,
        "和善解",
        ["shanjie-chat", "shanjie-formal"],
        "shanjie-chat",
        "同樣的集合與情境；成對比較對善解（聊天）。"
        "善解的正式配置是為書面語設計的，聊天配置是為對話設計的，兩者並列供參考。",
    )
    lines += slips_section(view)
    lines += latency_section(view)
    lines += limits(view)
    lines += reproduction(view)
    return "\n".join(lines).rstrip("\n") + "\n"


# What the page needs of a cell and of a latency result; slim() keeps only these, so the tracked record holds numbers.
CELL_FIELDS = ("status", "reason", "metrics", "unsupported", "context_dropped", "selection_states")
TASK_FIELDS = ("set", "situation", "builder", "n", "with_context", "info", "keys_sha256", "paired")


def rounded(value: Any) -> Any:
    """Fractions to 6 significant digits (the page shows one decimal of a percentage)."""
    if isinstance(value, float):
        return float(f"{value:.6g}")
    if isinstance(value, dict):
        return {k: rounded(v) for k, v in value.items()}
    if isinstance(value, list):
        return [rounded(v) for v in value]
    return value


def slim(record: dict[str, Any]) -> dict[str, Any]:
    """The record without per-sentence references and outputs (they stay in the full record under data/build):
    per-task scores, counts, paired tests, latency summaries, system versions and metadata."""
    tasks = {}
    for task_id, task in record["tasks"].items():
        entry = {k: task[k] for k in TASK_FIELDS if k in task}
        entry["cells"] = {name: {k: cell[k] for k in CELL_FIELDS if k in cell} for name, cell in task["cells"].items()}
        tasks[task_id] = rounded(entry)
    latency = {
        name: {**found, "result": {k: v for k, v in (found.get("result") or {}).items() if k != "items"}}
        for name, found in record.get("latency", {}).items()
    }
    return {**record, "tasks": tasks, "latency": latency, "full_record": "data/build/bench/compare/<label>/record.json"}


def write(record_path: Path = RECORD, page_path: Path = PAGE) -> Path:
    record = json.loads(record_path.read_text(encoding="utf-8"))
    page_path.write_text(render(record), encoding="utf-8")
    return page_path
