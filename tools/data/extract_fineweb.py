#!/usr/bin/env python3
"""FineWeb-2 (vie_Latn) -> câu văn đời thường cho ngữ cảnh: tin tức, blog, diễn đàn, rao vặt.

Giữ mọi câu có chen từ tiếng Anh, thêm câu thuần Việt cho đủ ~TARGET từ. Chia theo md5(url): train 90 / dev 5 / test 5;
dev/test lấy nửa câu chen tiếng Anh, nửa câu ngẫu nhiên.
usage: uv run --with pyarrow tools/data/extract_fineweb.py corpus/fineweb/vie_test.parquet
"""
import hashlib, os, random, sys, unicodedata

import pyarrow.parquet as pq

from extract_fineweb_lib import ROOT, WORD, en_only, sentences

OUT = os.path.join(ROOT, 'corpus', 'fineweb')
TARGET = 6_000_000            # số từ train
EVAL_SENTS = 3000             # mỗi bộ dev/test


def bucket(url):
    h = int(hashlib.md5(url.encode()).hexdigest(), 16) % 100
    return 'train' if h < 90 else 'dev' if h < 95 else 'test'


rng = random.Random(5)
mixed = {'train': [], 'dev': [], 'test': []}
plain = {'train': [], 'dev': [], 'test': []}
seen = set()
pf = pq.ParquetFile(sys.argv[1])
for g in range(pf.metadata.num_row_groups):
    t = pf.read_row_group(g, columns=['text', 'url'])
    for text, url in zip(t.column('text').to_pylist(), t.column('url').to_pylist()):
        b = bucket(url)
        for s, ws in sentences(unicodedata.normalize('NFC', text)):
            h = hash(s)
            if h in seen:
                continue
            seen.add(h)
            (mixed if any(w.lower() in en_only for w in ws) else plain)[b].append((s, len(ws)))

os.makedirs(OUT, exist_ok=True)
mixed_words = sum(n for _, n in mixed['train'])
rng.shuffle(plain['train'])
train, total = [s for s, _ in mixed['train']], mixed_words
for s, n in plain['train']:
    if total >= TARGET:
        break
    train.append(s)
    total += n
rng.shuffle(train)
open(os.path.join(OUT, 'train.txt'), 'w', encoding='utf-8').write('\n'.join(train) + '\n')
print(f'train: {len(train)} sentences, {total} words ({mixed_words} in {len(mixed["train"])} mixed sentences)')
for b in ('dev', 'test'):
    rng.shuffle(mixed[b])
    rng.shuffle(plain[b])
    pick = [s for s, _ in mixed[b][:EVAL_SENTS // 2]] + [s for s, _ in plain[b][:EVAL_SENTS // 2]]
    rng.shuffle(pick)
    open(os.path.join(OUT, b + '.txt'), 'w', encoding='utf-8').write('\n'.join(pick) + '\n')
    print(f'{b}: {len(pick)} sentences, {sum(len(WORD.findall(s)) for s in pick)} words (pool: {len(mixed[b])} mixed, {len(plain[b])} plain)')
