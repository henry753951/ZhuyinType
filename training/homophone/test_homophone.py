"""Small regression checks for leakage, alignment, categories and TSV input."""
import importlib.util
from pathlib import Path
import unittest
import tempfile
import numpy as np
import filter as filtering

from common import Exclusions, OUT, ROOT, positions, v3

spec = importlib.util.spec_from_file_location('homophone_eval',Path(__file__).with_name('eval.py'))
evaluation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluation)


class Checks(unittest.TestCase):
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


if __name__=='__main__':
    unittest.main()
