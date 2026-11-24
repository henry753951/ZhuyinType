#!/usr/bin/env python3
"""The release notes the apps carry, from docs/releases/vX.Y.Z.md.

    python3 -I tools/release_notes.py OUTPUT.json

Both apps show what an update changed without the network: build-app.sh puts the JSON into the macOS app's resources,
platforms/desktop/crates/zt-core/build.rs compiles it into the Linux and Windows app.  Only the standard library, so
either build can run it.

    {"releases": [{"version": "0.7.5", "date": "2026 年 11 月 23 日",
                   "sections": [{"title": "修正", "items": [{"title": "…", "text": "…", "platforms": ["windows"]}]}]}]}

Newest first, the sections 新功能, 改進 and 修正 (the rest is on the web page).  `title` is the bold name of a 新功能 item or
null; `platforms` lists the systems an item names at its start (「Windows：」, 「Windows 與 Linux：」), and is empty for
every system.  The text is plain: links, bold and code are reduced to what they say.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import TypedDict

SECTIONS = ("新功能", "改進", "修正")
PLATFORMS = {"macOS": "macos", "Windows": "windows", "Linux": "linux"}
# 「Windows：」, 「Windows 與 Linux：」, 「macOS、Linux：」 at the start of an item.
NAMES = "|".join(PLATFORMS)
PREFIX = re.compile(rf"^((?:{NAMES})(?:\s*(?:、|／|與|和)\s*(?:{NAMES}))*)\s*：\s*")
BOLD_TITLE = re.compile(r"^\*\*(.+?)\*\*\s*((?:（[^）]*）)?)\s*：?\s*")


class Item(TypedDict):
    title: str | None
    text: str
    platforms: list[str]


class Section(TypedDict):
    title: str
    items: list[Item]


class Release(TypedDict):
    version: str
    date: str
    sections: list[Section]


def plain(text: str) -> str:
    """Markdown as the words it shows."""
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"\*\*(.+?)\*\*", r"\1", text)
    text = re.sub(r"`([^`]+)`", r"\1", text)
    return re.sub(r"\s+", " ", text).strip()


def platforms(names: str) -> list[str]:
    return [PLATFORMS[name] for name in re.findall(NAMES, names)]


def item(markdown: str) -> Item:
    systems: list[str] = []
    title: str | None = None
    if match := PREFIX.match(markdown):
        systems = platforms(match[1])
        markdown = markdown[match.end() :]
    if match := BOLD_TITLE.match(markdown):
        title = plain(match[1])
        markdown = markdown[match.end() :]
        # 「**主題與透明效果**（Windows 與 Linux）：」
        if match[2] and not systems and re.fullmatch(rf"（(?:{NAMES}|[\s、／與和])+）", match[2]):
            systems = platforms(match[2])
    return {"title": title, "text": plain(markdown), "platforms": systems}


def bullets(lines: list[str]) -> list[str]:
    """Each top-level `- ` with its continuation lines and nested bullets."""
    found: list[str] = []
    for line in lines:
        if line.startswith("- "):
            found.append(line[2:].strip())
        elif found and line.startswith(" ") and line.strip():
            nested = re.sub(r"^\s*(?:- |\d+\. )?", "", line)
            found[-1] += nested.strip()
    return found


def parse(text: str, version: str) -> Release:
    lines = text.splitlines()
    date = next((line.split("：", 1)[1].strip() for line in lines if line.startswith("發布日期：")), "")
    sections: list[Section] = []
    title = ""
    body: list[str] = []
    for line in [*lines, "## "]:
        if not line.startswith("## "):
            body.append(line)
            continue
        if title in SECTIONS:
            items = [item(bullet) for bullet in bullets(body)]
            if items:
                sections.append({"title": title, "items": items})
        title, body = line[3:].strip(), []
    return {"version": version, "date": date, "sections": sections}


def releases(directory: Path) -> list[Release]:
    found = []
    for path in directory.glob("v*.md"):
        if match := re.fullmatch(r"v(\d+)\.(\d+)\.(\d+)", path.stem):
            found.append((tuple(int(part) for part in match.groups()), path))
    return [parse(path.read_text(encoding="utf-8"), path.stem[1:]) for _, path in sorted(found, reverse=True)]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    directory = Path(__file__).resolve().parent.parent / "docs" / "releases"
    found = releases(directory)
    if not found:
        sys.exit(f"release_notes.py: no release notes in {directory}")
    data = json.dumps({"releases": found}, ensure_ascii=False, indent=1)
    Path(sys.argv[1]).write_text(data + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
