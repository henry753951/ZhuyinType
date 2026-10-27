"""Small regression checks for leakage, alignment, categories and TSV input."""
import importlib.util
from pathlib import Path
import unittest
import tempfile
import json
import random
import subprocess
import sys
from unittest import mock
import numpy as np
import filter as filtering
import split as splitting
import common as shared
import experiment

from common import Exclusions, OUT, ROOT, positions, v3

spec = importlib.util.spec_from_file_location('homophone_eval',Path(__file__).with_name('eval.py'))
evaluation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluation)


class Checks(unittest.TestCase):
    def test_final_role_is_sealed_and_smoke_is_not_duplicated(self):
        with tempfile.TemporaryDirectory(dir=OUT,prefix='final-selftest-') as temp:
            root=Path(temp)
            (root/'choice.json').write_text(json.dumps(dict(models=['reranker','v3_homophone'])))
            old=experiment.OUT
            try:
                experiment.OUT=root
                argv=['experiment.py','final','--final-role','chosen','--cli',str(root/'fake-cli')]
                with mock.patch.object(sys,'argv',argv),mock.patch.object(subprocess,'run') as run:
                    experiment.main()
                    self.assertEqual(run.call_count,5)
                    self.assertTrue((root/'final-chosen-complete.json').exists())
                    with self.assertRaises(AssertionError): experiment.main()
                    with mock.patch.object(sys,'argv',['experiment.py','final','--cli',str(root/'fake-cli')]):
                        with self.assertRaises(AssertionError): experiment.main()
            finally:
                experiment.OUT=old

    def test_gpu_dispatch_preserves_order_and_cleans_up(self):
        with tempfile.TemporaryDirectory(dir=OUT,prefix='dispatch-selftest-') as temp:
            root=Path(temp)
            (root/'lexicon.tsv').write_text('ㄨㄛˇ\t我\t-1\nㄗㄞˋ\t再\t-1\nㄗㄞˋ\t在\t-1\n')
            (root/'inventory.json').write_text(json.dumps(dict(kept=[])))
            texts=['我我','再再','我再','再我']
            (root/'probe.txt').write_text('\n'.join(texts)+'\n')
            cli=root/'fake-cli'
            cli.write_text('#!'+sys.executable+'\nimport sys\nlookup='+repr(dict(zip(
                ['ji3ji3<enter>','y94y94<enter>','ji3y94<enter>','y94ji3<enter>'],texts)))+
                '\nfor line in sys.stdin: print(lookup[line.strip()])\n')
            cli.chmod(0o755)
            actual_popen=subprocess.Popen
            sockets=[]
            class Server:
                stopped=False
                def poll(self): return None
                def terminate(self): self.stopped=True
                def wait(self,timeout=None): return 0
            server=Server()
            def popen(command,**kwargs):
                if len(command)>1 and command[1].endswith('model_server.py'):
                    Path(command[command.index('--socket')+1]).touch()
                    return server
                sockets.append(command[command.index('--model-server')+1])
                return actual_popen(command,**kwargs)
            old=evaluation.OUT
            try:
                evaluation.OUT=root
                argv=['eval.py','--test',str(root/'probe.txt'),'--output',str(root/'result.json'),
                      '--lexicon',str(root/'lexicon.tsv'),'--cli',str(cli),'--v3',str(root/'v3'),
                      '--min-free-gb','1',
                      '--cli-arg=--model-server','--cli-arg='+str(root/'original.sock')]
                read_text=Path.read_text
                memory=iter([0,2**20])
                def read(path,*args,**kwargs):
                    return f'MemAvailable: {next(memory)} kB\n' if str(path)=='/proc/meminfo' else read_text(path,*args,**kwargs)
                with mock.patch.object(sys,'argv',argv),mock.patch.object(subprocess,'Popen',popen), \
                     mock.patch.object(Path,'read_text',read),mock.patch.object(evaluation.time,'sleep') as sleep:
                    evaluation.main()
                    sleep.assert_called_once_with(30)
                self.assertEqual(json.loads((root/'result.json').read_text())['predictions'],texts)
                self.assertEqual(len(set(sockets)),2)
                self.assertTrue(server.stopped)
                self.assertFalse(list(root.glob('*.sock-*')))
            finally:
                evaluation.OUT=old

    def test_short_context_is_not_a_forbidden_sentence(self):
        with tempfile.TemporaryDirectory(dir=OUT,prefix='held-selftest-') as temp:
            root=Path(temp)
            (root/'inputs').mkdir()
            (root/'smoke.txt').write_text('')
            for path in shared.HELD:
                (root/'inputs'/Path(path).name).write_text('')
            (root/'dev_zai.txt').write_text('，\t甲乙丙丁戊己庚辛\n無關短文\t壬癸子丑寅卯辰巳\n')
            old=shared.OUT
            try:
                shared.OUT=root
                held=shared.holdouts(new=True).compile()
                self.assertNotIn('這是無關，文字',held)
                self.assertNotIn('前無關短文後',held)
                self.assertIn('前甲乙丙丁戊己庚辛後',held)
                self.assertIn('今天我在家吃飯',held)
            finally:
                shared.OUT=old

    def test_exclusions(self):
        held = Exclusions(['甲乙丙丁戊己庚辛壬','我在家'])
        self.assertIn('前甲乙丙丁戊己庚辛後',held)
        self.assertIn('我在家',held)
        self.assertIn('今天我在家吃飯',held)
        self.assertNotIn('甲乙丙丁戊己庚壬',held)
        held.compile()
        self.assertIn('今天我在家吃飯',held)

    def test_alignment(self):
        self.assertEqual(evaluation.align('我再吃一碗','我再吃一碗'),dict(enumerate('我再吃一碗')))
        self.assertEqual(evaluation.align('我再吃一碗','我又再吃一碗')[1],'再')
        self.assertEqual(evaluation.align('我再吃一碗','我吃一碗')[1],'')

    def test_categories(self):
        v3()
        from readings import Lexicon
        lex = Lexicon(OUT/'lexicon.tsv')
        inventory = [dict(chars='在再',readings=[['ㄗㄞ',4]])]
        def category(text,index):
            return next(p for p in positions(text,lex,inventory) if p['index']==index)['category']
        self.assertEqual(category('我現在在忙',2),'fixed')
        self.assertEqual(category('我再吃一碗',1),'after_subject')
        self.assertEqual(category('再等我五分鐘',0),'initial')
        self.assertEqual(category('警方在調查',2),'after_subject')
        native = [dict(chars='在再',readings=[('ㄗㄞ',4)])]
        self.assertEqual(positions('我再吃飯',lex,native), positions('我再吃飯',lex,inventory))

    def test_metrics(self):
        rows = [dict(text='我再吃一碗',context='',positions=[dict(index=1,char='再',group='在再',category='after_subject')])]
        result = evaluation.metrics(rows,['我又再吃一碗'])
        self.assertEqual(result['positions']['all']['accuracy'],1)
        self.assertEqual(result['whole_accuracy'],0)

    def test_context_filter(self):
        with tempfile.TemporaryDirectory(dir=OUT, prefix='selftest-') as temp:
            root = Path(temp)
            (root/'encoded').mkdir()
            (root/'filtered').mkdir()
            vocab = ['<pad>','<bos>','<unk>','<sep>']+list('甲乙丙丁戊己庚辛在我再吃')
            ids = [1]+list(range(4,13))+[1,13,14,15]
            np.asarray(ids,np.uint16).tofile(root/'encoded/t.ids')
            np.save(root/'encoded/t.documents.npy',np.asarray([[0,10],[10,14]],np.int64))
            np.save(root/'encoded/t.examples.npy',np.asarray([[9,10,0],[11,14,10]],np.int64))
            old = filtering.BASE,filtering.OUT
            try:
                filtering.BASE=filtering.OUT=root
                filtering.HELD=Exclusions(['甲乙丙丁戊己庚辛'])
                filtering.VOCAB=vocab
                filtering.GROUPS=[dict(chars='在再')]
                filtering.CHAR_GROUPS={i:['在再'] if c in '在再' else [] for i,c in enumerate(vocab)}
                result=filtering.filter_shard(dict(name='t'))
                self.assertEqual((result['allowed'],result['rejected']),(1,1))
                self.assertEqual(np.load(root/'filtered/t.normal.npy').tolist(),[1])
            finally:
                filtering.BASE,filtering.OUT=old

    def test_balanced_splits_do_not_exhaust_rare_side(self):
        with tempfile.TemporaryDirectory(dir=OUT,prefix='split-selftest-') as temp:
            root = Path(temp)
            rng = random.Random(42)
            alphabet = [chr(n) for n in range(0x4e00,0x9fff) if chr(n) not in '在再']
            rows = []
            for c in '在再':
                for i in range(1000):
                    text = c+''.join(rng.choices(alphabet,k=10))
                    rows.append(dict(text=text,context='',source='taiwan_tongues_train',offset=0,
                         positions=[dict(index=0,char=c,group='在再',category='initial')],judge=dict(accepted=True)))
            (root/'inventory.json').write_text(json.dumps(dict(kept=[dict(chars='在再')])))
            (root/'lexicon.tsv').write_text(''.join('ㄗㄞˋ\t'+c+'\t-1\n' for c in alphabet+list('在再')))
            (root/'judged.jsonl').write_text('\n'.join(map(json.dumps,rows)))
            old = splitting.OUT,splitting.holdouts
            try:
                splitting.OUT,splitting.holdouts=root,Exclusions
                splitting.main()
                stats=json.loads((root/'splits.json').read_text())['sets']
                for split in ['dev_zai','test_zai']:
                    self.assertEqual(stats[split]['characters'],{'在':500,'再':500})
                    self.assertEqual(stats[split]['sources'],{'taiwan_tongues_train':1000})
            finally:
                splitting.OUT,splitting.holdouts=old


if __name__=='__main__':
    unittest.main()
