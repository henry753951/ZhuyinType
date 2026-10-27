"""zt bench: ZhuyinBench, the fixed evaluation of every release (zhuyintype_ml/eval/bench, results in docs/benchmarks/).

    zt bench run v0.4.0                # on the GPU host: prepare, run the suite, write the record and the pages
    zt bench run check-2 --quick       # 50 runs per set, to check the harness
    zt bench history                   # rebuild docs/benchmarks/README.md and the version pages

`run` first prepares the host the way a release needs it: the pinned data and models, zhuyin-cli built for this
platform, the compiled lexicon and language models, and the ONNX models (Linux).  --no-prepare skips that.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.eval.bench import history, suite


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "bench",
        description="ZhuyinBench: the fixed evaluation of every release.",
        help="ZhuyinBench (release evaluation)",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    actions = parser.add_subparsers(dest="action", metavar="ACTION")
    run_ = actions.add_parser("run", help="run the suite for a version and rebuild the pages")
    run_.add_argument("version", help="version label, e.g. v0.4.0 (records named v* appear in the history)")
    run_.add_argument("--quick", action="store_true", help="50 runs per set, for checking the harness")
    run_.add_argument("-n", type=int, default=400, help="runs per text set (default: 400)")
    run_.add_argument("--jobs", type=int, default=None, help="parallel engine processes (default: CPU count)")
    run_.add_argument("--out", type=Path, help="record file (default: docs/benchmarks/VERSION.json)")
    run_.add_argument(
        "--data",
        type=lambda p: Path(p).resolve(),
        default=paths.data_dir(),
        help="data directory (default: data/build)",
    )
    run_.add_argument(
        "--cli-args",
        action="append",
        default=[],
        metavar="OPTIONS",
        help="zhuyin-cli options for every configuration, for checks only (recorded in the record)",
    )
    run_.add_argument("--no-prepare", action="store_true", help="skip fetching, building and exporting")
    run_.set_defaults(func=run)
    actions.add_parser("history", help="rebuild docs/benchmarks/README.md and the version pages").set_defaults(
        func=lambda _args: history.main()
    )
    parser.set_defaults(func=lambda _args: parser.print_help())


def prepare() -> None:
    """What a release evaluation needs on the host: the pinned components of the `eval` group (lexicon, lm, lm_chat,
    reranker, v3, eval-sets, checkpoints), zhuyin-cli in release mode, the compiled data and, on Linux, the ONNX
    models exported from the checkpoints (with the prefix-tree reranker; skipped when current)."""
    from zhuyintype_ml.remote import models

    models.prepare(("eval",), tree=True)


def run(args: argparse.Namespace) -> int:
    if not args.no_prepare:
        prepare()
    n = 50 if args.quick else args.n
    record = suite.run_suite(args.version, n, args.jobs, args.data, args.cli_args)
    out = args.out or paths.DOCS / "benchmarks" / f"{args.version}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(record, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {paths.relative(out)}")
    return history.main()
