#!/usr/bin/env python3
"""Tài liệu kỹ thuật dịch sang tiếng Việt (Markdown, license mở) -> câu cho corpus kỹ thuật.

Nguồn (clone vào corpus/tech/src/, xem docs/data.md): React (vi.react.dev), Vue (docs-vi), Kubernetes (content/vi),
đều CC BY 4.0. Bỏ front matter, khối code, thẻ HTML/JSX; giữ chữ của link và code trong dòng (người ta gõ cả "useState").
Chỉ giữ câu có tiếng Việt (bản dịch còn sót trang tiếng Anh). Chia theo file: train 80 / dev 10 / test 10.
usage: python3 tools/data/extract_docs.py corpus/tech/src -> corpus/tech/docs_{train,dev,test}.txt
"""
import hashlib, os, re, sys, unicodedata

from extract_fineweb_lib import ROOT, sentences

OUT = os.path.join(ROOT, 'corpus', 'tech')


def bucket(name):
    h = int(hashlib.md5(name.encode()).hexdigest(), 16) % 100
    return 'train' if h < 80 else 'dev' if h < 90 else 'test'


def prose(md):
    md = re.sub(r'\A---\n.*?\n---\n', '', md, flags=re.S)                  # front matter
    md = re.sub(r'^(```|~~~).*?^\1[^\n]*$', '', md, flags=re.S | re.M)     # khối code
    md = re.sub(r'\{\{[<%].*?[%>]\}\}', ' ', md, flags=re.S)               # shortcode Hugo
    md = re.sub(r'\{[#/][^}\n]*\}', ' ', md)                                  # neo tiêu đề {#id} {/id/}
    md = re.sub(r'<!--.*?-->', ' ', md, flags=re.S)
    md = re.sub(r'</?[A-Za-z][^>]*>', ' ', md)                              # thẻ HTML/JSX
    md = re.sub(r'!\[[^\]]*\]\([^)]*\)', ' ', md)                           # ảnh
    md = re.sub(r'\[([^\]]*)\]\([^)]*\)', r'\1', md)                        # link -> chữ
    md = re.sub(r'\[([^\]]*)\]\[[^\]]*\]', r'\1', md)
    md = re.sub(r'`([^`\n]*)`', r'\1', md)
    md = re.sub(r'(\*\*|__|\*|_)(\S[^*_\n]*?)\1', r'\2', md)
    out = []
    for line in md.split('\n'):
        line = line.strip()
        if not line or line.startswith(('|', 'import ', 'export ', '{', '}')):
            out.append('')
            continue
        out.append(re.sub(r'^(#+|>|[-*+]|\d+\.)\s+', '', line))
    # đoạn văn Markdown xuống dòng giữa câu: nối lại theo đoạn
    return '\n'.join(' '.join(p.split('\n')) for p in '\n'.join(out).split('\n\n'))


def vietnamese(ws):
    return sum(not w.isascii() for w in ws) >= 2


def main():
    src = sys.argv[1]
    os.makedirs(OUT, exist_ok=True)
    files = {b: open(os.path.join(OUT, f'docs_{b}.txt'), 'w', encoding='utf-8') for b in ('train', 'dev', 'test')}
    words = {b: 0 for b in files}
    seen = set()
    for dirpath, _, names in os.walk(src):
        if '/.git' in dirpath:
            continue
        for n in sorted(names):
            if not n.endswith(('.md', '.mdx')):
                continue
            path = os.path.join(dirpath, n)
            b = bucket(os.path.relpath(path, src))
            text = unicodedata.normalize('NFC', open(path, encoding='utf-8', errors='ignore').read())
            for s, ws in sentences(prose(text)):
                if vietnamese(ws) and s not in seen:
                    seen.add(s)
                    files[b].write(s + '\n')
                    words[b] += len(ws)
    print(' '.join(f'{b} {n} words' for b, n in words.items()))


main()
