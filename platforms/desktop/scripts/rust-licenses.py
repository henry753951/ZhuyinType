#!/usr/bin/env python3
"""The licences of every Rust crate linked into a desktop binary, for the packages (Licenses/Rust-crates.txt).

    python3 scripts/rust-licenses.py OUTPUT [--target x86_64-unknown-linux-gnu] [--package zhuyintype]

Walks `cargo metadata --locked --filter-platform TARGET` from the package through its normal and build
dependencies (the macOS app's platforms/macos/scripts/rust-licenses.swift does the same for zhuyin-ffi) and copies
each crate's LICENSE*, COPYING* and NOTICE* files.  A crate published without licence files gets the text of the
licence its manifest names (MIT with the crate's authors, or Apache-2.0, printed once at the end), as cargo-about
does; the run fails when the licence is neither.  Run after a build, so cargo has the sources.  Standard library only.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DESKTOP = Path(__file__).resolve().parent.parent
APACHE = DESKTOP.parent / "macos" / "Resources" / "Licenses" / "Apache-2.0.txt"
MIT = """Copyright (c) {authors}

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE."""


def manifest_licence(package: dict) -> str | None:
    """The licence text for a crate without licence files: MIT (with its authors) or Apache-2.0, as its manifest's
    SPDX expression allows; None for anything else."""
    names = set(re.split(r"[\s()/]+|\bOR\b|\bAND\b", package.get("license") or ""))
    source = package.get("repository") or f"https://crates.io/crates/{package['name']}"
    if "MIT" in names:
        authors = ", ".join(a.split(" <")[0] for a in package.get("authors") or []) or f"the {package['name']} authors"
        return f"（套件未附授權檔，依其 Cargo.toml 的 MIT 授權；來源 {source}）\n\n" + MIT.format(authors=authors)
    if "Apache-2.0" in names:
        return f"（套件未附授權檔，依其 Cargo.toml 的 Apache-2.0 授權，全文在本檔最後；來源 {source}）"
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("output", type=Path)
    parser.add_argument("--target", default="x86_64-unknown-linux-gnu")
    parser.add_argument("--package", default="zhuyintype")
    args = parser.parse_args()

    command = ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", args.target]
    metadata = json.loads(subprocess.run(command, cwd=DESKTOP, check=True, capture_output=True, text=True).stdout)
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    roots = [p["id"] for p in metadata["packages"] if p["name"] == args.package and p["id"] in members]
    if not roots:
        raise SystemExit(f"rust-licenses: no workspace package {args.package}")

    selected: dict[str, dict] = {}
    pending, visited = list(roots), set()
    while pending:
        package_id = pending.pop()
        if package_id in visited or package_id not in nodes:
            continue
        visited.add(package_id)
        # Path dependencies (zhuyin-core in engine/) are ZhuyinType's own code.
        if package_id not in members and packages[package_id]["source"] is not None:
            selected[package_id] = packages[package_id]
        # Build dependencies are kept conservatively (their code can end up in the binary through build scripts).
        for dep in nodes[package_id]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])

    lines = [f"Rust 套件授權（{args.package}，{args.target}，包含建置相依套件）", ""]
    missing: list[str] = []
    apache = False
    for package in sorted(selected.values(), key=lambda p: (p["name"], p["version"])):
        directory = Path(package["manifest_path"]).parent
        names = ("LICENSE", "COPYING", "NOTICE")
        files = sorted(f for f in directory.iterdir() if f.is_file() and f.name.upper().startswith(names))
        title = f"{package['name']} {package['version']} — {package.get('license') or '未標示'}"
        text = None
        if not any(f.name.upper().startswith(("LICENSE", "COPYING")) for f in files):
            text = manifest_licence(package)
            if text is None:
                missing.append(title)
                continue
            apache = apache or "Apache-2.0" in text
        lines.append(f"===== {title} =====")
        if text:
            lines += ["", text, ""]
        for file in files:
            lines += ["", f"--- {file.name} ---", file.read_text(encoding="utf-8", errors="replace").rstrip(), ""]
    if apache:
        lines += ["===== Apache License 2.0 =====", "", APACHE.read_text(encoding="utf-8").rstrip(), ""]
    if missing:
        print("rust-licenses: crates without a licence file or an MIT/Apache-2.0 licence:\n  " + "\n  ".join(missing),
              file=sys.stderr)
        return 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"rust-licenses: {len(selected)} crates in {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
