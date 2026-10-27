"""Independent masked 在／再 review of the mined clauses by Qwen3-8B. One request per clause; errors are not retried.

    zt train homophone judge --option judge=http://127.0.0.1:8102/v1/chat/completions
    python -m nlp.training.homophone.judge [--server URL] [--previous OLD/judged.jsonl]

Needs an OpenAI-compatible server for Qwen3-8B (vLLM, JSON-schema output), started separately on the GPU host and
stopped after this step.  Every 在／再 of a clause is masked and the model fills each independently; a clause is
accepted only when every answer equals the original character and no strong collocation (現在, 再見, …) disagrees.
`--previous` reuses decisions for identical text and context.  Writes homophone/{judged.jsonl, judge-summary.json};
refuses to overwrite judged.jsonl.
"""

from __future__ import annotations

import argparse
import json
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

from .common import DATA, jsonl, save

SERVER = "http://127.0.0.1:8102/v1/chat/completions"


def judge(row: dict[str, Any], server: str = SERVER) -> dict[str, Any]:
    positions = [p for p in row["positions"] if p["group"] == "在再"]
    if not positions:
        return {**row, "judge": {"accepted": True, "method": "no_zai"}}
    chars = list(row["text"])
    for n, p in enumerate(positions):
        chars[p["index"]] = f"【{n + 1}】"
    masked = "".join(chars)
    prompt = (
        "請依臺灣繁體中文語法，獨立判斷每個空格應填「在」或「再」。在表示位置、存在或正在；再表示重複、接續或將來。"
        "若兩者皆合理或缺乏語境，填「?」。只回 JSON 字串陣列，依空格順序，每項僅一個字。"
        '不可改寫句子。例如：他【1】教室→["在"]；等天氣好【1】去→["再"]；'
        '我【1】看書→["在"]；請【1】幫我一次→["再"]。'
        f"前文：{row['context']}\n句子：{masked}"
    )
    data = {
        "model": "Qwen3-8B",
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0,
        "max_tokens": 100,
        "chat_template_kwargs": {"enable_thinking": False},
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "choices",
                "schema": {
                    "type": "array",
                    "items": {"type": "string", "enum": ["在", "再", "?"]},
                    "minItems": len(positions),
                    "maxItems": len(positions),
                },
            },
        },
    }
    request = urllib.request.Request(server, json.dumps(data).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=180) as response:
        answer = json.load(response)["choices"][0]["message"]["content"]
    try:
        labels = json.loads(answer.strip().removeprefix("```json").removeprefix("```").removesuffix("```").strip())
    except json.JSONDecodeError:
        labels = None
    accepted = isinstance(labels, list) and labels == [p["char"] for p in positions]
    # Strong collocations are checked independently as well; a model disagreement is still dropped.
    fixed = {"現在": "在", "正在": "在", "存在": "在", "實在": "在", "再見": "再", "不再": "再", "一再": "再"}
    rule_checks = []
    for p in positions:
        for word, expected in fixed.items():
            offset = word.index(expected)
            start = p["index"] - offset
            if start >= 0 and row["text"][start : start + len(word)] == word:
                rule_checks.append(dict(index=p["index"], word=word, agrees=p["char"] == expected))
    accepted = accepted and all(c["agrees"] for c in rule_checks)
    return {
        **row,
        "judge": dict(
            accepted=accepted,
            labels=labels,
            raw=answer,
            masked=masked,
            method="masked Qwen3-8B greedy, ambiguity rejected",
            rules=rule_checks,
        ),
    }


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--workers", type=int, default=32)
    p.add_argument("--server", default=SERVER, help=f"chat completions URL of the Qwen3-8B server (default: {SERVER})")
    p.add_argument(
        "--previous", type=Path, help="Reuse completed decisions for identical text/context; never retry errors."
    )
    args = p.parse_args(argv)
    target = DATA / "judged.jsonl"
    assert not target.exists(), "Output exists; inspect a partial failure before explicitly resuming."
    rows = list(jsonl(DATA / "candidates.jsonl"))
    previous = {(r["text"], r["context"].strip()): r["judge"] for r in jsonl(args.previous)} if args.previous else {}

    def review(row: dict[str, Any]) -> dict[str, Any]:
        key = row["text"], row["context"].strip()
        return {**row, "judge": previous[key]} if key in previous else judge(row, args.server)

    accepted = 0
    with target.open("x") as f, ThreadPoolExecutor(args.workers) as pool:
        for i, row in enumerate(pool.map(review, rows), 1):
            accepted += row["judge"]["accepted"]
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
            if i % 100 == 0:
                f.flush()
                print(i, "accepted", accepted, flush=True)
    save(
        DATA / "judge-summary.json",
        dict(
            candidates=len(rows),
            accepted=accepted,
            rejected=len(rows) - accepted,
            model="Qwen3-8B",
            note="independent label choice masked all 在/再 targets; no automatic retries",
        ),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
