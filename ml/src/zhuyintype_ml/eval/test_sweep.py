"""Regression tests for error reporting and resumable work."""

from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from zhuyintype_ml.eval import sweep


class StandardsFixes(unittest.TestCase):
    def test_sweep_rejects_collisions_before_running(self):
        matrix = {"configurations": [{"name": "a-b"}, {"name": "a"}], "sets": [{"name": "c"}, {"name": "b-c"}]}
        with tempfile.TemporaryDirectory() as d, patch.object(sweep, "load_matrix", return_value=matrix):
            with self.assertRaises(SystemExit):
                sweep.run(argparse.Namespace(matrix=Path("m"), output=Path(d)))
