"""Paths, held-out exclusions and confusion-position annotations shared by the homophone modules.

Three directories in the data directory:

    homophone/       the data: inventory, mined and judged clauses, the dev/test sets, smoke.txt, inputs/ snapshots
    homophone2/      the 0.4.0 training run: set copies, filtered shard indexes, real-source clauses, records, results
    v3_homophone2/   the continued v3 model (released as v3/)

The continuation starts from the 0.3.0 v3 in v3_030/ (nlp/training/v3).

    from nlp.training.homophone.common import Exclusions, positions
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Iterable, Iterator
from pathlib import Path
from typing import Any

from nlp.common import paths
from nlp.training.v3 import common as v3
from nlp.training.v3.readings import Lexicon

# The 0.3.0 v3 the continuation starts from: checkpoints, vocabularies and encoded shards.
BASE = v3.OUT
DATA = paths.data("homophone")
RUN = paths.data("homophone2")
MODEL = paths.data("v3_homophone2")

# Evaluation text copied to homophone/inputs/ (relative to the data directory).
HELD = [
    "lm/test_sentences.txt",
    "corpus/test_colloquial.txt",
    "corpus/dev_colloquial.txt",
    "corpus/dev_wikipedia.txt",
    "lm_chat/dev_chat_tt.txt",
    "lm_chat/test_chat_tt.txt",
]
# The 27 seed groups; mine.py keeps those with a shared toned standalone reading and enough occurrences.
SEEDS = [
    "在再",
    "做作",
    "已以",
    "他她它",
    "像向象項",
    "須需",
    "帳賬",
    "份分",
    "的得",
    "吧巴",
    "嗎嘛",
    "那哪",
    "到道",
    "是事",
    "時實",
    "試式",
    "受授",
    "權全",
    "利力",
    "治制",
    "記紀",
    "竟競",
    "帶戴",
    "副付",
    "坐座",
    "候後",
    "里裡",
]
SMOKE = (
    "我再一次跟你說,你在哪裡,我在家,明天再說吧,他在吃飯,我們下次再來,你在幹嘛,不要再這樣了,等一下再打給你,"
    "我現在在忙,再試試看,你再說一次,吃完再走,再等我五分鐘,他不在公司,在啦幹,我再吃一碗,我在吃一碗麵"
).split(",")
# Position categories, in priority order (the first flag that holds names the category).
CATEGORIES = ["fixed", "after_subject", "initial", "before_verb", "before_place_time", "other"]
VERBS = set("吃喝說講做作試等看想走來去買賣打玩睡洗讀寫問聽跑找用開關學幫給拿放住唱跳工作忙")
PLACES = set("家校店室院國臺台北南東西市鄉鎮村公司學校這那哪裡此上下午夜年月日時天路")
SUBJECTS = ["政府", "公司", "學校", "學生", "老師", "民眾", "大家", "孩子", "父母", "居民", "警方", "球員", "團隊"]


def save(path: Path | str, value: object) -> None:
    """Writes JSON atomically, creating the directory."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    temp.replace(path)


def digest(path: Path | str) -> str:
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for block in iter(lambda: f.read(8 << 20), b""):
            h.update(block)
    return h.hexdigest()


class Exclusions:
    """Texts that must not reach training: a text is excluded when it equals one, contains a short one (under 8
    characters) or shares an 8-character substring with one.  `compile` builds an Aho-Corasick automaton for speed;
    texts added afterwards need another `compile`."""

    def __init__(self, texts: Iterable[str] = ()) -> None:
        self.exact: set[str] = set()
        self.grams: set[str] = set()
        self.short: set[str] = set()
        self.machine: Any = None
        for text in texts:
            self.add(text)

    def add(self, text: str) -> None:
        if text:
            self.exact.add(text)
            if len(text) < 8:
                self.short.add(text)
            self.grams.update(text[i : i + 8] for i in range(len(text) - 7))

    def __contains__(self, text: object) -> bool:
        assert isinstance(text, str)
        if self.machine is not None:
            return text in self.exact or next(self.machine.iter(text), None) is not None
        return (
            text in self.exact
            or any(s in text for s in self.short)
            or any(text[i : i + 8] in self.grams for i in range(len(text) - 7))
        )

    def compile(self) -> Exclusions:
        import ahocorasick

        self.machine = ahocorasick.Automaton()
        for gram in self.grams | {s for s in self.exact if len(s) < 8}:
            self.machine.add_word(gram, True)
        self.machine.make_automaton()
        return self


def holdouts(new: bool = False, directory: Path = DATA) -> Exclusions:
    """The held-out snapshots in `directory`/inputs and the smoke sentences; with `new`, also the homophone dev and
    test sets.  A field before the last of a line (a context) counts only when it has 8 or more characters."""
    files = [directory / "inputs" / Path(p).name for p in HELD]
    if new:
        files += list(directory.glob("dev_*.txt")) + list(directory.glob("test_*.txt")) + [directory / "smoke.txt"]
    texts = list(SMOKE)
    for path in files:
        for line in path.read_text().splitlines():
            fields = line.split("\t")
            texts.append(fields[-1])
            # A short context such as a quote mark is not an evaluation sentence.
            texts.extend(c for c in fields[:-1] if len(c) >= 8)
            if len(fields) > 1:
                texts.append("".join(fields))
    return Exclusions(texts)


def positions(text: str, lex: Lexicon, inventory: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Every confusion position of `text`: a character of an inventory group read with the group's shared reading,
    with its category flags (lexicon word, after a subject, initial, before a verb, before a place or time)."""
    spans = list(lex.segments(text))
    result = []
    for group in inventory:
        readings = {tuple(r) for r in group["readings"]}
        for i, c in enumerate(text):
            if c not in group["chars"]:
                continue
            span = next(s for s in spans if s[0] <= i < s[1])
            reading = span[3][i - span[0]]
            if tuple(reading) not in readings:
                continue
            fixed = span[1] - span[0] > 1
            subject = i > 0 and (
                text[i - 1] in "我你妳他她它們人誰爸媽哥姐弟妹" or any(text[:i].endswith(s) for s in SUBJECTS)
            )
            flags = dict(
                fixed=fixed,
                after_subject=subject,
                initial=i == 0,
                before_verb=i + 1 < len(text) and text[i + 1] in VERBS,
                before_place_time=i + 1 < len(text) and text[i + 1] in PLACES,
            )
            category = next((key for key in CATEGORIES if flags.get(key)), "other")
            result.append(
                dict(index=i, char=c, group=group["chars"], reading=list(reading), category=category, flags=flags)
            )
    return sorted(result, key=lambda p: (p["index"], p["group"]))


def jsonl(path: Path | str) -> Iterator[dict[str, Any]]:
    with Path(path).open() as f:
        for line in f:
            yield json.loads(line)
