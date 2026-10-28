"""wiki-new-dev: names of Chinese Wikipedia articles created after the dump the corpora come from (time-split).

    python -m zhuyintype_ml.training.lexicon.newtitles --base BASE.tsv --out DIR [--since 2026-10-02 --until ...]

The 善解 word lists were drawn from its own packs, so a pack built from the same lists is measured in-sample there.
This set is independent of every source the lexicon uses: titles of articles first created on zh.wikipedia between
--since (the day after the 2026-10-01 dump that `zt train lm` and the miner read) and --until, from the MediaWiki
log of page creations.  Kept: titles of 2–8 Han characters after OpenCC s2twp, not already lexicon words, every
character with a reading; typed with the base lexicon's readings (longest match, as `zt eval conversion` does).
300 rows, seeded.  Titles are CC BY-SA 4.0 (Wikipedia contributors); the creation log is saved as the receipt.

Writes wiki-new.rows.txt (`|title|syllables`) and wiki-new.json (counts, date range, receipt of the API query).
Needs OpenCC (the `train` dependency group) and the network.
"""

from __future__ import annotations

import argparse
import json
import random
import re
import time
import urllib.parse
import urllib.request
from pathlib import Path

from zhuyintype_ml.core.zhuyin import Lexicon

API = "https://zh.wikipedia.org/w/api.php"


def creations(since: str, until: str) -> list[tuple[str, str]]:
    params = {
        "action": "query",
        "list": "logevents",
        "letype": "create",
        "lenamespace": "0",
        "lestart": f"{since}T00:00:00Z",
        "leend": f"{until}T00:00:00Z",
        "ledir": "newer",
        "lelimit": "500",
        "leprop": "title|timestamp",
        "format": "json",
    }
    found: list[tuple[str, str]] = []
    while True:
        request = urllib.request.Request(
            API + "?" + urllib.parse.urlencode(params), headers={"User-Agent": "ZhuyinType-evaluation/0.1"}
        )
        data = json.load(urllib.request.urlopen(request, timeout=60))
        found += [(e["title"], e["timestamp"]) for e in data["query"]["logevents"]]
        if "continue" not in data:
            return found
        params.update(data["continue"])
        time.sleep(0.5)


def main(argv: list[str] | None = None) -> int:
    from opencc import OpenCC

    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--since", default="2026-10-02")
    parser.add_argument("--until", default="2026-10-27")
    parser.add_argument("-n", type=int, default=300)
    args = parser.parse_args(argv)
    args.out.mkdir(parents=True, exist_ok=True)
    receipt = args.out / "creations.json"
    log: list[tuple[str, str]]
    if receipt.exists():  # the saved log: the same rows again, and no load on the API
        log = [(title, stamp) for title, stamp in json.loads(receipt.read_text(encoding="utf-8"))]
    else:
        log = creations(args.since, args.until)
        receipt.write_text(json.dumps(log, ensure_ascii=False) + "\n", encoding="utf-8")
    lexicon = Lexicon(args.base)
    convert = OpenCC("s2twp").convert
    rows = {}
    for title, _ in log:
        text = convert(title)
        if not re.fullmatch(r"[一-鿿]{2,8}", text) or text in lexicon.reading:
            continue
        readings = lexicon.readings(text)
        if readings and len(readings) == len(text):
            rows[text] = " ".join(readings)
    chosen = sorted(rows)
    random.Random(7).shuffle(chosen)
    chosen = chosen[: args.n]
    (args.out / "wiki-new.rows.txt").write_text("".join(f"|{t}|{rows[t]}\n" for t in chosen), encoding="utf-8")
    summary = {
        "created": len(log),
        "eligible": len(rows),
        "rows": len(chosen),
        "since": args.since,
        "until": args.until,
    }
    (args.out / "wiki-new.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
