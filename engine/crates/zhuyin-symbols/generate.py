"""Generates data/symbols.tsv, the symbol and emoji catalog of the 符號與 Emoji 面板.

Sources (Unicode License v3, see resources/sources.json `cldr`):
- Unicode CLDR 46 annotations and derived annotations (cldr-json 46.0.0) for zh-Hant, zh-Hant-HK and en;
- Unicode emoji-test.txt 15.1 (the emoji, their order and their groups).
Zhuyin readings of the Chinese keywords come from the app's lexicon (data/build/lexicon.tsv, `tools/zt data fetch`).

Usage (files downloaded into an empty directory first):
    python3 -I engine/crates/zhuyin-symbols/generate.py SOURCE_DIR data/build/lexicon.tsv

SOURCE_DIR holds annotations-{zh-Hant,zh-Hant-HK,en}.json (cldr-annotations-full),
derived-{zh-Hant,zh-Hant-HK,en}.json (cldr-annotations-derived-full) and emoji-test.txt.

Output lines: `#category<TAB>id<TAB>title`, then one symbol per line:
`text<TAB>name<TAB>zh keywords (|)<TAB>toneless readings (| per keyword, syllables by space)<TAB>en keywords (|)`.
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

OUTPUT = Path(__file__).resolve().parent / "data" / "symbols.tsv"
TONES = "ˊˇˋ˙"

SYMBOL_CATEGORIES: list[tuple[str, str, str]] = [
    ("punctuation", "常用標點", "，、。．？！；：「」『』（）…—～·《》〈〉＿／＼＂＇＃＆＊＠％"),
    ("brackets", "括號", "「」『』（）〔〕【】《》〈〉｛｝［］〖〗〘〙〚〛﹁﹂﹃﹄"),
    ("math", "數學", "＋－×÷＝≠≒≈＜＞≦≧±∓√∞∫∮∑∏∂∇∠⊥∥∵∴％‰°′″∈∉∋∩∪⊂⊃⊆⊇∀∃¬∧∨⊕⊗½⅓¼¾⅔¹²³ⁿ"),
    ("arrows", "箭頭", "←↑→↓↖↗↘↙↔↕⇐⇑⇒⇓⇔⇕↺↻⇄⇅➔➜➡⬅⬆⬇↩↪"),
    ("units", "單位", "℃℉㎎㎏㎜㎝㎞㎡㏄㎖㎗㎘㏕㎠㎢㎥㎦㎧㎨Å＄￥€￡₩₿¢元"),
    ("bopomofo", "注音符號", "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦˉˊˇˋ˙"),
    ("greek", "希臘字母", "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρστυφχψω"),
    ("box", "框線", "─│┌┐└┘├┤┬┴┼━┃┏┓┗┛┣┫┳┻╋═║╔╗╚╝╠╣╦╩╬╭╮╯╰▀▄█▌▐░▒▓■□▪▫▲△▼▽◆◇○●◎◐◑★☆"),
]

EMOJI_GROUPS: dict[str, tuple[str, str]] = {
    "Smileys & Emotion": ("smileys", "笑臉與情緒"),
    "People & Body": ("people", "人物"),
    "Animals & Nature": ("animals", "動物與自然"),
    "Food & Drink": ("food", "食物與飲料"),
    "Travel & Places": ("travel", "旅遊與地點"),
    "Activities": ("activities", "活動"),
    "Objects": ("objects", "物品"),
    "Symbols": ("symbols", "符號"),
    "Flags": ("flags", "旗幟"),
}
# Chinese names for the symbols CLDR does not annotate in zh-Hant.
NAMES: dict[str, str] = {
    "，": "逗號", "、": "頓號", "。": "句號", "．": "間隔號", "？": "問號", "！": "驚嘆號", "；": "分號", "：": "冒號",
    "「": "上引號", "」": "下引號", "『": "上雙引號", "』": "下雙引號", "（": "左括號", "）": "右括號",
    "…": "刪節號", "—": "破折號", "～": "波浪號", "·": "間隔號", "《": "書名號", "》": "書名號", "〈": "篇名號",
    "〉": "篇名號", "＿": "底線", "／": "斜線", "＼": "反斜線", "＂": "雙引號", "＇": "單引號", "＃": "井字號",
    "＆": "和", "＊": "星號", "＠": "小老鼠", "％": "百分比", "〔": "左六角括號", "〕": "右六角括號",
    "【": "左粗括號", "】": "右粗括號", "｛": "左大括號", "｝": "右大括號", "［": "左中括號", "］": "右中括號",
    "＋": "加號", "－": "減號", "×": "乘號", "÷": "除號", "＝": "等號", "≠": "不等於", "≒": "約等於",
    "≈": "約等於", "＜": "小於", "＞": "大於", "≦": "小於等於", "≧": "大於等於", "±": "正負號", "√": "根號",
    "∞": "無限大", "∫": "積分", "∑": "總和", "∠": "角", "⊥": "垂直", "∵": "因為", "∴": "所以", "‰": "千分比",
    "°": "度", "℃": "攝氏", "℉": "華氏", "㎎": "毫克", "㏄": "西西", "㎏": "公斤", "㎜": "毫米",
    "㎝": "公分", "㎞": "公里", "㎡": "平方公尺", "＄": "錢", "￥": "日圓", "€": "歐元", "￡": "英鎊", "元": "元",
}
SKIN_TONES = {chr(c) for c in range(0x1F3FB, 0x1F400)}

Annotation = dict[str, list[str]]


def load_annotations(source: Path, locale: str) -> dict[str, Annotation]:
    """Both CLDR files of a locale, the derived entries filling what the plain file lacks."""
    plain = json.loads((source / f"annotations-{locale}.json").read_text())["annotations"]["annotations"]
    derived = json.loads((source / f"derived-{locale}.json").read_text())["annotationsDerived"]["annotations"]
    merged: dict[str, Annotation] = dict(derived)
    merged.update(plain)
    return merged


def lookup(table: dict[str, Annotation], text: str) -> Annotation | None:
    """CLDR keys mostly drop U+FE0F."""
    return table.get(text) or table.get(text.replace("️", ""))


def load_readings(path: Path) -> dict[str, str]:
    """The most likely toneless reading of every word in the lexicon."""
    best: dict[str, tuple[float, str]] = {}
    for line in path.read_text().splitlines():
        if line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        reading, word, score = parts[0], parts[1], float(parts[2])
        if word not in best or score > best[word][0]:
            best[word] = (score, reading)
    return {word: "".join(c for c in reading if c not in TONES) for word, (_, reading) in best.items()}


def reading_of(word: str, readings: dict[str, str]) -> str:
    """The word's reading, or the readings of its characters; empty when a character has none."""
    if word in readings:
        return readings[word]
    syllables = []
    for char in word:
        if char not in readings:
            return ""
        syllables.append(readings[char])
    return " ".join(syllables)


def clean(words: list[str]) -> list[str]:
    seen: list[str] = []
    for word in words:
        word = word.strip().replace("\t", " ").replace("|", " ")
        if word and word not in seen:
            seen.append(word)
    return seen


def line(text: str, tables: list[dict[str, Annotation]], english: dict[str, Annotation], readings: dict[str, str]) -> str:
    zh: Annotation = {}
    for table in tables:
        found = lookup(table, text)
        if found:
            zh = found
            break
    en = lookup(english, text) or {}
    name = NAMES.get(text) or (zh.get("tts") or [""])[0]
    keywords = clean([name] + zh.get("tts", []) + zh.get("default", []))
    if not name and "\u3105" <= text <= "\u3129" or text in "ˉˊˇˋ˙":
        name = f"注音符號 {text}"
    if not name:
        name = (en.get("tts") or [unicodedata.name(text, "").lower() if len(text) == 1 else ""])[0]
    # "旗子：台灣" names a flag: its parts are keywords too.
    keywords = clean(keywords + [part for word in keywords for part in word.replace("：", " ").split()])
    zhuyin = [reading for reading in (reading_of(word, readings) for word in keywords) if reading]
    english_words = clean(en.get("tts", []) + en.get("default", []))
    return "\t".join([text, name, "|".join(keywords), "|".join(dict.fromkeys(zhuyin)), "|".join(english_words).lower()])


def emoji_groups(path: Path) -> list[tuple[str, list[str]]]:
    groups: list[tuple[str, list[str]]] = []
    for raw in path.read_text().splitlines():
        if raw.startswith("# group:"):
            groups.append((raw.split(":", 1)[1].strip(), []))
            continue
        if raw.startswith("#") or ";" not in raw or "fully-qualified" not in raw.split("#")[0]:
            continue
        text = "".join(chr(int(code, 16)) for code in raw.split(";")[0].split())
        if SKIN_TONES & set(text) or not groups:
            continue
        groups[-1][1].append(text)
    return groups


def main() -> None:
    source, lexicon = Path(sys.argv[1]), Path(sys.argv[2])
    tables = [load_annotations(source, "zh-Hant"), load_annotations(source, "zh-Hant-HK")]
    english = load_annotations(source, "en")
    readings = load_readings(lexicon)
    out = [
        "## Generated by engine/crates/zhuyin-symbols/generate.py from Unicode CLDR 46 annotations (zh-Hant,",
        "## zh-Hant-HK, en) and emoji-test.txt 15.1, Unicode License v3; readings from the ZhuyinType lexicon.",
    ]
    for category, title, symbols in SYMBOL_CATEGORIES:
        out.append(f"#category\t{category}\t{title}")
        out.extend(line(symbol, tables, english, readings) for symbol in dict.fromkeys(symbols))
    for group, texts in emoji_groups(source / "emoji-test.txt"):
        if group not in EMOJI_GROUPS:
            continue
        category, title = EMOJI_GROUPS[group]
        out.append(f"#category\t{category}\t{title}")
        out.extend(line(text, tables, english, readings) for text in texts)
    OUTPUT.parent.mkdir(exist_ok=True)
    OUTPUT.write_text("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
