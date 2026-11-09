//! Từ điển hai thứ tiếng + tra tiền tố. Cả hai dùng mảng đã sắp xếp + tìm nhị phân:
//! tiền tố nào cũng là một đoạn liên tiếp, nên "còn sống không" và "tổng tần suất phía sau" chỉ tốn 2 lần tìm.
//! Tra cứu không cấp phát: được gọi nhiều lần mỗi phím.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io;

use unicode_normalization::UnicodeNormalization;

use crate::store::{cmp_prefix_bytes, Reader, StrList, Writer};
use crate::syllable::{self, stop_coda, strip_mark, Tone};

const SMOOTH: f64 = 0.5;

struct VnEntry {
    marked: Vec<char>,
    stripped: String,
    stop: bool,
    freq: [f64; 6],
}

impl VnEntry {
    fn tone_ok(&self, t: Tone) -> bool {
        !self.stop || matches!(t, Tone::Sac | Tone::Nang)
    }
}

pub struct VnLex {
    /// Xếp theo (dạng không dấu, chữ có mũ/móc): tiền tố đang gõ là một đoạn liên tiếp.
    entries: Vec<VnEntry>,
    /// Chỉ số `entries` xếp theo chữ có mũ/móc, để tra đúng âm tiết.
    by_marked: Vec<u16>,
    /// Viết tắt toàn phụ âm có đ (đc, đt, sđt, bđs, vnđ): gõ bằng Telex nhưng không phải âm tiết. Học từ danh sách tần suất.
    abbr: Vec<(Vec<char>, f64)>,
    total: f64,
    /// Tỉ lệ tần suất danh sách vi khớp được vào tập âm tiết sinh ra (độ phủ của luật ghép vần).
    pub coverage: f64,
}

/// Chữ đang gõ có thể thành chữ `m` của âm tiết khi gõ thêm: w thêm móc/trăng bất cứ lúc nào,
/// mũ (aa ee oo) và đ chỉ ngay sau chữ cuối.
fn compatible(x: char, m: char, last: bool) -> bool {
    x == m
        || matches!((x, m), ('a', 'ă') | ('o', 'ơ') | ('u', 'ư') | ('â', 'ă') | ('ô', 'ơ'))
        || (last && matches!((x, m), ('a', 'â') | ('e', 'ê') | ('o', 'ô') | ('ă', 'â') | ('ơ', 'ô') | ('d', 'đ')))
}

/// Viết tắt tiếng Việt: chỉ phụ âm, có đ, tối đa 5 chữ.
fn is_abbr(w: &str) -> bool {
    w.contains('đ') && w.chars().count() <= 5 && w.chars().all(|c| "bđcdghklmnpqrstvx".contains(c))
}

/// `letters` (đang gõ dở) còn có thể thành âm tiết `marked` (chưa thanh) không.
pub fn compatible_prefix(letters: &[char], marked: impl IntoIterator<Item = char>) -> bool {
    let last = letters.len().saturating_sub(1);
    let mut m = marked.into_iter();
    letters.iter().enumerate().all(|(i, &x)| m.next().is_some_and(|c| compatible(x, c, i == last)))
}

impl VnLex {
    pub fn build(freq_text: &str) -> VnLex {
        let mut entries: Vec<VnEntry> = syllable::generate()
            .into_keys()
            .map(|s| VnEntry {
                stripped: s.chars().map(strip_mark).collect(),
                stop: stop_coda(&s),
                marked: s.chars().collect(),
                freq: [0.0; 6],
            })
            .collect();
        entries.sort_by(|a, b| a.stripped.cmp(&b.stripped).then(a.marked.cmp(&b.marked)));
        let by_marked: HashMap<String, usize> = entries.iter().enumerate().map(|(i, e)| (e.marked.iter().collect(), i)).collect();
        let (mut covered, mut all) = (0.0, 0.0);
        let mut abbr: HashMap<String, f64> = HashMap::new();
        for line in freq_text.lines() {
            let mut it = line.split_whitespace();
            let (Some(w), Some(n)) = (it.next(), it.next().and_then(|n| n.parse::<f64>().ok())) else { continue };
            // Danh sách lẫn cả dạng NFD ("là" = la + dấu huyền rời) với số đếm riêng.
            let w: String = w.to_lowercase().nfc().collect();
            if !w.chars().all(char::is_alphabetic) {
                continue;
            }
            all += n;
            if let Some((toneless, t)) = syllable::decompose(&w) {
                if let Some(&i) = by_marked.get(&toneless) {
                    if entries[i].tone_ok(t) {
                        entries[i].freq[t.idx()] += n;
                        covered += n;
                    }
                } else if is_abbr(&w) {
                    *abbr.entry(w).or_default() += n;
                }
            }
        }
        let mut abbr: Vec<(Vec<char>, f64)> = abbr.into_iter().map(|(w, n)| (w.chars().collect(), n)).collect();
        abbr.sort_by(|a, b| a.0.cmp(&b.0));
        VnLex::assemble(entries, abbr, covered, covered / all.max(1.0))
    }

    fn assemble(entries: Vec<VnEntry>, abbr: Vec<(Vec<char>, f64)>, total: f64, coverage: f64) -> VnLex {
        let mut by_marked: Vec<u16> = (0..entries.len() as u16).collect();
        by_marked.sort_by(|&a, &b| entries[a as usize].marked.cmp(&entries[b as usize].marked));
        VnLex { entries, by_marked, abbr, total, coverage }
    }

    pub fn write(&self, w: &mut Writer) {
        let marked: Vec<String> = self.entries.iter().map(|e| e.marked.iter().collect()).collect();
        w.strs(&StrList::from_strings(&marked));
        w.arr(&self.entries.iter().flat_map(|e| e.freq).collect::<Vec<f64>>());
        let abbr: Vec<String> = self.abbr.iter().map(|a| a.0.iter().collect()).collect();
        w.strs(&StrList::from_strings(&abbr));
        w.arr(&self.abbr.iter().map(|a| a.1).collect::<Vec<f64>>());
        w.arr(&[self.total, self.coverage]);
    }

    pub fn read(r: &mut Reader) -> io::Result<VnLex> {
        let marked = r.strs()?;
        let freq = r.arr::<f64>()?;
        let abbr_w = r.strs()?;
        let abbr_n = r.arr::<f64>()?;
        let tc = r.arr::<f64>()?;
        if freq.len() != marked.len() * 6 || abbr_n.len() != abbr_w.len() || tc.len() != 2 || marked.len() > u16::MAX as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad vn lexicon"));
        }
        let entries = (0..marked.len())
            .map(|i| {
                let s = marked.get(i);
                VnEntry {
                    marked: s.chars().collect(),
                    stripped: s.chars().map(strip_mark).collect(),
                    stop: stop_coda(s),
                    freq: freq[i * 6..i * 6 + 6].try_into().unwrap(),
                }
            })
            .collect();
        let abbr = (0..abbr_w.len()).map(|i| (abbr_w.get(i).chars().collect(), abbr_n[i])).collect();
        Ok(VnLex::assemble(entries, abbr, tc[0], tc[1]))
    }

    /// Tổng số đếm: xác suất × `total` = số lần gặp.
    pub fn total(&self) -> f64 {
        self.total
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn find(&self, toneless: &[char]) -> Option<&VnEntry> {
        let i = self.by_marked.binary_search_by(|&i| self.entries[i as usize].marked.as_slice().cmp(toneless)).ok()?;
        Some(&self.entries[self.by_marked[i] as usize])
    }

    /// P(âm tiết | tiếng Việt), None nếu không phải âm tiết có thật. Âm tiết đúng luật nhưng chưa từng gặp
    /// trong danh sách tần suất (rộng hơn mọi âm tiết đang dùng) coi như không có: "ROAS" không thành "ROÁ".
    pub fn word_prob(&self, toneless: &[char], t: Tone) -> Option<f64> {
        let Some(e) = self.find(toneless) else {
            let i = self.abbr.binary_search_by(|a| a.0.as_slice().cmp(toneless)).ok()?;
            return (t == Tone::None).then(|| self.abbr[i].1 / self.total);
        };
        (e.tone_ok(t) && e.freq[t.idx()] > 0.0).then(|| e.freq[t.idx()] / self.total)
    }

    /// Đúng luật ghép vần và thanh (kể cả âm tiết chưa gặp trong danh sách tần suất).
    pub fn is_syllable(&self, toneless: &[char], t: Tone) -> bool {
        self.find(toneless).is_some_and(|e| e.tone_ok(t))
    }

    /// Xác suất cho âm tiết đúng luật mà danh sách tần suất chưa gặp, như thể gặp `count` lần.
    pub fn unseen_prob(&self, toneless: &[char], t: Tone, count: f64) -> Option<f64> {
        (count > 0.0 && self.is_syllable(toneless, t)).then(|| count / self.total)
    }

    /// Tổng xác suất các âm tiết còn có thể gõ tiếp thành; None = hết đường.
    pub fn prefix_prob(&self, letters: &[char], t: Tone) -> Option<f64> {
        let mut buf = [0u8; 32];
        let mut n = 0;
        for &c in letters {
            let c = strip_mark(c);
            if !c.is_ascii() || n == buf.len() {
                return None;
            }
            buf[n] = c as u8;
            n += 1;
        }
        // SAFETY: chỉ chứa byte ASCII.
        let stripped = unsafe { std::str::from_utf8_unchecked(&buf[..n]) };
        let lo = self.entries.partition_point(|e| cmp_prefix_bytes(&e.stripped, stripped).is_lt());
        let hi = lo + self.entries[lo..].partition_point(|e| cmp_prefix_bytes(&e.stripped, stripped).is_eq());
        let mut mass = None;
        for e in &self.entries[lo..hi] {
            if compatible_prefix(letters, e.marked.iter().copied()) && (t == Tone::None || e.tone_ok(t)) {
                let f = if t == Tone::None { e.freq.iter().sum::<f64>() } else { e.freq[t.idx()] };
                *mass.get_or_insert(0.0) += f + SMOOTH;
            }
        }
        if t == Tone::None {
            for (_, n) in self.abbr.iter().filter(|(a, _)| compatible_prefix(letters, a.iter().copied())) {
                *mass.get_or_insert(0.0) += n;
            }
        }
        mass.map(|m| m / self.total)
    }
}

pub struct EnLex {
    /// Xếp theo thứ tự chuỗi.
    words: StrList,
    /// Tần suất cộng dồn (len + 1): tần suất một từ hay một khoảng tiền tố = hiệu hai phần tử.
    cum: Cow<'static, [f64]>,
    total: f64,
    /// Mô hình chữ cái cho từ Anh chưa gặp (viết tắt, tên riêng, thuật ngữ mới).
    pub chars: CharLm,
}

/// Bigram chữ cái học trên các từ (tính theo loại, không theo tần suất): xác suất một chuỗi bất kỳ là "trông như
/// từ tiếng Anh". "nist" cao hơn "truowfng" rất nhiều.
pub struct CharLm {
    /// p[trước][sau]: 0..26 = a..z; hàng 26 = đầu từ, cột 26 = hết từ.
    p: [[f64; 27]; 27],
}

fn letter_idx(c: char) -> Option<usize> {
    c.is_ascii_lowercase().then(|| c as usize - 'a' as usize)
}

impl CharLm {
    fn train<'a>(words: impl Iterator<Item = &'a str>) -> CharLm {
        let mut n = [[1.0f64; 27]; 27];
        for w in words {
            let mut prev = 26;
            for c in w.chars() {
                let Some(i) = letter_idx(c) else { break };
                n[prev][i] += 1.0;
                prev = i;
            }
            n[prev][26] += 1.0;
        }
        CharLm { p: n.map(|r| r.map(|x| x / r.iter().sum::<f64>())) }
    }

    /// P(chuỗi là đầu một từ); `whole`: tính cả xác suất kết thúc từ ở đây.
    pub fn prob(&self, w: &str, whole: bool) -> f64 {
        let mut prev = 26;
        let mut p = 1.0;
        for c in w.chars() {
            let Some(i) = letter_idx(c) else { return 0.0 };
            p *= self.p[prev][i];
            prev = i;
        }
        if whole {
            p *= self.p[prev][26];
        }
        p
    }
}

/// Phần còn lại khi dấu nháy tách từ (didn't -> didn + t).
const CONTRACTION_STEMS: &str = "don didn doesn isn wasn weren haven hasn hadn won wouldn couldn shouldn aren ain mustn needn ve ll re";

/// Từ 1-2 chữ thật. Danh sách phụ đề lẫn rất nhiều rác ngắn (ma, la, ng, aj) mà bộ kiểm chính tả vẫn nhận.
const SHORT_WORDS: &str = "a i to it of is in we me he my on do no be so go if up at oh as an us or by am uh ok hi ah um mr ha \
    ya eh tv yo ow ho id ex ad pm ox hm";

impl EnLex {
    /// `freq_text`: "word count" mỗi dòng. `extra_text`: một từ mỗi dòng (# là chú thích), tần suất `extra_count`.
    pub fn build(freq_text: &str, extra_text: &str, extra_count: f64) -> EnLex {
        let mut map: HashMap<String, f64> = HashMap::new();
        for line in freq_text.lines() {
            let mut it = line.split_whitespace();
            if let (Some(w), Some(n)) = (it.next(), it.next().and_then(|n| n.parse::<f64>().ok())) {
                if w.len() <= 2 && !SHORT_WORDS.split_whitespace().any(|s| s == w) {
                    continue;
                }
                *map.entry(w.to_lowercase()).or_default() += n;
            }
        }
        let extra = extra_text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
        for w in extra.chain(CONTRACTION_STEMS.split_whitespace()) {
            let f = map.entry(w.to_lowercase()).or_default();
            *f = f.max(extra_count);
        }
        let mut words: Vec<String> = map.keys().cloned().collect();
        words.sort();
        let mut cum = Vec::with_capacity(words.len() + 1);
        let mut acc = 0.0;
        cum.push(0.0);
        for w in &words {
            acc += map[w];
            cum.push(acc);
        }
        let chars = CharLm::train(words.iter().map(String::as_str));
        EnLex { words: StrList::from_strings(&words), cum: Cow::Owned(cum), total: acc, chars }
    }

    pub fn write(&self, w: &mut Writer) {
        w.strs(&self.words);
        w.arr(&self.cum);
        w.f64(self.total);
        w.arr(&self.chars.p.iter().flatten().copied().collect::<Vec<f64>>());
    }

    pub fn read(r: &mut Reader) -> io::Result<EnLex> {
        let words = r.strs()?;
        let cum = r.arr::<f64>()?;
        let total = r.f64()?;
        let p = r.arr::<f64>()?;
        if cum.len() != words.len() + 1 || p.len() != 27 * 27 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad en lexicon"));
        }
        let mut chars = CharLm { p: [[0.0; 27]; 27] };
        for (i, row) in chars.p.iter_mut().enumerate() {
            row.copy_from_slice(&p[i * 27..i * 27 + 27]);
        }
        Ok(EnLex { words, cum, total, chars })
    }

    /// Tổng số đếm: xác suất × `total` = số lần gặp.
    pub fn total(&self) -> f64 {
        self.total
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn word_prob(&self, w: &str) -> Option<f64> {
        let i = self.words.find(w)?;
        Some((self.cum[i + 1] - self.cum[i]) / self.total)
    }

    pub fn prefix_prob(&self, p: &str) -> Option<f64> {
        let (lo, hi) = self.words.prefix_range(p);
        (hi > lo).then(|| (self.cum[hi] - self.cum[lo]) / self.total)
    }
}
