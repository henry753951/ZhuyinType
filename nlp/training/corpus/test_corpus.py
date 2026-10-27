import unittest
from opencc import OpenCC
from clean import Deduper,normalized
from prepare import exclusion
from generate import TOPICS,prompt

class CorpusTest(unittest.TestCase):
    def test_cleaning_and_leakage(self):
        cc=OpenCC('s2t'); tw=OpenCC('s2twp')
        self.assertEqual(normalized('1. 等一下要不要去買便當？',cc,tw)[0],'等一下要不要去買便當？')
        self.assertEqual(normalized('这个软件真的很好用',cc,tw)[1],'simplified')
        self.assertEqual(normalized('這個視頻真的很好看',cc,tw)[1],'mainland')
        blocked=exclusion(['我們明天一起去吃牛肉麵吧','四字短句'])
        self.assertTrue(blocked('明天一起去吃牛肉麵吧，如何？'))
        self.assertTrue(blocked('四字短句'))
        self.assertFalse(blocked('晚上來我家打電動嗎'))
    def test_dedup_and_grid(self):
        d=Deduper(); self.assertTrue(d.add('今天晚上要不要去吃火鍋'))
        self.assertFalse(d.add('今天晚上要不要去吃火鍋'))
        self.assertTrue(d.add('這件事情我需要想一下'))
        self.assertEqual(len(TOPICS),200)
        self.assertNotEqual(prompt(0),prompt(200))

if __name__=='__main__': unittest.main()
