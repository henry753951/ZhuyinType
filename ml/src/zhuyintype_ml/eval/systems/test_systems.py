"""The adapters' own parts with stand-in programs: key scripts, 0c's framing and session handling against a fake
server, 善解's CLI files (dump, picks, summary) and the restart after a row without a decode path."""

from __future__ import annotations

import hashlib
import json
import os
import sys
import tempfile
import textwrap
import unittest
import unittest.mock
from pathlib import Path

from zhuyintype_ml.eval.systems import shanjie, zeroc
from zhuyintype_ml.eval.systems.base import Case, Key, Segment, Task, key_time_summary, parse_script, percentile

# A 0c-server stand-in: the framed protocol over a Unix socket; a session commits its typed keys upper-cased on Enter,
# and the committed text is the context of the next commit when the client confirms it.
FAKE_0C = r"""
import json, os, socket, struct, sys
args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
server = socket.socket(socket.AF_UNIX)
server.bind(args["-socket"])
server.listen(1)
connection, _ = server.accept()
buffer = b""
sessions, pending = {}, {}
def read():
    global buffer
    while len(buffer) < 4 or len(buffer) < 4 + struct.unpack("<I", buffer[:4])[0]:
        chunk = connection.recv(7)  # small reads: messages arrive in pieces
        if not chunk:
            sys.exit(0)
        buffer += chunk
    size = struct.unpack("<I", buffer[:4])[0]
    message, buffer = json.loads(buffer[4:4 + size]), buffer[4 + size:]
    return message
def send(reply):
    body = json.dumps(reply).encode()
    connection.sendall(struct.pack("<I", len(body)) + body)
while True:
    m = read()
    if m["op"] == "health":
        send({"ok": True, "info": {"selection_state": "ready"}})
    elif m["op"] == "open":
        sessions[len(sessions) + 1] = {"keys": "", "left": ""}
        send({"ok": True, "session": len(sessions)})
    elif m["op"] == "close":
        send({"ok": True})
    else:
        s, r = sessions[m["session"]], m["req"]
        snapshot = {"handled": True, "commit": ""}
        if r["type"] == "context":
            s["left"] = pending.pop(m["session"], "") if r.get("on") else ""
        elif r["key"] == "Enter":
            snapshot["commit"] = s["left"] + s["keys"].upper()
            pending[m["session"]] = snapshot["commit"][-1:] if not r.get("private") else ""
            snapshot["context_id"] = 7
            s["keys"], s["left"] = "", ""
        elif r["key"] == "[":
            snapshot["handled"] = False
        else:
            s["keys"] += r["key"].strip()
            snapshot["preedit"] = [{"text": s["left"] + s["keys"].upper(), "style": ""}]
        send({"ok": True, "snapshot": snapshot, "engine_us": 5})
"""

# A shanjie-eval stand-in: each row's candidates are its truth and its truth reversed; the pick is the reversed one
# when the context ends in 反.  Syllables containing ㄭ have no decode path.
FAKE_SHANJIE = r"""
import hashlib, json, sys
args = sys.argv[1:]
value = lambda name: args[args.index(name) + 1]
rows = [line.split("|") for line in open(value("--rows"), encoding="utf-8").read().splitlines()]
dump = open(value("--dump"), "w", encoding="utf-8")
scores = open(value("--tinylm-scores"), "w", encoding="utf-8")
chosen, stats = [], []
for i, (context, truth, syllables) in enumerate(rows, 1):
    if "ㄭ" in syllables:
        dump.close(); scores.close()
        print("error: no decode path (input length 3)", file=sys.stderr)
        sys.exit(1)
    candidates = [truth, truth[::-1]]
    for rank, text in enumerate(candidates, 1):
        dump.write(f"{i}\t{rank}\t{text}\t-1.0\n")
    pick = 1 if context.endswith("反") else 0
    scores.write(json.dumps({"i": i, "c": pick}) + "\n")
    chosen.append(candidates[pick])
    stats.append(f"{i}\t{int(candidates[pick] == truth)}\t0\t{len(truth)}\n")
open(value("--rowstats"), "w").write("".join(stats))
digest = hashlib.sha256("\n".join(chosen).encode()).hexdigest()
top1 = sum(c == r[1] for c, r in zip(chosen, rows))
n = len(rows)
print(f"## rows.txt  lm-test  {{'n': {n}, 'top1': {top1}, 'oracle@64': {n}, 'top1_sha256': '{digest}'}}")
"""


def executable(path: Path, body: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"#!{sys.executable}\n" + textwrap.dedent(body), encoding="utf-8")
    path.chmod(0o755)
    return path


class Scripts(unittest.TestCase):
    def test_parse_script(self):
        keys = parse_script("<dt:120.5>s<dt:80.000>u3 x<space><left><enter>")
        self.assertEqual([k.name for k in keys], ["s", "u", "3", "space", "x", "space", "left", "enter"])
        self.assertEqual((keys[0].gap_ms, keys[1].gap_ms, keys[2].gap_ms), (120.5, 80.0, None))
        self.assertTrue(keys[0].is_char)
        self.assertFalse(Key("space").is_char)
        with self.assertRaises(ValueError):
            parse_script("<down>")

    def test_case_keys_and_task_kinds(self):
        case = Case("0", "你好", (Segment("su3", ("ㄋㄧˇ",)), Segment("cl3", ("ㄏㄠˇ",))))
        self.assertEqual(case.keys, "su3<enter>cl3<enter>")
        self.assertTrue(case.exact)
        self.assertFalse(Case("1", "你", (Segment("su4"),)).exact)
        self.assertTrue(Task("s", "split", "zhuyintype", [case]).sequential)
        self.assertTrue(Task("s", "typing/hotspot", "zhuyintype", []).keys_only)
        self.assertFalse(Task("s", "rows", "shanjie", []).keys_only)

    def test_percentiles(self):
        self.assertEqual(percentile([3, 1, 2, 4], 0.5), 2)
        self.assertEqual(percentile([3, 1, 2, 4], 0.95), 4)
        summary = key_time_summary([[1, 2, 3], [10, 20, 30]])
        self.assertEqual(summary["median"]["p50_ms"], 11)
        self.assertEqual(summary["keys"], 6)


class ZeroCProtocol(unittest.TestCase):
    def test_frames(self):
        data = zeroc.frame({"op": "health"}) + zeroc.frame({"op": "open", "x": "注音"})
        first, rest = zeroc.unframe(data[:3])
        self.assertIsNone(first)
        first, rest = zeroc.unframe(data)
        self.assertEqual(first, {"op": "health"})
        second, rest = zeroc.unframe(rest)
        self.assertEqual(second, {"op": "open", "x": "注音"})
        self.assertEqual(rest, b"")
        self.assertEqual(data[:4], len(json.dumps({"op": "health"}).encode()).to_bytes(4, "little"))
        with self.assertRaises(ValueError):
            zeroc.unframe((zeroc.MAX_FRAME + 1).to_bytes(4, "little"))

    def test_sessions_against_a_fake_server(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            executable(root / "0c" / "bin" / "0c-server", FAKE_0C)
            (root / "0c" / "setup.json").write_text("{}", encoding="utf-8")
            with unittest.mock.patch.dict(os.environ, {"ZT_SYSTEMS_DIR": name}):
                system = zeroc.ZeroC("statistical", workers=2)
                system.start()
                try:
                    plain = [Case(str(i), "", (Segment(k),)) for i, k in enumerate(["ab", "c d", "x[y"])]
                    outcomes = system.convert(Task("t", "plain", "zhuyintype", plain), plain)
                    self.assertEqual([o.output for o in outcomes], ["AB", "CD", "[XY"])
                    split = [Case("0", "", (Segment("ab"), Segment("cd")))]
                    (outcome,) = system.convert(Task("t", "split", "zhuyintype", split), split)
                    # The second commit starts with the confirmed context of the first (the fake's last character).
                    self.assertEqual(outcome.output, "ABBCD")
                    self.assertEqual(system.unsupported(Task("t", "context", "zhuyintype", [])), "no-context")
                finally:
                    system.close()


class ShanjieCli(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "eval").mkdir()
        (self.root / "eval" / "variants.tsv").write_text("裏\t裡\n", encoding="utf-8")
        self.binary = executable(self.root / "shanjie-eval", FAKE_SHANJIE)

    def test_parsers(self):
        self.assertEqual(shanjie.parse_dump("1\t1\t甲\t-1\n1\t2\t乙\t-2\n2\t1\t丙\t-1\n"), {1: ["甲", "乙"], 2: ["丙"]})
        with self.assertRaises(ValueError):
            shanjie.parse_dump("1\t2\t甲\t-1\n")
        self.assertEqual(shanjie.parse_scores('{"i": 1, "c": 3}\n{"i": 2, "c": 0}\n'), {1: 3, 2: 0})
        summary = shanjie.parse_summary(
            "## a.txt  lm-chat+ctx  {'n': 3, 'top1': 2, 'oracle@64': 3, 'top1_sha256': 'ab12'}"
        )
        self.assertEqual(
            (summary["n"], summary["top1"], summary["sha256"], summary["tag"]), (3, 2, "ab12", "lm-chat+ctx")
        )
        self.assertEqual(shanjie.row_line("前|文", "真", ("ㄓㄣ",)), "前 文|真|ㄓㄣ")

    def test_rows_with_a_row_without_a_path(self):
        cli = shanjie.Cli("chat", workers=2, binary=self.binary, root=self.root)
        rows = ["|甲乙|ㄐㄧㄚˇ ㄧˇ", "反|丙丁|ㄅㄧㄥˇ ㄉㄧㄥ", "|戊己|ㄭ ㄐㄧˇ", "|庚辛|ㄍㄥ ㄒㄧㄣ"]
        self.assertEqual(cli.run(rows), ["甲乙", "丁丙", None, "庚辛"])
        self.assertEqual(cli.mismatches, 0)

    def test_split_cases_take_the_first_commit_as_context(self):
        system = shanjie.Shanjie("chat", workers=1, root=self.root)
        system.cli = shanjie.Cli("chat", 1, binary=self.binary, root=self.root)
        cases = [
            Case("0", "甲乙反丙", (Segment("x", ("ㄐ", "ㄧ", "ㄈ")), Segment("y", ("ㄅ",))), ""),
            Case("1", "丁戊", (Segment("x", ("ㄉ",)), Segment("y", ("ㄭ",)))),
        ]
        outcomes = system.convert(Task("t", "split", "zhuyintype", cases), cases)
        # The second part of case 0 follows 甲乙反, so the fake picks its reversed candidate (a single character).
        self.assertEqual([(o.output, o.status) for o in outcomes], [("甲乙反丙", "ok"), ("", "no-path")])

    def test_check_detects_a_wrong_recovery(self):
        cli = shanjie.Cli("chat", workers=1, binary=self.binary, root=self.root)
        digest = hashlib.sha256(b"x").hexdigest()
        stdout = f"## r  lm  {{'n': 1, 'top1': 1, 'oracle@64': 1, 'top1_sha256': '{digest}'}}"
        (self.root / "rowstats.tsv").write_text("1\t1\t0\t1\n", encoding="utf-8")
        with self.assertRaises(RuntimeError):
            cli.check(stdout, ["|y|ㄧ"], ["y"], self.root)


if __name__ == "__main__":
    unittest.main()
