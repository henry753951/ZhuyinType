"""靈犀注音（0c）through its unmodified server, over the protocol its shells use, one key at a time.

Written from the protocol as 0c documents it (bridge/ipc/ipc.go, core/proto/proto.go at the pinned commit; no 0c code
is copied or imported): each message is a 4-byte little-endian length and a JSON body; the client sends an envelope
{"op": ..., "session": ..., "req": {...}}, the server answers {"ok": ..., "snapshot": {...}, "engine_us": ...}.  A case
opens a session, sends every key ({"type": "key", "key": "s"}; Space as " ", Enter as "Enter"), collects the commits of
every snapshot, and closes the session.

Settings (settings.json beside the personal file, as 0c reads it): standard layout, mixed input, punctuation and
toneless input off, and the selection under test: `statistical` (its engine alone) or `neural` (its D512 selection
model with the default 100 ms deadline, one inference thread per server).  Learning: Enter is sent as private, so a
case teaches nothing; split commits need the committed text as context, which 0c keeps only for commits it may learn
from, so each such case gets a freshly started server with an empty personal file.  0c takes no arbitrary text
before the run (its shells confirm only text it committed itself), so line context is N/A.

With the model, every case records the server's selection state at its end (ready, fallback, loading), as 0c's
health operation reports it; fallbacks are counted as they come out, never retried.
"""

from __future__ import annotations

import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Any

from zhuyintype_ml.eval.systems.base import Case, Key, Outcome, Segment, System, Task, key_time_summary, parse_script
from zhuyintype_ml.eval.systems.references import ZEROC, temporary_dir

MAX_FRAME = 1 << 20
NAMED = {"space": " ", "enter": "Enter", "left": "Left"}
ENVIRONMENT = {"GOMAXPROCS": "1", "OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1"}


def frame(message: dict[str, Any]) -> bytes:
    body = json.dumps(message, ensure_ascii=False).encode()
    if len(body) > MAX_FRAME:
        raise ValueError("frame over the protocol's limit")
    return struct.pack("<I", len(body)) + body


def unframe(buffer: bytes) -> tuple[dict[str, Any] | None, bytes]:
    """The first complete message in `buffer` and what follows it, or (None, buffer) when incomplete."""
    if len(buffer) < 4:
        return None, buffer
    (size,) = struct.unpack("<I", buffer[:4])
    if size > MAX_FRAME:
        raise ValueError("frame over the protocol's limit")
    if len(buffer) < 4 + size:
        return None, buffer
    message: dict[str, Any] = json.loads(buffer[4 : 4 + size])
    return message, buffer[4 + size :]


def key_name(key: Key) -> str:
    return key.name if key.is_char else NAMED[key.name]


def settings(selection: str) -> dict[str, Any]:
    return {
        "schema": 2,
        "selection": selection,
        "mixed_input": False,
        "punctuation": False,
        "document_context": True,
        "keyboard_layout": "standard",
        "toneless": False,
    }


class Server:
    """One 0c-server process and a connection to it."""

    def __init__(self, selection: str, cpu: int | None = None) -> None:
        self.selection = selection
        self.directory = temporary_dir("zt0c-")
        self.process: subprocess.Popen[bytes] | None = None
        self.sock: socket.socket | None = None
        self.buffer = b""
        self.info: dict[str, Any] = {}
        self.cpu = cpu
        self.start()

    def command(self) -> list[str]:
        root, src = ZEROC.root, ZEROC.checkout
        command = [
            str(root / "bin" / "0c-server"),
            "-data",
            str(src / "assets" / "data"),
            "-user",
            str(self.directory / "personal.json"),
            "-socket",
            str(self.directory / "s"),
            "-install-watch",
            "off",
            "-neural-threads",
            "1",
        ]
        if self.selection == "neural":
            command += [
                "-neural-python",
                str(root / "venv" / "bin" / "python"),
                "-neural-package",
                str(src / "models" / "select"),
                "-neural-worker",
                str(src / "platforms" / "neural" / "runtime" / "sequence_runtime.py"),
            ]
        return command

    def start(self) -> None:
        for leftover in self.directory.iterdir():
            if leftover.is_dir():
                shutil.rmtree(leftover)
            else:
                leftover.unlink()
        (self.directory / "settings.json").write_text(json.dumps(settings(self.selection)), encoding="utf-8")
        command = self.command()
        if self.cpu is not None and shutil.which("taskset"):
            command = ["taskset", "-c", str(self.cpu), *command]
        log = open(self.directory / "server.log", "ab")
        self.process = subprocess.Popen(
            command, stdout=log, stderr=log, env=dict(os.environ, **ENVIRONMENT), start_new_session=True
        )
        log.close()
        path = self.directory / "s"
        deadline = time.monotonic() + 120
        while not path.exists():
            if self.process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError(f"0c-server did not start; see {self.directory / 'server.log'}")
            time.sleep(0.05)
        while True:  # the socket file appears when it is bound, a moment before the server listens
            self.sock = socket.socket(socket.AF_UNIX)
            self.sock.settimeout(60)
            try:
                self.sock.connect(str(path))
                break
            except (ConnectionRefusedError, FileNotFoundError):
                self.sock.close()
                if self.process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f"0c-server did not accept connections; see {self.directory}") from None
                time.sleep(0.05)
        while True:
            self.info = self.call({"op": "health"}).get("info", {})
            state = self.info.get("selection_state")
            if self.selection != "neural" or state == "ready":
                break
            if state not in ("loading", "starting", None) or time.monotonic() > deadline:
                raise RuntimeError(f"0c selection model not ready: {self.info}")
            time.sleep(0.1)

    def call(self, message: dict[str, Any]) -> dict[str, Any]:
        assert self.sock is not None
        self.sock.sendall(frame(message))
        while True:
            reply, self.buffer = unframe(self.buffer)
            if reply is not None:
                break
            chunk = self.sock.recv(65536)
            if not chunk:
                raise RuntimeError("0c-server closed the connection")
            self.buffer += chunk
        if not reply.get("ok"):
            raise RuntimeError(f"0c-server: {reply.get('error')}")
        return reply

    def handle(self, session: int, request: dict[str, Any]) -> dict[str, Any]:
        return self.call({"op": "handle", "session": session, "req": request})

    def stop(self) -> None:
        if self.sock:
            self.sock.close()
            self.sock = None
        self.buffer = b""
        if self.process and self.process.poll() is None:
            os.killpg(self.process.pid, signal.SIGTERM)
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait()

    def restart(self) -> None:
        self.stop()
        self.start()

    def close(self) -> None:
        self.stop()
        shutil.rmtree(self.directory, ignore_errors=True)


def type_case(server: Server, case: Case, sequential: bool) -> Outcome:
    """Opens a session, types the case, returns everything committed and the time of each key."""
    session = int(server.call({"op": "open"})["session"])
    commits: list[str] = []
    key_ms: list[float] = []
    engine_us: list[int] = []
    shown_mismatch = 0
    try:
        if sequential:
            server.handle(session, {"type": "context", "on": False, "context_enabled": True})
        for index, segment in enumerate(case.segments):
            snapshot: dict[str, Any] = {}
            for key in parse_script(segment.keys):
                if key.name == "enter":
                    raise ValueError("Enter inside a segment")
                started = time.perf_counter()
                reply = server.handle(session, {"type": "key", "key": key_name(key)})
                key_ms.append((time.perf_counter() - started) * 1000)
                engine_us.append(int(reply.get("engine_us", 0)))
                snapshot = reply["snapshot"]
                commits.append(snapshot.get("commit", ""))
                if not snapshot.get("handled") and key.is_char:
                    commits.append(key.name)  # passed through: the app inserts the character
            shown = "".join(part.get("text", "") for part in snapshot.get("preedit") or [])
            enter: dict[str, Any] = {"type": "key", "key": "Enter"}
            if not sequential:
                enter["private"] = True
            reply = server.handle(session, enter)
            committed = reply["snapshot"].get("commit", "")
            shown_mismatch += committed != shown
            commits.append(committed)
            context_id = reply["snapshot"].get("context_id")
            if sequential and index + 1 < len(case.segments) and context_id:
                server.handle(
                    session, {"type": "context", "on": True, "context_id": context_id, "context_enabled": True}
                )
    finally:
        server.call({"op": "close", "session": session})
    info: dict[str, Any] = {"key_ms": key_ms, "engine_us": engine_us}
    if shown_mismatch:
        info["enter_changed_display"] = shown_mismatch
    if server.selection == "neural":
        health = server.call({"op": "health"}).get("info", {})
        info["selection_state"] = health.get("selection_state")
        if health.get("selection_reason"):
            info["selection_reason"] = health.get("selection_reason")
    return Outcome("".join(commits), info=info)


def ready_share(outcomes: list[Outcome]) -> float:
    return sum(o.info.get("selection_state") == "ready" for o in outcomes) / max(1, len(outcomes))


def wait_for_quiet_host() -> None:
    """The selection model has a 100 ms deadline: on a busy host it times out and 0c falls back to its statistical
    engine, which would measure the host rather than 0c.  Waits (up to $ZT_0C_WAIT minutes, default 240) until the
    1-minute load is below $ZT_0C_MAX_LOAD (default: half the cores)."""
    limit = float(os.environ.get("ZT_0C_MAX_LOAD", (os.cpu_count() or 2) / 2))
    deadline = time.monotonic() + 60 * float(os.environ.get("ZT_0C_WAIT", "240"))
    while os.getloadavg()[0] > limit and time.monotonic() < deadline:
        print(f"0c-full: host load {os.getloadavg()[0]:.0f} > {limit:.0f}, waiting", flush=True)
        time.sleep(60)


class ZeroC(System):
    family = "0c"
    capabilities = frozenset()

    def __init__(self, selection: str, workers: int = 4, first_cpu: int | None = None) -> None:
        if selection not in ("statistical", "neural"):
            raise ValueError(selection)
        self.selection = selection
        self.name = "0c-statistical" if selection == "statistical" else "0c-full"
        self.label = "靈犀（統計）" if selection == "statistical" else "靈犀（完整）"
        self.workers = workers
        self.first_cpu = first_cpu
        self.servers: list[Server] = []
        self.lock = threading.Lock()

    def start(self) -> None:
        ZEROC.setup_record()
        if self.selection == "neural":
            wait_for_quiet_host()
        cpus = [None if self.first_cpu is None else self.first_cpu + i for i in range(self.workers)]
        with ThreadPoolExecutor(self.workers) as pool:
            self.servers = list(pool.map(lambda cpu: Server(self.selection, cpu), cpus))

    def close(self) -> None:
        for server in self.servers:
            server.close()
        self.servers = []

    def convert(self, task: Task, cases: list[Case]) -> list[Outcome]:
        """With the model: when fewer than half the cases end with the model ready (it timed out under load), the
        task is typed again once the host is quieter (at most twice), and the last attempt is kept as it is."""
        outcomes = self.type_all(task, cases)
        for _ in range(2):
            if self.selection != "neural" or ready_share(outcomes) >= 0.5:
                break
            print(f"0c-full: {task.id}: model ready in {ready_share(outcomes):.0%} of cases; again later", flush=True)
            wait_for_quiet_host()
            for server in self.servers:
                server.restart()
            outcomes = self.type_all(task, cases)
        return outcomes

    def type_all(self, task: Task, cases: list[Case]) -> list[Outcome]:
        sequential = task.sequential
        results: list[Outcome | None] = [None] * len(cases)

        def work(number: int) -> None:
            server = self.servers[number]
            for i in range(number, len(cases), len(self.servers)):
                try:
                    if sequential:
                        server.restart()  # an empty personal file: what this case learns stays with it
                    outcome = type_case(server, cases[i], sequential)
                    outcome.info = {k: v for k, v in outcome.info.items() if k not in ("key_ms", "engine_us")}
                    results[i] = outcome
                except (RuntimeError, OSError, TimeoutError) as problem:
                    results[i] = Outcome("", "error", {"error": str(problem)[:200]})
                    server.restart()

        with ThreadPoolExecutor(len(self.servers)) as pool:
            list(pool.map(work, range(len(self.servers))))
        outcomes = [r for r in results if r is not None]
        if len(outcomes) != len(cases):
            raise RuntimeError("0c: missing outcomes")
        return outcomes

    def latency(self, scripts: list[str]) -> dict[str, Any] | None:
        """One server, one sentence at a time: round trip per key and 0c's own engine time per key."""
        server = self.servers[0]
        server.restart()
        cases = [Case(str(i), "", (Segment(s.removesuffix("<enter>")),)) for i, s in enumerate(scripts)]
        for case in cases[:3]:
            type_case(server, case, False)  # warm-up
        outcomes = [type_case(server, case, False) for case in cases]
        wall = key_time_summary([o.info["key_ms"] for o in outcomes])
        engine = key_time_summary([[us / 1000 for us in o.info["engine_us"]] for o in outcomes])
        return {
            **wall,
            "engine": engine["median"],
            "measured": "round trip over 0c's socket per key; engine: 0c's own engine_us per key",
        }

    def describe(self) -> dict[str, Any]:
        record = ZEROC.setup_record()
        return {
            "repository": ZEROC.url,
            "commit": ZEROC.commit,
            "release_assets": ZEROC.release,
            "settings": settings(self.selection),
            "server_options": ["-neural-threads", "1", "-install-watch", "off"]
            + (["-neural-timeout", "100ms (default)"] if self.selection == "neural" else []),
            "servers": self.workers,
            "files": record.get("files", {}),
            "startup": self.servers[0].info if self.servers else {},
        }
