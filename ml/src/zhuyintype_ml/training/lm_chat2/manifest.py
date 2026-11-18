"""chat2 provenance: source-manifest.json, attribution.jsonl and NOTICE.md, before any training.

source-manifest.json records the hash and size of every reused input (the 0.3.0 colloquial corpus, Common Voice
splits, the seed lexicon, the estimator code, the Qwen3-8B weights), every raw-download receipt and the licences;
attribution.jsonl lists each source document (gazette links and dates, talk-page revisions and history links);
NOTICE.md states the terms.  Keep all three with the model.  Standard library only.

`extend` adds the sources of the variants shipped since 0.7.2 (PTT Gossiping, TOCP, the filtered simplified-Chinese
corpora, the Claude Haiku themed sentences) to a copy of these files: their notice, licences, download receipts and
the SHA-256 of each generated file the variant read.  `zt train lm_chat2 ship --option variant=NAME` calls it.

    zt --remote train lm_chat2 manifest
    python -m zhuyintype_ml.training.lm_chat2.manifest
"""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.lm.files import digest, raw, save_json

OUT = paths.data("lm_chat2")
RAW = raw("lm_chat2")
# The training code of the estimator and of the 0.3.0 chat model the chat2 models reuse.
CODE = [paths.ROOT / "zhuyintype_ml/training/lm/mkn.py", paths.ROOT / "zhuyintype_ml/training/lm_chat/train.py"]


# The terms of the sources the 0.7.2 variants add (docs/research/corpus-sources.md).
ADDED_LICENSES = {
    "ptt_gossiping": (
        "Apache-2.0 (zake7749/Gossiping-Chinese-Corpus 2.0, compiler's statement); "
        "the titles and pushes belong to their PTT authors"
    ),
    "tocp": "CC BY-SA 4.0 (NTOU NLP Lab, Hsu Yang and Chuan-Jie Lin); sentences by PTT and Twitch users",
    "zh_meme": "MIT tag on Hugging Face, Apache-2.0 badge in its README (GaryYang123)",
    "lccc": "MIT tag (silver/lccc mirror); Tsinghua CoAI LCCC states research use",
    "mnbvc_zhihu": "MIT tag (liwu/MNBVC); answers by Zhihu users",
    "generated": "Anthropic Claude Haiku 5.5 output, used under Anthropic's terms",
}
ADDED_NOTICE = (
    "\nSince 0.7.2 (variant {variant}) the model also counts the following text. source-manifest.json keeps each "
    "download's URL, revision and SHA-256 (added_receipts), the SHA-256 of each generated file (generated_files) "
    "and the counts per source (shipped_variant).\n\n"
    "PTT Gossiping (八卦版) title/push pairs from zake7749/Gossiping-Chinese-Corpus 2.0, commit "
    "65b7e3630a560223a2b4d702d78d120d5ff1e8dd, https://github.com/zake7749/Gossiping-Chinese-Corpus "
    "(marked Apache-2.0 by the compiler; the titles and pushes belong to their PTT authors). Only the text is "
    "read, never account names or votes.\n\n"
    "TOCP v1.0, NTOU NLP Lab (Hsu Yang, Chuan-Jie Lin; LREC 2020 TRAC-2), "
    "https://nlp.cse.ntou.edu.tw/resources/TOCP/ , CC BY-SA 4.0; the sentences are by PTT and Twitch users.\n\n"
    "Simplified-Chinese corpora, converted with OpenCC s2twp and filtered to the Taiwan-like sentences "
    "(cross-entropy difference, without mainland-specific words; select.py keeps about a fifth): "
    "GaryYang123/zh-meme-sft-8k revision 84838bb5b1b023325499012ba956c485bbf592b4 (marked MIT on Hugging Face, "
    "Apache-2.0 in its README; comments by Douyin, Xiaohongshu and Bilibili users, LLM-augmented); LCCC-base by "
    "Tsinghua CoAI (CDial-GPT, Wang et al. 2020), silver/lccc mirror revision "
    "5bd582fa28cd7143f2f9c852e08e23089d677c44 (MIT tag; released for research; Weibo users' posts); MNBVC by "
    "the liwu community, revision c153d877b4af9c66026ee6fa4eefbc29dd8c11b0, 知乎 shard qa/20230196 (MIT tag; "
    "Zhihu users' answers). MNBVC's game text and the Gossiping pushes archive (reply.7z) are cached with "
    "receipts but not used.\n\n"
    "Themed Taiwanese colloquial sentences written by Anthropic Claude Haiku 5.5 (2026-11-17) from topic and "
    "category prompts only, used under Anthropic's terms. No user text went into the prompts, and the sentences "
    "seeded with a user's corrections (seed-*.txt) are not in this model.\n\n"
    "Neither PTT, the compilers nor the platforms endorse this model. The source texts are not redistributed.\n"
)


def extend(target: Path, variant: str, generated: list[Path]) -> None:
    """Adds the 0.7.2 sources to target's NOTICE.md and source-manifest.json (copies of the files `main` writes)."""
    receipts = {
        paths.relative(p): json.loads(p.read_text())
        for folder in ["ptt", "others"]
        for p in sorted((RAW / folder).rglob("*.receipt.json"))
    }
    manifest = json.loads((target / "source-manifest.json").read_text())
    manifest["licenses"] = {**manifest["licenses"], **ADDED_LICENSES}
    manifest["added_receipts"] = receipts
    manifest["generated_files"] = {p.name: {"sha256": digest(p), "bytes": p.stat().st_size} for p in generated}
    save_json(target / "source-manifest.json", manifest)
    notice = (target / "NOTICE.md").read_text(encoding="utf-8")
    notice = notice.split(ADDED_NOTICE[: ADDED_NOTICE.index("{")])[0]  # a previous extend's text
    (target / "NOTICE.md").write_text(notice + ADDED_NOTICE.format(variant=variant), encoding="utf-8")


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
