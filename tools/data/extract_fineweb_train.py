#!/usr/bin/env python3
"""Shard train lớn của FineWeb-2 (vie_Latn) -> thêm câu học cho ngữ cảnh, đọc dần từng row group.

Lấy câu chen tiếng Anh tới MIXED_CAP từ, câu thuần Việt lấy ngẫu nhiên PLAIN_RATE. Dev/test vẫn lấy từ phần test
của FineWeb (tools/data/extract_fineweb.py), khác tài liệu với shard train.
usage: uv run --with pyarrow tools/data/extract_fineweb_train.py corpus/fineweb/vie_train_000.parquet corpus/fineweb/train_big.txt
"""
import random, sys, unicodedata

import pyarrow.parquet as pq

import extract_fineweb_lib as lib

src, out = sys.argv[1], sys.argv[2]

MIXED_CAP = 10_000_000
PLAIN_RATE = 0.006

rng = random.Random(9)
seen = set()
mixed_words = plain_words = 0
with open(out, 'w', encoding='utf-8') as f:
    pf = pq.ParquetFile(src)
    for g in range(pf.metadata.num_row_groups):
        for text in pf.read_row_group(g, columns=['text']).column('text').to_pylist():
            for s, ws in lib.sentences(unicodedata.normalize('NFC', text)):
                h = hash(s)  # chỉ nhớ câu đã giữ, không nhớ cả shard (hàng trăm triệu câu)
                if h in seen:
                    continue
                if any(w.lower() in lib.en_only for w in ws):
                    if mixed_words >= MIXED_CAP:
                        continue
                    mixed_words += len(ws)
                elif rng.random() < PLAIN_RATE:
                    plain_words += len(ws)
                else:
                    continue
                seen.add(h)
                f.write(s + '\n')
        if g % 20 == 0:
            print(f'row group {g}/{pf.metadata.num_row_groups}: mixed {mixed_words}, plain {plain_words}', flush=True)
print(f'done: mixed {mixed_words} words, plain {plain_words} words -> {out}')
