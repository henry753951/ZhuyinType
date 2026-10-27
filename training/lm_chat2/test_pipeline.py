"""Small parser, context-selection, leakage and estimator regressions (Linux only)."""
import tempfile
from pathlib import Path
import unittest
import json
import sys
import numpy as np
from common import lm, chat
from prepare import gazette_turns, wiki_threads
from evaluate import selected_inputs, run_batch


class Checks(unittest.TestCase):
    def test_failed_batch_retains_diagnostics(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'failure.json'
            command = [sys.executable, '-c', 'import sys; print("partial"); sys.stderr.write("fixture error"); sys.exit(7)']
            with self.assertRaisesRegex(RuntimeError, 'returncode=7'):
                run_batch(command, ['keys'], path)
            receipt = json.loads(path.read_text())
            self.assertEqual(receipt['inputs'], ['keys'])
            self.assertEqual(receipt['returncode'], 7)
            self.assertEqual(receipt['stderr'], 'fixture error')

    def test_speaker_boundaries(self):
        text = '主席：請你說明一下。\n陳委員小明：這個要怎麼處理？\n說明：附件\n王部長小華：我們會再看看。'
        turns = gazette_turns(text)
        self.assertEqual(len(turns), 4)
        self.assertEqual(turns[0]['text'], '')
        self.assertEqual(turns[1]['text'], '這個要怎麼處理？')
        self.assertEqual(turns[2]['text'], '')

    def test_signed_threads(self):
        raw = ('==第一段==\n這個我覺得可以。--[[User:Alice|Alice]] 2025年1月1日 (三) 10:00 (UTC)\n'
               ':那你要不要再確認？--[[User:Bob|Bob]] 2025年1月1日 (三) 10:01 (UTC)\n'
               '==第二段==\n這段沒有回應。--[[User:Alice|Alice]] 2025年1月1日 (三) 10:02 (UTC)')
        result = wiki_threads(raw)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0][1]['text'], '那你要不要再確認？')
        self.assertNotIn('Alice', result[0][0]['text'])
        mentioned = raw.replace('那你要不要再確認？', '那你要不要再確認？[[User:Other|某人]]')
        self.assertEqual(wiki_threads(mentioned)[0][1]['speaker'], 'Bob')

    def test_target_only_tsv(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'dev_dialogue.txt'
            path.write_text('你好不好\t今天下雨\n')
            lex = {c: 'ㄧ' for c in '你好不今天下雨'}
            wanted, scripts = selected_inputs(path, True, lex, 400)
            self.assertEqual(wanted, ['今天下雨'])
            self.assertTrue(scripts[0].startswith('你好不好\t'))

    def test_heldout_and_pruned_probability(self):
        held = {'測試資料千萬不要進去訓練'}
        machine = lm.automaton(held)
        self.assertIsNotNone(next(machine.iter('前文測試資料千萬不要後文'), None))
        keys = np.array([(0 << 32) | 3, (3 << 32) | 1, (0 << 32) | 4, (4 << 32) | 1], dtype=np.uint64)
        counts = np.array([3, 3, 2, 2], dtype=np.uint64)
        model = lm.fit_mkn(keys, counts, 5, 10)
        with tempfile.TemporaryDirectory() as d:
            chat.export(Path(d), ['<s>', '</s>', '<unk>', '今天', '下雨'], model)
            self.assertLess(chat.Model(Path(d)).verify()['all_contexts_max_error'], 1e-9)


if __name__ == '__main__': unittest.main()
