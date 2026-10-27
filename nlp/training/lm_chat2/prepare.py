"""chat2 corpus: turns from the cached sources, document-disjoint real holdouts, leakage-filtered training text.

    extract   real_dialogues.jsonl (speaker turns of each gazette document and signed talk-page thread, with
              provenance links) and source_samples.json (20 sampled turn pairs per source, for review)
    split     dev_dialogue.txt and test_dialogue.txt: 1,000 `previous<TAB>target` pairs each (900 gazette, 100
              talk page), whole documents reserved to one split; refuses to resample once splits.json exists
    prepare   training sentence shards and turn pairs for --variant real (no_generated/: real turns and Common
              Voice) or mixed (also the 0.3.0 colloquial corpus); refuses to run twice

Every training text equal to, or sharing 8 characters with, a holdout or any frozen evaluation file is removed,
also after OpenCC s2twp and 臺→台 normalization.  GPU host for prepare (24 workers).

    zt --remote train lm_chat2 extract split real mixed
    python -m nlp.training.lm_chat2.prepare prepare --variant real --workers 24
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import unicodedata
from collections import Counter
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from opencc import OpenCC

from nlp.common import paths
from nlp.common.sets import BY_NAME
from nlp.training.lm import mkn
from nlp.training.lm.files import digest, raw, save_json
from nlp.training.lm_chat.train import grams, normalized

OUT = paths.data("lm_chat2")
RAW = raw("lm_chat2")


def evaluation_files() -> list[Path]:
    """The frozen copies in inputs/evaluation when present, otherwise every current dev/test text file."""
    frozen = OUT / "inputs/evaluation"
    if frozen.exists():
        return sorted(p for p in frozen.rglob("*") if p.is_file() and p.suffix in (".txt", ".jsonl"))
    files = [paths.data("lm", "test_sentences.txt"), BY_NAME["smoke"].file]
    files += sorted(paths.data("corpus").glob("dev*.txt"))
    files += sorted(paths.data("corpus").glob("test*.txt"))
    files += sorted(paths.data("lm_chat").glob("*_chat_tt.txt"))
    for pattern in ("dev_*", "test_*", "smoke.txt"):
        files += sorted(paths.data("homophone").glob(pattern))
    return sorted(set(p for p in files if p.is_file() and p.suffix in (".txt", ".jsonl")))


def strings(value: Any) -> Iterator[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def blocked_text(files: list[Path]) -> set[str]:
    """Every text of the files: lines (TSV fields separately) or every string in each JSON line."""
    texts: list[str] = []
    for path in files:
        for line in path.read_text().splitlines():
            texts.extend(strings(json.loads(line)) if path.suffix == ".jsonl" else line.split("\t"))
    return set(texts)


def excluded(text: str, exact: set[str], machine: Any) -> bool:
    return text in exact or next(machine.iter(text), None) is not None


TW = OpenCC("s2twp")
SPEAKER = re.compile(r"(?m)^([^\s：:，。！？、（）()\d]{2,22})[：:]")
STAMP = re.compile(r"\d{4}年\d{1,2}月\d{1,2}日[^\n]{0,35}\(UTC\)")
OFF_REGISTER = re.compile(
    "貌似|掰扯|秀下限|丫的|閣下|鄙人|敝人|俺|咋|咱|麼|咩|嘅|唔|視頻|軟件|鼠標|出租車|地鐵|靠譜|性價比|點贊"
)


def normalize(text: str) -> str:
    """OpenCC s2twp, NFC, whitespace collapsed."""
    return " ".join(unicodedata.normalize("NFC", TW.convert(text)).split())


def valid(text: str) -> bool:
    return (
        4 <= len(text) <= 1200
        and len(mkn.HAN.findall(text)) >= 0.65 * len(text)
        and not mkn.BAD.search(text)
        and not re.search(r"\b\S+@\S+\b|\d{8,}", text)
    )


def gazette_turns(text: str) -> list[dict[str, Any]]:
    """Speaker turns of a gazette transcript; the chair, procedure and attachments become empty separators."""
    matches = list(SPEAKER.finditer(text))
    turns = []
    for i, match in enumerate(matches):
        body = text[match.end() : matches[i + 1].start() if i + 1 < len(matches) else len(text)]
        body = re.sub(r"^\s*（[^）]{0,30}）", "", body)
        body = normalize(body)
        speaker = match[1]
        # Tables, written attachments and procedural formulae are poor conversational evidence.
        if speaker == "主席" or any(
            s in speaker for s in ("說明", "提案", "決議", "案由", "時間", "地點", "議事", "秘書長", "出席", "列席")
        ):
            turns.append({"speaker": speaker, "text": "", "position": i})
            continue
        if valid(body) and not re.search(
            "書面質詢|附錄|附件|臨時提案|表決結果|有無異議|議事處|備質詢|進行專題報告|擬請院會|請審議案|請查照案|第[一二三四五六七八九十百]+條",
            body,
        ):
            turns.append({"speaker": speaker, "text": body, "position": i})
        else:
            turns.append({"speaker": speaker, "text": "", "position": i})
    return turns


def wiki_threads(raw: str) -> list[list[dict[str, Any]]]:
    """Signed turns of each talk-page section with at least two turns; usernames and timestamps are removed."""
    import mwparserfromhell

    threads: list[list[dict[str, Any]]] = []
    turns: list[dict[str, Any]] = []
    buffer: list[str] = []

    def flush() -> None:
        nonlocal turns
        if len(turns) >= 2:
            threads.append(turns)
        turns = []

    for line in raw.splitlines():
        if re.match(r"^==+", line):
            flush()
            buffer = []
            continue
        buffer.append(line)
        if not STAMP.search(line):
            continue
        content = "\n".join(buffer)
        buffer = []
        signatures = list(re.finditer(r"\[\[(?:User|使用者|用户|用戶):([^\]|/]+)", line, re.I))
        if not signatures:
            turns.append({"speaker": "", "text": "", "position": len(turns)})
            continue
        signature = signatures[-1]
        # Remove the signed tail before markup stripping; do not train on usernames or timestamps.
        signature_start = content.rfind(line) + signature.start()
        content = content[:signature_start]
        content = STAMP.sub("", content)
        if re.search(r"<(?:blockquote|ref)|\{\{|https?://|^\s*[|!]", content, re.I | re.M):
            text = ""
        else:
            code = mwparserfromhell.parse(content)
            for link in list(code.filter_wikilinks()):
                if ":" in str(link.title):
                    code.remove(link)
            text = normalize(re.sub(r"(?m)^[:*#;]+", "", code.strip_code()))
            text = re.sub(r"--[^\s]{1,40}\s*$", "", text)
            text = text.strip(" -—：:")
            if not valid(text) or OFF_REGISTER.search(text) or re.search(r"User:|使用者:|「|」|“|”", text, re.I):
                text = ""
        turns.append({"speaker": signature[1], "text": text, "position": len(turns)})
    flush()
    return threads


def extract() -> None:
    """real_dialogues.jsonl and source_samples.json from the cached raw responses."""
    records = []
    index = RAW / "gazette-index.json"
    if index.exists():
        entries = json.loads(index.read_text())
    else:
        entries = {}
        for line in (RAW / "gazette-agenda.jsonl").read_text().splitlines():
            metadata = json.loads(line)
            for doc in metadata.get("處理後公報網址", []):
                if doc["type"] == "txt":
                    name = doc["url"].split("/")[-2]
                    if (RAW / "gazette" / (name + ".txt.receipt.json")).exists():
                        entries[name] = {"id": name, "url": doc["url"], "metadata": metadata}
        entries = [entries[k] for k in sorted(entries)]
    for entry in entries:
        path = RAW / "gazette" / (entry["id"] + ".txt")
        turns = gazette_turns(path.read_text())
        records.append(
            {
                "source": "gazette",
                "document": entry["id"],
                "url": entry["url"],
                "metadata": entry["metadata"],
                "turns": turns,
            }
        )
    seen = set()
    for path in sorted((RAW / "wiki").glob("*.json")):
        if path.name.endswith(".receipt.json"):
            continue
        response = json.loads(path.read_text())
        if "error" in response:
            raise ValueError(response["error"])
        for page in response.get("query", {}).get("pages", []):
            if page["pageid"] in seen or not page.get("revisions"):
                continue
            seen.add(page["pageid"])
            rev = page["revisions"][0]
            for i, turns in enumerate(wiki_threads(rev["slots"]["main"].get("content", ""))):
                records.append(
                    {
                        "source": "wiki_talk",
                        "document": str(page["pageid"]),
                        "thread": i,
                        "title": page["title"],
                        "revision": rev["revid"],
                        "timestamp": rev["timestamp"],
                        "url": f"https://zh.wikipedia.org/w/index.php?oldid={rev['revid']}",
                        "turns": turns,
                    }
                )
    with (OUT / "real_dialogues.jsonl").open("w") as f:
        for record in records:
            f.write(json.dumps(record, ensure_ascii=False) + "\n")
    counts: Counter[str] = Counter()
    samples = {}
    rng = random.Random(20261027)
    for source in ("gazette", "wiki_talk"):
        pool = [r for r in records if r["source"] == source]
        counts[source + "_documents"] = len({r["document"] for r in pool})
        candidates = []
        for record in pool:
            for a, b in zip(record["turns"], record["turns"][1:]):
                if a["text"] and b["text"] and a["speaker"] != b["speaker"]:
                    candidates.append({"url": record["url"], "previous": a["text"], "target": b["text"]})
        counts[source + "_pairs"] = len(candidates)
        samples[source] = rng.sample(candidates, min(20, len(candidates)))
    save_json(OUT / "source_samples.json", {"counts": dict(counts), "samples": samples})
    print(dict(counts), flush=True)


def pairs(record: dict[str, Any]) -> Iterator[tuple[str, str]]:
    """Adjacent non-empty turns by different speakers."""
    for a, b in zip(record["turns"], record["turns"][1:]):
        if a["text"] and b["text"] and a["speaker"] != b["speaker"]:
            yield a["text"], b["text"]


def split() -> None:
    """Freezes the dev and test dialogue holdouts (once)."""
    if (OUT / "splits.json").exists():
        raise ValueError("holdouts already frozen; refusing to resample")
    records = [json.loads(s) for s in (OUT / "real_dialogues.jsonl").read_text().splitlines()]
    files = evaluation_files()
    held = blocked_text(files)
    used = set().union(*(grams(s) for s in held))
    by_source: dict[str, list[tuple[str, list[dict[str, Any]]]]] = {}
    for source in ("gazette", "wiki_talk"):
        documents: dict[str, list[dict[str, Any]]] = {}
        for record in records:
            if record["source"] == source:
                documents.setdefault(record["document"], []).append(record)
        names = sorted(documents)
        random.Random(20261027).shuffle(names)
        by_source[source] = [(name, documents[name]) for name in names]
    rows: dict[str, list[tuple[str, str]]] = {"dev": [], "test": []}
    reserved = {}
    provenance = []
    for source, docs in by_source.items():
        quota = 100 if source == "wiki_talk" else 900
        used_docs = set()
        for kind in rows:
            selected: list[tuple[str, str]] = []
            for doc, group in docs:
                if doc in used_docs:
                    continue
                used_docs.add(doc)
                reserved[source + ":" + doc] = kind
                for record in group:
                    for previous, target in pairs(record):
                        if not 4 <= len(target) <= 160 or not re.search(r"[\u4e00-\u9fff]{4,20}", target):
                            continue
                        pair_grams = grams(previous) | grams(target)
                        if pair_grams & used or previous in held or target in held:
                            continue
                        used.update(pair_grams)
                        held.update((previous, target))
                        selected.append((previous, target))
                        provenance.append(
                            {
                                "split": kind,
                                "source": source,
                                "document": doc,
                                "url": record["url"],
                                "previous": previous,
                                "target": target,
                            }
                        )
                        if len(selected) == quota:
                            break
                    if len(selected) == quota:
                        break
                if len(selected) == quota:
                    break
            if len(selected) != quota:
                raise ValueError(f"{source}/{kind}: only {len(selected)} eligible real turns; need {quota}")
            rows[kind].extend(selected)
    for kind, values in rows.items():
        random.Random(20261027).shuffle(values)
        (OUT / f"{kind}_dialogue.txt").write_text("".join(a + "\t" + b + "\n" for a, b in values))
    save_json(
        OUT / "splits.json",
        {
            "seed": 20261027,
            "each": 1000,
            "source_each": {"gazette": 900, "wiki_talk": 100},
            "reserved_documents": reserved,
            "provenance": provenance,
            "real_sha256": digest(OUT / "real_dialogues.jsonl"),
            "files": {p.name: digest(p) for p in OUT.glob("*_dialogue.txt")},
            "existing_evaluation": {paths.relative(p): digest(p) for p in files},
        },
    )


def prepare(variant: str, workers: int) -> None:
    """Training sentence shards and turn pairs of one variant (once)."""
    out = OUT if variant == "mixed" else OUT / "no_generated"
    work = out / "work"
    work.mkdir(parents=True, exist_ok=True)
    marker = work / "prepared.json"
    if marker.exists():
        raise ValueError("training corpus already prepared")
    held = blocked_text(evaluation_files() + [OUT / "dev_dialogue.txt", OUT / "test_dialogue.txt"])
    # Canonical variants are also blocked so 台/臺 conversion cannot bypass the guard.
    held.update(normalize(s).replace("臺", "台") for s in list(held))
    machine = mkn.automaton(held)
    splits = json.loads((OUT / "splits.json").read_text())
    reserved = splits["reserved_documents"]
    directory = work / "sentences"
    directory.mkdir(exist_ok=True)
    handles = [(directory / f"{i:03d}.txt").open("w") for i in range(workers)]
    counts: Counter[str] = Counter()
    chars: set[str] = set()
    seen: set[bytes] = set()
    contexts = (work / "context_pairs.tsv").open("w")
    context_seen: set[bytes] = set()

    def add_context(previous: str, target: str) -> None:
        if any(
            excluded(s, held, machine) or excluded(normalize(s).replace("臺", "台"), held, machine)
            for s in (previous, target)
        ):
            return
        key = hashlib.blake2b((previous + "\t" + target).encode(), digest_size=16).digest()
        if key in context_seen:
            return
        context_seen.add(key)
        contexts.write(previous + "\t" + target + "\n")
        counts["context_pairs"] += 1

    def add(text: str, source: str) -> None:
        text = normalized(text)
        counts[source + "_input"] += 1
        if not text or excluded(text, held, machine) or excluded(normalize(text).replace("臺", "台"), held, machine):
            counts[source + "_heldout"] += 1
            return
        key = hashlib.blake2b(text.encode(), digest_size=16).digest()
        if key in seen:
            counts[source + "_duplicates"] += 1
            return
        seen.add(key)
        assert "\t" not in text and "\n" not in text
        handles[counts["training_sentences"] % workers].write(text + "\n")
        counts["training_sentences"] += 1
        counts[source + "_kept"] += 1
        chars.update(text)

    try:
        for line in (OUT / "real_dialogues.jsonl").open():
            record = json.loads(line)
            if record["source"] + ":" + record["document"] in reserved:
                continue
            for previous, target in pairs(record):
                add_context(previous, target)
            for turn in record["turns"]:
                for sentence in mkn.SPLIT.split(turn["text"]):
                    sentence = sentence.strip()
                    if 4 <= len(sentence) <= 200:
                        add(sentence, record["source"])
        for line in (paths.data("corpus", "train_commonvoice.txt")).open():
            add(line.strip(), "commonvoice")
        if variant == "mixed":
            # Provenance of the 0.3.0 v3 colloquial corpus (data/build/v3 then) excludes TT: synthetic + Common Voice.
            for line in paths.data("v3_030", "colloquial.txt").open():
                add(line.strip(), "old_colloquial")
            # Generated dialogues count only with a review approving their exact file; the two pilots failed it,
            # so this file is empty or absent (the pilot generator and cleaner were removed, see REPORT.md).
            synthetic = OUT / "synthetic_dialogues.jsonl"
            if synthetic.exists() and synthetic.stat().st_size:
                review = json.loads((OUT / "generation-review.json").read_text())
                if review.get("approved_sha256") != digest(synthetic):
                    raise ValueError("generated corpus lacks a matching sample-quality review")
                for line in synthetic.open():
                    turns = json.loads(line)["turns"]
                    for previous, target in zip(turns, turns[1:]):
                        add_context(previous, target)
                    for text in turns:
                        add(text, "new_generated")
    finally:
        contexts.close()
        for handle in handles:
            handle.close()
    save_json(marker, {**counts, "training_characters": sorted(chars)})
    print(dict(counts), flush=True)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("phase", choices=["extract", "split", "prepare"])
    p.add_argument("--variant", choices=["mixed", "real"], default="mixed")
    p.add_argument("--workers", type=int, default=24)
    a = p.parse_args(argv)
    OUT.mkdir(parents=True, exist_ok=True)
    if a.phase == "extract":
        extract()
    elif a.phase == "split":
        split()
    else:
        prepare(a.variant, a.workers)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
