"""Regression tests for error reporting and resumable work."""

from __future__ import annotations

import unittest
from pathlib import Path

from zhuyintype_ml.remote import models


class StandardsFixes(unittest.TestCase):
    def test_fp16_outputs(self):
        files = models.outputs(Path("r"), Path("v"), True, True)
        self.assertIn(Path("r/RerankerTree.fp16.onnx"), files)
        self.assertIn(Path("v/ContextEncoder.fp16.onnx"), files)
