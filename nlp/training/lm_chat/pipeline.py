"""zt train lm_chat: the 0.3.0 chat word bigram and the Taiwan Tongues dev/test sets (chat-dev, chat-test).

    zt --remote train lm_chat                                 # GPU host: test, train into data/build/lm_chat
    zt --remote train lm_chat train --option output=lm_chat_rerun --option workers=48

Retired: 0.4.0 ships the chat2 model in lm_chat/ and keeps these TSVs as lm_chat_v1/.  `train` writes into
data/build/<output> (default lm_chat, as when it was trained), replacing the shipped TSVs there; give another output
to train it again.  Frozen inputs are read from data/build/lm_chat/inputs.
"""

from __future__ import annotations

from nlp.training.lm.pipeline import run, tests, workers
from nlp.training.pipeline import Context, Pipeline, Step


def output(ctx: Context) -> str:
    return str(ctx.data / str(ctx.options.get("output", "lm_chat")))


PIPELINE = Pipeline(
    name="lm_chat",
    help="0.3.0 chat word bigram (retired) and the Taiwan Tongues dev/test sets",
    output="lm_chat",
    steps=(
        Step("test", "unit tests of TSV filtering, holdouts and the serialized model", tests("nlp/training/lm_chat")),
        Step(
            "train",
            "holdouts, hard-EM MKN bigram, dev scores (GPU host; replaces lm_chat/ unless --option output=…)",
            run(
                "nlp.training.lm_chat.train",
                "--output",
                output,
                "--inputs",
                lambda ctx: str(ctx.data / "lm_chat" / "inputs"),
                "--workers",
                workers(48),
            ),
        ),
    ),
)
