"""Cache the licensed real dialogue sources with raw responses and SHA-256 receipts (bounded retries).

    government   Legislative Yuan gazette transcripts via OpenFun LYAPI (CC BY 4.0; originals OGDL v1): a seeded
                 sample of --documents documents from the pinned gazette agenda metadata
    wikipedia    Chinese Wikipedia talk pages of at least 8,000 bytes (CC BY-SA 4.0), --batches API pages of 50,
                 ten seconds apart, honouring HTTP 429 Retry-After
    similar      one data.gov.tw meeting-minutes ODT, to check whether it holds speech (it holds only decisions)

Every response is kept under data/raw/lm_chat2 with a .receipt.json (URL, licence, SHA-256, size, date, headers); a
cached file whose hash no longer matches its receipt is an error.  Standard library only; may run on the Mac.

    zt train lm_chat2 sources
    python -m nlp.training.lm_chat2.sources government --documents 800
    python -m nlp.training.lm_chat2.sources wikipedia --batches 20
"""

from __future__ import annotations

import argparse
import datetime
import json
import random
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

from nlp.common import paths
from nlp.training.lm.files import digest, raw, save_json

REV = "16860178b7fbf234201db7d5c030f3fbab869a7a"
WIKI = "https://zh.wikipedia.org/w/api.php"


def fetch(url: str, path: Path, license: str) -> bytes:
    """The body of `url`, from the cache when its receipt matches, otherwise downloaded (three attempts)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    receipt = path.with_suffix(path.suffix + ".receipt.json")
    if receipt.exists():
        old = json.loads(receipt.read_text())
        if old["url"] != url or digest(path) != old["sha256"]:
            raise ValueError(f"cache mismatch: {path}")
        return path.read_bytes()
    for attempt in range(3):
        try:
            req = Request(url, headers={"User-Agent": "ZhuyinType-corpus/1.0 (open-source Taiwan IME; research)"})
            with urlopen(req, timeout=90) as response:
                body = response.read()
                headers = dict(response.headers)
            break
        except Exception as error:
            if attempt == 2:
                raise
            if isinstance(error, HTTPError) and error.code == 429:
                delay = max(60, int(error.headers.get("Retry-After", "60")))
            else:
                delay = 2**attempt
            print(f"{url}: retry after {delay}s: {error}", flush=True)
            time.sleep(delay)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_bytes(body)
    tmp.replace(path)
    save_json(
        receipt,
        {
            "url": url,
            "license": license,
            "sha256": digest(path),
            "bytes": len(body),
            "retrieved_utc": datetime.datetime.now(datetime.UTC).isoformat(),
            "headers": {k: v for k, v in headers.items() if k.lower() in ("etag", "last-modified", "content-type")},
        },
    )
    return body


def government(directory: Path, limit: int) -> None:
    fetch("https://v2.ly.govapi.tw/", directory / "lyapi-license.html", "CC BY 4.0 data / BSD code")
    fetch("https://data.gov.tw/license", directory / "ogdl.html", "Taiwan OGDL v1")
    url = f"https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda/resolve/{REV}/"
    fetch(url + "README.md", directory / "gazette-README.md", "CC BY 4.0")
    body = fetch(url + "gazette_agenda-11.jsonl", directory / "gazette-agenda.jsonl", "CC BY 4.0")
    docs = {}
    for line in body.decode().splitlines():
        row = json.loads(line)
        for doc in row.get("處理後公報網址", []):
            if doc["type"] == "txt":
                docs[doc["url"]] = row
    items = sorted(docs.items())
    random.Random(20261027).shuffle(items)
    items = items[:limit]

    def download(item: tuple[str, Any]) -> dict[str, Any]:
        url, row = item
        name = url.split("/")[-2]
        fetch(url, directory / "gazette" / (name + ".txt"), "CC BY 4.0; Legislative Yuan original OGDL v1")
        return {"id": name, "url": url, "metadata": row}

    with ThreadPoolExecutor(1) as pool:
        records = list(pool.map(download, items))
    save_json(directory / "gazette-index.json", records)


def wikipedia(directory: Path, batches: int) -> None:
    fetch(
        "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use", directory / "wikimedia-terms.html", "CC BY-SA 4.0"
    )
    # Require substantial talk text instead of downloading thousands of assessment-only stubs.
    continuation: dict[str, Any] = {}
    for i in range(batches):
        params = dict(
            action="query",
            generator="allpages",
            gapnamespace=1,
            gaplimit=50,
            gapminsize=8000,
            gapfilterredir="nonredirects",
            prop="revisions",
            rvprop="ids|timestamp|content",
            rvslots="main",
            format="json",
            formatversion=2,
        )
        params.update(continuation)
        body = fetch(
            WIKI + "?" + urlencode(params),
            directory / "wiki" / f"large-{i:04d}.json",
            "CC BY-SA 4.0; Chinese Wikipedia contributors",
        )
        response = json.loads(body)
        if "error" in response:
            raise ValueError(response["error"])
        continuation = response.get("continue")
        if not continuation:
            break
        if i % 10 == 0:
            print("wiki batches", i + 1, flush=True)
        time.sleep(10)


def similar(directory: Path) -> None:
    url = "https://file.data.gov.tw/" + quote("行政院資料開放諮詢小組會議紀錄.json")
    rows = json.loads(fetch(url, directory / "data-gov-16827.json", "Taiwan OGDL v1"))
    # Inspect an original ODT, not the accompanying slide decks or a third-party transcript.
    row = next(r for r in rows if r["會議紀錄連結"].strip().endswith(".odt"))
    url = row["會議紀錄連結"].strip().replace("http:", "https:")
    url = quote(url, safe=":/%")
    fetch(url, directory / "data-gov-16827-sample.odt", "Taiwan OGDL v1")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", choices=["government", "wikipedia", "similar"])
    parser.add_argument("--documents", type=int, default=800)
    parser.add_argument("--batches", type=int, default=100)
    parser.add_argument("--raw", type=Path, default=raw("lm_chat2"), help="default: data/raw/lm_chat2")
    a = parser.parse_args(argv)
    paths.data("lm_chat2").mkdir(parents=True, exist_ok=True)
    if a.source == "government":
        government(a.raw, a.documents)
    elif a.source == "wikipedia":
        wikipedia(a.raw, a.batches)
    else:
        similar(a.raw)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
