"""zt train corpus: the colloquial and Wikipedia dev/test sets, then the generated colloquial corpus.

    zt --remote train corpus splits          # needs the Wikipedia training shards (zt train lm train)
    zt --remote train corpus generate        # GPU host: Qwen3-8B server, generator and cleaner until 6M sentences

`splits` freezes the evaluation sets before anything is generated.  `generate` starts the vLLM server (log:
data/build/corpus/vllm.log), runs the generator and the cleaner side by side, and stops the server when both are done.
To stop early, create data/build/corpus/STOP_GENERATION, wait for the generator to exit, then create STOP_CLEANER;
remove both files to resume.  The 0.3.0 v3 colloquial corpus (v3_030/colloquial.txt) is built from this output.
"""

from __future__ import annotations

from nlp.training.lm.pipeline import run, tests
from nlp.training.pipeline import Context, Pipeline, Step


def generate(ctx: Context) -> None:
    out = str(ctx.out)
    server = ctx.spawn("nlp.training.corpus.serve", group="generate", log="vllm.log")
    try:
        cleaner = ctx.spawn("nlp.training.corpus.clean", "--output", out, group="train", threads=1)
        try:
            ctx.python("nlp.training.corpus.generate", "--output", out, group="train")
            if cleaner.wait():
                raise RuntimeError(f"cleaner exited with {cleaner.returncode}")
        finally:
            if cleaner.poll() is None:
                cleaner.terminate()
                cleaner.wait()
    finally:
        server.terminate()
        server.wait()


PIPELINE = Pipeline(
    name="corpus",
    help="colloquial and Wikipedia dev/test sets; Qwen3-8B colloquial corpus",
    output="corpus",
    steps=(
        Step(
            "test",
            "unit tests of cleaning, held-out exclusion, deduplication and prompts",
            tests("nlp/training/corpus"),
        ),
        Step(
            "splits",
            "Common Voice colloquial dev/test/train and Wikipedia dev, frozen before generation",
            run("nlp.training.corpus.prepare", "--output", lambda ctx: str(ctx.out)),
        ),
        Step("generate", "vLLM Qwen3-8B, generator and cleaner until 6M clean sentences (GPU host)", generate),
    ),
)
