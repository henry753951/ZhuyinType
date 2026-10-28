"""Regression tests for error reporting and resumable work."""

from __future__ import annotations

import unittest
from types import SimpleNamespace
from unittest.mock import patch

from zhuyintype_ml.core import environment


class StandardsFixes(unittest.TestCase):
    def test_empty_version(self):
        with patch.object(
            environment.subprocess, "run", return_value=SimpleNamespace(stdout="", stderr="", returncode=0)
        ):
            self.assertEqual(environment._version(["tool"]), "")
