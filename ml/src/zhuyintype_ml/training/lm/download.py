"""Download (or resume) the latest Chinese Wikipedia pages-articles dump, check Wikimedia's SHA-1, record a receipt.

The receipt (download.json: dated snapshot URL, dump date, Last-Modified, size, published and actual SHA-1, SHA-256)
pins the moving `latest` alias to one snapshot; a checksum mismatch, including the alias changing mid-download, is
rejected.  Standard library and curl only.

    zt --remote train lm download
    python -m zhuyintype_ml.training.lm.download data/raw/lm
"""

from __future__ import annotations

import argparse
import datetime
import re
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

from .files import digest, raw, save_json

DUMP_URL = "https://dumps.wikimedia.org/zhwiki/latest/zhwiki-latest-pages-articles.xml.bz2"


def fetch(url: str, *options: str) -> bytes:
    return subprocess.check_output(
        ["curl", "--fail", "--location", "--retry", "8", "--silent", "--show-error", *options, url]
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("directory", type=Path, nargs="?", default=raw("lm"), help="default: data/raw/lm")
    args = parser.parse_args(argv)
    args.directory.mkdir(parents=True, exist_ok=True)
    dump = args.directory / DUMP_URL.rsplit("/", 1)[1]
    # Fetch the receipt first; reject a moving 'latest' alias rather than mixing dumps.
    base = DUMP_URL.rsplit("/", 1)[0]
    rss = fetch(DUMP_URL + "-rss.xml")
    item = ET.fromstring(rss).find("channel/item")
    assert item is not None
    link = re.search(r'href="([^"]+)', item.findtext("description", ""))
    assert link is not None
    dated_url = link.group(1).replace("http://download.wikimedia.org/", "https://dumps.wikimedia.org/")
    dated = re.search(r"/zhwiki/(\d{8})/", dated_url)
    assert dated is not None
    date = dated.group(1)
    checksums = fetch(base + "/zhwiki-latest-sha1sums.txt").decode()
    names = {dump.name, dated_url.rsplit("/", 1)[1]}
    expected = next(line.split()[0] for line in checksums.splitlines() if line.split()[-1] in names)
    headers = fetch(DUMP_URL, "--head").decode()
    size = int(re.findall(r"(?im)^content-length:\s*(\d+)", headers)[-1])
    modified = re.findall(r"(?im)^last-modified:\s*([^\r\n]+)", headers)[-1]
    if not dump.exists() or dump.stat().st_size != size:
        subprocess.run(
            ["curl", "--fail", "--location", "--retry", "8", "--continue-at", "-", "--output", str(dump), DUMP_URL],
            check=True,
        )
    actual = digest(dump, "sha1")
    if actual != expected:
        raise ValueError(f"published SHA1 mismatch: expected {expected}, actual {actual}")
    (args.directory / "dump-rss.xml").write_bytes(rss)
    (args.directory / "sha1sums.txt").write_text(checksums)
    receipt = {
        "requested_url": DUMP_URL,
        "dated_url": dated_url,
        "dump_date": date,
        "downloaded_utc": datetime.datetime.now(datetime.UTC).isoformat(),
        "http_last_modified": modified,
        "published_sha1": expected,
        "actual_sha1": actual,
        "sha256": digest(dump),
        "bytes": size,
        "checksum_url": base + "/zhwiki-latest-sha1sums.txt",
        "rss_url": DUMP_URL + "-rss.xml",
    }
    save_json(args.directory / "download.json", receipt)
    print(receipt, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
