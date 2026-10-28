"""Evaluation sets by name, so commands and records say `colloquial-dev` instead of a path.

Text sets hold sentences; the tools sample Han runs from them (zhuyintype_ml.core.zhuyin.sample_runs).  Row sets hold
`context|truth|syllables` rows typed exactly as given.  `role` says what a set may be used for: dev sets choose
settings, test sets only track releases (docs/ml/EVALUATION.md).  Paths are relative to the data directory unless they
start with ml/.  `zt eval sets` lists them and whether each is present.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from . import paths


@dataclass(frozen=True)
class EvalSet:
    name: str
    path: str
    kind: str  # "text", "rows" or "corrections" (app correction logs, given as files)
    role: str  # "dev", "test" or "external"
    source: str  # how to get it: an assets.json component, "tracked", or "local"
    description: str

    @property
    def file(self) -> Path:
        return paths.ROOT / self.path if self.path.startswith("ml/") else paths.data(self.path)

    @property
    def present(self) -> bool:
        return self.file.exists()


SHANJIE = "ml/eval-sets/external/shanjie/eval"

SETS = [
    EvalSet("wiki-dev", "corpus/dev_wikipedia.txt", "text", "dev", "eval-sets", "維基百科，開發集"),
    EvalSet("wiki-test", "lm/test_sentences.txt", "text", "test", "eval-sets", "維基百科，測試集"),
    EvalSet("colloquial-dev", "corpus/dev_colloquial.txt", "text", "dev", "eval-sets", "台灣口語，開發集"),
    EvalSet("colloquial-test", "corpus/test_colloquial.txt", "text", "test", "eval-sets", "台灣口語，測試集"),
    EvalSet("chat-dev", "lm_chat/dev_chat_tt.txt", "text", "dev", "eval-sets", "Taiwan Tongues 對話，開發集"),
    EvalSet("chat-test", "lm_chat/test_chat_tt.txt", "text", "test", "eval-sets", "Taiwan Tongues 對話，測試集"),
    EvalSet("smoke", "ml/eval-sets/colloquial_smoke.txt", "text", "test", "tracked", "日常短句 68 句"),
    EvalSet("latency", "ml/eval-sets/latency_sentences.txt", "text", "test", "tracked", "量測延遲的固定 10 句"),
    EvalSet("zai-dev", "homophone2/dev_zai.txt", "text", "dev", "local", "在／再，開發集"),
    EvalSet("zai-test", "homophone2/test_zai.txt", "text", "test", "local", "在／再，測試集"),
    EvalSet(
        "shanjie-dev302",
        f"{SHANJIE}/bench/suite-v1/dev302.clean.rows.txt",
        "rows",
        "external",
        "tracked",
        "善解 dev302（CC0）",
    ),
    EvalSet(
        "shanjie-typing76",
        f"{SHANJIE}/bench/suite-v1/typing76.clean.rows.txt",
        "rows",
        "external",
        "tracked",
        "善解 typing76（CC0）",
    ),
    EvalSet(
        "shanjie-reported",
        f"{SHANJIE}/bench/suite-v2/user-reported.clean.rows.txt",
        "rows",
        "external",
        "tracked",
        "善解使用者回報的錯誤句（CC0）",
    ),
    EvalSet(
        "shanjie-fw", f"{SHANJIE}/bench/suite-v2/fw.clean.rows.txt", "rows", "external", "tracked", "善解虛詞集（CC0）"
    ),
    EvalSet("shanjie-trap", f"{SHANJIE}/sets/trap.clean.rows.txt", "rows", "external", "tracked", "善解陷阱句（CC0）"),
    EvalSet(
        "shanjie-daily", f"{SHANJIE}/sets/daily.clean.rows.txt", "rows", "external", "tracked", "善解日常句（CC0）"
    ),
]
MISREAD = "lexicon_coverage/misread"
MISREAD_SUBSETS = {
    "taiwan": "台灣讀音",
    "mainland": "標準讀音改成台灣讀音前的讀音",
    "neutral": "輕聲打本調",
    "colloquial": "口語讀音",
    "misread": "有邊讀邊",
    "interjection": "語氣詞的聲調",
    "examples": "使用者回報的例子",
}
SETS += [
    EvalSet("misread-dev", f"{MISREAD}/misread-dev.rows.txt", "rows", "dev", "local", "開發集照實際讀音打（全部）"),
    EvalSet(
        "wiki-new-dev",
        "lexicon_coverage/wiki-new/wiki-new.rows.txt",
        "rows",
        "dev",
        "local",
        "2026-10-02 之後新建的維基條目名稱（時間切分）",
    ),
    *[
        EvalSet(
            f"misread-dev-{name}", f"{MISREAD}/{name}.rows.txt", "rows", "dev", "local", f"開發集照實際讀音打：{text}"
        )
        for name, text in MISREAD_SUBSETS.items()
    ],
]
BY_NAME = {s.name: s for s in SETS}

# Named groups, for the common choices.
GROUPS = {
    "dev": ["colloquial-dev", "wiki-dev", "chat-dev"],
    "test": ["colloquial-test", "wiki-test", "chat-test", "smoke"],
    "shanjie": [s.name for s in SETS if s.name.startswith("shanjie-")],
    # Built by `zt train lexicon misread` (ml/src/zhuyintype_ml/training/lexicon/misread.py).
    "misread": [f"misread-dev-{name}" for name in MISREAD_SUBSETS],
}


def resolve(names: list[str], kind: str | None = None) -> list[EvalSet]:
    """Sets for names, group names or file paths (a path becomes an ad-hoc set named after its file)."""
    result: list[EvalSet] = []
    for name in names:
        if name in GROUPS:
            result += [BY_NAME[n] for n in GROUPS[name]]
        elif name in BY_NAME:
            result.append(BY_NAME[name])
        elif Path(name).exists():
            path = Path(name).resolve()
            result.append(EvalSet(path.stem, str(path), kind or "text", "local", "file", paths.relative(path)))
        else:
            known = ", ".join(list(GROUPS) + list(BY_NAME))
            raise SystemExit(f"zt: unknown set {name!r}. Known sets: {known}; or give a file path.")
    if kind:
        wrong = [s.name for s in result if s.kind != kind]
        if wrong:
            raise SystemExit(f"zt: these sets are not {kind} sets: {', '.join(wrong)}")
    missing = [s for s in result if not s.file.exists()]
    if missing:
        hint = "; ".join(f"{s.name} ({s.source})" for s in missing)
        raise SystemExit(f"zt: missing sets: {hint}. Run `zt data fetch eval-sets` for eval-sets.")
    return result
