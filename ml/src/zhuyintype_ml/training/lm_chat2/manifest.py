"""chat2 provenance: source-manifest.json, attribution.jsonl and NOTICE.md, before any training.

source-manifest.json records the hash and size of every reused input (the 0.3.0 colloquial corpus, Common Voice
splits, the seed lexicon, the estimator code, the Qwen3-8B weights), every raw-download receipt and the licences;
attribution.jsonl lists each source document (gazette links and dates, talk-page revisions and history links);
NOTICE.md states the terms.  Keep all three with the model.  Standard library only.

    zt --remote train lm_chat2 manifest
    python -m zhuyintype_ml.training.lm_chat2.manifest
"""

from __future__ import annotations

import argparse
import json
import shutil
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.lm.files import digest, raw, save_json

OUT = paths.data("lm_chat2")
RAW = raw("lm_chat2")
# The training code of the estimator and of the 0.3.0 chat model the chat2 models reuse.
CODE = [paths.ROOT / "zhuyintype_ml/training/lm/mkn.py", paths.ROOT / "zhuyintype_ml/training/lm_chat/train.py"]


def main(argv: list[str] | None = None) -> int:
    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args(argv)
    inputs = OUT / "inputs"
    inputs.mkdir(exist_ok=True)
    reused = [
        paths.data("v3_030", "colloquial.txt"),
        paths.data("v3_030", "colloquial.json"),
        paths.data("corpus", "train_commonvoice.txt"),
        paths.data("corpus", "splits.json"),
        paths.data("lm_chat", "inputs", "lexicon.tsv"),
        *CODE,
    ]
    weights = sorted(raw("Qwen3-8B").glob("*.safetensors"))
    files = {paths.relative(p): {"sha256": digest(p), "bytes": p.stat().st_size} for p in reused + weights}
    qwen_license = raw("Qwen3-8B", "LICENSE")
    if not qwen_license.exists():
        qwen_license = paths.data("lm_chat", "Qwen3-8B-LICENSE")
    shutil.copyfile(qwen_license, inputs / "Qwen3-8B-LICENSE")
    receipts = {paths.relative(p): json.loads(p.read_text()) for p in sorted(RAW.rglob("*.receipt.json"))}
    documents: dict[tuple[str, str], dict[str, Any]] = {}
    for line in (OUT / "real_dialogues.jsonl").open():
        row = json.loads(line)
        key = row["source"], row["document"]
        record = {k: row[k] for k in ("source", "document", "url")}
        if row["source"] == "wiki_talk":
            record.update({k: row[k] for k in ("revision", "timestamp", "title")})
            record["history_url"] = f"https://zh.wikipedia.org/w/index.php?curid={row['document']}&action=history"
            record["license"] = "CC BY-SA 4.0"
        else:
            record["original_pdf"] = row["metadata"]["公報完整PDF網址"]
            record["date"] = row["metadata"]["會議日期"]
            record["license"] = "OpenFun CC BY 4.0; Legislative Yuan OGDL v1"
        documents[key] = record
    with (OUT / "attribution.jsonl").open("w") as f:
        for key in sorted(documents):
            f.write(json.dumps(documents[key], ensure_ascii=False) + "\n")
    save_json(
        OUT / "source-manifest.json",
        {
            "files": files,
            "raw_receipts": receipts,
            "commonvoice": json.loads(paths.data("corpus", "splits.json").read_text()),
            "generator_revision": "b968826d9c46dd6066d109eabc6255188de91218",
            "generator_license_sha256": digest(inputs / "Qwen3-8B-LICENSE"),
            "gazette_metadata_revision": "16860178b7fbf234201db7d5c030f3fbab869a7a",
            "licenses": {
                "gazette": "CC BY 4.0 (OpenFun conversion); OGDL v1 (Legislative Yuan originals)",
                "wiki_talk": "CC BY-SA 4.0; Chinese Wikipedia contributors",
                "commonvoice": "CC0 1.0",
                "old_synthetic": "Qwen3-8B Apache-2.0 weights; no output-use restrictions",
                "Taiwan_Tongues": "EXCLUDED from both new training variants; existing baseline/evaluation only",
            },
            "real_dialogues_sha256": digest(OUT / "real_dialogues.jsonl"),
        },
    )
    (OUT / "NOTICE.md").write_text(
        "# ZhuyinType chat2 language models\n\n"
        "Model TSVs and the combined derived corpus: CC BY-SA 4.0, "
        "https://creativecommons.org/licenses/by-sa/4.0/ . Code: AGPL-3.0-only.\n\n"
        "Attribution: Legislative Yuan, Taiwan (2024–2025 gazettes), and OpenFun Ltd., "
        "LYAPI https://v2.ly.govapi.tw/ (CC BY 4.0). Original government data: "
        "https://data.gov.tw/license (OGDL v1). Gazette agenda metadata: "
        "https://huggingface.co/datasets/openfun/tw-ly-gazette_agenda . "
        "attribution.jsonl retains original gazette links, dates and document IDs.\n\n"
        "Chinese Wikipedia contributors, talk-page revisions fetched 2026-10-27 (CC BY-SA 4.0). "
        "attribution.jsonl retains titles, page IDs, revision IDs and exact revision links. "
        "For attribution/history use https://zh.wikipedia.org/w/index.php?curid=PAGE_ID&action=history . "
        "Terms: https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content .\n\n"
        "Common Voice sentence-collector contributors, CC0 1.0; exact commit and URL in source-manifest.json "
        "and inputs/local-input-manifest.json. Legacy Qwen synthetic text uses the existing project corpus; "
        "Qwen3-8B weight license is retained in inputs/Qwen3-8B-LICENSE. New generator pilots failed sample "
        "review and are not training data. Taiwan Tongues is excluded from both new model variants.\n\n"
        "Seed lexicon: ZhuyinType, CC BY-SA 4.0; CC-CEDICT (CC BY-SA 4.0), Unihan "
        "(Unicode License v3), wordfreq data (CC BY-SA 4.0); preserve the existing component notices.\n\n"
        "Changes: speaker/thread extraction, removal of signatures/markup/quoted or procedural material, "
        "OpenCC s2twp, document holdouts and 8-character exclusions, exact deduplication, three-pass "
        "Viterbi segmentation, supplemental turn-boundary word counts, MKN smoothing and pruning. "
        "Retain this notice, source-manifest.json, attribution.jsonl and the component notices when "
        "redistributing. Neither the Legislative Yuan nor OpenFun nor Wikimedia endorses this model.\n"
    )
    print("manifest frozen:", len(files), "reused files,", len(receipts), "raw receipts", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
