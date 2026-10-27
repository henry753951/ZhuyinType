"""compile_data: binary layout, determinism and TSV parsing on a small fixture (standard library only).

python -m unittest discover -s ml/src/zhuyintype_ml/training/lm -t ml/src
"""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import struct
import tempfile
import unittest
from pathlib import Path

from zhuyintype_ml.training.lm.compile_data import compile_lexicon, compile_lm


class CompilerChecks(unittest.TestCase):
    def test_layout_and_determinism(self) -> None:
        with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
            root = Path(d)
            source = root / "source"
            source.mkdir()
            (source / "unigram.tsv").write_text("</s>\t-1\n<unk>\t-8\n!\t-2\n八\t-3\n#\t-7\n \t-6\n")
            (source / "bigram.tsv").write_text("<s>\t八\t-0.1\n八\t!\t-0.2\n")
            (source / "backoff.tsv").write_text("八\t-0.4\n<s>\t-0.7\n")
            lexicon = source / "lexicon.tsv"
            lexicon.write_text("ㄅㄚ\t八\t-3\nㄅㄚ\t吧\t-10\nㄅㄚ˙\t吧\t-2\n")
            for target in [root / "a", root / "b"]:
                compile_lm(source, target / "lm")
                compile_lexicon(lexicon, target)
            for name in ["lm/lm.bin", "lm/lm.json", "lexicon.bin", "lexicon.json"]:
                self.assertEqual((root / "a" / name).read_bytes(), (root / "b" / name).read_bytes())
            lm = (root / "a/lm/lm.bin").read_bytes()
            magic, version, order, v, b, t, vo, uo, bo, to, size = struct.unpack_from("<4s5I5Q", lm)
            self.assertEqual((magic, version, order, b, t, to, size), (b"ZTLM", 1, 2, 2, 0, 0, len(lm)))
            offsets = struct.unpack_from(f"<{v + 1}I", lm, vo)
            blob = vo + 4 * (v + 1)
            words = [lm[blob + a : blob + b].decode() for a, b in zip(offsets, offsets[1:])]
            self.assertEqual(words, ["</s>", "<s>", "<unk>", "!", "八"])
            self.assertAlmostEqual(struct.unpack_from("<f", lm, uo + 4 * (v + 4))[0], -0.4, places=6)
            metadata = json.loads((root / "a/lm/lm.json").read_text())
            self.assertEqual(metadata["missing_unigrams"], [1])
            self.assertEqual(metadata["sha256"], hashlib.sha256(lm).hexdigest())
            data = (root / "a/lexicon.bin").read_bytes()
            header = struct.unpack_from("<4s7I12Q", data)
            self.assertEqual(header[:8], (b"ZTLX", 1, 2, 2, 3, 1, 1, 2))
            entries = header[10]
            self.assertEqual(struct.unpack_from("<2I2d", data, entries + 24), (1, 0, -2.0, -8.0))
            self.assertEqual(struct.unpack_from("<2I2d", data, entries + 48), (1, 1, -2.0, 0.0))
            self.assertEqual(header[17], len(data))


if __name__ == "__main__":
    unittest.main()
