#!/usr/bin/env python3
"""Huấn luyện bộ chọn lại Việt/Anh cho các từ engine còn phân vân, từ mẫu của `songngu ambig`.

Mẫu: nhãn, lựa chọn của engine, ln P(việt)/P(anh) của engine, kiểu hoa, 4 từ trước, hai phương án, 3 từ sau.
Mô hình: nhúng từ (băm FNV-1a vào H ô, để engine Rust tính lại không cần từ điển) -> MLP một lớp ẩn -> khoản hiệu
chỉnh cộng vào ln-ratio của engine. Lúc huấn luyện che ngẫu nhiên 0-3 từ sau (lúc gõ, từ sau chưa có hoặc mới có vài từ).
Đo: số từ sai trước/sau trên các bộ dev, theo số từ sau đã biết và theo vùng phân vân áp dụng.
usage: uv run --with torch --with numpy tools/eval/rerank_train.py --train a.tsv,b.tsv --dev x.tsv,y.tsv [--epochs N] [--save m.pt]
"""
import argparse, math, random, sys, time

import numpy as np
import torch
import torch.nn as nn

H = 1 << 17      # số ô băm
D = 16           # chiều nhúng (--dim)
HID = 64         # (--hid)
LEFT, RIGHT = 4, 3
MASK = '<?>'     # từ sau chưa gõ tới


def fnv1a(s):
    h = 0xcbf29ce484222325
    for b in s.encode('utf-8'):
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


class Buckets:
    """Từ -> ô băm, tính FNV một lần cho mỗi từ khác nhau."""
    def __init__(self):
        self.cache = {}

    def __call__(self, tok):
        b = self.cache.get(tok)
        if b is None:
            b = self.cache[tok] = fnv1a(tok) % H
        return b


def load(paths, bk, band=None, base='engine'):
    """`path*N` lặp file N lần (tăng trọng số chat); `band` chỉ giữ mẫu |ln-ratio| < ln band."""
    rows = []
    for spec in paths:
        p, _, rep = spec.partition('*')
        lines = open(p, encoding='utf-8').read().splitlines() * int(rep or 1)
        for line in lines:
            f = line.rstrip('\n').split('\t')
            if len(f) not in (8, 9):
                continue
            y, pick, lr, case = int(f[0]), int(f[1]), float(f[2]), int(f[3])
            if band and abs(lr) >= math.log(band):
                continue
            left = f[4].lower().split(' ')
            right = f[7].lower().split(' ')
            toks = [bk(t) for t in left] + [bk('v:' + f[5].lower()), bk('e:' + f[6].lower())] + [bk(t) for t in right]
            # cột 9: lựa chọn cuối của bộ gõ (sau lookahead); so với nó thay vì engine nếu có
            fin = int(f[8]) if len(f) == 9 else pick
            if base == 'session':
                pick = fin
            rows.append((y, pick, lr, case, toks, fin))
    n = len(rows)
    y = np.array([r[0] for r in rows], np.float32)
    pick = np.array([r[1] for r in rows], np.int8)
    lr = np.array([r[2] for r in rows], np.float32)
    case = np.array([r[3] for r in rows], np.int64)
    toks = np.array([r[4] for r in rows], np.int64).reshape(n, LEFT + 2 + RIGHT)
    fin = np.array([r[5] for r in rows], np.int64)
    return y, pick, lr, case, toks, fin


class Reranker(nn.Module):
    def __init__(self, drop=0.0):
        super().__init__()
        self.drop = nn.Dropout(drop)
        self.emb = nn.Embedding(H, D)
        nn.init.normal_(self.emb.weight, std=0.05)
        self.case = nn.Embedding(6, 4)
        # lựa chọn của bộ gõ sau lookahead (--stack): 0 Anh, 1 Việt, 2 khác
        self.fin = nn.Embedding(3, 4)
        inp = (LEFT + 2 + RIGHT) * D + 4 + 4 + 2
        self.hid = nn.Linear(inp, HID)
        self.out = nn.Linear(HID, 1)
        nn.init.zeros_(self.out.weight)
        nn.init.zeros_(self.out.bias)
        self.scale = nn.Parameter(torch.tensor(1.0))

    def forward(self, toks, case, lr, fin):
        b = toks.shape[0]
        x = self.drop(self.emb(toks)).reshape(b, -1)
        z = torch.clamp(lr / 10.0, -1.5, 1.5).unsqueeze(1)
        x = torch.cat([x, self.case(case), self.fin(fin), z, z * z], 1)
        delta = self.out(torch.relu(self.hid(x))).squeeze(1)
        return self.scale * lr + delta


def masked(toks, r, mask_b):
    """Chỉ giữ r từ sau đầu tiên."""
    t = toks.clone()
    for k in range(RIGHT):
        hide = r <= k
        t[hide, LEFT + 2 + k] = mask_b
    return t


def evaluate(model, data, mask_b, label, rights=(0, RIGHT), bands=(3, 10, 30, 100), stack=False):
    y, pick, lr, case, toks, fin = data
    fin = torch.from_numpy(fin if stack else np.full(len(y), 2))
    rights = (1, RIGHT) if stack else rights
    yi = y.astype(np.int8)
    out = [f'{label:>16} sai {int((pick != yi).sum()):3d} |']
    model.eval()
    with torch.no_grad():
        for r in rights:
            t = masked(torch.from_numpy(toks), torch.full((len(y),), r), mask_b)
            new = (model(t, torch.from_numpy(case), torch.from_numpy(lr), fin).numpy() >= 0).astype(np.int8)
            cells = []
            for band in bands:
                choice = np.where(np.abs(lr) < math.log(band), new, pick)
                fixed = int(((pick != yi) & (choice == yi)).sum())
                broke = int(((pick == yi) & (choice != yi)).sum())
                cells.append(f'{int((choice != yi).sum()):3d}(+{fixed}/-{broke})')
            out.append(f' r{r}: ' + ' '.join(cells))
    model.train()
    return ''.join(out)


def main():
    global D, HID
    ap = argparse.ArgumentParser()
    ap.add_argument('--train', required=True)
    ap.add_argument('--dev', required=True)
    ap.add_argument('--epochs', type=int, default=3)
    ap.add_argument('--batch', type=int, default=1024)
    ap.add_argument('--lr', type=float, default=2e-3)
    ap.add_argument('--save')
    ap.add_argument('--train-band', type=float)
    ap.add_argument('--base', default='session', help='so với engine hay session (sau lookahead)')
    ap.add_argument('--stack', action='store_true', help='cho mô hình biết lựa chọn của bộ gõ, chỉ áp từ 1 từ sau')
    ap.add_argument('--wd', type=float, default=0.0)
    ap.add_argument('--drop', type=float, default=0.0)
    ap.add_argument('--dim', type=int, default=D)
    ap.add_argument('--hid', type=int, default=HID)
    a = ap.parse_args()
    torch.manual_seed(1)
    random.seed(1)
    bk = Buckets()
    mask_b = bk(MASK)
    t0 = time.time()
    D, HID = a.dim, a.hid
    train = load(a.train.split(','), bk, a.train_band)
    devs = [(p.split('/')[-1], load([p], bk, base=a.base)) for p in a.dev.split(',')]
    print(f'train {len(train[0])} mẫu, {len(bk.cache)} từ khác nhau, nạp {time.time() - t0:.0f}s', flush=True)
    model = Reranker(a.drop)
    opt = torch.optim.AdamW(model.parameters(), lr=a.lr, weight_decay=a.wd)
    y, pick, lr, case, toks, fin = (torch.from_numpy(x) for x in train)
    if not a.stack:
        fin = torch.full_like(fin, 2)
    n = len(y)
    lossf = nn.BCEWithLogitsLoss()
    for ep in range(a.epochs):
        perm = torch.randperm(n)
        tot = 0.0
        for i in range(0, n, a.batch):
            idx = perm[i:i + a.batch]
            r = torch.randint(1 if a.stack else 0, RIGHT + 1, (len(idx),))
            logit = model(masked(toks[idx], r, mask_b), case[idx], lr[idx], fin[idx])
            loss = lossf(logit, y[idx])
            opt.zero_grad()
            loss.backward()
            opt.step()
            tot += loss.item() * len(idx)
        print(f'epoch {ep + 1}: loss {tot / n:.4f}  scale {model.scale.item():.3f}  ({time.time() - t0:.0f}s)   cột: vùng áp dụng 3/10/30/100', flush=True)
        for name, d in devs:
            print(evaluate(model, d, mask_b, name, stack=a.stack), flush=True)
    if a.save:
        torch.save(model.state_dict(), a.save)


main()
