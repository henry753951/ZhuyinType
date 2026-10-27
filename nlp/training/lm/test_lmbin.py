"""Small normalized fixture: UTF-8 ids, pruning, fallback, corruption rejection."""
import math
from pathlib import Path
import tempfile
import unittest
import lmbin

class BinaryTest(unittest.TestCase):
    def test_unigram_fallback_in_order_two(self):
        with tempfile.TemporaryDirectory() as d:
            arpa=Path(d)/'empty.arpa'; binary=Path(d)/'lm.bin'
            arpa.write_text('\\data\\\nngram 1=3\nngram 2=0\n\n\\1-grams:\n-0.301029995664\t</s>\t0\n-30\t<s>\t0\n-0.301029995664\t<unk>\t0\n\n\\2-grams:\n\n\\end\\\n')
            result=lmbin.write(arpa,binary)
            self.assertEqual(result['bigrams'],0)
            self.assertEqual(result['order'],2)
            self.assertAlmostEqual(math.exp(lmbin.Model(binary).score('new',['<s>'])),.5,places=6)
            with self.assertRaises(ValueError): lmbin.write(arpa,binary,max_trigrams=-1)

    def test_roundtrip_prune_and_corruption(self):
        with tempfile.TemporaryDirectory() as d:
            arpa=Path(d)/'tiny.arpa'; binary=Path(d)/'lm.bin'
            # P(甲)=.4, P(乙)=.3; P(乙|甲)=.6; P(乙|甲甲)=.8.
            uni=[('</s>',.2),('<s>',1e-30),('<unk>',.1),('乙',.3),('甲',.4)]
            rows=['\\data\\','ngram 1=5','ngram 2=2','ngram 3=1','', '\\1-grams:']
            rows += [f'{math.log10(p)}\t{w}\t0' for w,p in uni]
            rows += ['', '\\2-grams:',f'{math.log10(.2)}\t甲 甲\t0',f'{math.log10(.6)}\t甲 乙\t0',
                     '', '\\3-grams:', f'{math.log10(.8)}\t甲 甲 乙', '', '\\end\\']
            arpa.write_text('\n'.join(rows)+'\n')
            result=lmbin.write(arpa,binary)
            self.assertEqual(result['trigrams'],1)
            m=lmbin.Model(binary)
            self.assertAlmostEqual(math.exp(m.score('乙',['甲','甲'])),.8,places=6)
            self.assertAlmostEqual(math.exp(m.score('乙',['甲'])),.6,places=6)
            self.assertAlmostEqual(math.exp(m.score('新詞',[])),.1,places=6)
            lmbin.write(arpa,binary,max_trigrams=0)
            self.assertAlmostEqual(math.exp(lmbin.Model(binary).score('乙',['甲','甲'])),.6,places=6)
            data=bytearray(binary.read_bytes()); data[0]=0; binary.write_bytes(data)
            with self.assertRaises(ValueError): lmbin.verify(binary)

if __name__=='__main__': unittest.main()
