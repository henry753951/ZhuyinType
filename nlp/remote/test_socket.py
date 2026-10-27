#!/usr/bin/env python3
"""Regression: a short JSON reply must be read while the Unix socket remains open."""
import argparse
import json
from pathlib import Path
import socketserver
import subprocess
import tempfile
import threading


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cli', default='.build/release/zhuyin-cli')
    a = p.parse_args()
    requests = []

    class Handler(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                request = json.loads(line)
                assert request['op'] == 'rerank'
                requests.append(request)
                self.wfile.write(json.dumps({'output': [0.] * len(request['ids'])}).encode() + b'\n')
                self.wfile.flush()
                # Deliberately leave the connection open for the next request.

    class Server(socketserver.ThreadingUnixStreamServer):
        daemon_threads = True

    with tempfile.TemporaryDirectory(prefix='zt-socket-') as temporary:
        root = Path(temporary)
        (root / 'vocab.json').write_text(json.dumps(['<pad>', '<bos>', '<unk>', '你', '好']))
        socket = str(root / 'model.sock')
        with Server(socket, Handler) as server:
            threading.Thread(target=server.serve_forever, daemon=True).start()
            try:
                result = subprocess.run([a.cli, '--model-server', socket, '--reranker', str(root), '--batch'],
                                        input='su3cl3<enter>\nsu3cl3<enter>\n', text=True,
                                        capture_output=True, check=True, timeout=15)
                assert result.stdout.splitlines() == ['你好', '你好'], result
                assert len(requests) >= 2, requests
            finally:
                server.shutdown()
    print(f'Open-socket short replies passed ({len(requests)} requests).')

if __name__ == '__main__':
    main()
