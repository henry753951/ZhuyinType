"""zt train lexicon: reading coverage (the reading channel) and mined vocabulary packs on top of the base lexicon.

    zt train lexicon sources base wiktionary channel        # any machine: pinned sources, the channel
    zt --remote --push data/raw/lexicon train lexicon       # GPU host (CPU only): every step not finished yet
    zt train lexicon merge --force --option packs=wiki,chat --option max_cost=8

Options (--option KEY=VALUE):
    wiki       directory of Wikipedia training sentences (default: lm/work/sentences in the data directory)
    workers    counting processes (default: 16)
    packs      packs: the lm step re-counts with them (default: wiki,chat,registry,places); merge puts them
               into lexicon.tsv only when given (default: none, as data-4)
    offset     extra cost of the candidate's reading rows (default: 2)
    standard   0: keep CC-CEDICT's readings as the words' readings (default 1: strong Taiwan readings are the
               standard, and misread-dev and every text dev set are typed with them)
    expanded   1: every reading class expanded into rows (the A/B baseline for the engine's reading channel)
    max_cost   lowest log P(typed | word) of an expanded misreading variant (default: 8)
    char_classes  channel classes whose single-character readings become rows (default: taiwan,word)
    min_taiwan    lowest strength of a Taiwan reading carried into words (default: 0.5)
    exclude    extra directory of evaluation rows kept out of the mining corpora (every *.txt in it)
    wiki_work  a finished `zt train lm` work directory (default: lm/work), for the lm step
    dump       the Wikipedia dump that work directory came from (default: data/raw/lm/...)
    chat_data  a data directory with lm_chat2/ prepared (default: the data directory)

Output in data/build/lexicon_coverage/: base.tsv, wiktionary.tsv, channel/, mined/, packs/, lexicon.tsv.
"""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.lm.pipeline import tests
from zhuyintype_ml.training.pipeline import Context, Pipeline, Step

from . import sources

CORPORA = {
    "wiki": {"min_count": 20},
    "chat": {"min_count": 5},
    "registry": {"min_count": 10},
    "places": {"min_count": 10},
}
EVALUATION = (
    "corpus/dev_colloquial.txt",
    "corpus/dev_wikipedia.txt",
    "corpus/test_colloquial.txt",
    "lm/test_sentences.txt",
    "lm_chat/dev_chat_tt.txt",
    "lm_chat/test_chat_tt.txt",
    "lm_chat2/dev_dialogue.txt",
    "lm_chat2/test_dialogue.txt",
)


def module(name: str, *args: str | Callable[[Context], str | list[str]], group: str | None = None) -> Callable:
    def step(ctx: Context) -> None:
        resolved: list[str] = []
        for a in args:
            value = a(ctx) if callable(a) else a
            resolved.extend(value if isinstance(value, list) else [value])
        ctx.python(f"zhuyintype_ml.training.lexicon.{name}", *resolved, group=group, threads=1)

    return step


def out(*parts: str) -> Callable[[Context], str]:
    return lambda ctx: str(ctx.out.joinpath(*parts))


def excludes(ctx: Context) -> list[str]:
    files = [ctx.data / p for p in EVALUATION if (ctx.data / p).exists()]
    files += sorted((paths.EVAL_SETS).rglob("*.txt"))
    extra = ctx.options.get("exclude")
    if extra:
        files += sorted(Path(str(extra)).rglob("*.txt"))
    return [arg for f in files for arg in ("--exclude", str(f))]


def corpus_files(name: str, ctx: Context) -> list[str]:
    if name == "wiki":
        directory = Path(str(ctx.options.get("wiki", ctx.data / "lm" / "work" / "sentences")))
        return [str(p) for p in sorted(directory.glob("*.txt"))]
    if name == "chat":
        return [str(ctx.data / "lm_chat2" / "real_dialogues.jsonl"), str(ctx.data / "corpus" / "train_commonvoice.txt")]
    return [str(sources.path("BGMOPEN1.zip"))]


def prepare(ctx: Context) -> None:
    for name in CORPORA:
        field = ["--registry-field", "address"] if name == "places" else []
        ctx.python(
            "zhuyintype_ml.training.lexicon.mine",
            "prepare",
            *corpus_files(name, ctx),
            "--corpus",
            name,
            "--out",
            str(ctx.out / "mined"),
            *field,
            *excludes(ctx),
            group="train",
        )


def count(ctx: Context) -> None:
    for name, settings in CORPORA.items():
        ctx.python(
            "zhuyintype_ml.training.lexicon.mine",
            "count",
            "--corpus",
            name,
            "--out",
            str(ctx.out / "mined"),
            "--workers",
            str(ctx.options.get("workers", 16)),
            "--min-count",
            str(settings["min_count"]),
            group="train",
            threads=1,
        )
        ctx.python(
            "zhuyintype_ml.training.lexicon.mine",
            "select",
            "--corpus",
            name,
            "--out",
            str(ctx.out / "mined"),
            "--lexicon",
            str(ctx.out / "base.tsv"),
            "--lm",
            str(ctx.data / "lm"),
            group="train",
        )


def merge(ctx: Context) -> None:
    # data-4 has no packs: with the re-counted models they change nothing measurable and cost p95 per key
    # (docs/research/log.md 2026-10-28); --option packs=wiki,chat,registry,places builds that candidate.
    packs = [p for p in str(ctx.options.get("packs", "")).split(",") if p]
    ctx.python(
        "zhuyintype_ml.training.lexicon.merge",
        "--base",
        str(ctx.out / "base.tsv"),
        "--channel",
        str(ctx.out / "channel"),
        "--wiktionary",
        str(ctx.out / "wiktionary.tsv"),
        *[arg for p in packs for arg in ("--pack", str(ctx.out / "packs" / f"{p}.tsv"))],
        "--max-cost",
        str(ctx.options.get("max_cost", 8)),
        *variant_options(ctx),
        "--out",
        str(ctx.out / "lexicon.tsv"),
    )


# The merge candidate (docs/research/log.md 2026-10-28): strong Taiwan readings as the words' readings, readings
# Taiwan's sources attest as rows 2 nats below the word's reading; the engine's reading channel takes the rest.
# --option expanded=1 builds the A/B baseline with every class expanded into rows; --option standard=0 keeps
# CC-CEDICT's readings as the words' readings.
CANDIDATE_CLASSES = ("char", "taiwan", "mcbopomofo", "variants")
CANDIDATE_OFFSET = 2.0
# Each single-character row is matched at every syllable: CNS11643's and Wiktionary's other character readings
# (3,700 rows) cost 8% CPU per key on the Mac with no change on any set, so only the readings Taiwan's sources and
# dictionary words give a character stay rows (蚵 ㄎㄜ); the rest are the engine's.  Taiwan readings weaker than 0.5
# carried into every word with the character (17,000 rows) change nothing either.
CANDIDATE_CHARACTERS = "taiwan,word"
CANDIDATE_MIN_TAIWAN = 0.5


def standard(ctx: Context) -> bool:
    return str(ctx.options.get("standard", "1")) == "1"


def variant_options(ctx: Context) -> list[str]:
    if str(ctx.options.get("expanded", "0")) == "1":
        return []
    classes = ["standard", *CANDIDATE_CLASSES] if standard(ctx) else CANDIDATE_CLASSES
    offset = str(ctx.options.get("offset", CANDIDATE_OFFSET))
    return [
        "--data-only",
        "--mechanisms",
        ",".join(classes),
        *[arg for c in CANDIDATE_CLASSES for arg in ("--offset", f"{c}={offset}")],
        "--char-classes",
        str(ctx.options.get("char_classes", CANDIDATE_CHARACTERS)),
        "--min-taiwan",
        str(ctx.options.get("min_taiwan", CANDIDATE_MIN_TAIWAN)),
    ]


def misread(ctx: Context) -> None:
    texts = [ctx.data / p for p in ("corpus/dev_wikipedia.txt", "corpus/dev_colloquial.txt", "lm_chat/dev_chat_tt.txt")]
    spoken = [
        ctx.data / p for p in ("corpus/dev_colloquial.txt", "lm_chat/dev_chat_tt.txt", "lm_chat2/dev_dialogue.txt")
    ]
    ctx.python(
        "zhuyintype_ml.training.lexicon.misread",
        *[str(p) for p in texts],
        "--spoken",
        *[str(p) for p in spoken if p.exists()],
        "--base",
        str(ctx.out / "base.tsv"),
        "--channel",
        str(ctx.out / "channel"),
        "--wiktionary",
        str(ctx.out / "wiktionary.tsv"),
        *(["--standard"] if standard(ctx) else []),
        "--out",
        str(ctx.out / "misread"),
    )


def language_models(ctx: Context) -> None:
    packs = [p for p in str(ctx.options.get("packs", ",".join(CORPORA))).split(",") if p]
    ctx.python(
        "zhuyintype_ml.training.lexicon.lm",
        "--base",
        str(ctx.out / "base.tsv"),
        *[arg for p in packs for arg in ("--pack", str(ctx.out / "packs" / f"{p}.tsv"))],
        "--out",
        str(ctx.out),
        "--wiki-work",
        str(ctx.options.get("wiki_work", ctx.data / "lm" / "work")),
        "--dump",
        str(ctx.options["dump"])
        if "dump" in ctx.options
        else str(paths.raw("lm", "zhwiki-latest-pages-articles.xml.bz2")),
        "--chat-data",
        str(ctx.options.get("chat_data", ctx.data)),
        "--workers",
        str(ctx.options.get("workers", 16)),
    )


def base(ctx: Context) -> None:
    ctx.python("zhuyintype_ml.data.lexicon", str(ctx.out / "base.tsv"), group="data")


PIPELINE = Pipeline(
    name="lexicon",
    help="reading channel and mined vocabulary packs over the base lexicon",
    output="lexicon_coverage",
    steps=(
        Step("test", "unit tests: Tâi-lô mapping, channel, mining, merging", tests("zhuyintype_ml/training/lexicon")),
        Step("sources", "pinned downloads into data/raw/lexicon, SHA-256 checked", module("sources")),
        Step("base", "the base lexicon (CC-CEDICT, Unihan, wordfreq) as base.tsv", base),
        Step(
            "wiktionary",
            "readings, Hokkien readings and phonetic components from Wiktionary",
            module("wiktionary", out("wiktionary.tsv")),
        ),
        Step(
            "channel",
            "P(reading | character) from dictionaries, Unihan, phonetic series and Hokkien readings",
            module(
                "channel",
                "--base",
                out("base.tsv"),
                "--wiktionary",
                out("wiktionary.tsv"),
                "--spoken",
                lambda ctx: str(ctx.data / "lm_chat2" / "real_dialogues.jsonl"),
                lambda ctx: str(ctx.data / "corpus" / "train_commonvoice.txt"),
                "--chat-lm",
                lambda ctx: str(ctx.data / "lm_chat" / "unigram.tsv"),
                "--wiki-lm",
                lambda ctx: str(ctx.data / "lm" / "unigram.tsv"),
                "--out",
                out("channel"),
            ),
        ),
        Step("misread", "misread-dev: dev sentences typed with realistic reading variants (zt eval rows)", misread),
        Step(
            "newtitles",
            "wiki-new-dev: names of Wikipedia articles created after the dump (needs the network)",
            module("newtitles", "--base", out("base.tsv"), "--out", out("wiki-new"), group="train"),
        ),
        Step("prepare", "corpora as code-point arrays, evaluation overlap removed (GPU host)", prepare),
        Step("mine", "n-gram counts, branching entropy, word selection per corpus (GPU host, numpy)", count),
        Step(
            "vocabulary",
            "readings (v3 polyphone classifier) and calibrated frequencies of the mined words, by corpus",
            module(
                "vocabulary",
                "--mined",
                out("mined"),
                "--lexicon",
                out("base.tsv"),
                "--corpus",
                lambda ctx: list(CORPORA),
                "--out",
                out("packs"),
                "--polyphone",
                lambda ctx: str(ctx.data / "v3_030"),
                "--check",
                lambda ctx: str(ctx.data),
                "--jobs",
                lambda ctx: str(ctx.options.get("workers", 16)),
                group="train",
            ),
        ),
        Step("lm", "Wikipedia and chat bigrams re-segmented with the packs' words (GPU host)", language_models),
        Step("merge", "lexicon.tsv: base rows, channel variants, packs", merge),
    ),
)
