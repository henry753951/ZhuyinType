"""zt systems: the other input methods the comparison runs (zhuyintype_ml/eval/systems), pinned and built locally.

    zt --remote systems setup                # on the GPU host: clone 0c and 善解 at the pinned commits, build them
    zt systems setup shanjie --jobs 8        # one of them, with 8 build jobs
    zt systems status                        # what is set up here

Everything goes under $ZT_SYSTEMS_DIR (default data/build/systems), never into this repository.
"""

from __future__ import annotations

import argparse

from zhuyintype_ml.eval.systems import references


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "systems",
        description="Other input methods for the comparison: pinned, built locally.",
        help="other input methods for zt bench compare",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    actions = parser.add_subparsers(dest="action", metavar="ACTION")
    setup = actions.add_parser("setup", help="clone at the pinned commits, fetch their assets, build")
    setup.add_argument("names", nargs="*", metavar="NAME", help="0c, shanjie (default: both)")
    setup.add_argument("--jobs", type=int, default=None, help="build jobs (default: $ZT_BUILD_JOBS or up to 8)")
    setup.set_defaults(func=run_setup)
    actions.add_parser("status", help="what is set up here").set_defaults(func=lambda _a: references.status())
    parser.set_defaults(func=lambda _args: parser.print_help())


def run_setup(args: argparse.Namespace) -> int:
    unknown = [n for n in args.names if n not in references.REFERENCES]
    if unknown:
        raise SystemExit(f"zt systems: unknown {', '.join(unknown)}; known: {', '.join(references.REFERENCES)}")
    return references.setup(args.names or list(references.REFERENCES), args.jobs)
