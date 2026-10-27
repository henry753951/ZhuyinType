"""chat2 regression checks: gazette and talk-page turn parsing, dialogue context selection, failure diagnostics,
held-out matching and the estimator.  Skipped without the train dependency group.

    python -m unittest discover -s ml/src/zhuyintype_ml/training/lm_chat2 -t ml/src
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

try:
    import mwparserfromhell  # noqa: F401  (wiki_threads imports it lazily)
    import numpy as np

    from zhuyintype_ml.core.zhuyin import Lexicon
    from zhuyintype_ml.training.lm import mkn
    from zhuyintype_ml.training.lm_chat2.evaluate import run_batch, selected_inputs
    from zhuyintype_ml.training.lm_chat2.prepare import gazette_turns, wiki_threads
except ImportError:  # numpy, opencc, pyahocorasick, mwparserfromhell: the train dependency group
    mkn = None  # type: ignore[assignment]


@unittest.skipIf(mkn is None, "needs the train dependency group")
class Checks(unittest.TestCase):
    def test_failed_batch_retains_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "failure.json"
            command = [
                sys.executable,
                "-c",
                'import sys; print("partial"); sys.stderr.write("fixture error"); sys.exit(7)',
            ]
            with self.assertRaisesRegex(RuntimeError, "returncode=7"):
                run_batch(command, ["keys"], path)
            receipt = json.loads(path.read_text())
            self.assertEqual(receipt["inputs"], ["keys"])
            self.assertEqual(receipt["returncode"], 7)
            self.assertEqual(receipt["stderr"], "fixture error")

    def test_speaker_boundaries(self) -> None:
        text = "主席：請你說明一下。\n陳委員小明：這個要怎麼處理？\n說明：附件\n王部長小華：我們會再看看。"
        turns = gazette_turns(text)
        self.assertEqual(len(turns), 4)
        self.assertEqual(turns[0]["text"], "")
        self.assertEqual(turns[1]["text"], "這個要怎麼處理？")
        self.assertEqual(turns[2]["text"], "")

    def test_signed_threads(self) -> None:
        raw = (
            "==第一段==\n這個我覺得可以。--[[User:Alice|Alice]] 2025年1月1日 (三) 10:00 (UTC)\n"
            ":那你要不要再確認？--[[User:Bob|Bob]] 2025年1月1日 (三) 10:01 (UTC)\n"
            "==第二段==\n這段沒有回應。--[[User:Alice|Alice]] 2025年1月1日 (三) 10:02 (UTC)"
        )
        result = wiki_threads(raw)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0][1]["text"], "那你要不要再確認？")
        self.assertNotIn("Alice", result[0][0]["text"])
        mentioned = raw.replace("那你要不要再確認？", "那你要不要再確認？[[User:Other|某人]]")
        self.assertEqual(wiki_threads(mentioned)[0][1]["speaker"], "Bob")

    def test_target_only_tsv(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "dev_dialogue.txt"
            path.write_text("你好不好\t今天下雨\n")
            lexicon = Path(d) / "lexicon.tsv"
            lexicon.write_text("".join(f"ㄧ\t{c}\t-1\n" for c in "你好不今天下雨"))
            wanted, scripts = selected_inputs(path, True, Lexicon(lexicon), 400)
            self.assertEqual(wanted, ["今天下雨"])
            self.assertTrue(scripts[0].startswith("你好不好\t"))

    def test_heldout_and_pruned_probability(self) -> None:
        held = {"測試資料千萬不要進去訓練"}
        machine = mkn.automaton(held)
        self.assertIsNotNone(next(machine.iter("前文測試資料千萬不要後文"), None))
        keys = np.array([(0 << 32) | 3, (3 << 32) | 1, (0 << 32) | 4, (4 << 32) | 1], dtype=np.uint64)
        counts = np.array([3, 3, 2, 2], dtype=np.uint64)
        model = mkn.fit_mkn(keys, counts, 5, 10)
        with tempfile.TemporaryDirectory() as d:
            mkn.write_tsv(Path(d), ["<s>", "</s>", "<unk>", "今天", "下雨"], model)
            self.assertLess(mkn.TsvModel(Path(d)).verify()["all_contexts_max_error"], 1e-9)


if __name__ == "__main__":
    unittest.main()
