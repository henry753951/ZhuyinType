#!/bin/sh
# Download the open data the basic lexicon is built from into data/raw/.
set -eu
cd "$(dirname "$0")/../../../data"
mkdir -p raw
cd raw
curl -fsSLo Unihan.zip https://www.unicode.org/Public/UCD/latest/ucd/Unihan.zip
unzip -oq Unihan.zip
curl -fsSLo cedict.txt.gz https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.txt.gz
gunzip -f cedict.txt.gz
