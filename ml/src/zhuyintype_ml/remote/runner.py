"""Running a zt command somewhere: here (LocalRunner) or on the GPU host (RemoteRunner).  Strategy pattern: `zt
--remote` builds a Runner from .remote.env and hands it the command; the steps are the same either way.

A run on the host:

1. checks that no other run holds the work directory (<base>/work/<name>, one per checkout by default);
2. syncs the checkout's source files (tracked and untracked-but-not-ignored, as `git ls-files` lists them) plus any
   --push paths, and removes files an earlier sync copied that no longer exist here;
3. runs `zt setup --quiet`, `zt data fetch` (assets come from the pinned releases, checked by hash; never from this
   machine), optionally `zt models export`, then the command itself, with output streamed and logged under
   <base>/logs;
4. copies back the files the command was asked to write (--json/--out/--output), the work directory's
   data/build/remote_eval/ and any --pull paths, also after a failure.

With --detach the run continues on the host after the connection closes; `zt remote status|logs|pull|stop` follow it
and `zt remote wait` blocks until it ends.  --nice lowers a run's priority, so long sweeps leave the CPU to runs
someone is waiting for.
"""

from __future__ import annotations

import os
import shlex
import subprocess
import sys
import tempfile
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.remote.host import Host

# Options whose value is a file the command writes; it is copied back after the run.
OUTPUT_OPTIONS = ("--json", "--out", "--output")
REMOTE_OUTPUTS = "data/build/remote_eval"
BUSY = 75
# `zt remote wait` gave up while the run is still going (the `timeout` command's status).
STILL_RUNNING = 124
# Commands that read the data directory, so a run fetches the evaluation data first unless --fetch says otherwise.
DATA_COMMANDS = ("eval", "bench", "models", "train")
# Commands that run the engine: the lexicon and language models are compiled first (as the app loads them).
ENGINE_COMMANDS = ("eval", "bench")
# `zt eval` tools that do not run engine processes (or, for latency, must not: it measures the app's own path).
UNSERVED_TOOLS = ("stats", "sets", "latency")


@dataclass
class Options:
    workdir: str
    push: list[str] = field(default_factory=list)
    pull: list[str] = field(default_factory=list)
    # Data components to fetch first; None: the `eval` group for commands that read data (DATA_COMMANDS), else none.
    fetch: list[str] | None = None
    export: bool = False
    # cpu, cuda, or auto: cuda for evaluations (development numbers), cpu for everything else and ZhuyinBench.
    backend: str = "auto"
    detach: bool = False
    env: list[str] = field(default_factory=list)
    groups: list[str] = field(default_factory=list)
    # Priority on the host: 0 (normal) to 19 (only what other runs leave).
    nice: int = 0


def backend(argv: list[str], options: Options) -> str:
    """Where the models run: --backend when it is cpu or cuda; otherwise the GPU model server for `zt eval` tools that
    run the engine (development sweeps) and ONNX Runtime on the CPU for everything else, ZhuyinBench included (release
    numbers)."""
    if options.backend != "auto":
        return options.backend
    tool = argv[1] if argv[:1] == ["eval"] and len(argv) > 1 else ""
    return "cuda" if tool and not tool.startswith("-") and tool not in UNSERVED_TOOLS else "cpu"


def steps(argv: list[str], options: Options) -> list[list[str]]:
    """The zt commands of a run, in order: prepare the machine, then the command."""
    runtime = backend(argv, options)
    groups = list(dict.fromkeys(options.groups + (["export"] if options.export or runtime == "cuda" else [])))
    result = []
    if argv[:1] != ["setup"] or groups:
        result.append(["setup", "--quiet", *[flag for g in groups for flag in ("--group", g)]])
    fetch = options.fetch if options.fetch is not None else (["eval"] if argv[:1] and argv[0] in DATA_COMMANDS else [])
    if fetch:
        result.append(["data", "fetch", "--quiet", *fetch])
        if argv[:1] and argv[0] in ENGINE_COMMANDS:
            result.append(["data", "compile", "--quiet"])
    if options.export:
        result.append(["models", "export", "--tree"])
    if runtime == "cuda":
        result.append(["models", "serve", "--device", "cuda", "--", *argv])
    else:
        result.append(argv)
    return result


def environment(options: Options) -> dict[str, str]:
    values = {}
    for item in options.env:
        key, sep, value = item.partition("=")
        if not sep or not key.isidentifier():
            raise SystemExit(f"zt: --env needs KEY=VALUE, not {item!r}")
        values[key] = value
    return values


class Runner(ABC):
    """Runs a zt command (its arguments after `zt`) with the run options; returns its exit status."""

    @abstractmethod
    def run(self, argv: list[str], options: Options) -> int: ...


class LocalRunner(Runner):
    """The same steps in this checkout (ZHUYIN_REMOTE_HOST=local, for example on the GPU host itself)."""

    def run(self, argv: list[str], options: Options) -> int:
        if options.detach or options.push or options.pull:
            raise SystemExit("zt: --detach, --push and --pull need a remote host")
        env = dict(os.environ, **environment(options))
        for step in steps(argv, options):
            status = subprocess.run([str(paths.ROOT / "tools" / "zt"), *step], env=env).returncode
            if status:
                return status
        return 0


@dataclass
class Output:
    """A file the command writes: where it is on the host (relative to the work directory) and where it goes here."""

    remote: str
    local: Path


def rewrite_outputs(argv: list[str], cwd: Path | None = None) -> tuple[list[str], list[Output]]:
    """Points output options at paths inside the remote work directory: a path inside the checkout keeps its place,
    any other path goes to data/build/remote_eval/out/.  Returns the new arguments and what to copy back."""
    cwd = cwd or Path.cwd()
    result: list[str] = []
    outputs: list[Output] = []

    def place(value: str) -> str:
        local = Path(os.path.abspath(cwd / value))
        try:
            remote = str(local.relative_to(paths.ROOT))
        except ValueError:
            remote = f"{REMOTE_OUTPUTS}/out/{local.name}"
        outputs.append(Output(remote, local))
        return remote

    i = 0
    while i < len(argv):
        arg = argv[i]
        name, sep, value = arg.partition("=")
        if arg == "--":
            result += argv[i:]
            break
        if name in OUTPUT_OPTIONS and sep:
            result.append(f"{name}={place(value)}")
        elif arg in OUTPUT_OPTIONS and i + 1 < len(argv):
            result += [arg, place(argv[i + 1])]
            i += 1
        else:
            result.append(arg)
        i += 1
    return result, outputs


def source_files() -> list[str]:
    """The checkout's files as git sees them: tracked and untracked-but-not-ignored, existing, no directories (a
    symlinked data/ is not a source file)."""
    listed = (
        subprocess.run(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=paths.ROOT,
            capture_output=True,
            check=True,
        )
        .stdout.decode()
        .split("\0")
    )
    result = []
    for name in listed:
        path = paths.ROOT / name
        if name and "\n" not in name and (path.is_symlink() or path.is_file()) and not path.is_dir():
            result.append(name)
    return sorted(set(result))


class RemoteRunner(Runner):
    """The steps on the GPU host, in a work directory of its own."""

    def __init__(self, host: Host) -> None:
        self.host = host

    # MARK: - Paths on the host

    def work(self, workdir: str, *parts: str) -> str:
        return self.host.home_path("work", workdir, *parts)

    def rsync_work(self, workdir: str, relative: str = "") -> str:
        return self.host.remote("work", workdir, *([relative] if relative else []))

    # MARK: - Sync

    def busy(self, workdir: str) -> bool:
        script = (
            f"cd {self.work(workdir)} 2>/dev/null && [ -e .zt/lock ] && ! flock -n .zt/lock true "
            "&& echo busy || echo free"
        )
        done = self.host.run(script, check=False, capture=True)
        if done.returncode != 0:
            raise SystemExit(f"zt: cannot reach the host ({done.returncode}): {done.stderr.strip()}")
        return done.stdout.strip() == "busy"

    def sync(self, workdir: str, push: list[str]) -> None:
        if self.busy(workdir):
            raise SystemExit(
                f"zt: a run is in progress in {workdir} on the host; wait for it (`zt remote status`) "
                "or use another --workdir"
            )
        files = source_files()
        self.host.run(f"mkdir -p {self.work(workdir, '.zt')} {self.host.home_path('logs')}")
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as listing:
            listing.write("\n".join(files) + "\n")
        try:
            subprocess.run(
                self.host.rsync(
                    "-az", f"--files-from={listing.name}", f"{paths.ROOT}/", self.rsync_work(workdir) + "/"
                ),
                check=True,
            )
            subprocess.run(self.host.rsync("-az", listing.name, self.rsync_work(workdir, ".zt/files.new")), check=True)
        finally:
            os.unlink(listing.name)
        # Remove what an earlier sync copied and this checkout no longer has; nothing else is touched.
        self.host.run(
            f"cd {self.work(workdir)} && export LC_ALL=C && if [ -f .zt/files ]; then "
            "sort -u .zt/files > .zt/files.old && sort -u .zt/files.new > .zt/files.cur && "
            'comm -23 .zt/files.old .zt/files.cur | while IFS= read -r f; do rm -f -- "$f"; done; '
            "rm -f .zt/files.old .zt/files.cur; fi; mv .zt/files.new .zt/files"
        )
        self.push(workdir, push)
        print(
            f"synced {len(files)} files to {workdir}" + (f" and pushed {len(push)} path(s)" if push else ""), flush=True
        )

    def push(self, workdir: str, items: list[str]) -> None:
        pairs = []
        for item in items:
            local = Path(os.path.abspath(item))
            try:
                inside = local.relative_to(paths.ROOT)
            except ValueError:
                raise SystemExit(f"zt: --push {item}: only paths inside the checkout can be pushed") from None
            if not local.exists():
                raise SystemExit(f"zt: --push {item}: no such file or directory")
            pairs.append((local.resolve(), str(inside)))
        if not pairs:
            return
        parents = " ".join(self.work(workdir, str(Path(r).parent)) for _, r in pairs)
        self.host.run(f"mkdir -p {parents}")
        for source, relative in pairs:
            if source.is_dir():
                subprocess.run(
                    self.host.rsync("-az", f"{source}/", self.rsync_work(workdir, relative) + "/"), check=True
                )
            else:
                subprocess.run(self.host.rsync("-az", str(source), self.rsync_work(workdir, relative)), check=True)

    # MARK: - Run

    def job(self, workdir: str, argv: list[str], options: Options) -> str:
        """The bash script of a run on the host."""
        exports = {"PYTHONUNBUFFERED": "1", **environment(options)}
        lines = [
            "set -euo pipefail",
            f"cd {self.work(workdir)}",
            f'export ZHUYIN_ASSET_CACHE="${{ZHUYIN_ASSET_CACHE:-$HOME/{self.host.base}/asset-cache}}"',
            *[f"export {key}={shlex.quote(value)}" for key, value in exports.items()],
            f"mkdir -p {REMOTE_OUTPUTS}",
            'echo "[zt] $(hostname) $(date +%Y-%m-%dT%H:%M:%S) $(pwd)"',
        ]
        for step in steps(argv, options):
            lines.append(f"echo {shlex.quote('[zt] zt ' + shlex.join(step))}")
            if step[:1] == ["setup"] and step != argv:
                # A missing tool matters only to the commands that need it (cargo for evaluations, not for training).
                lines.append(f"tools/zt {shlex.join(step)} || echo '[zt] setup found missing tools (above)'")
            else:
                lines.append(f"tools/zt {shlex.join(step)}")
        return "\n".join(lines) + "\n"

    def run(self, argv: list[str], options: Options) -> int:
        argv, outputs = rewrite_outputs(argv)
        self.sync(options.workdir, options.push)
        stamp = time.strftime("%Y%m%dT%H%M%S")
        job = f".zt/job-{stamp}.sh"
        log = self.host.home_path("logs", f"{options.workdir}-{stamp}.log")
        self.host.run(f"cat > {self.work(options.workdir, job)}", stdin=self.job(options.workdir, argv, options))
        inner = shlex.quote(f"bash {job}; s=$?; echo $s > .zt/last-status; exit $s")
        shell = f"nice -n {options.nice} bash" if options.nice else "bash"
        # `;` rather than `&&` before the job: a trailing `&` would otherwise put the cd in the background too.
        prefix = f"cd {self.work(options.workdir)} || exit 1; ln -sfn {log} .zt/last.log; "
        if options.detach:
            script = (
                prefix + f"nohup setsid flock -n -E {BUSY} .zt/lock {shell} -c {inner} > {log} 2>&1 < /dev/null "
                '& echo $! > .zt/run.pid; echo "started in the background: pid $(cat .zt/run.pid), '
                'log $(readlink .zt/last.log)"'
            )
            self.host.run(script)
            print(
                f"follow it with `zt remote logs -f --workdir {options.workdir}`, wait for it with `zt remote wait "
                f"--workdir {options.workdir}`; "
                f"copy results back with `zt remote pull --workdir {options.workdir}`"
            )
            return 0
        script = prefix + f"flock -n -E {BUSY} .zt/lock {shell} -c {inner} 2>&1 | tee {log}; exit ${{PIPESTATUS[0]}}"
        tty = sys.stdin.isatty() and sys.stdout.isatty()
        try:
            status = self.host.run(script, check=False, tty=tty).returncode
        finally:
            self.pull(options.workdir, options.pull, outputs)
        if status == BUSY:
            print(f"zt: another run holds {options.workdir} on the host", file=sys.stderr)
        return status

    # MARK: - Results and inspection

    def pull(self, workdir: str, extra: list[str] | None = None, outputs: list[Output] | None = None) -> None:
        """Copies back data/build/remote_eval/ (into remote_eval/<workdir>/ of the local data directory), the
        command's output files and any other paths (relative to the checkout)."""
        local = paths.data("remote_eval", workdir)
        local.mkdir(parents=True, exist_ok=True)
        # The default results directory is optional when the remote job failed before creating it.
        if subprocess.run(
            self.host.rsync("-az", self.rsync_work(workdir, REMOTE_OUTPUTS) + "/", f"{local}/"), check=False
        ).returncode:
            print("zt: optional results directory could not be retrieved", file=sys.stderr)
        for output in outputs or []:
            # Both paths end in the same name, so copying into the parent works for files and directories alike.
            output.local.parent.mkdir(parents=True, exist_ok=True)
            source = self.rsync_work(workdir, output.remote.rstrip("/"))
            subprocess.run(self.host.rsync("-az", source, f"{output.local.parent}/"), check=True)
            print(f"copied back {paths.relative(output.local)}")
        for item in extra or []:
            target = paths.ROOT / item
            target.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(
                self.host.rsync("-az", self.rsync_work(workdir, item.rstrip("/")), f"{target.parent}/"), check=True
            )
        print(f"results: {paths.relative(local)}")

    def status(self, workdir: str) -> int:
        script = (
            f"uptime; cd {self.work(workdir)} 2>/dev/null || {{ echo 'no work directory {workdir}'; exit 0; }}; "
            'if [ -f .zt/run.pid ] && kill -0 "$(cat .zt/run.pid)" 2>/dev/null; then '
            'echo "running: pid $(cat .zt/run.pid)"; '
            'ps -o pid,etime,pcpu,rss,args --ppid "$(cat .zt/run.pid)" 2>/dev/null | head -5; '
            "else echo 'not running'; fi; "
            '[ -f .zt/last-status ] && echo "last exit status: $(cat .zt/last-status)"; '
            '[ -e .zt/last.log ] && { echo "log: $(readlink .zt/last.log)"; tail -n 5 .zt/last.log; }; true'
        )
        return self.host.run(script, check=False).returncode

    def wait(self, workdir: str, timeout: int) -> int:
        """Blocks until the run holding this work directory (detached or not) ends; returns its exit status, or
        STILL_RUNNING when `timeout` seconds (0: no limit) pass first.  The loop runs on the host, in one connection,
        and prints a line every five minutes so the connection stays alive."""
        script = (
            f"cd {self.work(workdir)} 2>/dev/null || {{ echo 'no work directory {workdir}'; exit 1; }}; t=0; "
            "while [ -e .zt/lock ] && ! flock -n .zt/lock true; do "
            f"if [ {int(timeout)} -gt 0 ] && [ $t -ge {int(timeout)} ]; then "
            f'echo "still running after ${{t}}s"; exit {STILL_RUNNING}; fi; '
            '[ $t -gt 0 ] && [ $((t % 300)) -eq 0 ] && echo "still running ($((t / 60)) min)"; '
            "sleep 10; t=$((t + 10)); done; "
            's=$(cat .zt/last-status 2>/dev/null || echo 0); echo "finished: exit status $s"; '
            '[ -e .zt/last.log ] && tail -n 5 .zt/last.log; exit "$s"'
        )
        return self.host.run(script, check=False).returncode

    def logs(self, workdir: str, follow: bool, lines: int) -> int:
        script = f"tail -n {int(lines)} {'-F ' if follow else ''}{self.work(workdir, '.zt/last.log')}"
        return self.host.run(script, check=False, tty=follow and sys.stdout.isatty()).returncode

    def stop(self, workdir: str) -> int:
        """Stops the detached run of this work directory by its recorded PID (its process group), nothing else."""
        script = (
            f"cd {self.work(workdir)} && [ -f .zt/run.pid ] && pid=$(cat .zt/run.pid) && "
            'kill -0 "$pid" 2>/dev/null && kill -TERM -- -"$pid" && echo "stopped process group $pid" '
            "|| echo 'no run in progress'"
        )
        return self.host.run(script, check=False).returncode

    def shell(self, workdir: str) -> int:
        script = f'mkdir -p {self.work(workdir)} && cd {self.work(workdir)} && exec "${{SHELL:-bash}}" -l'
        return self.host.run(script, check=False, tty=True).returncode

    def list(self) -> int:
        """Every work directory on the host: its last exit status and, while it runs, for how long and at what
        priority; the host's load first, so it is clear who is using the CPU."""
        script = (
            f"echo \"load $(cut -d' ' -f1-3 /proc/loadavg) on $(nproc) cores\"; "
            f"cd {self.host.home_path('work')} 2>/dev/null || exit 0; "
            "printf '%-40s %-6s %s\\n' 'work directory' 'exit' 'run'; for d in */.zt; do d=${d%/.zt}; "
            's=$(cat "$d/.zt/last-status" 2>/dev/null || echo -); r=; '
            'if [ -e "$d/.zt/lock" ] && ! flock -n "$d/.zt/lock" true; then r=running; '
            'p=$(cat "$d/.zt/run.pid" 2>/dev/null); if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then '
            'r="running $(ps -o etime= -p "$p" | tr -d " "), nice $(ps -o ni= -p "$p" | tr -d " ")"; fi; fi; '
            'printf \'%-40s %-6s %s\\n\' "$d" "$s" "$r"; done'
        )
        return self.host.run(script, check=False).returncode


def runner(host: Host) -> Runner:
    return LocalRunner() if host.is_local else RemoteRunner(host)
