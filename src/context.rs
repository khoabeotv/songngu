//! Ngữ cảnh học từ văn bản song ngữ thật (tiếng Việt chen tiếng Anh):
//! - tần suất trong miền: từ Anh hay chen trong câu Việt (task, deploy) khác hẳn tần suất phụ đề phim;
//! - xác suất đổi thứ tiếng giữa hai từ liền nhau;
//! - cặp từ (bigram) nội suy Witten-Bell, nên phần này không có tham số cần chỉnh.
//!
//! Tra theo chữ bề mặt (dạng `canon`): hai nhánh của engine so nhau bằng chính chữ sẽ hiện.
//! Bảng cặp từ dạng CSR trên mảng số (xem `store`): nạp bằng mmap, không tốn bộ nhớ riêng của tiến trình.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;

use unicode_normalization::UnicodeNormalization;

use crate::engine::Lang;
use crate::lexicon::{EnLex, VnLex};
use crate::store::{cmp_prefix, Arr, Reader, StrList, Writer};
use crate::syllable::{decompose, strip_mark, Tone};

/// "Từ trước" ở đầu câu và ngay sau số.
pub const START: &str = "<s>";
pub const NUM: &str = "<num>";

/// Khoá tra cứu: chữ thường, dấu thanh tách ra cuối (hoà và hòa là một).
pub fn canon(w: &str) -> String {
    let w: String = w.to_lowercase().nfc().collect();
    match decompose(&w) {
        Some((s, t)) if t != Tone::None => format!("{s}{}", t.idx()),
        Some((s, _)) => s,
        None => w,
    }
}

/// Ký tự ngăn cách đổi "từ trước" thế nào: kết câu -> START, số -> NUM, còn lại (space, phẩy, ngoặc) giữ nguyên.
pub fn after_separator(c: char) -> Option<&'static str> {
    match c {
        '.' | '!' | '?' | '\n' | ';' | ':' => Some(START),
        '0'..='9' => Some(NUM),
        _ => None,
    }
}

pub const CASES: usize = 6;

/// Âm tiết Việt đúng luật nhưng ngoài từ điển: số lần gặp tối thiểu trong corpus để tính là có thật.
const MIN_NEW_VN: f64 = 2.0;

/// Toàn chữ hoa, từ hai chữ trở lên.
pub fn all_caps(word: &str) -> bool {
    word.chars().nth(1).is_some() && word.chars().all(|c| !c.is_lowercase())
}

/// Kiểu chữ hoa của từ: 0 thường, 1 hoa đầu câu, 2 hoa giữa câu (tên riêng); toàn hoa: 3 giữa câu thường (viết
/// tắt: GIF, NIST), 4 sau một từ cũng toàn hoa (tiêu đề), 5 đầu câu. Trong văn bản kỹ thuật, viết tắt và tên riêng
/// giữa câu (Nest, Max) phần lớn là tiếng Anh, còn tiêu đề viết hoa lại phần lớn là tiếng Việt.
pub fn case_class(word: &str, sentence_start: bool, prev_caps: bool) -> usize {
    match word.chars().next() {
        Some(c) if c.is_uppercase() => match (all_caps(word), sentence_start, prev_caps) {
            (true, true, _) => 5,
            (true, false, true) => 4,
            (true, false, false) => 3,
            (false, true, _) => 1,
            (false, false, _) => 2,
        },
        _ => 0,
    }
}

pub const JOINS: usize = 4;

/// Kiểu nối với từ trước qua chuỗi ký tự ngăn cách `run`: 0 cách bằng khoảng trắng (hoặc có số, hoặc không có từ
/// trước); dính liền: 1 bằng `.` (tên file, domain, `obj.field`), 2 bằng `-`, 3 bằng ký tự khác (`my_var`, `a/b`).
/// Dính liền thì `.` không phải hết câu.
pub fn join_class(run: &str, after_word: bool) -> usize {
    if !after_word || run.is_empty() || run.chars().any(|c| c.is_whitespace() || c.is_ascii_digit()) {
        0
    } else if run == "." {
        1
    } else if run == "-" {
        2
    } else {
        3
    }
}

fn lang_idx(l: Lang) -> usize {
    match l {
        Lang::Vn => 0,
        Lang::En => 1,
    }
}

/// Một từ trong bảng cặp từ, nhìn từ engine.
#[derive(Clone, Copy)]
pub struct WordRef<'a> {
    /// Khoá `canon`: âm tiết chưa thanh + số thanh ("trương2"), hoặc từ Anh.
    pub key: &'a str,
    /// Là âm tiết tiếng Việt có thật (theo từ điển chung).
    pub vn: bool,
}

impl WordRef<'_> {
    /// Âm tiết chưa thanh: khoá bỏ số thanh ở cuối.
    pub fn toneless(&self) -> &str {
        self.key.trim_end_matches(|c: char| c.is_ascii_digit())
    }

    pub fn tone(&self) -> Tone {
        let d = self.key.chars().last().and_then(|c| c.to_digit(10));
        d.and_then(|d| Tone::ALL.get(d as usize).copied()).unwrap_or(Tone::None)
    }
}

/// Dạng không dấu của khoá ("trương2" -> "truong"): bảng cặp từ xếp theo dạng này để tiền tố đang gõ là một đoạn
/// liên tiếp.
fn stripped(key: &str) -> impl Iterator<Item = char> + '_ {
    key.chars().filter(|c| !c.is_ascii_digit()).map(strip_mark)
}

/// Thứ tự của từ điển cặp từ: dạng không dấu, rồi đến khoá.
fn vocab_cmp(a: &str, b: &str) -> Ordering {
    stripped(a).cmp(stripped(b)).then_with(|| a.cmp(b))
}

fn vn_valid(key: &str, vn_sub: &VnLex) -> bool {
    let w = WordRef { key, vn: false };
    let s: Vec<char> = w.toneless().chars().collect();
    vn_sub.word_prob(&s, w.tone()).is_some()
}

/// Các từ theo sau một từ: bảng corpus và bộ nhớ riêng (nhỏ, thêm dần) cùng dùng giao diện này.
pub trait Successors {
    fn total(&self) -> f64;
    /// Số từ theo sau khác nhau (kể cả từ đã cắt vì quá hiếm), cho λ Witten-Bell.
    fn types(&self) -> f64;
    fn count(&self, key: &str) -> f64;
    /// Tổng số đếm các từ có dạng không dấu bắt đầu bằng `p` và thoả `keep`.
    fn prefix_sum(&self, p: &str, keep: &dyn Fn(WordRef) -> bool) -> f64;
    /// Như `prefix_sum` khi bộ lọc chỉ là lớp từ (`VN` hoặc `ASCII`): chỉ xem cờ, không đọc chuỗi.
    fn prefix_class_sum(&self, p: &str, class: u8) -> f64;
}

/// Lớp từ (cờ trong bảng cặp từ): âm tiết Việt có thật; khoá toàn ASCII (từ Anh, hoặc chữ Việt không mũ/móc).
pub const VN: u8 = 1;
pub const ASCII: u8 = 2;

fn class_of(key: &str, vn: bool) -> u8 {
    (if vn { VN } else { 0 }) | (if key.is_ascii() { ASCII } else { 0 })
}

pub struct WordInfo {
    pub key: String,
    vn: bool,
}

/// Bảng cặp từ của bộ nhớ riêng: thêm dần từng từ, xếp theo `vocab_cmp`.
#[derive(Default)]
pub struct Follow {
    pub total: f64,
    pub types: f64,
    pub succ: Vec<(WordInfo, f64)>,
}

impl Follow {
    pub fn add(&mut self, key: &str, count: f64, vn_sub: &VnLex) {
        self.total += count;
        match self.succ.binary_search_by(|s| vocab_cmp(&s.0.key, key)) {
            Ok(i) => self.succ[i].1 += count,
            Err(i) => {
                self.succ.insert(i, (WordInfo { key: key.to_string(), vn: vn_valid(key, vn_sub) }, count));
                self.types += 1.0;
            }
        }
    }
}

impl Follow {
    /// Bớt số đếm (người dùng huỷ một lần lật); hết số đếm thì bỏ từ.
    pub fn remove(&mut self, key: &str, count: f64) {
        let Ok(i) = self.succ.binary_search_by(|s| vocab_cmp(&s.0.key, key)) else { return };
        let n = count.min(self.succ[i].1);
        self.succ[i].1 -= n;
        self.total -= n;
        if self.succ[i].1 <= 0.0 {
            self.succ.remove(i);
            self.types -= 1.0;
        }
    }
}

impl Successors for Follow {
    fn total(&self) -> f64 {
        self.total
    }
    fn types(&self) -> f64 {
        self.types
    }
    fn count(&self, key: &str) -> f64 {
        self.succ.binary_search_by(|s| vocab_cmp(&s.0.key, key)).map_or(0.0, |i| self.succ[i].1)
    }
    fn prefix_sum(&self, p: &str, keep: &dyn Fn(WordRef) -> bool) -> f64 {
        let lo = self.succ.partition_point(|s| cmp_prefix(stripped(&s.0.key), p.chars()).is_lt());
        self.succ[lo..]
            .iter()
            .take_while(|s| cmp_prefix(stripped(&s.0.key), p.chars()).is_eq())
            .filter(|s| keep(WordRef { key: &s.0.key, vn: s.0.vn }))
            .map(|s| s.1)
            .sum()
    }
    fn prefix_class_sum(&self, p: &str, class: u8) -> f64 {
        self.prefix_sum(p, &|w| class_of(w.key, w.vn) & class != 0)
    }
}

/// Cặp từ trước khi gọn: từ trước -> (tổng, số loại, [(từ sau, số đếm)]).
type RawPairs = HashMap<String, (f64, f64, Vec<(String, f64)>)>;

/// Cặp từ gặp ít hơn ngần này lần không lưu (vẫn tính vào tổng và số loại): bảng nhỏ đi nhiều, độ chính xác gần như giữ nguyên.
const MIN_PAIR: f64 = 2.0;

/// Bảng cặp từ corpus dạng CSR. Số hiệu từ = vị trí trong `vocab` (xếp theo `vocab_cmp`), nên mọi tiền tố đang gõ
/// là một khoảng số hiệu, và mỗi hàng xếp theo số hiệu thì cộng một tiền tố chỉ là quét một đoạn.
struct Pairs {
    vocab: StrList,
    /// Dạng không dấu của từng từ (cùng thứ tự): tìm khoảng tiền tố bằng so chuỗi thường.
    strip: StrList,
    /// Lớp từ (`VN` | `ASCII`).
    class: Arr<u8>,
    /// Từ theo sau từ i: succ[row[i]..row[i+1]] (len vocab + 1).
    row: Arr<u32>,
    /// Số lần gặp từ i ở vị trí đứng trước, và số từ theo sau khác nhau (kể cả cặp đã cắt).
    total: Arr<f32>,
    types: Arr<f32>,
    succ: Arr<u32>,
    n: Arr<f32>,
    /// next_lang[2i + l]: số lần từ ngay sau từ i là tiếng l (từ có nghĩa ở cả hai tiếng không đếm).
    next_lang: Arr<f32>,
}

/// Từ trước gặp ít hơn nghần này lần thì không thêm vào từ điển cặp từ chỉ để giữ `next_lang`.
const MIN_NEXT_LANG: f64 = 3.0;

impl Pairs {
    fn build(raw: &RawPairs, next_lang: &HashMap<String, [f64; 2]>, vn_lex: &[&VnLex]) -> Pairs {
        let mut keys: Vec<&str> = raw.keys().map(String::as_str).collect();
        keys.extend(raw.values().flat_map(|f| f.2.iter().map(|s| s.0.as_str())));
        keys.extend(next_lang.iter().filter(|(_, c)| c[0] + c[1] >= MIN_NEXT_LANG).map(|(k, _)| k.as_str()));
        keys.sort_unstable_by(|a, b| vocab_cmp(a, b));
        keys.dedup();
        let id: HashMap<&str, u32> = keys.iter().enumerate().map(|(i, k)| (*k, i as u32)).collect();
        let (mut row, mut total, mut types, mut succ, mut n) = (vec![0u32], Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for k in &keys {
            let (t, ty) = match raw.get(*k) {
                Some((t, ty, list)) => {
                    let mut ids: Vec<(u32, f32)> = list.iter().map(|(s, c)| (id[s.as_str()], *c as f32)).collect();
                    ids.sort_unstable_by_key(|x| x.0);
                    succ.extend(ids.iter().map(|x| x.0));
                    n.extend(ids.iter().map(|x| x.1));
                    (*t as f32, *ty as f32)
                }
                None => (0.0, 0.0),
            };
            total.push(t);
            types.push(ty);
            row.push(succ.len() as u32);
        }
        let nl = keys.iter().flat_map(|k| next_lang.get(*k).copied().unwrap_or([0.0; 2]).map(|x| x as f32)).collect();
        Pairs {
            next_lang: Cow::Owned(nl),
            class: Cow::Owned(keys.iter().map(|k| class_of(k, vn_lex.iter().any(|l| vn_valid(k, l)))).collect()),
            strip: StrList::from_strings(&keys.iter().map(|k| stripped(k).collect::<String>()).collect::<Vec<_>>()),
            vocab: StrList::from_strings(&keys),
            row: Cow::Owned(row),
            total: Cow::Owned(total),
            types: Cow::Owned(types),
            succ: Cow::Owned(succ),
            n: Cow::Owned(n),
        }
    }

    fn write(&self, w: &mut Writer) {
        w.strs(&self.vocab);
        w.strs(&self.strip);
        w.arr(&self.class);
        w.arr(&self.row);
        w.arr(&self.total);
        w.arr(&self.types);
        w.arr(&self.succ);
        w.arr(&self.n);
        w.arr(&self.next_lang);
    }

    fn read(r: &mut Reader) -> io::Result<Pairs> {
        let p = Pairs {
            vocab: r.strs()?,
            strip: r.strs()?,
            class: r.arr()?,
            row: r.arr()?,
            total: r.arr()?,
            types: r.arr()?,
            succ: r.arr()?,
            n: r.arr()?,
            next_lang: r.arr()?,
        };
        let v = p.vocab.len();
        let sizes_ok = p.strip.len() == v
            && p.class.len() == v
            && p.row.len() == v + 1
            && p.total.len() == v
            && p.types.len() == v
            && p.next_lang.len() == 2 * v;
        if !sizes_ok || p.n.len() != p.succ.len() || p.row.last().is_some_and(|&e| e as usize > p.succ.len()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad pair table"));
        }
        Ok(p)
    }

    fn find(&self, key: &str) -> Option<u32> {
        let q: String = stripped(key).collect();
        let lo = self.strip.partition(0, self.strip.len(), |s| s < q.as_str());
        let hi = self.strip.partition(lo, self.strip.len(), |s| s <= q.as_str());
        (lo..hi).find(|&i| self.vocab.get(i) == key).map(|i| i as u32)
    }

    fn view(&self, id: u32) -> Option<RowView<'_>> {
        let i = id as usize;
        let (lo, hi) = (*self.row.get(i)? as usize, *self.row.get(i + 1)? as usize);
        (lo < hi && hi <= self.succ.len()).then(|| RowView {
            p: self,
            succ: &self.succ[lo..hi],
            n: &self.n[lo..hi],
            total: self.total[i] as f64,
            types: self.types[i] as f64,
        })
    }

    fn word(&self, id: u32) -> WordRef<'_> {
        WordRef { key: self.vocab.get(id as usize), vn: self.class.get(id as usize).is_some_and(|c| c & VN != 0) }
    }
}

/// Một hàng của bảng cặp từ hoặc bảng ba từ: các từ theo sau một ngữ cảnh, xếp theo số hiệu.
pub struct RowView<'a> {
    p: &'a Pairs,
    succ: &'a [u32],
    n: &'a [f32],
    total: f64,
    types: f64,
}

impl RowView<'_> {
    /// Các phần tử có số hiệu trong khoảng tiền tố không dấu `prefix`.
    fn in_prefix(&self, prefix: &str) -> impl Iterator<Item = (u32, f32)> + '_ {
        let (vlo, vhi) = self.p.strip.prefix_range(prefix);
        let start = self.succ.partition_point(|&x| (x as usize) < vlo);
        self.succ[start..].iter().zip(&self.n[start..]).take_while(move |(&x, _)| (x as usize) < vhi).map(|(&x, &n)| (x, n))
    }
}

impl Successors for RowView<'_> {
    fn total(&self) -> f64 {
        self.total
    }
    fn types(&self) -> f64 {
        self.types
    }
    fn count(&self, key: &str) -> f64 {
        let Some(w) = self.p.find(key) else { return 0.0 };
        self.succ.binary_search(&w).map_or(0.0, |i| self.n[i] as f64)
    }
    fn prefix_sum(&self, prefix: &str, keep: &dyn Fn(WordRef) -> bool) -> f64 {
        self.in_prefix(prefix).filter(|&(x, _)| keep(self.p.word(x))).map(|(_, n)| n as f64).sum()
    }
    fn prefix_class_sum(&self, prefix: &str, class: u8) -> f64 {
        let ok = |x: u32| self.p.class.get(x as usize).is_some_and(|c| c & class != 0);
        self.in_prefix(prefix).filter(|&(x, _)| ok(x)).map(|(_, n)| n as f64).sum()
    }
}

pub struct Context {
    pub vn: VnLex,
    pub en: EnLex,
    /// switch[a][b][c] = P(từ này tiếng c | hai từ trước tiếng a, b); chỉ số 2 = đầu câu. Bậc hai vì thuật ngữ Anh
    /// lẻ trong câu Việt (Việt-Anh -> Việt) khác hẳn câu tiếng Anh (Anh-Anh -> Anh).
    switch: [[[f64; 2]; 3]; 3],
    /// case[k][l] = P(tiếng l | kiểu chữ hoa k) / P(tiếng l). Chia theo tiếng từ trước (như `join`) kém hơn trên dev.
    case: [[f64; 2]; CASES],
    /// join[k][p][l] = P(tiếng l | nối kiểu k với từ trước tiếng p) / P(tiếng l | từ trước tiếng p); p = 2: không rõ.
    /// Theo tiếng từ trước vì `-` sau từ Việt là phiên âm (Ga-li-lê), sau từ Anh là từ ghép Anh (low-fidelity).
    join: [[[f64; 2]; 3]; JOINS],
    /// join_next[k][l] = P(tiếng l | từ sau nối vào nó kiểu k) / P(tiếng l): từ sau xem lại từ trước (Max-SA).
    join_next: [[f64; 2]; JOINS],
    pairs: Pairs,
    /// P(từ Anh kế tiếp là từ chưa gặp), ước lượng Good-Turing: số từ chỉ gặp một lần / tổng số từ Anh.
    pub en_unseen: f64,
}

enum Label {
    Vn,
    En,
    Both,
    Foreign,
}

struct Counter<'a> {
    vn_sub: &'a VnLex,
    en_sub: &'a EnLex,
    vn: HashMap<String, f64>,
    en: HashMap<String, f64>,
    switch: [[[f64; 2]; 3]; 3],
    case: [[f64; 2]; CASES],
    join: [[[f64; 2]; 3]; JOINS],
    join_next: [[f64; 2]; JOINS],
    next_lang: HashMap<String, [f64; 2]>,
    /// Tiếng của từ vừa đếm (None: cả hai, hoặc chưa có), chờ biết kiểu nối của từ sau.
    last_lang: Option<usize>,
    pairs: HashMap<(String, String), f64>,
    prev: String,
    prev_lang: usize,
    prev2_lang: usize,
    prev_caps: bool,
    /// Kiểu nối của từ đang đọc với từ trước.
    pending_join: usize,
}

impl Counter<'_> {
    fn label(&self, w: &str) -> Label {
        let d = decompose(w).map(|(s, t)| (s.chars().collect::<Vec<_>>(), t));
        let is_vn = d.as_ref().is_some_and(|(s, t)| self.vn_sub.word_prob(s, *t).is_some());
        if !w.is_ascii() {
            // âm tiết đúng luật mà danh sách tần suất (phụ đề) thiếu: xịn xò, lổm ngổm, tập tọe
            let valid = is_vn || d.as_ref().is_some_and(|(s, t)| self.vn_sub.is_syllable(s, *t));
            return if valid { Label::Vn } else { Label::Foreign };
        }
        match (is_vn, self.en_sub.word_prob(w).is_some()) {
            (true, true) => Label::Both,
            (true, false) => Label::Vn,
            _ => Label::En,
        }
    }

    fn reset(&mut self, prev: &str) {
        self.prev = prev.into();
        if prev == START {
            self.prev_lang = 2;
            self.prev2_lang = 2;
        }
    }

    /// Kết thúc từ vừa đếm: từ sau nối vào nó kiểu `join` (0 nếu hết dòng).
    fn close(&mut self, join: usize) {
        if let Some(b) = self.last_lang.take() {
            self.join_next[join][b] += 1.0;
        }
    }

    fn word(&mut self, raw: &str, join: usize) {
        self.close(join);
        let prev_caps = std::mem::replace(&mut self.prev_caps, all_caps(raw));
        let w: String = raw.to_lowercase().nfc().collect();
        let lang = match self.label(&w) {
            Label::Foreign => return self.reset(START),
            Label::Vn => {
                *self.vn.entry(w.clone()).or_default() += 1.0;
                Some(0)
            }
            Label::En => {
                *self.en.entry(w.clone()).or_default() += 1.0;
                Some(1)
            }
            Label::Both => {
                *self.vn.entry(w.clone()).or_default() += 1.0;
                *self.en.entry(w.clone()).or_default() += 1.0;
                None
            }
        };
        let key = canon(&w);
        if let Some(b) = lang {
            self.case[case_class(raw, self.prev == START, prev_caps)][b] += 1.0;
            self.join[join][self.prev_lang][b] += 1.0;
        }
        self.last_lang = lang;
        if let Some(b) = lang {
            self.next_lang.entry(self.prev.clone()).or_default()[b] += 1.0;
        }
        *self.pairs.entry((std::mem::take(&mut self.prev), key.clone())).or_default() += 1.0;
        self.prev = key;
        if let Some(b) = lang {
            self.switch[self.prev2_lang][self.prev_lang][b] += 1.0;
            self.prev2_lang = self.prev_lang;
            self.prev_lang = b;
        }
    }
}

impl Context {
    /// Học từ văn bản thô (mỗi dòng một câu/đoạn). `vn_sub`/`en_sub`: từ điển chung, dùng để gán thứ tiếng cho từ.
    pub fn train(text: &str, vn_sub: &VnLex, en_sub: &EnLex) -> Context {
        let mut c = Counter {
            vn_sub,
            en_sub,
            vn: HashMap::new(),
            en: HashMap::new(),
            switch: [[[1.0; 2]; 3]; 3],
            case: [[1.0; 2]; CASES],
            join: [[[1.0; 2]; 3]; JOINS],
            join_next: [[1.0; 2]; JOINS],
            next_lang: HashMap::new(),
            last_lang: None,
            pairs: HashMap::new(),
            prev: START.into(),
            prev_lang: 2,
            prev2_lang: 2,
            prev_caps: false,
            pending_join: 0,
        };
        for line in text.lines() {
            c.close(0);
            c.reset(START);
            c.prev_caps = false;
            let (mut word, mut run) = (String::new(), String::new());
            let mut after_word = false;
            for ch in line.chars().chain(std::iter::once('\n')) {
                if ch.is_alphabetic() {
                    if word.is_empty() {
                        // dính liền thì chuỗi ngăn cách không đổi "từ trước" (Nest.js: trước "js" là "nest")
                        let join = join_class(&run, after_word);
                        if join == 0 {
                            for p in run.chars().filter_map(after_separator) {
                                c.reset(p);
                            }
                        }
                        c.pending_join = join;
                        run.clear();
                    }
                    word.push(ch);
                    continue;
                }
                if !word.is_empty() {
                    c.word(&word, c.pending_join);
                    word.clear();
                    after_word = true;
                }
                run.push(ch);
            }
        }
        let mut raw: RawPairs = HashMap::new();
        for ((p, key), n) in c.pairs {
            let f = raw.entry(p).or_default();
            f.0 += n;
            f.1 += 1.0;
            if n >= MIN_PAIR {
                f.2.push((key, n));
            }
        }
        raw.retain(|_, f| !f.2.is_empty());
        let mut switch = [[[0.0; 2]; 3]; 3];
        for (a, rows) in c.switch.iter().enumerate() {
            for (b, r) in rows.iter().enumerate() {
                switch[a][b] = [r[0] / (r[0] + r[1]), r[1] / (r[0] + r[1])];
            }
        }
        // P(l | hàng) / P(l), P(l) lấy từ `total` của các hàng cùng bảng
        let ratio = |r: [f64; 2], total: [f64; 2]| {
            let all = total[0] + total[1];
            [0, 1].map(|l| (r[l] / (r[0] + r[1])) / (total[l] / all))
        };
        // bảng [kiểu][tiếng từ trước][tiếng]: chia theo từng tiếng từ trước
        fn by_prev<const K: usize>(t: &[[[f64; 2]; 3]; K], ratio: impl Fn([f64; 2], [f64; 2]) -> [f64; 2]) -> [[[f64; 2]; 3]; K] {
            let mut out = [[[0.0; 2]; 3]; K];
            for p in 0..3 {
                let total: [f64; 2] = [0, 1].map(|l| t.iter().map(|k| k[p][l]).sum());
                for k in 0..K {
                    out[k][p] = ratio(t[k][p], total);
                }
            }
            out
        }
        let total: [f64; 2] = [0, 1].map(|l| c.case.iter().map(|r| r[l]).sum());
        let case = c.case.map(|r| ratio(r, total));
        let next_total: [f64; 2] = [0, 1].map(|l| c.join_next.iter().map(|r| r[l]).sum());
        let join_next = c.join_next.map(|r| ratio(r, next_total));
        let join = by_prev(&c.join, ratio);
        let en_tokens: f64 = c.en.values().sum();
        let en_unseen = c.en.values().filter(|&&n| n == 1.0).count() as f64 / en_tokens.max(1.0);
        // Từ Anh lạ chỉ gặp một lần thường là tên riêng hoặc gõ nhầm.
        let mut en = c.en;
        en.retain(|w, n| *n >= 2.0 || en_sub.word_prob(w).is_some());
        // Âm tiết ngoài từ điển chỉ giữ khi gặp đủ nhiều, như từ Anh lạ.
        let mut vn = c.vn;
        vn.retain(|w, n| {
            *n >= MIN_NEW_VN
                || decompose(w).is_some_and(|(s, t)| vn_sub.word_prob(&s.chars().collect::<Vec<_>>(), t).is_some())
        });
        let lines = |m: HashMap<String, f64>| m.into_iter().map(|(w, n)| format!("{w} {n}\n")).collect::<String>();
        let vn = VnLex::build(&lines(vn));
        let pairs = Pairs::build(&raw, &c.next_lang, &[vn_sub, &vn]);
        Context { vn, en: EnLex::build(&lines(en), "", 0.0), switch, case, join, join_next, pairs, en_unseen }
    }

    pub fn write(&self, w: &mut Writer) {
        w.arr(&self.switch.iter().flatten().flatten().copied().collect::<Vec<f64>>());
        w.arr(&self.case.iter().flatten().copied().collect::<Vec<f64>>());
        w.arr(&self.join.iter().flatten().flatten().copied().collect::<Vec<f64>>());
        w.arr(&self.join_next.iter().flatten().copied().collect::<Vec<f64>>());
        w.f64(self.en_unseen);
        self.vn.write(w);
        self.en.write(w);
        self.pairs.write(w);
    }

    pub fn read(r: &mut Reader) -> io::Result<Context> {
        let sw = r.arr::<f64>()?;
        let cs = r.arr::<f64>()?;
        let js = r.arr::<f64>()?;
        let jn = r.arr::<f64>()?;
        if sw.len() != 18 || cs.len() != 2 * CASES || js.len() != 6 * JOINS || jn.len() != 2 * JOINS {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad context header"));
        }
        let mut switch = [[[0.0; 2]; 3]; 3];
        for (i, v) in sw.iter().enumerate() {
            switch[i / 6][(i / 2) % 3][i % 2] = *v;
        }
        let mut case = [[0.0; 2]; CASES];
        for (i, v) in cs.iter().enumerate() {
            case[i / 2][i % 2] = *v;
        }
        let mut join = [[[0.0; 2]; 3]; JOINS];
        for (i, v) in js.iter().enumerate() {
            join[i / 6][(i / 2) % 3][i % 2] = *v;
        }
        let mut join_next = [[0.0; 2]; JOINS];
        for (i, v) in jn.iter().enumerate() {
            join_next[i / 2][i % 2] = *v;
        }
        let en_unseen = r.f64()?;
        let (vn, en, pairs) = (VnLex::read(r)?, EnLex::read(r)?, Pairs::read(r)?);
        Ok(Context { vn, en, switch, case, join, join_next, pairs, en_unseen })
    }

    pub fn switch(&self, prev2: Option<Lang>, prev: Option<Lang>, lang: Lang) -> f64 {
        self.switch[prev2.map_or(2, lang_idx)][prev.map_or(2, lang_idx)][lang_idx(lang)]
    }

    pub fn case_ratio(&self, class: usize, lang: Lang) -> f64 {
        self.case[class][lang_idx(lang)]
    }

    pub fn join_ratio(&self, class: usize, prev: Option<Lang>, lang: Lang) -> f64 {
        self.join[class][prev.map_or(2, lang_idx)][lang_idx(lang)]
    }

    pub fn join_next_ratio(&self, class: usize, lang: Lang) -> f64 {
        self.join_next[class][lang_idx(lang)]
    }

    /// Số hiệu của một từ trong bảng cặp từ (để engine nhớ sẵn từ trước, không tra lại mỗi phím).
    pub fn word_id(&self, key: &str) -> Option<u32> {
        self.pairs.find(key)
    }

    pub fn follow_id(&self, id: u32) -> Option<RowView<'_>> {
        self.pairs.view(id)
    }

    /// Số lần từ ngay sau từ `id` là [tiếng Việt, tiếng Anh].
    pub fn next_lang(&self, id: u32) -> [f64; 2] {
        let i = 2 * id as usize;
        match self.pairs.next_lang.get(i..i + 2) {
            Some(s) => [s[0] as f64, s[1] as f64],
            None => [0.0; 2],
        }
    }

    pub fn follow(&self, prev: &str) -> Option<RowView<'_>> {
        self.pairs.view(self.pairs.find(prev)?)
    }

    pub fn stats(&self) -> String {
        format!(
            "context: en {} words, {} pairs over {} words; P(en|vn,vn) {:.3}  P(en|vn,en) {:.3}  P(en|en,en) {:.3}  P(en|start) {:.3}; case ratio en: {:.2?}; join ratio en: {:.2?}; P(unseen en) {:.4}",
            self.en.len(),
            self.pairs.succ.len(),
            self.pairs.vocab.len(),
            self.switch[0][0][1],
            self.switch[0][1][1],
            self.switch[1][1][1],
            self.switch[2][2][1],
            self.case.map(|r| r[1]),
            self.join.map(|r| r.map(|x| (x[1] * 100.0).round() / 100.0)),
            self.en_unseen
        )
    }
}

/// Ngược của `canon` cho âm tiết: "truong2" -> "truòng" (đủ để `decompose` lấy lại thanh).
pub fn key_to_word(key: &str) -> String {
    let Some(d) = key.chars().last().and_then(|c| c.to_digit(10)) else { return key.to_string() };
    let s: Vec<char> = key[..key.len() - 1].chars().collect();
    crate::syllable::apply_tone(&s, Tone::ALL[d as usize], true).into_iter().collect()
}

