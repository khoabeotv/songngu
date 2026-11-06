"""Bộ lọc câu dùng chung cho tools/data/extract_fineweb*.py."""
import os, re, unicodedata

ROOT = os.path.join(os.path.dirname(__file__), '..', '..')
WORD = re.compile(r'[^\W\d_]+')


def words_of(path, col=0):
    return {l.split()[col].lower() for l in open(path, encoding='utf-8') if l.strip() and not l.startswith('#')}


en = words_of(os.path.join(ROOT, 'data', 'en_words.txt')) | words_of(os.path.join(ROOT, 'data', 'en_dev.txt'))
vi = {unicodedata.normalize('NFC', w) for f in ('vi_50k.txt', 'vi_extra.txt') for w in words_of(os.path.join(ROOT, 'data', f))}
vi_ascii = {w for w in vi if w.isascii()}
# Từ chỉ có thể là tiếng Anh: câu chứa nó là câu chen tiếng Anh.
en_only = {w for w in en if w not in vi_ascii and len(w) >= 2}


def sentences(text):
    # Chỉ nhận từ có dấu trong danh sách tần suất: mở rộng ra mọi âm tiết đúng luật (để có Bình Thạnh, bún riêu)
    # đã thử và kém hơn trên dev chat; âm tiết hiếm do engine lo (`Config::unseen_vn`).
    for line in text.split('\n'):
        line = re.sub(r'https?://\S+|www\.\S+', ' ', line)
        for s in re.split(r'(?<=[.!?])\s+', re.sub(r'\s+', ' ', line).strip()):
            ws = WORD.findall(s)
            # Từ có dấu phải là từ Việt có thật: loại chữ dính do crawl ("LiverpoolKhông"), tiếng Tây Ban Nha, ký tự Cyrillic.
            if any(not w.isascii() and w.lower() not in vi for w in ws):
                continue
            if 4 <= len(ws) <= 60 and sum(map(len, ws)) >= 0.6 * len(s):
                yield s, ws
