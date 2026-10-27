"""The GPU host: where it is, how to reach it, and where ZhuyinType's directories are on it.

The host comes from the environment or from .remote.env at the repository root (never from tracked files):

    ZHUYIN_REMOTE_HOST   user@host for ssh and rsync (required; `local` runs commands on this machine instead)
    ZHUYIN_REMOTE_KEY    SSH private key (optional; otherwise ssh's own configuration decides)
    ZHUYIN_REMOTE_BASE   ZhuyinType's directory on the host, relative to its home (default: zhuyintype)

The environment wins over the file.  On the host, work directories are <base>/work/<name>, logs <base>/logs and the
shared download cache of the data assets <base>/asset-cache.
"""

from __future__ import annotations

import hashlib
import os
import re
import shlex
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from zhuyintype_ml.core import paths

NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")


def read_env(path: Path = paths.REMOTE_ENV) -> dict[str, str]:
    """KEY=VALUE lines of a .env file, with $HOME and a leading ~ expanded; the environment overrides them."""
    values: dict[str, str] = {}
    if path.exists():
        for line in path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            value = value.strip().strip("'\"")
            values[key.strip()] = os.path.expanduser(
                value.replace("$HOME", str(Path.home())).replace("${HOME}", str(Path.home()))
            )
    for key in ("ZHUYIN_REMOTE_HOST", "ZHUYIN_REMOTE_KEY", "ZHUYIN_REMOTE_BASE"):
        if os.environ.get(key):
            values[key] = os.environ[key]
    return values


def default_workdir() -> str:
    """One work directory per checkout, so two checkouts never share a host directory."""
    digest = hashlib.sha256(str(paths.ROOT).encode()).hexdigest()[:8]
    name = re.sub(r"[^A-Za-z0-9._-]", "-", paths.ROOT.name)[:40]
    return f"{name}-{digest}"


def check_name(name: str) -> str:
    if not NAME.match(name) or name in (".", ".."):
        raise SystemExit(f"zt: invalid work directory name {name!r} (letters, digits, '.', '_' and '-')")
    return name


@dataclass(frozen=True)
class Host:
    address: str
    key: str | None = None
    base: str = "zhuyintype"

    @staticmethod
    def load() -> Host:
        values = read_env()
        address = values.get("ZHUYIN_REMOTE_HOST")
        if not address:
            raise SystemExit(
                "zt: no GPU host. Set ZHUYIN_REMOTE_HOST in .remote.env (see .remote.env.example) "
                "or in the environment."
            )
        base = values.get("ZHUYIN_REMOTE_BASE", "zhuyintype").strip("/")
        if not base or ".." in base.split("/"):
            raise SystemExit(f"zt: invalid ZHUYIN_REMOTE_BASE {base!r}")
        return Host(address, values.get("ZHUYIN_REMOTE_KEY") or None, base)

    @property
    def is_local(self) -> bool:
        return self.address == "local"

    def ssh_options(self) -> list[str]:
        # Keepalives survive long quiet steps; one shared connection makes the several calls of a run cheap.
        # The socket path must stay short (Unix sockets allow ~100 bytes; macOS's $TMPDIR is long).
        ssh_dir = Path.home() / ".ssh"
        control = (ssh_dir if ssh_dir.is_dir() else Path(tempfile.gettempdir())) / "zt-%C"
        options = [
            "-o",
            "ConnectTimeout=15",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=4",
            "-o",
            "ControlMaster=auto",
            "-o",
            f"ControlPath={control}",
            "-o",
            "ControlPersist=120",
        ]
        return options + (["-i", self.key] if self.key else [])

    def ssh(self, script: str, tty: bool = False) -> list[str]:
        """ssh running `script` with bash on the host."""
        return ["ssh", *self.ssh_options(), *(["-t"] if tty else []), self.address, "bash -c " + shlex.quote(script)]

    def rsync(self, *args: str) -> list[str]:
        return ["rsync", "-e", shlex.join(["ssh", *self.ssh_options()]), *args]

    def run(
        self, script: str, check: bool = True, capture: bool = False, stdin: str | None = None, tty: bool = False
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(self.ssh(script, tty), check=check, text=True, input=stdin, capture_output=capture)

    def home_path(self, *parts: str) -> str:
        """A path under the base directory, for the remote shell ($HOME expands there)."""
        return '"$HOME"/' + shlex.quote("/".join([self.base, *parts]))

    def remote(self, *parts: str) -> str:
        """The same path for rsync (relative to the remote home)."""
        return f"{self.address}:{'/'.join([self.base, *parts])}"
