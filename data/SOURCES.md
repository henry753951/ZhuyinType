# 資料來源

| 資料 | 用途 | 授權 |
|---|---|---|
| [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict) | 詞、正體字形、讀音（有標注時優先採用台灣讀音） | CC BY-SA 4.0 |
| [Unicode Unihan](https://www.unicode.org/charts/unihan.html) | Big5 字元與讀音 | Unicode License v3 |
| [wordfreq](https://github.com/rspeer/wordfreq) | 詞頻 | 資料 CC BY-SA 4.0 |

由這些資料產生的詞庫（`data/build/lexicon.tsv`）依 CC BY-SA 4.0 釋出。程式碼依 AGPL-3.0-only 授權。

重新產生：

```sh
data/tools/fetch.sh
python3 -m venv .venv && .venv/bin/pip install wordfreq
.venv/bin/python data/tools/build_lexicon.py
```
