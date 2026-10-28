"""Regression tests for error reporting and resumable work."""

from __future__ import annotations

import unittest
from types import SimpleNamespace
from unittest.mock import patch

from zhuyintype_ml.cli.commands import train


class StandardsFixes(unittest.TestCase):
    def test_missing_inner_module_is_not_ignored(self):
        module = SimpleNamespace(ispkg=True, name="sample")
        error = ModuleNotFoundError("missing", name="dependency")
        with (
            patch.object(train.pkgutil, "iter_modules", return_value=[module]),
            patch.object(train.importlib, "import_module", side_effect=error),
            self.assertRaises(ModuleNotFoundError),
        ):
            train.pipelines()
