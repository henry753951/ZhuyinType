"""zt remote: run zt commands on the GPU host, and follow them (nlp/remote/runner.py).

    zt --remote eval conversion --config engine -n 3 --set smoke     # any zt command, on the host
    zt --remote --workdir onnx-eval-x --export eval conversion --json out.json
    zt --remote --backend cuda eval conversion --set colloquial-dev  # model calls on the GPU (PyTorch server)
    zt --remote --push data/build/v3_new --detach train v3          # copy local files, keep running after logout
    zt remote status | logs [-f] | pull [PATH ...] | stop | shell | sync | list

The host comes from ZHUYIN_REMOTE_HOST / ZHUYIN_REMOTE_KEY in .remote.env or the environment (.remote.env.example).
Each checkout has its own work directory on the host (<base>/work/<checkout>-<hash>) unless --workdir names one;
runs log to <base>/logs.  Options of `zt --remote` come before the command:

  --workdir NAME      work directory under <base>/work (default: one per checkout)
  --push PATH         also copy this local file or directory (inside the checkout) to the host (repeatable)
  --pull PATH         also copy this path (relative to the checkout) back after the run (repeatable)
  --fetch COMPONENTS  data to fetch first, comma-separated (default: the eval group for eval, bench, models and
                      train, nothing for other commands; `none` skips it)
  --export            export the ONNX models from the checkpoints first (cached; needed for --config app)
  --backend cpu|cuda  cpu: ONNX Runtime in every engine process (default); cuda: one PyTorch model server on the GPU
  --group GROUP       also install a dependency group on the host (repeatable)
  --env KEY=VALUE     environment for the run, e.g. ZT_BUILD_JOBS=16 (repeatable)
  --detach            keep running on the host after this command returns (nohup); see `zt remote status`
"""

from __future__ import annotations

import argparse
import sys

from nlp.remote.host import Host, check_name, default_workdir
from nlp.remote.runner import Options, RemoteRunner, runner


def run_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--workdir", type=check_name, default=None, help="work directory under <base>/work")
    parser.add_argument("--push", action="append", default=[], metavar="PATH", help="local path to copy over")
    parser.add_argument("--pull", action="append", default=[], metavar="PATH", help="path to copy back")
    parser.add_argument(
        "--fetch",
        default=None,
        metavar="COMPONENTS",
        help="data components to fetch, comma-separated (default: eval for eval/bench/models/train; none: skip)",
    )
    parser.add_argument("--export", action="store_true", help="export the ONNX models first")
    parser.add_argument("--backend", choices=["cpu", "cuda"], default="cpu", help="model runtime (default: cpu)")
    parser.add_argument("--group", action="append", default=[], help="dependency group to install on the host")
    parser.add_argument("--env", action="append", default=[], metavar="KEY=VALUE", help="environment for the run")
    parser.add_argument("--detach", action="store_true", help="keep running after this command returns")


def options_from(args: argparse.Namespace) -> Options:
    fetch = (
        None if args.fetch is None else [] if args.fetch in ("", "none") else [c for c in args.fetch.split(",") if c]
    )
    return Options(
        args.workdir or default_workdir(),
        args.push,
        args.pull,
        fetch,
        args.export,
        args.backend,
        args.detach,
        args.env,
        args.group,
    )


def run_remote(argv: list[str]) -> int:
    """`zt --remote [options] COMMAND ...`."""
    parser = argparse.ArgumentParser(
        prog="zt --remote", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    run_options(parser)
    parser.add_argument("command", nargs=argparse.REMAINDER, help="the zt command and its arguments")
    args = parser.parse_args(argv)
    if not args.command:
        parser.error("give a zt command to run on the host, e.g. `zt --remote setup`")
    return runner(Host.load()).run(args.command, options_from(args))


def remote_runner() -> RemoteRunner:
    host = Host.load()
    if host.is_local:
        raise SystemExit("zt: ZHUYIN_REMOTE_HOST is `local`; there is no host to inspect")
    return RemoteRunner(host)


def register(sub: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = sub.add_parser(
        "remote",
        description="Run zt commands on the GPU host and follow them.",
        help="the GPU host: run, status, logs, pull, stop, shell",
        epilog=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    actions = parser.add_subparsers(dest="action", metavar="ACTION")

    def workdir(action: argparse.ArgumentParser) -> argparse.ArgumentParser:
        action.add_argument(
            "--workdir",
            type=check_name,
            default=None,
            help="work directory under <base>/work (default: this checkout's)",
        )
        return action

    run = actions.add_parser("run", help="run a zt command on the host (the same as `zt --remote`)")
    run_options(run)
    run.add_argument("command", nargs=argparse.REMAINDER)
    run.set_defaults(
        func=lambda a: (
            runner(Host.load()).run(a.command, options_from(a)) if a.command else run.error("give a zt command")
        )
    )

    sync = workdir(actions.add_parser("sync", help="copy the checkout's sources (and --push paths) to the host"))
    sync.add_argument("--push", action="append", default=[], metavar="PATH")
    sync.set_defaults(func=lambda a: remote_runner().sync(a.workdir or default_workdir(), a.push))

    workdir(actions.add_parser("status", help="host load, the run in progress, its last lines")).set_defaults(
        func=lambda a: remote_runner().status(a.workdir or default_workdir())
    )

    logs = workdir(actions.add_parser("logs", help="the last run's log"))
    logs.add_argument("-f", "--follow", action="store_true", help="keep printing as it grows")
    logs.add_argument("-n", "--lines", type=int, default=40, help="lines to show (default: 40)")
    logs.set_defaults(func=lambda a: remote_runner().logs(a.workdir or default_workdir(), a.follow, a.lines))

    pull = workdir(actions.add_parser("pull", help="copy back data/build/remote_eval and other paths"))
    pull.add_argument("paths", nargs="*", help="paths relative to the checkout")
    pull.set_defaults(func=lambda a: remote_runner().pull(a.workdir or default_workdir(), a.paths))

    workdir(actions.add_parser("stop", help="stop the detached run (by its recorded PID)")).set_defaults(
        func=lambda a: remote_runner().stop(a.workdir or default_workdir())
    )
    workdir(actions.add_parser("shell", help="a login shell in the work directory")).set_defaults(
        func=lambda a: remote_runner().shell(a.workdir or default_workdir())
    )
    actions.add_parser("list", help="work directories on the host and their last exit status").set_defaults(
        func=lambda a: remote_runner().list()
    )
    parser.set_defaults(func=lambda a: parser.print_help())


if __name__ == "__main__":
    sys.exit(run_remote(sys.argv[1:]))
