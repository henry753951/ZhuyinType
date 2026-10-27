"""File helpers the language-model pipelines share: atomic JSON, SHA-256 digests, log lines, the raw-download directory.

Standard library only, so download steps can use it without the train dependency group.

    from nlp.training.lm.files import digest, log, raw, save_json
    save_json(raw("lm") / "download.json", {"sha256": digest(path)})
"""

from __future__ import annotations

import datetime
import hashlib
import json
from pathlib import Path

from nlp.common import paths


def log(message: str) -> None:
    print(f"{datetime.datetime.now(datetime.UTC).isoformat()} {message}", flush=True)


def save_json(path: Path | str, value: object) -> None:
    """Writes `value` as indented UTF-8 JSON through a temporary file, so a reader never sees half a file."""
    path = Path(path)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    tmp.replace(path)


def digest(path: Path | str, algorithm: str = "sha256") -> str:
    h = hashlib.new(algorithm)
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(8 * 1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


raw = paths.raw  # kept here for the modules that import it with the other file helpers
