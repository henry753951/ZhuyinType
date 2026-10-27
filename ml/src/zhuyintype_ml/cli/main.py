"""zt: one command line for ZhuyinType's data, builds, evaluations, training and the GPU host.

Each module in ml/src/zhuyintype_ml/cli/commands registers one command group (command pattern): it adds its parser with
`register(subparsers)` and sets `func` to the handler.  Adding a command is adding a module; this file never changes.

    zt help                     every command
    zt <command> --help         its options
    zt --remote <command> ...   the same command on the GPU host (ml/src/zhuyintype_ml/cli/commands/remote.py)
"""

from __future__ import annotations

import argparse
import importlib
import pkgutil
import sys

from zhuyintype_ml.cli import commands


def parser() -> tuple[argparse.ArgumentParser, argparse._SubParsersAction[argparse.ArgumentParser]]:
    root = argparse.ArgumentParser(prog="zt", description="ZhuyinType tools. `zt help` lists every command.")
    root.add_argument(
        "--remote",
        action="store_true",
        help="run the command on the GPU host from .remote.env (see `zt remote --help`)",
    )
    sub = root.add_subparsers(dest="command", metavar="COMMAND")
    for module in sorted(pkgutil.iter_modules(commands.__path__), key=lambda m: m.name):
        importlib.import_module(f"zhuyintype_ml.cli.commands.{module.name}").register(sub)
    return root, sub


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["--remote"]:
        from zhuyintype_ml.cli.commands.remote import run_remote

        return run_remote(argv[1:])
    root, sub = parser()
    if not argv or argv[0] == "help":
        root.print_help()
        print("\ncommands:")
        for name, action in sub.choices.items():
            print(f"  {name:10} {action.description or ''}")
        return 0
    args = root.parse_args(argv)
    if not getattr(args, "func", None):
        root.parse_args([args.command, "--help"])
    return int(args.func(args) or 0)


if __name__ == "__main__":
    sys.exit(main())
