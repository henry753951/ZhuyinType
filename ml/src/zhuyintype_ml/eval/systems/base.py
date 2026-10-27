"""What every compared input method implements (strategy pattern), and what it is given and returns.

A Task is one evaluation set in one situation (plain, rows as given, line context, split commits, keyboard slips,
timed typing errors).  Its Cases are what a person types: one or more segments of physical keys, each committed with
Enter, optionally after some text already on the line.  A segment typed exactly also carries its readings, so a system
that takes readings instead of keys (善解's evaluation CLI) receives the same syllables; a segment with typing errors
carries keys only.

    system = registry["0c-full"](references)
    system.start()
    outcomes = system.convert(task, task.cases)
    system.close()

Keys use zhuyin-cli's script syntax (zhuyintype_ml/core/zhuyin.py): one character per key, `<space>` or a space for
Space, `<enter>`, `<left>`, and `<dt:MS>` before a key for the time since the previous key.  `parse_script` turns a
script into Key events for the adapters that send keys one at a time.
"""

from __future__ import annotations

import math
import re
import statistics
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

# Situations whose cases carry typing errors: the keys are what was typed, there are no readings to give instead.
KEY_SITUATIONS = ("typo", "typing")
# What a system can be designed to do beyond exact typing.  A task that needs a capability a system does not declare
# is N/A (context: the input cannot be given) or measured on a small sample only and marked unsupported.
CAPABILITIES = {
    "context": "arbitrary text before the run",
    "key-errors": "tolerance of wrong, missing, extra or swapped keys",
    "no-space": "a first tone typed without Space",
}


@dataclass(frozen=True)
class Key:
    """One key press: a keycap character, or a named key (space, enter, left); `gap_ms` since the previous key."""

    name: str
    gap_ms: float | None = None

    @property
    def is_char(self) -> bool:
        return len(self.name) == 1


TOKEN = re.compile(r"<dt:([0-9.]+)>|<([a-z]+)>|(.)", re.S)


def parse_script(script: str) -> list[Key]:
    """Key events of a zhuyin-cli key script."""
    result: list[Key] = []
    gap: float | None = None
    for match in TOKEN.finditer(script):
        delay, named, char = match.groups()
        if delay is not None:
            gap = float(delay)
            continue
        name = named if named is not None else ("space" if char == " " else char)
        if name not in ("space", "enter", "left") and len(name) != 1:
            raise ValueError(f"unsupported key <{name}> in a script")
        result.append(Key(name, gap))
        gap = None
    return result


@dataclass(frozen=True)
class Segment:
    """Keys committed with one Enter; `readings` when they were typed exactly (one syllable per character)."""

    keys: str
    readings: tuple[str, ...] | None = None


@dataclass(frozen=True)
class Case:
    """What one person types for one reference text: segments after an optional text already on the line."""

    id: str
    reference: str
    segments: tuple[Segment, ...]
    context: str = ""

    @property
    def exact(self) -> bool:
        return all(s.readings is not None for s in self.segments)

    @property
    def keys(self) -> str:
        return "<enter>".join(s.keys for s in self.segments) + "<enter>"


@dataclass
class Task:
    """One set in one situation.  `builder` is who made the set (zhuyintype, shanjie, 0c)."""

    set: str
    situation: str
    builder: str
    cases: list[Case]
    description: str = ""
    # Whether the cases' context is part of the situation (rows as given, line context); systems that cannot take
    # arbitrary text before the run get the case without it and the cell records how many contexts were left out.
    uses_context: bool = False
    info: dict[str, Any] = field(default_factory=dict)

    @property
    def id(self) -> str:
        return f"{self.situation}/{self.set}"

    @property
    def keys_only(self) -> bool:
        return self.situation.split("/")[0] in KEY_SITUATIONS

    @property
    def requires(self) -> tuple[str, ...]:
        """The capabilities this task measures (see CAPABILITIES)."""
        kind, _, condition = self.situation.partition("/")
        if kind == "context":
            return ("context",)
        if kind == "typo" or (kind == "typing" and condition == "mixed"):
            return ("key-errors",) + (("no-space",) if condition == "mixed" else ())
        if kind == "typing" and condition.startswith("nospace"):
            return ("no-space",)
        if kind == "typing" and condition not in ("exact", "exact-fast"):
            return ("key-errors",)
        return ()

    @property
    def sequential(self) -> bool:
        """Several commits in one case, the later ones after the earlier ones on the same line."""
        return any(len(case.segments) > 1 for case in self.cases)


@dataclass
class Outcome:
    """What a system committed for a case.  status: ok, no-path (the system has no conversion for the readings),
    error (the system failed on this case; counted wrong)."""

    output: str
    status: str = "ok"
    info: dict[str, Any] = field(default_factory=dict)


class System(ABC):
    """One input method in one setup.  Adapters drive the real program, unmodified, through its own interface."""

    name: str  # record key, e.g. "0c-full"
    label: str  # column title on the page
    family: str  # "zhuyintype", "0c" or "shanjie"
    # What it is designed to do beyond exact typing (keys of CAPABILITIES).
    capabilities: frozenset[str] = frozenset()

    @property
    def context(self) -> bool:
        """Accepts arbitrary text before the run (line context)."""
        return "context" in self.capabilities

    def missing(self, task: Task) -> list[str]:
        return [c for c in task.requires if c not in self.capabilities]

    def start(self) -> None:  # noqa: B027 (optional hook)
        """Start processes or load libraries; called once before the tasks."""

    def close(self) -> None:  # noqa: B027 (optional hook)
        """Stop what start started."""

    def resize(self, processes: int) -> None:  # noqa: B027 (optional hook)
        """Use this many processes from the next task on (systems whose processes start once ignore it)."""

    def unsupported(self, task: Task) -> str | None:
        """Why this system cannot run the task at all (the cell is N/A), or None."""
        if "context" in self.missing(task):
            return "no-context"
        return None

    @abstractmethod
    def convert(self, task: Task, cases: list[Case]) -> list[Outcome]:
        """One outcome per case, in order.  Every case starts from a fresh composition and learns nothing that
        another case could see."""

    def latency(self, scripts: list[str]) -> dict[str, Any] | None:
        """Per-key time over sentences typed one key at a time (each script one sentence, ending in Enter), or None
        when not measurable here."""
        return None

    @abstractmethod
    def describe(self) -> dict[str, Any]:
        """Version and settings, for the record."""


def percentile(values: list[float], p: float) -> float:
    """The nearest-rank percentile (the definition zhuyin-cli --timing uses)."""
    ordered = sorted(values)
    return ordered[max(0, min(len(ordered) - 1, math.ceil(len(ordered) * p) - 1))]


def key_time_summary(per_sentence: list[list[float]]) -> dict[str, Any]:
    """Medians over sentences of each sentence's per-key p50, p95 and max (ms), as `zt eval latency` reports them."""
    rows = [
        {"keys": len(times), "p50_ms": percentile(times, 0.5), "p95_ms": percentile(times, 0.95), "max_ms": max(times)}
        for times in per_sentence
        if times
    ]

    def median(name: str) -> float | None:
        values = [row[name] for row in rows]
        return round(statistics.median(values), 2) if values else None

    return {
        "sentences": len(rows),
        "keys": sum(row["keys"] for row in rows),
        "median": {name: median(name) for name in ("p50_ms", "p95_ms", "max_ms")},
        "items": rows,
    }
