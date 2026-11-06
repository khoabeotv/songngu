//! Bộ ghép Telex: biến chuỗi phím thành âm tiết (chữ có mũ/móc + thanh). Không tự quyết đúng/sai;
//! việc âm tiết còn hợp lệ hay không do `lexicon` kiểm.

use crate::syllable::{apply_tone, is_vowel, strip_mark, Tone};

#[derive(Clone, Copy, Debug)]
pub struct Letter {
    pub ch: char,
    pub upper: bool,
    /// Vị trí trong `literal` của phím đã thêm mũ/móc/gạch cho chữ này (để huỷ khi gõ lặp).
    mark_key: Option<usize>,
}

impl Letter {
    fn new(ch: char, upper: bool) -> Self {
        Letter { ch, upper, mark_key: None }
    }
}

/// Tuỳ chọn kiểu gõ.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TelexOpts {
    /// VNI: 1-5 dấu thanh, 6 mũ, 7 móc, 8 trăng, 9 đ, 0 xoá dấu; chữ cái không bao giờ là phím dấu.
    pub vni: bool,
    /// `[` = ơ, `]` = ư (Telex).
    pub brackets: bool,
}

impl TelexOpts {
    /// Phím không phải chữ cái nhưng là một phần của từ khi gõ kiểu này.
    pub fn word_key(&self, c: char, in_word: bool) -> bool {
        (self.brackets && "[]{}".contains(c)) || (self.vni && in_word && c.is_ascii_digit())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Composer {
    pub opts: TelexOpts,
    pub letters: Vec<Letter>,
    pub tone: Tone,
    tone_key: Option<usize>,
    /// Đã gõ lặp để huỷ (ss, ww, aaa, ddd): từ đó mọi phím chỉ là chữ.
    pub undone: bool,
    /// Số thao tác người gõ Telex hiếm làm: đổi sang dấu thanh khác, thêm mũ/móc sau dấu thanh.
    pub odd: u32,
    /// Số lần đặt mũ hoặc đ không liền sau chữ của nó (thaya, cana, duod): từ Anh hay ra dạng này (did, theme).
    pub late: u32,
    /// Phím gốc trừ phím bị huỷ ("verr" -> "ver"): chữ Telex cổ điển hiện ra khi khôi phục.
    literal: Vec<Option<(char, bool)>>,
}

fn w_mark(c: char) -> Option<char> {
    match c {
        'a' | 'â' => Some('ă'),
        'o' | 'ô' => Some('ơ'),
        'u' => Some('ư'),
        _ => None,
    }
}

impl Composer {
    pub fn new(opts: TelexOpts) -> Self {
        Composer { opts, ..Default::default() }
    }

    pub fn push(&mut self, key: char) {
        let upper = key.is_ascii_uppercase() || key == '{' || key == '}';
        let c = key.to_ascii_lowercase();
        self.literal.push(Some((c, upper)));
        let li = self.literal.len() - 1;
        if self.undone {
            self.letters.push(Letter::new(c, upper));
            return;
        }
        if self.opts.brackets && "[]{}".contains(c) {
            let ch = if c == '[' || c == '{' { 'ơ' } else { 'ư' };
            self.letters.push(Letter { ch, upper, mark_key: Some(li) });
            return;
        }
        if self.opts.vni {
            if !c.is_ascii_digit() || !self.vni_digit(c, li) {
                self.letters.push(Letter::new(c, upper));
            }
            return;
        }
        if let Some(t) = Tone::from_key(c) {
            if self.letters.iter().any(|l| is_vowel(l.ch)) {
                if self.tone == t {
                    self.tone = Tone::None;
                    let k = self.tone_key.take();
                    self.drop_key(k);
                    self.letters.push(Letter::new(c, upper));
                    self.undone = true;
                } else {
                    if self.tone != Tone::None {
                        self.odd += 1;
                    }
                    self.tone = t;
                    self.tone_key = Some(li);
                }
                return;
            }
        } else if c == 'z' && self.tone != Tone::None {
            self.tone = Tone::None;
            let k = self.tone_key.take();
            self.drop_key(k);
            self.literal[li] = None;
            return;
        }
        let handled = match c {
            'w' => self.apply_w(li, upper),
            'a' | 'e' | 'o' => self.apply_hat(c, li),
            'd' => self.apply_stroke(li),
            _ => false,
        };
        if !handled {
            self.letters.push(Letter::new(c, upper));
        } else if self.tone != Tone::None && !self.undone {
            self.odd += 1;
        }
    }

    /// Phím số của VNI; false nếu không có gì để đặt dấu (số thành chữ thường).
    fn vni_digit(&mut self, d: char, li: usize) -> bool {
        let has_vowel = self.letters.iter().any(|l| is_vowel(l.ch));
        let (vs, ve) = self.vowel_span();
        let rightmost = |s: &Self, bases: &[char]| (vs..ve).rev().find(|&k| bases.contains(&strip_mark(s.letters[k].ch)));
        let set = |s: &mut Self, k: usize, ch: char| {
            s.letters[k].ch = ch;
            s.letters[k].mark_key = Some(li);
        };
        match d {
            '1'..='5' if has_vowel => {
                let t = Tone::ALL[d as usize - '0' as usize];
                if self.tone == t {
                    self.tone = Tone::None;
                    self.letters.push(Letter::new(d, false));
                    self.undone = true;
                } else {
                    self.tone = t;
                }
                true
            }
            '0' if self.tone != Tone::None => {
                self.tone = Tone::None;
                true
            }
            '6' => match rightmost(self, &['a', 'e', 'o']) {
                Some(k) => {
                    let ch = match strip_mark(self.letters[k].ch) {
                        'a' => 'â',
                        'e' => 'ê',
                        _ => 'ô',
                    };
                    set(self, k, ch);
                    true
                }
                None => false,
            },
            '7' => {
                if let Some(k) = (vs..ve.saturating_sub(1)).find(|&k| strip_mark(self.letters[k].ch) == 'u' && strip_mark(self.letters[k + 1].ch) == 'o') {
                    set(self, k, 'ư');
                    set(self, k + 1, 'ơ');
                    return true;
                }
                match rightmost(self, &['o', 'u']) {
                    Some(k) => {
                        let ch = if strip_mark(self.letters[k].ch) == 'o' { 'ơ' } else { 'ư' };
                        set(self, k, ch);
                        true
                    }
                    None => false,
                }
            }
            '8' => match rightmost(self, &['a']) {
                Some(k) => {
                    set(self, k, 'ă');
                    true
                }
                None => false,
            },
            '9' => match self.letters.iter().position(|l| l.ch == 'd') {
                Some(k) => {
                    set(self, k, 'đ');
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn drop_key(&mut self, k: Option<usize>) {
        if let Some(k) = k {
            self.literal[k] = None;
        }
    }

    fn undo_mark(&mut self, idx: &[usize], plain: &[char], c: char, upper: bool) -> bool {
        let k = self.letters[idx[0]].mark_key;
        for (&i, &p) in idx.iter().zip(plain) {
            self.letters[i].ch = p;
            self.letters[i].mark_key = None;
        }
        self.drop_key(k);
        self.letters.push(Letter::new(c, upper));
        self.undone = true;
        true
    }

    /// Khoảng nguyên âm chính (bỏ "u" của qu, "i" của gi).
    fn vowel_span(&self) -> (usize, usize) {
        let s: Vec<char> = self.letters.iter().map(|l| strip_mark(l.ch)).collect();
        let n = s.len();
        let mut i = if n >= 2 && s[0] == 'q' && s[1] == 'u' {
            2
        } else if n >= 3 && s[0] == 'g' && s[1] == 'i' && is_vowel(s[2]) {
            2
        } else {
            0
        };
        while i < n && !is_vowel(s[i]) {
            i += 1;
        }
        let vs = i;
        while i < n && is_vowel(s[i]) {
            i += 1;
        }
        (vs, i)
    }

    fn apply_w(&mut self, li: usize, upper: bool) -> bool {
        let (vs, ve) = self.vowel_span();
        let ch = |s: &Self, k: usize| s.letters[k].ch;
        // ươ: gõ w sau "uo" (kể cả sau phụ âm cuối: truongw)
        for k in vs..ve.saturating_sub(1) {
            let (a, b) = (ch(self, k), ch(self, k + 1));
            if matches!(a, 'u' | 'ư') && matches!(b, 'o' | 'ơ') {
                if a == 'ư' && b == 'ơ' {
                    return self.undo_mark(&[k, k + 1], &['u', 'o'], 'w', upper);
                }
                self.letters[k].ch = 'ư';
                self.letters[k + 1].ch = 'ơ';
                self.letters[k].mark_key = Some(li);
                self.letters[k + 1].mark_key = Some(li);
                return true;
            }
        }
        let base = |s: &Self, k: usize| strip_mark(s.letters[k].ch);
        let rightmost = |s: &Self, b: char| (vs..ve).rev().find(|&k| base(s, k) == b);
        let u_first = (vs..ve).find(|&k| base(self, k) == 'u' && (k + 1 == ve || matches!(base(self, k + 1), 'a' | 'i' | 'u')));
        let target = u_first.or_else(|| rightmost(self, 'a')).or_else(|| rightmost(self, 'o')).or_else(|| rightmost(self, 'u'));
        match target {
            Some(k) => {
                let cur = ch(self, k);
                if matches!(cur, 'ă' | 'ơ' | 'ư') {
                    return self.undo_mark(&[k], &[strip_mark(cur)], 'w', upper);
                }
                self.letters[k].ch = w_mark(cur).unwrap();
                self.letters[k].mark_key = Some(li);
                true
            }
            None if vs == ve && !self.letters.iter().any(|l| is_vowel(l.ch)) => {
                // w đứng riêng là ư: "w" -> ư, "nhw" -> như
                self.letters.push(Letter { ch: 'ư', upper, mark_key: Some(li) });
                true
            }
            None => false,
        }
    }

    /// aa ee oo: mũ cho nguyên âm đó trong vần. Gõ liền sau thì gõ lại để huỷ; gõ muộn hơn (sau nguyên âm kế: thaya,
    /// sau phụ âm cuối: cana) đếm vào `late`.
    fn apply_hat(&mut self, c: char, li: usize) -> bool {
        let Some(last) = self.letters.last().copied() else { return false };
        let hat = match c {
            'a' => 'â',
            'e' => 'ê',
            _ => 'ô',
        };
        if strip_mark(last.ch) == c && is_vowel(last.ch) {
            let k = self.letters.len() - 1;
            if last.ch == hat {
                return self.undo_mark(&[k], &[c], c, last.upper);
            }
            self.letters[k].ch = hat;
            self.letters[k].mark_key = Some(li);
            return true;
        }
        let (vs, ve) = self.vowel_span();
        let Some(k) = (vs..ve).rev().find(|&k| self.letters[k].ch == c) else { return false };
        self.letters[k].ch = hat;
        self.letters[k].mark_key = Some(li);
        self.late += 1;
        true
    }

    /// dd: đ cho d đầu âm tiết. Gõ liền thì gõ lại để huỷ; gõ sau nguyên âm (duod) đếm vào `late`.
    fn apply_stroke(&mut self, li: usize) -> bool {
        let Some(k) = self.letters.len().checked_sub(1) else { return false };
        match self.letters[k].ch {
            'd' => {
                self.letters[k].ch = 'đ';
                self.letters[k].mark_key = Some(li);
                true
            }
            'đ' => {
                let up = self.letters[k].upper;
                self.undo_mark(&[k], &['d'], 'd', up)
            }
            _ if self.letters[0].ch == 'd' && self.letters.iter().any(|l| is_vowel(l.ch)) => {
                self.letters[0].ch = 'đ';
                self.letters[0].mark_key = Some(li);
                self.late += 1;
                true
            }
            _ => false,
        }
    }

    /// Âm tiết chưa thanh, chữ thường.
    pub fn toneless(&self) -> Vec<char> {
        self.letters.iter().map(|l| l.ch).collect()
    }

    pub fn render(&self, modern: bool) -> String {
        let toned = apply_tone(&self.toneless(), if self.undone { Tone::None } else { self.tone }, modern);
        toned.iter().zip(&self.letters).flat_map(|(&c, l)| case(c, l.upper)).collect()
    }

    /// Chuỗi phím Telex chuẩn tạo ra các chữ đang hiện, bỏ `drop` chữ cuối: mũ/móc gõ liền sau chữ, dấu thanh ở cuối.
    /// Dấu thanh mất theo chữ mang nó. Dùng khi Backspace xoá một chữ tiếng Việt: "tiếng" -> "tiến" -> "ti".
    pub fn canonical_keys(&self, drop: usize, modern: bool) -> String {
        let keep = self.letters.len().saturating_sub(drop);
        let tone_at = crate::syllable::tone_pos(&self.toneless(), modern);
        let tone = if tone_at.is_some_and(|p| p < keep) { self.tone } else { Tone::None };
        let letters: Vec<(char, bool)> = self.letters[..keep].iter().map(|l| (l.ch, l.upper)).collect();
        encode(&letters, tone, self.opts.vni)
    }

    pub fn literal(&self) -> String {
        self.literal.iter().flatten().map(|&(c, up)| if up { c.to_ascii_uppercase() } else { c }).collect()
    }
}

fn case(c: char, upper: bool) -> Vec<char> {
    if upper {
        c.to_uppercase().collect()
    } else {
        vec![c]
    }
}

/// Phím chuẩn để gõ ra một từ có sẵn (mở lại từ đã chốt để sửa). None nếu từ có ký tự không gõ được.
pub fn keys_for(word: &str, vni: bool) -> Option<String> {
    let mut tone = Tone::None;
    let mut letters = Vec::new();
    for c in word.chars() {
        let (b, t) = crate::syllable::split_tone(c.to_lowercase().next()?);
        if t != Tone::None {
            tone = t;
        }
        if !strip_mark(b).is_ascii_lowercase() {
            return None;
        }
        letters.push((b, c.is_uppercase()));
    }
    Some(encode(&letters, tone, vni))
}

/// Chữ (đã có mũ/móc, chưa thanh) + thanh -> phím: mũ/móc liền sau chữ, dấu thanh cuối từ.
fn encode(letters: &[(char, bool)], tone: Tone, vni: bool) -> String {
    let all_upper = letters.len() > 1 && letters.iter().all(|l| l.1);
    let case = |k: char| if all_upper { k.to_ascii_uppercase() } else { k };
    let mut out = String::new();
    for &(ch, upper) in letters {
        let base = strip_mark(ch);
        out.push(if upper { base.to_ascii_uppercase() } else { base });
        let m = match (ch, vni) {
            ('â' | 'ê' | 'ô', true) => Some('6'),
            ('ă', true) => Some('8'),
            ('ơ' | 'ư', true) => Some('7'),
            ('đ', true) => Some('9'),
            ('â' | 'ê' | 'ô' | 'đ', false) => Some(base),
            ('ă' | 'ơ' | 'ư', false) => Some('w'),
            _ => None,
        };
        if let Some(m) = m {
            out.push(case(m));
        }
    }
    if tone != Tone::None {
        let k = if vni { (b'0' + tone.idx() as u8) as char } else { b"_sfrxj"[tone.idx()] as char };
        out.push(case(k));
    }
    out
}

/// Gõ cả chuỗi phím, trả về chữ Telex hiện ra.
pub fn telex(keys: &str, modern: bool) -> String {
    let mut c = Composer::default();
    keys.chars().for_each(|k| c.push(k));
    c.render(modern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes() {
        let cases = [
            ("truowfng", "trường"),
            ("truongwf", "trường"),
            ("truwownfg", "trường"),
            ("tieengs", "tiếng"),
            ("vieetj", "việt"),
            ("VIEETJ", "VIỆT"),
            ("Hoom", "Hôm"),
            ("dduowcj", "được"),
            ("nhw", "như"),
            ("muaw", "mưa"),
            ("hoawcj", "hoặc"),
            ("quaws", "quắ"),
            ("giuwx", "giữ"),
            ("hoaf", "hoà"),
            ("xooong", "xoong"),
            ("verr", "ver"),
            ("thiss", "this"),
            ("cuowis", "cưới"),
            ("uwowcs", "ước"),
            ("thayas", "thấy"),
            ("caua", "câu"),
            ("toio", "tôi"),
            ("tieue", "tiêu"),
            ("ddaua", "đâu"),
            ("keuej", "kệu"),
            ("canaf", "cần"),
            ("tienges", "tiếng"),
            ("duodwcj", "được"),
            ("duowcjd", "được"),
            ("dieemdr", "điểm"),
            ("diemerd", "điểm"),
        ];
        for (k, want) in cases {
            assert_eq!(telex(k, true), want, "{k}");
        }
    }

    #[test]
    fn canonical_keys_rebuild_without_last_letter() {
        for (keys, want) in [("tieengs", "tiến"), ("truowfng", "trườn"), ("VIEETJ", "VIỆ"), ("dduowcj", "đượ"), ("mas", "m"), ("tieesn", "ti")] {
            let mut c = Composer::default();
            keys.chars().for_each(|k| c.push(k));
            let drop = if keys == "tieesn" { 2 } else { 1 };
            assert_eq!(telex(&c.canonical_keys(drop, true), true), want, "{keys}");
        }
    }

    fn with(opts: TelexOpts, keys: &str) -> String {
        let mut c = Composer::new(opts);
        keys.chars().for_each(|k| c.push(k));
        c.render(true)
    }

    #[test]
    fn options() {
        let vni = TelexOpts { vni: true, ..Default::default() };
        assert_eq!(with(vni, "Vie65t"), "Việt");
        assert_eq!(with(vni, "tru7o72ng"), "trường");
        assert_eq!(with(vni, "d9a"), "đa");
        assert_eq!(with(vni, "test"), "test");
        let br = TelexOpts { brackets: true, ..Default::default() };
        assert_eq!(with(br, "t]"), "tư");
        assert_eq!(with(br, "m[f"), "mờ");

    }

    #[test]
    fn keys_for_round_trips() {
        for w in ["trường", "Việt", "VIỆT", "được", "hoà", "người", "khuỷu"] {
            assert_eq!(telex(&keys_for(w, false).unwrap(), true), w, "{w}");
            assert_eq!(with(TelexOpts { vni: true, ..Default::default() }, &keys_for(w, true).unwrap()), w, "vni {w}");
        }
        assert_eq!(keys_for("test", false).as_deref(), Some("test"));
        assert_eq!(keys_for("cafésü", false), None);
    }

    #[test]
    fn literal_drops_undone_key() {
        let mut c = Composer::default();
        "verr".chars().for_each(|k| c.push(k));
        assert_eq!(c.literal(), "ver");
        assert!(c.undone);
    }
}
