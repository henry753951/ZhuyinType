"""zt data: the data and models that are not in git (nlp/data).

zt data fetch                     # the app's components into data/build (pinned in nlp/data/assets.json)
zt data fetch eval                # everything evaluations need, including the checkpoints for ONNX export
zt data status                    # what is present, and whether it is the pinned version
zt data pack data-4 v3            # archive changed components into dist/assets/ and pin them
zt data publish data-4            # upload them as a GitHub release (gh)
zt data lexicon [--rebuild]       # build lexicon.tsv from CC-CEDICT, Unihan and wordfreq
zt data compile [OUT]             # lexicon and language models as mmap binaries (lexicon.bin, lm/lm.bin, …)
"""

from __future__ import annotations

import argparse

from nlp.common import paths
from nlp.data import assets


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "data",
        description="Data and models that are not in git.",
        help="fetch, pack and build data",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    actions = parser.add_subparsers(dest="action", metavar="ACTION")
    groups = "; ".join(f"{name} = {' '.join(items)}" for name, items in assets.GROUPS.items() if name != "all")

    fetch = actions.add_parser(
        "fetch",
        help="download and unpack pinned components (default: app)",
        description=f"Components or groups ({groups}; all).",
    )
    fetch.add_argument("components", nargs="*", default=["app"], metavar="COMPONENT")
    fetch.add_argument("--quiet", action="store_true", help="print downloads only")
    fetch.set_defaults(func=lambda a: assets.fetch(assets.expand(a.components), a.quiet))

    status = actions.add_parser("status", help="which components are present and current")
    status.add_argument("components", nargs="*", default=["all"], metavar="COMPONENT")
    status.set_defaults(func=show_status)

    pack = actions.add_parser("pack", help="archive components into dist/assets/ and pin them in assets.json")
    pack.add_argument("release", help="release name, e.g. data-4")
    pack.add_argument("components", nargs="*", default=["all"], metavar="COMPONENT")
    pack.set_defaults(func=lambda a: assets.pack(a.release, assets.expand(a.components)))

    publish = actions.add_parser("publish", help="upload a packed release to GitHub (gh)")
    publish.add_argument("release")
    publish.set_defaults(func=lambda a: assets.publish(a.release))

    lexicon = actions.add_parser("lexicon", help="build lexicon.tsv from open data (reused when it exists)")
    lexicon.add_argument("--rebuild", action="store_true", help="build it even when it exists")
    lexicon.set_defaults(func=build_lexicon)

    compile_ = actions.add_parser("compile", help="compile the lexicon and language models into mmap binaries")
    compile_.add_argument(
        "output",
        nargs="?",
        default=None,
        help="output directory (default: next to the TSV files, where evaluations load them)",
    )
    compile_.add_argument("--source", default=None, help="data directory to read (default: the data directory)")
    compile_.set_defaults(func=compile_data)

    parser.set_defaults(func=lambda a: parser.print_help())


def show_status(args: argparse.Namespace) -> int:
    rows = assets.status(assets.expand(args.components))
    print(f"data directory: {paths.relative(paths.data_dir())}")
    for row in rows:
        mark = "✓" if row.current else ("~" if row.present else "·")
        state = "current" if row.current else ("present, not from the pinned archive" if row.present else "missing")
        print(f"{mark} {row.name:12} {row.pinned or '-':8} {state}")
    return 0


def build_lexicon(args: argparse.Namespace) -> int:
    from nlp.data import lexicon

    out = lexicon.output()
    if out.exists() and out.stat().st_size and not args.rebuild:
        print(f"沿用詞庫：{paths.relative(out)}")
        return 0
    if not (lexicon.RAW / "cedict.txt").exists() or not (lexicon.RAW / "Unihan_Readings.txt").exists():
        lexicon.fetch_sources()
    return paths.run_python("nlp.data.lexicon", groups=("data",))


def compile_data(args: argparse.Namespace) -> int:
    from pathlib import Path

    from nlp.data import compiled

    compiled.compile_all(
        Path(args.source).resolve() if args.source else None, Path(args.output).resolve() if args.output else None
    )
    return 0
