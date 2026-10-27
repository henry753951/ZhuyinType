"""The model server's pure parts: both wire formats parse to the same request, prefix trees, bucket sizes.  Needs the
export group (PyTorch, NumPy); skipped without it.

    zt --remote check python --group export --start src/zhuyintype_ml/remote
"""

from __future__ import annotations

import importlib.util
import struct
import unittest
from typing import Any

HAS_TORCH = importlib.util.find_spec("torch") is not None and importlib.util.find_spec("numpy") is not None


@unittest.skipUnless(HAS_TORCH, "needs the export dependency group")
class ModelServerTests(unittest.TestCase):
    def models(self) -> Any:
        from zhuyintype_ml.remote import model_server

        models = object.__new__(model_server.Models)
        models.vocab = 100
        models.limits = {"context": 100, "ids": 50, "tones": 6, "costs": 4, "boundary": 2, "candidates": 100}
        return models

    def test_binary_and_json_requests_agree(self) -> None:
        from zhuyintype_ml.remote import model_server

        models = self.models()
        ids, targets = [1, 5, 6, 1, 5, 7], [5, 6, 0, 5, 7, 8]
        frame = struct.pack("<14i", 2, 3, *ids, *targets)
        binary = models.binary_request(model_server.RERANK, frame)
        text = models.json_request({"op": "rerank", "batch": 2, "length": 3, "ids": ids, "targets": targets})
        self.assertEqual(binary.ids.tolist(), [[1, 5, 6], [1, 5, 7]])
        self.assertEqual((binary.ids.tolist(), binary.targets.tolist()), (text.ids.tolist(), text.targets.tolist()))
        syllable = [1, 9, 9], [3, 4], [1, 4], [0, 1], [0, 0], [[10, 11], [12, 0]]
        frame = struct.pack("<18i", 3, 2, 2, *[x for part in syllable[:5] for x in part], 10, 11, 12, 0)
        binary = models.binary_request(model_server.SYLLABLE, frame)
        keys = ("context", "ids", "tones", "costs", "boundary", "candidates")
        text = models.json_request({"op": "syllable", **dict(zip(keys, syllable))})
        self.assertEqual(binary.context, (1, 9, 9))
        self.assertEqual(binary.candidates.tolist(), text.candidates.tolist())
        with self.assertRaises(model_server.RequestError):
            models.binary_request(model_server.RERANK, struct.pack("<14i", 2, 3, *ids, 5, 6, 0, 5, 7, 100))

    def test_rows_share_their_prefixes(self) -> None:
        from zhuyintype_ml.remote import model_server

        request = self.models().json_request(
            {
                "op": "rerank",
                "batch": 2,
                "length": 4,
                "ids": [1, 5, 6, 0, 1, 5, 7, 8],
                "targets": [5, 6, 9, 0, 5, 7, 8, 9],
            }
        )
        tokens, depths, parents, state = model_server.Models.pack(request)
        self.assertEqual((tokens, depths, parents), ([1, 5, 6, 7, 8], [0, 1, 2, 2, 3], [-1, 0, 1, 1, 3]))
        self.assertEqual(state.tolist(), [[0, 1, 2, -1], [0, 1, 3, 4]])

    def test_buckets(self) -> None:
        from zhuyintype_ml.remote.model_server import bucket

        self.assertEqual([bucket(v, 4) for v in (1, 4, 5, 33)], [4, 4, 8, 64])
        self.assertEqual([bucket(v, 16, step=True) for v in (1, 16, 17)], [16, 16, 32])


if __name__ == "__main__":
    unittest.main()
