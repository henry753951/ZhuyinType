"""Regression tests for error reporting and resumable work."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from zhuyintype_ml.training.pipeline import Context, Pipeline, Step


class StandardsFixes(unittest.TestCase):
    def test_failed_step_clears_marker(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            marker = root / "steps" / "one.json"
            marker.parent.mkdir()
            marker.write_text("old")

            def fail(_context):
                raise RuntimeError("failed")

            pipeline = Pipeline("test", "", "", (Step("one", "", fail),))
            with patch.object(Pipeline, "context", return_value=Context(root, root)), self.assertRaises(RuntimeError):
                pipeline.run(force=True)
            self.assertFalse(marker.exists())
