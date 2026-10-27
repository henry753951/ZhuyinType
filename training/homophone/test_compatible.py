"""Focused checks for source routing, holdout parsing and paired result identity."""
import json
from pathlib import Path
import sys
import tempfile
import subprocess
import unittest
from unittest.mock import patch

from common import ROOT, jsonl
from compatible import OUT, blocked_fields, exclusions
from ship_eval import model_args, result_rows


class CompatibleTests(unittest.TestCase):
    def test_nohup_evaluator_survives_launcher_disconnect(self):
        import experiment
        from unittest.mock import Mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/'data/build/remote_eval').mkdir(parents=True)
            child=Mock(pid=123)
            child.wait.return_value=0
            with patch.object(experiment,'ROOT',root),patch.object(Path,'home',return_value=root), \
                 patch.object(sys,'argv',['experiment.py','compatible','--role','chosen','--phase','test']), \
                 patch.object(experiment.subprocess,'Popen',return_value=child) as start:
                experiment.main()
            args,kwargs=start.call_args
            self.assertEqual(args[0][0],'nohup')
            self.assertTrue(kwargs['start_new_session'])
            self.assertEqual(kwargs['stdin'],subprocess.DEVNULL)
            self.assertEqual((root/'data/build/remote_eval/compatible-evaluation.pid').read_text(),'123\n')

    def test_holdout_columns_and_join(self):
        self.assertEqual(blocked_fields('abcdefgh|目標句子|ㄇㄨˋ ㄅㄧㄠ'),['目標句子','abcdefgh','abcdefgh目標句子'])
        self.assertEqual(blocked_fields('短\t測試句子'),['測試句子','短測試句子'])
        self.assertEqual(blocked_fields('# comment'),[])
        held=exclusions()
        self.assertIn('有人說我在吃一碗麵但是沒吃完',held)
        paths=json.loads((OUT/'inputs.json').read_text())['holdouts']
        self.assertTrue(any('typing76.clean.rows.txt' in p for p in paths))
        self.assertTrue(any('test_dialogue' in p for p in paths))

    def test_current_stack_and_paired_identity(self):
        args=model_args('v3_homophone2')
        self.assertEqual(args[args.index('--v3-weight')+1],'0.5')
        self.assertEqual(args[args.index('--chat-prior')+1],'0.3')
        self.assertEqual(args[args.index('--rerank-weight')+1],'0.5')
        a=result_rows(['臺灣','我在吃'],['台灣','我再吃'],['keys1','keys2'])
        b=result_rows(['臺灣','我在吃'],['臺灣','我在吃'],['keys1','keys2'])
        self.assertEqual([r['id'] for r in a],[r['id'] for r in b])
        self.assertEqual((a[0]['ok_raw'],a[0]['ok_variants']),(0,1))
        self.assertEqual(a[1]['errors_raw'],1)
        with self.assertRaises(ValueError): result_rows(['甲'],[],['x'])
        import ship_eval
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)
            (out/'results').mkdir()
            for r in b: r['cluster']='same-source-line'
            ship_eval.write_rows(out/'results/baseline-check.jsonl',a)
            ship_eval.write_rows(out/'results/chosen-check.jsonl',b)
            with patch.object(ship_eval,'OUT',out): ship_eval.paired()
            stats=json.loads((out/'results/paired-check.json').read_text())
            self.assertEqual(stats['raw']['bootstrap_unit'],'cluster')
            self.assertEqual(stats['raw']['net'],2)
            cli=out/'fake-cli'
            cli.write_text('never executed')
            with patch.object(ship_eval,'OUT',out), patch.object(sys,'argv',[
                    'ship_eval.py','--cli',str(cli),'--role','baseline','--phase','test']):
                with self.assertRaisesRegex(AssertionError,'before opening test'): ship_eval.main()
            self.assertFalse((out/'baseline-test-started.json').exists())
            (out/'results/baseline-test_pairs-ctx0.log').write_text('already opened')
            with patch.object(ship_eval,'OUT',out):
                with self.assertRaisesRegex(AssertionError,'already started'):
                    ship_eval.homophone(cli,1,'baseline','v3','test_pairs',False,recover=True)

    def test_compatible_sampler_never_opens_tt(self):
        import train_v3
        self.assertTrue(train_v3.target_text('再試另外一種方法','再'))
        self.assertFalse(train_v3.target_text('再生能源發電','再'))
        self.assertTrue(train_v3.target_text('我們在吃晚餐','在'))
        self.assertTrue(train_v3.target_text('今天我在吃晚餐','在'))
        self.assertFalse(train_v3.target_text('我在家裡等你','在'))
        self.assertFalse(train_v3.target_text('我再吃一碗','在'))
        opened=[]
        def checked(path):
            self.assertEqual(Path(path).name,'real-train.jsonl')
            opened.append(str(path))
            return jsonl(path)
        train_v3.v3()
        with patch.object(train_v3,'jsonl',checked):
            stream=iter(train_v3.Batches(2,OUT,compatible=True))
            for _ in range(12):
                tensors=next(stream)
                self.assertEqual(len(tensors),7)
                self.assertEqual(tensors[0].shape[0],2)
        self.assertEqual(len(opened),1)


if __name__=='__main__':
    assert sys.platform=='linux'
    unittest.main()
