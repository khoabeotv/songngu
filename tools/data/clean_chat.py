#!/usr/bin/env python3
"""Lọc tin nhắn chat do LLM sinh thành corpus dev: mỗi dòng một tin.

Xoá emoji và tên người gửi đầu dòng ("Linh: ..."). Bỏ: dòng ít hơn 3 từ, lời dẫn của model (kết thúc bằng ':'),
ký tự Telex không gõ được (chữ ngoài Latin/Việt), tin trùng nhau, và tin trùng một dòng trong các file `--exclude`
(bộ test khoá: dev không được chứa câu của test).
usage: tools/data/clean_chat.py out.txt in1.txt [in2.txt ...] [--exclude test1.txt ...]
"""
import re, sys, unicodedata

QUOTES = str.maketrans({'\u2018': "'", '\u2019': "'", '\u201c': '"', '\u201d': '"', '\u2013': '-', '\u2014': '-', '\u00a0': ' '})
MARKS = set('\u0300\u0301\u0303\u0309\u0323\u0302\u0306\u031b')
BULLET = re.compile(r'^\s*(?:[-*\u2022]|\d+[.)])\s+')
SPEAKER = re.compile(r'^[A-ZĐ][^\W\d_]{1,12}(?: [A-ZĐ][^\W\d_]{1,12})?:\s+(?=\S)')
# lời của tác tử sinh dữ liệu, không phải tin nhắn
META = re.compile(r'`|/Users/|^(?:I\'ll|I will|Here (?:are|is)|Written to|Write to)\b|chat messages')


def typable(ch):
    if ch.isascii():
        return ch.isprintable()
    if ch in 'đĐ':
        return True
    base, *marks = unicodedata.normalize('NFD', ch)
    return 'a' <= base.lower() <= 'z' and all(m in MARKS for m in marks)


def clean(line):
    line = unicodedata.normalize('NFC', line.translate(QUOTES)).replace('\u2026', '...').strip()
    # emoji, ký hiệu, variation selector, ZWJ: bỏ ký tự, giữ phần chữ của tin
    line = ''.join(c for c in line if c.isascii() or unicodedata.category(c) not in ('So', 'Sk', 'Cf', 'Mn', 'Cs'))
    line = BULLET.sub('', ' '.join(line.split())).strip().strip('"').strip()
    return SPEAKER.sub('', line)


def key(line):
    return ' '.join(re.findall(r'\w+', line.lower()))


def main():
    args = sys.argv[1:]
    out_path, rest = args[0], args[1:]
    if '--exclude' in rest:
        i = rest.index('--exclude')
        inputs, excludes = rest[:i], rest[i + 1:]
    else:
        inputs, excludes = rest, []
    banned = {key(clean(l)) for p in excludes for l in open(p, encoding='utf-8')}
    seen, kept = set(), []
    drop = {'short': 0, 'preface': 0, 'untypable': 0, 'duplicate': 0, 'in test': 0}
    total = 0
    for p in inputs:
        for raw in open(p, encoding='utf-8'):
            line = clean(raw)
            if not line:
                continue
            total += 1
            k = key(line)
            if len(line.split()) < 3:
                drop['short'] += 1
            elif line.endswith(':') or META.search(line):
                drop['preface'] += 1
            elif not all(typable(c) for c in line):
                drop['untypable'] += 1
            elif k in banned:
                drop['in test'] += 1
            elif k in seen:
                drop['duplicate'] += 1
            else:
                seen.add(k)
                kept.append(line)
    with open(out_path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(kept) + '\n')
    words = sum(len(l.split()) for l in kept)
    print(f'{total} lines in, {len(kept)} kept ({words} words); dropped {drop}', file=sys.stderr)


main()
