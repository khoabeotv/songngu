#!/usr/bin/env python3
"""Gộp corpus kỹ thuật: tài liệu dịch (docs_*) + trang kỹ thuật FineWeb (web_*) -> corpus/tech/{train,dev,test}.txt.

- train: bỏ câu nằm trong bất kỳ bộ dev/test nào (chat, FineWeb, kỹ thuật).
- dev/test: toàn bộ câu tài liệu của bộ đó + câu web ngẫu nhiên cho đủ EVAL_SENTS; bỏ câu trùng phần học nào đó.
usage: python3 tools/data/build_tech.py
"""
import os, random

from extract_fineweb_lib import ROOT

C = os.path.join(ROOT, 'corpus')
EVAL_SENTS = 3000


def lines(*paths):
    out = []
    for p in paths:
        if os.path.exists(os.path.join(C, p)):
            out += [l.rstrip('\n') for l in open(os.path.join(C, p), encoding='utf-8') if l.strip()]
    return out


def write(name, ls):
    open(os.path.join(C, 'tech', name), 'w', encoding='utf-8').write('\n'.join(ls) + '\n')
    print(f'{name}: {len(ls)} sentences, {sum(len(l.split()) for l in ls)} words')


rng = random.Random(11)
evals = {}
for b in ('dev', 'test'):
    docs, web = lines(f'tech/docs_{b}.txt'), lines(f'tech/web_{b}.txt')
    rng.shuffle(web)
    evals[b] = docs + web[:max(0, EVAL_SENTS - len(docs))]
other_eval = set(lines('chatgen/dev_a.txt', 'chatgen/dev_b.txt', 'chatgen/test.txt', 'fineweb/dev.txt', 'fineweb/test.txt'))
banned = other_eval | set(evals['dev']) | set(evals['test'])
train = [l for l in lines('tech/docs_train.txt', 'tech/web_train.txt') if l not in banned]
learned = set(train) | set(lines('fineweb/train.txt', 'fineweb/train_big.txt', 'chatgen/own.txt'))
write('train.txt', train)
for b in ('dev', 'test'):
    ls = [l for l in evals[b] if l not in learned]
    rng.shuffle(ls)
    write(f'{b}.txt', ls)
