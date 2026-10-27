"""Pinned downloads for the lexicon coverage pipeline: each source's URL, SHA-256, licence and attribution.

    python -m zhuyintype_ml.training.lexicon.sources            # download what is missing into data/raw/lexicon, check
    python -m zhuyintype_ml.training.lexicon.sources --check    # only check the files that are there

A moving URL (Wiktextract's latest extraction, the daily business registry) is pinned by its digest: when the
upstream file has changed, the download is rejected rather than silently mixed in, and the run needs the archived
copy (`data/raw/lexicon/`, to be packed as a data component when the lexicon ships).  Standard library only.
"""

from __future__ import annotations

import argparse
import shutil
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.lm.files import digest, save_json


@dataclass(frozen=True)
class Source:
    name: str
    url: str
    sha256: str
    retrieved: str
    licence: str
    attribution: str
    stable: bool  # the URL always serves these bytes


SOURCES = (
    Source(
        "Unihan-18.0.0.zip",
        "https://www.unicode.org/Public/18.0.0/ucd/Unihan.zip",
        "4c93ea9c1f636451729a840978f1667a53886af37ba854fdcce109721c63d43e",
        "2026-10-28",
        "Unicode License v3",
        "Unicode, Inc., Unihan Database 18.0.0",
        True,
    ),
    Source(
        "kaikki-Chinese.jsonl.gz",
        "https://kaikki.org/dictionary/Chinese/kaikki.org-dictionary-Chinese.jsonl.gz",
        "4e54d7a3d2974b5efb6bf80ec0a6d8c71d916b90b30a350bdd71bdfda0dce36b",
        "2026-10-28 (Last-Modified 2026-10-25)",
        "CC BY-SA 4.0 (and GFDL), Wiktionary text; extraction by Wiktextract (kaikki.org)",
        "English Wiktionary contributors; Tatu Ylonen, Wiktextract (kaikki.org)",
        False,
    ),
    Source(
        "IDS.TXT",
        "https://www.babelstone.co.uk/CJK/IDS.TXT",
        "cc2a0a97e6a51ed6ebe59870ef2af66e83f3c3c27387d79b4441bb2d791d78b1",
        "2026-10-28 (File Date 2025-06-27)",
        "No rights claimed: the maintainer states IDS sequences are facts and waives claims to the format",
        "Andrew West (BabelStone), Ideographic Description Sequences for CJK Unified Ideographs",
        False,
    ),
    Source(
        "mcbpmf-data.txt",
        "https://raw.githubusercontent.com/Nanako0129/shanjie/bd3b6095e170012e5675b15b9443786a04103790/data/lexicon/"
        "mcbpmf-data.txt",
        "0deae7b7c1dcde1d7a30d139e7068543e0c0e7112e944b63eb52947bca1db7ac",
        "2026-10-28 (McBopomofo 3.1.1 data.txt, as copied unmodified by shanjie at bd3b609)",
        "MIT (McBopomofo); phrases from libtabe tsi.src (BSD)",
        "McBopomofo contributors (openvanilla/McBopomofo); libtabe",
        True,
    ),
    Source(
        "CNS11643-Properties.zip",
        "https://data.cns11643.gov.tw/opendata/Properties.zip",
        "3d56ef14cc8099893245dac58fe4718d2fa64812b9159352a98a4588ad3efa5c",
        "2026-10-28 (data.gov.tw 5961, CNS_phonetic.txt of 2026-08-05)",
        "政府資料開放授權條款第 1 版 (Open Government Data License, version 1.0)",
        "國家發展委員會，CNS11643 中文標準交換碼全字庫（data.gov.tw 5961）",
        False,
    ),
    Source(
        "CNS11643-MapingTables.zip",
        "https://data.cns11643.gov.tw/opendata/MapingTables.zip",
        "4502fcf7b433d679dee51127298929543ec7f4aa99be93cd219df1552bc3d2bf",
        "2026-10-28 (data.gov.tw 5961)",
        "政府資料開放授權條款第 1 版 (Open Government Data License, version 1.0)",
        "國家發展委員會，CNS11643 中文標準交換碼全字庫（data.gov.tw 5961）",
        False,
    ),
    Source(
        "BGMOPEN1.zip",
        "https://eip.fia.gov.tw/data/BGMOPEN1.zip",
        "4fb8ce0377d3528c921b66805701bc2d850d39bf1fd1a8113009be70e3668e1b",
        "2026-10-28 (Last-Modified 2026-10-26)",
        "政府資料開放授權條款第 1 版 (Open Government Data License, version 1.0)",
        "財政部財政資訊中心，全國營業(稅籍)登記資料集（data.gov.tw 9400）",
        False,
    ),
)
BY_NAME = {s.name: s for s in SOURCES}


def directory() -> Path:
    return paths.raw("lexicon")


def path(name: str) -> Path:
    return directory() / name


def fetch(check_only: bool = False) -> list[dict[str, str]]:
    """Downloads missing sources and checks every digest; returns the receipts (also written to sources.json)."""
    directory().mkdir(parents=True, exist_ok=True)
    receipts = []
    for source in SOURCES:
        target = path(source.name)
        if not target.exists() and not check_only:
            print(f"downloading {source.url}", flush=True)
            partial = target.with_suffix(target.suffix + ".part")
            with urllib.request.urlopen(source.url) as response, open(partial, "wb") as out:
                shutil.copyfileobj(response, out, 1 << 20)
            partial.replace(target)
        if not target.exists():
            raise SystemExit(f"missing {paths.relative(target)}")
        actual = digest(target)
        if actual != source.sha256:
            hint = "" if source.stable else " (the upstream file moves; use the archived copy)"
            raise SystemExit(f"{source.name}: SHA-256 {actual} differs from the pinned {source.sha256}{hint}")
        receipts.append(
            {
                "name": source.name,
                "url": source.url,
                "sha256": actual,
                "retrieved": source.retrieved,
                "licence": source.licence,
                "attribution": source.attribution,
            }
        )
    save_json(directory() / "sources.json", receipts)
    return receipts


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="only check the digests of files already there")
    args = parser.parse_args(argv)
    for receipt in fetch(args.check):
        print(f"ok {receipt['name']} {receipt['sha256'][:12]}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
