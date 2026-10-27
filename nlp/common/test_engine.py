"""Unit tests for nlp.common.engine."""

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from nlp.common.engine import EngineConfig


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


if __name__ == "__main__":
    unittest.main()
