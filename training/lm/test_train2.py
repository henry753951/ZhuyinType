"""Token export retains sentence boundaries, compatibility and 台 preference."""
from collections import Counter
from pathlib import Path
import tempfile
import unittest
import numpy as np
import pipeline
import train2

class TokenExportTest(unittest.TestCase):
    def test_snapshot_without_phase_marker(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d); corpus=root/'data/build/corpus'
            corpus.mkdir(parents=True); (root/'data/build/lm').mkdir()
            (root/'work/sentences').mkdir(parents=True); (root/'work2').mkdir()
            for name in ['test_colloquial.txt','dev_colloquial.txt','dev_wikipedia.txt']:
                (corpus/name).write_text('保留這個完整測試句子\n')
            (root/'data/build/lm/test_sentences.txt').write_text('另外測試句子\n')
            (corpus/'train_commonvoice.txt').write_text('明天一起出門\n')
            (corpus/'synthetic_phase1.txt').write_text('不該使用這句\n')
            snapshot=corpus/'synthetic_snapshot1.txt'
            snapshot.write_text('今天心情很好\n保留這個完整測試句子\n明天一起出門\n')
            with self.assertRaisesRegex(ValueError,'phase one'):
                train2.prepare(root,2)
            info=train2.prepare(root,2,snapshot)
            self.assertEqual(info[snapshot.name],1)
            self.assertEqual(info[snapshot.name+'_input_sentences'],3)
            self.assertEqual(info[snapshot.name+'_removed'],1)
            self.assertEqual(info['colloquial_sentences'],2)
            actual=''.join(p.read_text() for p in (root/'work2/colloq').glob('*.txt'))
            self.assertNotIn('不該使用',actual)
            self.assertIn('今天心情很好',actual)

    def test_tokens_and_preference(self):
        words=['</s>','<s>','<unk>',' ','!','台灣','我','臺灣']
        ids={w:i for i,w in enumerate(words)}; lexicon={'台灣':.1,'臺灣':.1,'我':.2}
        train2.STATE={'words':words,'ids':ids,'lexicon':lexicon,'trie':pipeline.make_trie(words,lexicon),
                      'scores':np.array([-20,-20,-20,-20,-20,-2,-1,-2.]),'unknown':-20}
        with tempfile.TemporaryDirectory() as d:
            source=Path(d)/'input'; target=Path(d)/'output'
            source.write_text('我臺灣!\n'*10)
            counts,n=train2.worker((str(source),str(target),True))
            lines=target.read_text().splitlines()
            self.assertEqual(n,10)
            self.assertEqual(lines[0],'我 臺灣 �')
            self.assertEqual(lines[1:],["我 台灣 �"]*9)
            self.assertEqual(int(counts[ids['臺灣']]),10)
            self.assertEqual(int(counts.sum()),30)

if __name__=='__main__': unittest.main()
