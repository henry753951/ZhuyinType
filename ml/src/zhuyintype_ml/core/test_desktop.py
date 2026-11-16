"""zt build desktop / zt check desktop hand their options to platforms/desktop/scripts/build.sh, on Linux only."""

import unittest
from unittest.mock import Mock, patch

from zhuyintype_ml.core import desktop


class ArgumentTests(unittest.TestCase):
    def test_defaults_build_linux_debug(self):
        self.assertEqual(desktop.arguments(), ["linux"])

    def test_flags_follow_the_target(self):
        self.assertEqual(
            desktop.arguments("all", release=True, package=True, native=True),
            ["all", "--release", "--package", "--native"],
        )

    def test_unknown_target_is_rejected(self):
        with self.assertRaises(ValueError):
            desktop.arguments("macos")


class RunTests(unittest.TestCase):
    @patch.object(desktop.sys, "platform", "linux")
    @patch.object(desktop.subprocess, "run", return_value=Mock(returncode=3))
    def test_script_status_is_returned(self, run):
        self.assertEqual(desktop.build("linux", package=True), 3)
        command = run.call_args.args[0]
        self.assertEqual(command[:2], ["bash", str(desktop.SCRIPT)])
        self.assertEqual(command[2:], ["linux", "--package"])

    @patch.object(desktop.sys, "platform", "linux")
    @patch.object(desktop.subprocess, "run", return_value=Mock(returncode=0))
    def test_check_runs_the_tests(self, run):
        self.assertEqual(desktop.check(), 0)
        self.assertEqual(run.call_args.args[0][2:], ["linux", "--test"])

    @patch.object(desktop.sys, "platform", "darwin")
    @patch.object(desktop.subprocess, "run")
    def test_other_systems_build_nothing(self, run):
        with self.assertRaises(SystemExit):
            desktop.build()
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
