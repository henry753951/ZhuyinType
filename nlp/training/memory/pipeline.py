"""zt train memory: compile the lexicon and both language models into the mmap binaries measured in REPORT.md.

    zt train memory                      # data/build/memory/binary/{lexicon.bin, lm/lm.bin, lm_chat/lm.bin}

Standard library only; runs on the Mac or the GPU host and gives byte-identical files on both.  The app build compiles
the same way into its Resources (`zt data compile`).  Memory and latency are measured with
`nlp/eval/mac_metrics.py`; TSV/binary output parity with `zt --remote eval` (REPORT.md).
"""

from __future__ import annotations

from nlp.training.lm.pipeline import run
from nlp.training.pipeline import Pipeline, Step

PIPELINE = Pipeline(
    name="memory",
    help="mmap lexicon and language-model binaries (ZTLX/1, ZTLM/1) for the memory and parity checks",
    output="memory",
    steps=(
        Step(
            "compile",
            "lexicon.tsv, lm/ and lm_chat/ TSVs → memory/binary (deterministic)",
            run("nlp.training.lm.compile_data", lambda ctx: str(ctx.out / "binary"), group=None),
        ),
    ),
)
