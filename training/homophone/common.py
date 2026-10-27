"""Shared paths, exclusion checks and position annotations (no engine rules)."""
import hashlib
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'data/build/homophone'
BASE = ROOT / 'data/build/v3'
HELD = ['data/build/lm/test_sentences.txt', 'data/build/corpus/test_colloquial.txt',
        'data/build/corpus/dev_colloquial.txt', 'data/build/corpus/dev_wikipedia.txt',
        'data/build/lm_chat/dev_chat_tt.txt', 'data/build/lm_chat/test_chat_tt.txt']
SEEDS = ['在再', '做作', '已以', '他她它', '像向象項', '須需', '帳賬', '份分',
         '的得', '吧巴', '嗎嘛', '那哪', '到道', '是事', '時實', '試式', '受授',
         '權全', '利力', '治制', '記紀', '竟競', '帶戴', '副付', '坐座', '候後', '里裡']
SMOKE = '我再一次跟你說,你在哪裡,我在家,明天再說吧,他在吃飯,我們下次再來,你在幹嘛,不要再這樣了,等一下再打給你,我現在在忙,再試試看,你再說一次,吃完再走,再等我五分鐘,他不在公司,在啦幹,我再吃一碗,我在吃一碗麵'.split(',')
CATEGORIES = ['fixed', 'after_subject', 'initial', 'before_verb', 'before_place_time', 'other']
VERBS = set('吃喝說講做作試等看想走來去買賣打玩睡洗讀寫問聽跑找用開關學幫給拿放住唱跳工作忙')
PLACES = set('家校店室院國臺台北南東西市鄉鎮村公司學校這那哪裡此上下午夜年月日時天路')


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + '.tmp')
    temp.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
    temp.replace(path)


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda: f.read(8 << 20), b''):
            h.update(block)
    return h.hexdigest()


class Exclusions:
    def __init__(self, texts=()):
        self.exact, self.grams, self.short = set(), set(), set()
        for text in texts:
            self.add(text)

    def add(self, text):
        if text:
            self.exact.add(text)
            if len(text) < 8:
                self.short.add(text)
            self.grams.update(text[i:i+8] for i in range(len(text)-7))

    def __contains__(self, text):
        if hasattr(self, 'machine'):
            return text in self.exact or next(self.machine.iter(text), None) is not None
        return text in self.exact or any(s in text for s in self.short) or any(text[i:i+8] in self.grams for i in range(len(text)-7))

    def compile(self):
        import ahocorasick
        self.machine = ahocorasick.Automaton()
        for gram in self.grams | {s for s in self.exact if len(s) < 8}:
            self.machine.add_word(gram, True)
        self.machine.make_automaton()
        return self


def holdouts(new=False):
    paths = [OUT / 'inputs' / Path(p).name for p in HELD]
    if new:
        paths += list(OUT.glob('dev_*.txt')) + list(OUT.glob('test_*.txt')) + [OUT / 'smoke.txt']
    texts = list(SMOKE)
    for path in paths:
        for line in path.read_text().splitlines():
            fields = line.split('\t')
            texts.append(fields[-1])
            # A short context such as a quote mark is not an evaluation sentence.
            texts.extend(c for c in fields[:-1] if len(c) >= 8)
            if len(fields)>1:
                texts.append(''.join(fields))
    return Exclusions(texts)


def v3():
    # Existing v3 uses flat imports; load it without rewriting or bytecode writes.
    sys.path.insert(0, str(ROOT / 'training/v3'))
    import importlib.util
    spec = importlib.util.spec_from_file_location('common', ROOT / 'training/v3/common.py')
    module = importlib.util.module_from_spec(spec)
    sys.modules['common'] = module
    spec.loader.exec_module(module)


def positions(text, lex, inventory):
    spans = list(lex.segments(text))
    result = []
    for group in inventory:
        readings = {tuple(r) for r in group['readings']}
        for i, c in enumerate(text):
            if c not in group['chars']:
                continue
            span = next(s for s in spans if s[0] <= i < s[1])
            reading = span[3][i-span[0]]
            if tuple(reading) not in readings:
                continue
            fixed = span[1]-span[0] > 1
            subject = i > 0 and (text[i-1] in '我你妳他她它們人誰爸媽哥姐弟妹' or
                      any(text[:i].endswith(s) for s in ['政府','公司','學校','學生','老師','民眾','大家','孩子','父母','居民','警方','球員','團隊']))
            flags = dict(fixed=fixed, after_subject=subject,
                         initial=i == 0, before_verb=i+1 < len(text) and text[i+1] in VERBS,
                         before_place_time=i+1 < len(text) and text[i+1] in PLACES)
            category = next((key for key in CATEGORIES if flags.get(key)), 'other')
            result.append(dict(index=i, char=c, group=group['chars'], reading=list(reading),
                               category=category, flags=flags))
    return sorted(result, key=lambda p: (p['index'], p['group']))


def jsonl(path):
    with Path(path).open() as f:
        for line in f:
            yield json.loads(line)
