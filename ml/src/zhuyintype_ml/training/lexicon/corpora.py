"""Reading the corpora the lexicon pipeline mines and measures: one line of text per sentence or utterance.

    from zhuyintype_ml.training.lexicon.corpora import corpus_lines
    lines = corpus_lines("chat", [Path("data/build/lm_chat2/real_dialogues.jsonl")], "name")

Text files are one sentence per line.  The chat corpus (real_dialogues.jsonl) gives every utterance of every document
except the documents its splits.json reserves for the dialogue dev and test sets.  The business registry
(BGMOPEN1.zip) gives the business names, or the addresses with field "address".  Standard library only.
"""

from __future__ import annotations

import ast
import csv
import io
import json
import zipfile
from pathlib import Path


def corpus_lines(kind: str, files: list[Path], registry_field: str) -> list[str]:
    lines: list[str] = []
    for path in files:
        if kind == "chat" and path.suffix == ".jsonl":
            splits = path.with_name("splits.json")
            reserved = json.loads(splits.read_text())["reserved_documents"] if splits.exists() else {}
            with open(path, encoding="utf-8") as handle:
                for raw in handle:
                    doc = json.loads(raw)
                    if f"{doc['source']}:{doc['document']}" in reserved:
                        continue
                    turns = doc["turns"]
                    turns = ast.literal_eval(turns) if isinstance(turns, str) else turns
                    lines.extend(t["text"] for t in turns if t.get("text"))
        elif path.suffix == ".zip":
            with zipfile.ZipFile(path) as archive, archive.open(archive.namelist()[0]) as raw_file:
                reader = csv.reader(io.TextIOWrapper(raw_file, encoding="utf-8"))
                header = next(reader)
                column = header.index("營業地址" if registry_field == "address" else "營業人名稱")
                lines.extend(row[column] for row in reader if len(row) > column)
        else:
            lines.extend(path.read_text(encoding="utf-8").splitlines())
    return lines
