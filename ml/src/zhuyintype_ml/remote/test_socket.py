"""Regression: zhuyin-cli reads a short model-server reply while the Unix socket stays open (the Linux `recv` path;
macOS keeps its original read, which waits for the connection to close, so the test runs on Linux only).

    zt --remote check python --start src/zhuyintype_ml/remote      # skipped until zhuyin-cli is built (`zt build cli`)
"""

from __future__ import annotations

import json
import socketserver
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from typing import Any

from zhuyintype_ml.core import paths

CLI = paths.scratch() / "release" / "zhuyin-cli"


@unittest.skipUnless(sys.platform == "linux", "the model server socket path is tested on Linux (the GPU host)")
@unittest.skipUnless(CLI.exists(), "zhuyin-cli is not built")
class OpenSocketTests(unittest.TestCase):
    def test_short_replies_on_an_open_socket(self) -> None:
        requests: list[dict[str, Any]] = []

        class Handler(socketserver.StreamRequestHandler):
            def handle(self) -> None:
                for line in self.rfile:
                    request = json.loads(line)
                    assert request["op"] == "rerank"
                    requests.append(request)
                    self.wfile.write(json.dumps({"output": [0.0] * len(request["ids"])}).encode() + b"\n")
                    self.wfile.flush()
                    # Deliberately leave the connection open for the next request.

        class Server(socketserver.ThreadingUnixStreamServer):
            daemon_threads = True

        with tempfile.TemporaryDirectory(prefix="zt-socket-") as temporary:
            root = Path(temporary)
            (root / "vocab.json").write_text(json.dumps(["<pad>", "<bos>", "<unk>", "你", "好"]))
            socket = str(root / "model.sock")
            with Server(socket, Handler) as server:
                threading.Thread(target=server.serve_forever, daemon=True).start()
                try:
                    result = subprocess.run(
                        [str(CLI), "--model-server", socket, "--reranker", str(root), "--batch"],
                        input="su3cl3<enter>\nsu3cl3<enter>\n",
                        text=True,
                        capture_output=True,
                        check=True,
                        timeout=15,
                    )
                finally:
                    server.shutdown()
        self.assertEqual(result.stdout.splitlines(), ["你好", "你好"])
        self.assertGreaterEqual(len(requests), 2)


if __name__ == "__main__":
    unittest.main()
