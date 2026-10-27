"""zt eval: measure the engine on evaluation sets (ml/src/zhuyintype_ml/eval), compare results, run sweeps.

    zt eval sets                                       evaluation sets and whether each is present
    zt eval conversion --set colloquial-dev            one evaluation (zt eval NAME --help for its options)
    zt eval stats base.json new.json                   paired comparison of two reports
    zt eval sweep ml/src/zhuyintype_ml/eval/matrices/validation.json    configurations × sets

Evaluations are registered in ml/src/zhuyintype_ml/eval/registry.py; each adds its own subcommand here.
"""

from __future__ import annotations

import argparse
import sys

from zhuyintype_ml.core import sets
from zhuyintype_ml.eval import registry, stats, sweep


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "eval", description="Measure the engine on evaluation sets.", help="measure the engine on evaluation sets"
    )
    tools = parser.add_subparsers(dest="tool", metavar="TOOL")
    for evaluator in registry.all():
        tool = tools.add_parser(
            evaluator.name,
            description=sys.modules[type(evaluator).__module__].__doc__,
            help=evaluator.help,
            formatter_class=argparse.RawDescriptionHelpFormatter,
        )
        evaluator.configure(tool)
        tool.set_defaults(func=lambda args, e=evaluator: e.run(args) and 0)
    for name, module, text in (
        ("stats", stats, "paired comparison of two results (McNemar, bootstrap)"),
        ("sweep", sweep, "configurations × sets from a matrix file"),
    ):
        command = tools.add_parser(
            name, description=module.__doc__, help=text, formatter_class=argparse.RawDescriptionHelpFormatter
        )
        module.add_arguments(command)
        command.set_defaults(func=module.run)
    listing = tools.add_parser("sets", help="list evaluation sets and whether each is present")
    listing.set_defaults(func=list_sets)


def list_sets(_args: argparse.Namespace) -> int:
    for s in sets.SETS:
        print(f"{'✓' if s.present else '·'} {s.name:18} {s.role:8} {s.kind:5} {s.description}  [{s.source}]")
    print("\ngroups: " + "; ".join(f"{k} = {' '.join(v)}" for k, v in sets.GROUPS.items()))
    return 0
