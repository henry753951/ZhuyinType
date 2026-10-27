"""Results every tool returns: what was measured, with what, and the numbers; printed for people, saved as JSON.

A Report holds one Section per evaluation set.  `zt eval ... --json FILE` saves it; `zt bench` combines reports into
the release record; `zt eval stats` pairs two reports' per-item outputs.
"""

from __future__ import annotations

import datetime
import json
import platform
import subprocess
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from . import paths


@dataclass
class Section:
    name: str
    metrics: dict
    # Per-item records for paired comparisons: {"id", "reference", "output", ...}.
    items: list[dict] = field(default_factory=list)
    examples: list[tuple[str, str]] = field(default_factory=list)


@dataclass
class Report:
    tool: str
    config: dict
    options: dict
    sections: list[Section] = field(default_factory=list)
    environment: dict = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.environment:
            commit = subprocess.run(
                ["git", "rev-parse", "--short", "HEAD"], cwd=paths.ROOT, capture_output=True, text=True
            ).stdout.strip()
            self.environment = {
                "date": datetime.datetime.now().isoformat(timespec="seconds"),
                "platform": f"{platform.system()} {platform.machine()}",
                "commit": commit,
            }

    def add(self, section: Section) -> Section:
        self.sections.append(section)
        return section

    def save(self, path: Path | str) -> None:
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        Path(path).write_text(json.dumps(asdict(self), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")

    @staticmethod
    def load(path: Path | str) -> Report:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        sections = [Section(**s) for s in data.pop("sections")]
        return Report(sections=sections, **data)


def percent(value: float) -> str:
    return f"{100 * value:.1f}%"


def print_report(report: Report, describe: Callable[[dict[str, Any]], str]) -> None:
    """One line per section from `describe(metrics) -> str`, then a few examples of what went wrong."""
    print(f"{report.tool} · {report.config.get('name')} · {report.config.get('data', '')}")
    for section in report.sections:
        print(f"[{section.name}] {describe(section.metrics)}")
        for want, got in section.examples:
            print(f"  {want}  →  {got}")
