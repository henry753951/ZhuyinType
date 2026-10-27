"""The runner's pure parts: the steps of a run, output paths on the host, the host settings, asset groups.

zt check python --start src/zhuyintype_ml/remote
"""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from zhuyintype_ml.core import paths
from zhuyintype_ml.data import assets
from zhuyintype_ml.remote import host, runner


class StepsTests(unittest.TestCase):
    def test_default_steps_prepare_then_run(self) -> None:
        steps = runner.steps(["eval", "conversion"], runner.Options("w", backend="cpu"))
        self.assertEqual(
            steps,
            [
                ["setup", "--quiet"],
                ["data", "fetch", "--quiet", "eval"],
                ["data", "compile", "--quiet"],
                ["eval", "conversion"],
            ],
        )

    def test_evaluations_default_to_the_gpu_and_the_bench_to_the_cpu(self) -> None:
        options = runner.Options("w")
        self.assertEqual(runner.steps(["eval", "typo"], options)[-1][:4], ["models", "serve", "--device", "cuda"])
        for argv in (["bench", "run", "v1"], ["eval", "stats", "a", "b"], ["eval", "latency"], ["train", "v3"]):
            self.assertEqual(runner.steps(argv, options)[-1], argv)
        self.assertNotIn(["data", "compile", "--quiet"], runner.steps(["train", "v3"], options))

    def test_setup_runs_once_and_fetches_nothing(self) -> None:
        self.assertEqual(runner.steps(["setup"], runner.Options("w")), [["setup"]])

    def test_cuda_wraps_the_command_and_needs_the_export_group(self) -> None:
        options = runner.Options("w", fetch=[], backend="cuda")
        steps = runner.steps(["eval", "conversion"], options)
        self.assertEqual(steps[0], ["setup", "--quiet", "--group", "export"])
        self.assertEqual(steps[-1], ["models", "serve", "--device", "cuda", "--", "eval", "conversion"])

    def test_export_step(self) -> None:
        steps = runner.steps(["bench", "run", "v1"], runner.Options("w", fetch=["eval"], export=True))
        self.assertIn(["models", "export", "--tree"], steps)

    def test_env_must_be_key_value(self) -> None:
        with self.assertRaises(SystemExit):
            runner.environment(runner.Options("w", env=["nonsense"]))
        self.assertEqual(runner.environment(runner.Options("w", env=["A=1=2"])), {"A": "1=2"})


class OptionTests(unittest.TestCase):
    def test_nice_is_parsed_and_bounded(self) -> None:
        import argparse

        from zhuyintype_ml.cli.commands import remote

        parser = argparse.ArgumentParser()
        remote.run_options(parser)
        self.assertEqual(remote.options_from(parser.parse_args(["--nice", "10"])).nice, 10)
        self.assertEqual(remote.options_from(parser.parse_args([])).nice, 0)
        with self.assertRaises(argparse.ArgumentTypeError):
            remote.nice_value("20")


class OutputTests(unittest.TestCase):
    def test_inside_the_checkout_keeps_its_place(self) -> None:
        argv, outputs = runner.rewrite_outputs(["eval", "conversion", "--json", "data/build/x.json"], paths.ROOT)
        self.assertEqual(argv, ["eval", "conversion", "--json", "data/build/x.json"])
        self.assertEqual(outputs[0].remote, "data/build/x.json")
        self.assertEqual(outputs[0].local, paths.ROOT / "data/build/x.json")

    def test_outside_goes_to_remote_eval(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            argv, outputs = runner.rewrite_outputs(["eval", "x", "--out=r.json"], Path(directory))
        self.assertEqual(argv, ["eval", "x", "--out=data/build/remote_eval/out/r.json"])
        self.assertEqual(outputs[0].local.name, "r.json")

    def test_after_double_dash_is_untouched(self) -> None:
        argv, outputs = runner.rewrite_outputs(["models", "serve", "--", "eval", "--json", "a.json"], paths.ROOT)
        self.assertEqual(argv[-2:], ["--json", "a.json"])
        self.assertEqual(outputs, [])


class HostTests(unittest.TestCase):
    def test_env_file_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            env = Path(directory) / ".remote.env"
            env.write_text("# comment\nZHUYIN_REMOTE_HOST=me@there\nZHUYIN_REMOTE_KEY=$HOME/.ssh/k\n")
            with mock.patch.dict(os.environ, {"ZHUYIN_REMOTE_HOST": ""}, clear=False):
                values = host.read_env(env)
            self.assertEqual(values["ZHUYIN_REMOTE_HOST"], "me@there")
            self.assertEqual(values["ZHUYIN_REMOTE_KEY"], str(Path.home() / ".ssh" / "k"))
            with mock.patch.dict(os.environ, {"ZHUYIN_REMOTE_HOST": "other@host"}):
                self.assertEqual(host.read_env(env)["ZHUYIN_REMOTE_HOST"], "other@host")

    def test_work_directory_names(self) -> None:
        self.assertEqual(host.check_name("onnx-eval-x"), "onnx-eval-x")
        for bad in ["../x", "a/b", "", ".hidden", "a b"]:
            with self.assertRaises(SystemExit):
                host.check_name(bad)
        self.assertTrue(host.NAME.match(host.default_workdir()))

    def test_paths_are_quoted_for_the_remote_shell(self) -> None:
        h = host.Host("me@there", None, "zhuyintype")
        self.assertEqual(h.home_path("work", "w"), '"$HOME"/zhuyintype/work/w')
        self.assertEqual(host.Host("me@there", None, "my dir").home_path("w"), "\"$HOME\"/'my dir/w'")
        self.assertEqual(h.remote("work", "w"), "me@there:zhuyintype/work/w")


class AssetTests(unittest.TestCase):
    def test_groups_expand_in_order_without_repeats(self) -> None:
        self.assertEqual(assets.expand(["engine", "lexicon", "eval-sets"]), ["lexicon", "lm", "eval-sets"])
        with self.assertRaises(SystemExit):
            assets.expand(["nothing"])

    def test_every_pinned_component_is_known(self) -> None:
        self.assertEqual(set(assets.entries()), set(assets.COMPONENTS))


if __name__ == "__main__":
    unittest.main()
