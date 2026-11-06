#!/usr/bin/env python3
"""Shard train FineWeb-2 (vie_Latn) -> trang kỹ thuật/công việc cho corpus kỹ thuật.

Trang kỹ thuật: ít nhất MIN_HITS từ trong data/en_dev.txt (thuật ngữ IT/công việc) và chiếm ít nhất MIN_DENSITY số từ.
Bỏ trang của viblo.asia (nội dung không có license mở). Chia theo md5(url): train 90 / dev 5 / test 5. Câu nằm trong
bộ đo khác không vào train; câu dev/test trùng phần học khác bị bỏ (xem tools/data/build_tech.py).
usage: uv run --with pyarrow tools/data/extract_fineweb_tech.py corpus/fineweb/vie_train_001.parquet
"""
import hashlib, os, sys, unicodedata
from collections import Counter
from urllib.parse import urlparse

import pyarrow.parquet as pq

import extract_fineweb_lib as lib

OUT = os.path.join(lib.ROOT, 'corpus', 'tech')
MIN_WORDS, MIN_HITS, MIN_DENSITY = 100, 10, 0.015
SKIP_HOSTS = ('viblo.asia',)
TECH = {w for w in lib.words_of(os.path.join(lib.ROOT, 'data', 'en_dev.txt')) if w not in lib.vi_ascii}


def bucket(url):
    h = int(hashlib.md5(url.encode()).hexdigest(), 16) % 100
    return 'train' if h < 90 else 'dev' if h < 95 else 'test'


def main():
    os.makedirs(OUT, exist_ok=True)
    files = {b: open(os.path.join(OUT, f'web_{b}.txt'), 'w', encoding='utf-8') for b in ('train', 'dev', 'test')}
    words = Counter()
    docs = Counter()
    skipped = 0
    seen = set()
    for src in sys.argv[1:]:
        pf = pq.ParquetFile(src)
        for g in range(pf.metadata.num_row_groups):
            t = pf.read_row_group(g, columns=['text', 'url'])
            for text, url in zip(t.column('text').to_pylist(), t.column('url').to_pylist()):
                host = urlparse(url).hostname or ''
                ws = lib.WORD.findall(text)
                hits = sum(w.lower() in TECH for w in ws)
                if len(ws) < MIN_WORDS or hits < MIN_HITS or hits < MIN_DENSITY * len(ws):
                    continue
                if any(host == h or host.endswith('.' + h) for h in SKIP_HOSTS):
                    skipped += 1
                    continue
                b = bucket(url)
                docs[b] += 1
                for s, sw in lib.sentences(unicodedata.normalize('NFC', text)):
                    h = hash(s)
                    if h in seen:
                        continue
                    seen.add(h)
                    files[b].write(s + '\n')
                    words[b] += len(sw)
            if g % 20 == 0:
                print(f'{os.path.basename(src)} row group {g}/{pf.metadata.num_row_groups}: docs {dict(docs)} words {dict(words)}', flush=True)
    print(f'done: docs {dict(docs)} words {dict(words)}, skipped {skipped} docs from {SKIP_HOSTS}')


main()
