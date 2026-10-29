"""Build failures stop the Qt pipeline before tests run."""

import unittest
from unittest.mock import Mock, patch

from zhuyintype_ml.core import desktop


class DesktopBuildTests(unittest.TestCase):
    @patch.object(desktop.paths, "cargo_command", return_value=["cargo", "build"])
    @patch.object(desktop.shutil, "which", side_effect=lambda name: name)
    @patch.object(desktop.subprocess, "run")
    def test_build_failure_stops_checks(self, run, _which, _cargo):
        run.side_effect = [Mock(returncode=0), Mock(returncode=2)]
        self.assertEqual(desktop.build(check=True), 2)
        self.assertEqual(run.call_count, 2)

    @patch.object(desktop.paths, "cargo_command", return_value=["cargo", "build"])
    @patch.object(desktop.shutil, "which", side_effect=lambda name: name)
    @patch.object(desktop.subprocess, "run", return_value=Mock(returncode=0))
    def test_checks_use_multiconfig_release(self, run, _which, _cargo):
        self.assertEqual(desktop.build(check=True), 0)
        command = run.call_args.args[0]
        self.assertEqual(command[0], "ctest")
        self.assertIn("Release", command)


class NativeBuildTests(unittest.TestCase):
    @patch.object(desktop.sys, "platform", "linux")
    @patch.object(desktop, "build", return_value=7)
    @patch.object(desktop.subprocess, "run")
    def test_host_failure_stops_native_build(self, run, host):
        self.assertEqual(desktop.build_input_method(check=True), 7)
        host.assert_called_once_with(check=True)
        run.assert_not_called()

    @patch.object(desktop.sys, "platform", "darwin")
    @patch.object(desktop, "build")
    def test_unsupported_host_changes_nothing(self, host):
        with self.assertRaises(SystemExit):
            desktop.build_input_method()
        host.assert_not_called()
