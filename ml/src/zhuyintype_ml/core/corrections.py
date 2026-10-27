"""The app's correction log (「記錄改字」, ~/Library/Application Support/ZhuyinType/corrections.jsonl): reading it,
what to replay for each kind of correction, rows for row sets, and a summary of what users corrected.

Each line is one composition in which the user corrected the engine (schema in docs/product/DESIGN.md §2.7):

    records = load("corrections.jsonl")
    for item in replay_items(records[0]):       # what `zt eval corrections` types: keys, context, truth
        print(item.group, item.keys, item.truth)
    rows(records)                               # context|truth|syllables lines, when the keys spell syllables
    summarize(records)                          # (deleted keys → retyped keys) and (shown → final) counts

A replay asks whether the engine now gets the composition right without the correction: the keys of the final
composition for a pick (and the other in-place corrections), the keys as first typed for a Backspace or Esc
correction, and the previous composition's keys for a commit deleted and typed again.
"""

from __future__ import annotations

import json
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .zhuyin import KEYS, TONES, syllable_keys

SCHEMA = 1
CORRECTIONS = (
    "pick",
    "backspace",
    "escape",
    "literal_zhuyin",
    "undo_literal",
    "select",
    "add_phrase",
    "cursor_edit",
    "delete_after_commit",
)
# Corrections that replace keys: the replay types the keys as first typed.
RETYPED = ("backspace", "escape")
REQUIRED = ("schema", "id", "settings", "keys", "typing", "replayable", "original_typing", "original_replayable")
DELAY = re.compile(r"<dt:[0-9.]+>")
FUZZY = ("zh", "ch", "sh", "en", "an")
SYMBOL = {key: symbol for symbol, key in KEYS.items()}
# Slot of each Zhuyin symbol in a syllable: initial, medial, final (tones end the syllable).
INITIALS = set("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ")
MEDIALS = set("ㄧㄨㄩ")


def load(path: Path | str) -> list[dict[str, Any]]:
    """Records of a correction log; blank lines are skipped, anything else invalid is an error."""
    records: list[dict[str, Any]] = []
    for number, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"{path}:{number}: not JSON ({error.msg})") from None
        if not isinstance(record, dict) or any(key not in record for key in REQUIRED):
            raise ValueError(f"{path}:{number}: not a correction record")
        if record["schema"] != SCHEMA:
            raise ValueError(f"{path}:{number}: schema {record['schema']}, this tool reads {SCHEMA}")
        records.append(record)
    return records


def plain(keys: str) -> str:
    """Keys without their `<dt:MS>` delays."""
    return DELAY.sub("", keys)


def single_line(text: str) -> str:
    """Text for a batch line or a row: tabs, line breaks and | become spaces (a sentence boundary either way)."""
    return re.sub(r"[\t\r\n|]", " ", text)


def cli_options(settings: dict[str, Any]) -> list[str]:
    """zhuyin-cli options for the recorded settings that change what keys produce."""
    options = [
        "--typo",
        str(settings.get("typo_tolerance", "standard")),
        "--punctuation",
        str(settings.get("punctuation_keys", "windows")),
        "--forms",
        str(settings.get("character_forms", "common")),
    ]
    fuzzy = [name for name in settings.get("fuzzy_sounds", []) if name in FUZZY]
    return options + (["--fuzzy", ",".join(fuzzy)] if fuzzy else [])


@dataclass(frozen=True)
class Item:
    """One replay: `keys` after `context`, then <enter>; right when it gives `truth`, unchanged when `shown`."""

    id: str
    group: str
    context: str
    keys: str
    truth: str
    shown: str
    options: tuple[str, ...]

    @property
    def line(self) -> str:
        return single_line(self.context) + "\t" + self.keys + "<enter>"


def replay_items(record: dict[str, Any]) -> list[Item]:
    """One item per kind of correction in the record that can be replayed (see the module docstring)."""
    options = tuple(cli_options(record["settings"]))
    context = record.get("context") or ""
    items: list[Item] = []
    for kind in record.get("corrections", []):
        identity = f"{record['id']}:{kind}"
        if kind == "delete_after_commit":
            event = next((e for e in record.get("events", []) if e.get("type") == kind), None)
            if not event or not event.get("previous_replayable"):
                continue
            deleted, previous = event["deleted"], event["previous_committed"]
            truth = previous[: len(previous) - len(deleted)] + record["committed"][: len(deleted)]
            items.append(
                Item(
                    identity,
                    kind,
                    event.get("previous_context") or "",
                    event["previous_typing"],
                    truth,
                    previous,
                    options,
                )
            )
        elif kind in RETYPED:
            if record["original_replayable"]:
                keys = record["original_typing"]
                items.append(Item(identity, kind, context, keys, record["committed"], record["shown"], options))
        elif record["replayable"]:
            items.append(Item(identity, kind, context, record["typing"], record["committed"], record["shown"], options))
    return items


def syllables(keys: str) -> list[str] | None:
    """The syllables the keys spell, when every syllable ends with a tone key or Space and nothing else was pressed;
    None otherwise (Backspace, punctuation, a slot typed twice, an unfinished syllable)."""
    result: list[str] = []
    slots: dict[int, str] = {}
    for token in re.findall(r"<[^>]*>|.", plain(keys)):
        if token == "<space>" or token in ("3", "4", "6", "7"):
            if not slots:
                return None
            tone = "" if token == "<space>" else SYMBOL[token]
            result.append("".join(slots[slot] for slot in sorted(slots)) + tone)
            slots = {}
            continue
        symbol = SYMBOL.get(token)
        if symbol is None or symbol in TONES:
            return None
        slot = 0 if symbol in INITIALS else 1 if symbol in MEDIALS else 2
        if slot in slots:
            return None
        slots[slot] = symbol
    return result if result and not slots else None


def rows(records: list[dict[str, Any]]) -> list[str]:
    """`context|truth|syllables` lines for row sets (`zt eval rows`), one per replay item whose keys spell one syllable
    per character of an all-Han truth, in the order typed; duplicates once."""
    lines: list[str] = []
    for record in records:
        for item in replay_items(record):
            spelled = syllables(item.keys)
            if not spelled or len(spelled) != len(item.truth) or not re.fullmatch(r"[一-鿿]+", item.truth):
                continue
            text = " ".join(spelled)
            # Keys typed out of order spell the syllable in order; the row would type other keys.
            if syllable_keys(text) != plain(item.keys):
                continue
            line = f"{single_line(item.context)}|{item.truth}|{text}"
            if line not in lines:
                lines.append(line)
    return lines


def summarize(records: list[dict[str, Any]], top: int = 20) -> dict[str, Any]:
    """Counts of correction kinds, the most frequent (deleted keys → retyped keys) of Backspace and Esc corrections,
    and the most frequent (shown → final) changes of picks, retyping and commits deleted and typed again."""
    kinds: Counter[str] = Counter()
    keys: Counter[tuple[str, str]] = Counter()
    words: Counter[tuple[str, str]] = Counter()
    for record in records:
        kinds.update(record.get("corrections", []))
        for event in record.get("events", []):
            kind = event.get("type")
            if kind in RETYPED and event.get("deleted_keys") and event.get("retyped_keys"):
                keys[(event["deleted_keys"], event["retyped_keys"])] += 1
                if event.get("removed") and event.get("replaced_by") and event["removed"] != event["replaced_by"]:
                    words[(event["removed"], event["replaced_by"])] += 1
            elif kind == "pick" and event.get("removed") != event.get("inserted"):
                words[(event.get("removed", ""), event.get("inserted", ""))] += 1
            elif kind == "delete_after_commit" and "delete_after_commit" in record.get("corrections", []):
                deleted = event.get("deleted", "")
                words[(deleted, record["committed"][: len(deleted)])] += 1
    return {
        "records": len(records),
        "corrections": dict(kinds.most_common()),
        "keys": [{"typed": a, "retyped": b, "count": n} for (a, b), n in keys.most_common(top)],
        "words": [{"shown": a, "final": b, "count": n} for (a, b), n in words.most_common(top)],
    }
