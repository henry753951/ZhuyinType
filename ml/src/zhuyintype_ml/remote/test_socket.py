"""Regression: zhuyin-cli reads a short model-server reply while the Unix socket stays open (the Linux `recv` path),
in the binary frames of zhuyintype_ml/remote/model_server.py.

    zt --remote check python --start src/zhuyintype_ml/remote      # skipped until zhuyin-cli is built (`zt build cli`)
"""

from __future__ import annotations

import json
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

from zhuyintype_ml.core import paths

CLI = paths.engine_target() / "release" / "zhuyin-cli"
HEADER = struct.Struct("<BBHI")


@unittest.skipUnless(sys.platform == "linux", "the model server socket path is tested on Linux (the GPU host)")
@unittest.skipUnless(CLI.exists(), "zhuyin-cli is not built")
class OpenSocketTests(unittest.TestCase):
    def test_short_replies_on_an_open_socket(self) -> None:
        requests: list[tuple[int, int]] = []

        class Handler(socketserver.StreamRequestHandler):
            def handle(self) -> None:
                while header := self.rfile.read(HEADER.size):
                    marker, kind, version, size = HEADER.unpack(header)
                    assert (marker, kind, version) == (0, 1, 1)
                    values = struct.unpack(f"<{size // 4}i", self.rfile.read(size))
                    batch, length = values[:2]
                    requests.append((batch, length))
                    self.wfile.write(HEADER.pack(0, 0, 1, 4 * batch * length) + bytes(4 * batch * length))
                    self.wfile.flush()
                    # Deliberately leave the connection open for the next request.

        class Server(socketserver.ThreadingUnixStreamServer):
            daemon_threads = True

        with tempfile.TemporaryDirectory(prefix="zt-socket-") as temporary:
            root = Path(temporary)
            (root / "vocab.json").write_text(json.dumps(["<pad>", "<bos>", "<unk>", "你", "好"]))
            # Two readings with two characters each, so the reranker is asked to choose.
            lexicon = "ㄋㄧˇ\t你\t-1\nㄋㄧˇ\t妳\t-1.5\nㄏㄠˇ\t好\t-1\nㄏㄠˇ\t郝\t-1.5\n"
            (root / "lexicon.tsv").write_text(lexicon, encoding="utf-8")
            socket = str(root / "model.sock")
            with Server(socket, Handler) as server:
                threading.Thread(target=server.serve_forever, daemon=True).start()
                try:
                    result = subprocess.run(
                        [
                            str(CLI),
                            "--model-server",
                            socket,
                            "--lexicon",
                            str(root / "lexicon.tsv"),
                            "--reranker",
                            str(root),
                        ]
                        + ["--batch"],
                        input="su3cl3<enter>\nsu3cl3<enter>\n",
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=15,
                    )
                finally:
                    server.shutdown()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["你好", "你好"])
        self.assertGreaterEqual(len(requests), 2)


if __name__ == "__main__":
    unittest.main()
