"""zt train lm_chat2: the chat word bigram 0.4.0 ships as lm_chat/ (gazette, talk pages, Common Voice).

    zt train lm_chat2 sources                     # may run on the Mac: cached downloads with receipts
    zt --remote train lm_chat2                    # GPU host: test … ship
    zt --remote train lm_chat2 perplexity --force

Steps run in the order they were run.  Review data/build/lm_chat2/source_samples.json after `extract` and before
`split`.  `split`, `real` and `mixed` never redo finished work (their modules refuse to); `--force` on them only
reports that.  `real` is the shipped variant (no_generated/); `ship` copies it with its provenance into lm_chat/,
first archiving the 0.3.0 TSVs there as lm_chat_v1/.

The web experiment (2026-11-17, docs/research/slang-inventory.md proposal 2) adds public-written text:
`web-sources` caches 公共政策網路參與平臺 proposals and the vTaiwan conversation, `control` retrains the shipped
real variant in a fresh tree (control/), and `web` trains real plus that text (web/).  Neither replaces lm_chat/.
"""

from __future__ import annotations

import json
import shutil
from collections.abc import Callable

from zhuyintype_ml.training.lm.pipeline import run, tests
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

MODULE = "zhuyintype_ml.training.lm_chat2"


def sources(ctx: Context) -> None:
    """The three sources as cached for the experiment (the gazette API stopped at 506 of 800 documents)."""
    for source, *options in (["government", "--documents", "800"], ["wikipedia", "--batches", "20"], ["similar"]):
        ctx.python(f"{MODULE}.sources", source, *options)


def web_sources(ctx: Context) -> None:
    """公共政策網路參與平臺 proposals 2015–2026 and the vTaiwan uberx comments, with receipts."""
    ctx.python(f"{MODULE}.sources", "join")
    ctx.python(f"{MODULE}.sources", "vtaiwan")


def once(marker: str, step: Callable[[Context], None]) -> Callable[[Context], None]:
    """Runs `step` unless data/build/lm_chat2/<marker> exists: frozen splits and prepared corpora are never redone."""

    def guarded(ctx: Context) -> None:
        if (ctx.out / marker).exists():
            ctx.log(f"{marker} exists; kept as frozen")
            return
        step(ctx)

    return guarded


def freeze(ctx: Context) -> None:
    """The seed lexicon and the 0.3.0 chat tokenizer, copied into inputs/ unless already frozen there."""
    inputs = ctx.out / "inputs"
    tokenizer = inputs / "tokenizer"
    tokenizer.mkdir(parents=True, exist_ok=True)
    for source, target in [
        (ctx.data / "lm_chat" / "inputs" / "lexicon.tsv", inputs / "lexicon.tsv"),
        (ctx.data / "lm_chat" / "work" / "vocabulary.json", tokenizer / "vocabulary.json"),
        (ctx.data / "lm_chat" / "work" / "segmentation_scores.npy", tokenizer / "segmentation_scores.npy"),
    ]:
        if not target.exists():
            shutil.copy2(source, target)
            ctx.log(f"{source} → {target}")


def variant(name: str) -> Callable[[Context], None]:
    """Prepares (once) and trains one variant: real → no_generated/, mixed → lm_chat2/, control/ and web/."""
    directory = {"real": "no_generated/work", "mixed": "work"}.get(name, f"{name}/work")
    prepare = once(
        f"{directory}/prepared.json",
        run(f"{MODULE}.prepare", "prepare", "--variant", name, "--workers", "24"),
    )
    train = run(f"{MODULE}.train", "--variant", name, "--workers", "24")

    def step(ctx: Context) -> None:
        prepare(ctx)
        train(ctx)

    return step


SHIPPED = ["unigram.tsv", "bigram.tsv", "backoff.tsv"]
PROVENANCE = ["NOTICE.md", "attribution.jsonl", "source-manifest.json"]


def ship(ctx: Context) -> None:
    """A variant and its provenance into lm_chat/: `--option variant=NAME` (0.7.2: ptt-cns-genpub; default real, as
    0.4.0 shipped it).  The 0.3.0 model goes to lm_chat_v1/.  source-manifest.json gains the variant's name and its
    per-source counts (prepared.json without the character list); a PTT variant also gets its added sources
    (manifest.extend)."""
    from .manifest import extend
    from .prepare import generated_files, variant_directory

    name = str(ctx.options.get("variant", "real"))
    source = variant_directory(name)
    target = ctx.data / "lm_chat"
    archive = ctx.data / "lm_chat_v1"
    if not archive.exists():
        archive.mkdir()
        for name in SHIPPED + ["NOTICE.md"]:
            shutil.copy2(target / name, archive / name)
        ctx.log(f"0.3.0 chat model archived in {archive}")
    for file in SHIPPED:
        shutil.copy2(source / file, target / file)
    for file in PROVENANCE:
        # Where `manifest` has not run (a copy of the data directory), keep the shipped provenance in lm_chat/.
        if (ctx.out / file).exists():
            shutil.copy2(ctx.out / file, target / file)
    prepared = source / "work" / "prepared.json"
    if prepared.exists():
        manifest = json.loads((target / "source-manifest.json").read_text())
        counts = {k: v for k, v in json.loads(prepared.read_text()).items() if k != "training_characters"}
        manifest["shipped_variant"] = {"name": name, "counts": counts}
        (target / "source-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n")
    if name.startswith("ptt"):
        extend(target, name, generated_files(name))
    ctx.log(f"chat2 {name} ({source}) → {target}")


PIPELINE = Pipeline(
    name="lm_chat2",
    help="chat word bigram from real dialogue, shipped as lm_chat/ since 0.4.0",
    output="lm_chat2",
    steps=(
        Step(
            "test",
            "unit tests of turn parsing, context selection, leakage and the estimator",
            tests("ml/src/zhuyintype_ml/training/lm_chat2"),
        ),
        Step("sources", "cache gazette transcripts, talk pages and a data.gov.tw sample with receipts", sources),
        Step(
            "extract",
            "speaker turns and review samples (review source_samples.json next)",
            run(f"{MODULE}.prepare", "extract"),
        ),
        Step(
            "split",
            "1,000 dev and 1,000 test turn pairs, whole documents per split (once)",
            once("splits.json", run(f"{MODULE}.prepare", "split")),
        ),
        Step("freeze", "seed lexicon and 0.3.0 chat tokenizer into inputs/", freeze),
        Step("manifest", "source-manifest.json, attribution.jsonl, NOTICE.md", run(f"{MODULE}.manifest", group=None)),
        Step("real", "real dialogue and Common Voice model, no_generated/ (GPU host)", variant("real")),
        Step("mixed", "real plus the 0.3.0 colloquial corpus (GPU host)", variant("mixed")),
        Step(
            "perplexity",
            "dev word perplexity of four models on one tokenization (GPU host)",
            run(f"{MODULE}.evaluate", "--phase", "perplexity"),
        ),
        Step(
            "conversion",
            "dev conversion at chat priors 0.3/0.5/0.7; stopped at 7 of 24 rows (GPU host)",
            run(f"{MODULE}.evaluate", "--phase", "conversion"),
        ),
        Step("web-sources", "cache 公共政策網路參與平臺 proposals and vTaiwan comments with receipts", web_sources),
        Step("control", "the real variant retrained in a fresh tree, control/ (GPU host)", variant("control")),
        Step("web", "real plus 公共政策網路參與平臺 and vTaiwan text, web/ (GPU host)", variant("web")),
        Step("web3", "real plus a fixed third of the proposals and vTaiwan, web3/ (GPU host)", variant("web3")),
        Step(
            "ptt-sources",
            "cache PTT Gossiping title/push pairs and TOCP with receipts",
            run(f"{MODULE}.sources", "ptt"),
        ),
        Step("ptt", "real plus PTT Gossiping pairs and TOCP sentences, ptt/ (GPU host)", variant("ptt")),
        Step(
            "others-sources",
            "cache zh-meme-sft-8k, LCCC-base and MNBVC 知乎 with receipts",
            run(f"{MODULE}.sources", "others"),
        ),
        Step("ptt-meme", "ptt plus zh-meme-sft-8k, ptt-meme/ (GPU host)", variant("ptt-meme")),
        Step("ptt-lccc", "ptt plus an eighth of LCCC-base, ptt-lccc/ (GPU host)", variant("ptt-lccc")),
        Step("ptt-zhihu", "ptt plus an eighth of the MNBVC 知乎 shard, ptt-zhihu/ (GPU host)", variant("ptt-zhihu")),
        Step("ptt-others", "ptt plus all three, ptt-others/ (GPU host)", variant("ptt-others")),
        Step("cn", "zh-meme, LCCC and 知乎 alone, the mainland model for selection, cn/ (GPU host)", variant("cn")),
        Step(
            "cn-select",
            "keep the Taiwan-like mainland sentences by cross-entropy difference (GPU host)",
            run(f"{MODULE}.select", "--workers", "24"),
        ),
        Step(
            "ptt-cn0",
            "ptt plus mainland sentences scored at least as Taiwan-like, ptt-cn0/ (GPU host)",
            variant("ptt-cn0"),
        ),
        Step("ptt-cn20", "ptt plus the most Taiwan-like fifth of them, ptt-cn20/ (GPU host)", variant("ptt-cn20")),
        Step(
            "ptt-gen",
            "ptt plus the generated sentences of data/build/private/gen, ptt-gen/ (GPU host)",
            variant("ptt-gen"),
        ),
        Step(
            "ptt-cn20-gen", "ptt-cn20 plus the generated sentences, ptt-cn20-gen/ (GPU host)", variant("ptt-cn20-gen")
        ),
        Step(
            "ptt-cns",
            "ptt plus the strict selection (no mainland-specific word), ptt-cns/ (GPU host)",
            variant("ptt-cns"),
        ),
        Step("ptt-cns-gen", "ptt-cns plus the generated sentences, ptt-cns-gen/ (GPU host)", variant("ptt-cns-gen")),
        Step(
            "ptt-genpub",
            "ptt plus the themed generated sentences only (no seeds), ptt-genpub/ (GPU host)",
            variant("ptt-genpub"),
        ),
        Step(
            "ptt-cns-genpub",
            "ptt-cns plus the themed generated sentences only, ptt-cns-genpub/ (GPU host)",
            variant("ptt-cns-genpub"),
        ),
        Step(
            "ptt-cns-genpub2",
            "ptt-cns-genpub plus the second themed batch of data/build/private/gen2, ptt-cns-genpub2/ (GPU host)",
            variant("ptt-cns-genpub2"),
        ),
        Step(
            "ptt-replies-sources",
            "cache every push of the PTT Gossiping articles (reply.7z) with a receipt",
            run(f"{MODULE}.sources", "ptt-replies"),
        ),
        Step(
            "pttr-cns-genpub2",
            "ptt-cns-genpub2 plus every push of the Gossiping articles, pttr-cns-genpub2/ (GPU host)",
            variant("pttr-cns-genpub2"),
        ),
        Step("ptt14", "the ptt corpus pruned to the shipped model's 14 MB, ptt14/ (GPU host)", variant("ptt14")),
        Step("ship", "real variant and provenance into lm_chat/, the 0.3.0 model into lm_chat_v1/", ship),
    ),
)
