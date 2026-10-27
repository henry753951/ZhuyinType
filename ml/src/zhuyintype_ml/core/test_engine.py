"""Unit tests for zhuyintype_ml.core.engine."""

import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from zhuyintype_ml.core.engine import EngineConfig, EnginePool


class ReadingLexiconTest(unittest.TestCase):
    def test_bundle_without_tsv_uses_default_data(self) -> None:
        with tempfile.TemporaryDirectory() as bundle, tempfile.TemporaryDirectory() as default:
            (Path(bundle) / "lexicon.bin").write_bytes(b"")
            (Path(default) / "lexicon.tsv").write_text("ㄅㄚ\t八\t-1\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"ZT_DATA": default}):
                config = EngineConfig("app", Path(bundle))
                self.assertEqual(config.reading_lexicon, Path(default).resolve() / "lexicon.tsv")
                self.assertEqual(config.lexicon, Path(bundle) / "lexicon.tsv")

    def test_directory_with_tsv_uses_its_own(self) -> None:
        with tempfile.TemporaryDirectory() as data:
            (Path(data) / "lexicon.tsv").write_text("", encoding="utf-8")
            self.assertEqual(EngineConfig("engine", Path(data)).reading_lexicon, Path(data) / "lexicon.tsv")


def fake_cli(directory: Path, body: str) -> Path:
    """A zhuyin-cli stand-in answering each line with `body` (an expression of `line`, `argv` and `os`), flushed."""
    path = directory / "zhuyin-cli"
    path.write_text(
        f"#!{sys.executable}\nimport os, sys\nargv = sys.argv[1:]\n"
        f"for line in sys.stdin:\n    line = line.rstrip('\\n')\n    print({body}, flush=True)\n",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


class EnginePoolTest(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.config = EngineConfig("engine", self.root)

    def tearDown(self) -> None:
        self.directory.cleanup()

    def test_outputs_keep_the_input_order(self) -> None:
        cli = fake_cli(self.root, "line.upper() + ' ' + argv[-1]")
        lines = [f"line {i}" for i in range(25)]
        got = EnginePool(self.config, jobs=4, cli=cli).run(lines, "--typo", "off")
        self.assertEqual(got, [f"LINE {i} off" for i in range(25)])

    def test_run_many_runs_each_batch_with_its_options(self) -> None:
        cli = fake_cli(self.root, "line + argv[-1]")
        batches = [(["a", "b", "c"], ("--typo", "off")), ([], ("--typo", "x")), (["d"] * 7, ("--typo", "on"))]
        got = EnginePool(self.config, jobs=3, cli=cli).run_many(batches)
        self.assertEqual(got, [["aoff", "boff", "coff"], [], ["don"] * 7])

    def test_a_kept_pool_reuses_its_processes(self) -> None:
        cli = fake_cli(self.root, "os.getpid()")
        with EnginePool(self.config, jobs=2, cli=cli) as pool:
            first = set(pool.run(["x"] * 10))
            second = set(pool.run(["y"] * 10))
            self.assertLessEqual(len(first | second), 2)
        self.assertEqual(pool.idle, [])

    def test_a_failing_cli_raises(self) -> None:
        cli = fake_cli(self.root, "line if line != 'bad' else sys.exit(3)")
        with self.assertRaises(RuntimeError):
            EnginePool(self.config, jobs=2, cli=cli).run(["good", "bad", "good"])


if __name__ == "__main__":
    unittest.main()
