//! Người gõ giả lập để đo: văn bản -> phím Telex từng từ. Mặc định gõ chuẩn (mũ/móc liền sau chữ, dấu thanh cuối từ);
//! `Habits` thêm các cách gõ khác ra cùng chữ và lỗi gõ. Biến thể chỉ dùng khi Telex của nó ra đúng chữ như cách chuẩn.

use crate::syllable::{split_tone, Tone};
use crate::telex::{keys_for, telex};

/// Phím Backspace trong chuỗi phím.
pub const BS: char = '\u{8}';
/// Chữ đích của từ không chấm (gõ nhầm không sửa: sai đằng nào người gõ cũng phải sửa).
pub const SKIP: &str = "\u{1}";

#[derive(Clone, Copy, Default, Debug)]
pub struct Habits {
    /// Tỉ lệ từ có thanh gõ dấu ngay sau nguyên âm thay vì cuối từ (tieesng).
    pub mid_tone: f64,
    /// Tỉ lệ từ có ư/ơ/ă gõ một w ở cuối (truongwf).
    pub late_w: f64,
    /// Tỉ lệ từ có â/ê/ô gõ phím mũ muộn hơn, sau nguyên âm kế hoặc phụ âm cuối (thayas, canaf).
    pub late_hat: f64,
    /// Tỉ lệ từ có đ gõ d thứ hai muộn hơn (duowdcj).
    pub late_d: f64,
    /// Tỉ lệ từ gõ nhầm một phím rồi Backspace sửa ngay.
    pub fixed_typo: f64,
    /// Tỉ lệ từ gõ nhầm không sửa (thay phím bên cạnh, sót, lặp hoặc đảo phím).
    pub typo: f64,
    /// Tỉ lệ từ có dấu gõ đảo hai phím liền nhau, vẫn chấm theo chữ đúng (đo sửa gõ đảo: nyaf -> này).
    pub swap: f64,
}

impl Habits {
    pub fn realistic() -> Self {
        Habits { mid_tone: 0.3, late_w: 0.2, late_hat: 0.2, late_d: 0.2, fixed_typo: 0.03, typo: 0.0, swap: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Unit {
    /// Phím gõ và chữ phải hiện ra (`SKIP` = không chấm).
    Word { keys: String, want: String },
    Sep(String),
}

const ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
/// Phím gõ nhầm rồi xoá ngay: phụ âm không làm đổi chữ trước nó trong Telex, nên một Backspace là về đúng chỗ cũ.
const INERT: &str = "bcghklmnpqtvz";

/// Phím cạnh `c` trên bàn phím QWERTY (cùng hàng và hai hàng kề).
fn neighbors(c: char) -> Vec<char> {
    let Some((r, i)) = ROWS.iter().enumerate().find_map(|(r, row)| row.find(c).map(|i| (r as isize, i as isize))) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // hàng trên lệch phải nửa phím so với hàng dưới
    for (dr, ds) in [(-1isize, [0isize, 1]), (0, [-1, 1]), (1, [-1, 0])] {
        let rr = r + dr;
        if !(0..3).contains(&rr) {
            continue;
        }
        for d in ds {
            if i + d >= 0 {
                out.extend(ROWS[rr as usize].chars().nth((i + d) as usize));
            }
        }
    }
    out
}

fn same_case(k: char, like: char) -> char {
    if like.is_uppercase() {
        k.to_ascii_uppercase()
    } else {
        k
    }
}

pub struct Typist {
    pub habits: Habits,
    rng: u64,
}

impl Typist {
    pub fn new(habits: Habits, seed: u64) -> Self {
        Typist { habits, rng: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    fn next(&mut self) -> u64 {
        // splitmix64
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn chance(&mut self, p: f64) -> bool {
        p > 0.0 && ((self.next() >> 11) as f64) < p * (1u64 << 53) as f64
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// Một dòng (một tin nhắn) -> đơn vị gõ. Từ = chuỗi chữ cái liền nhau; từ không gõ được bằng Telex thành dấu cách.
    pub fn line(&mut self, line: &str) -> Vec<Unit> {
        let mut out = Vec::new();
        let mut word = String::new();
        for c in line.chars().chain(std::iter::once('\n')) {
            if c.is_alphabetic() {
                word.push(c);
                continue;
            }
            if !word.is_empty() {
                out.push(match self.word(&word) {
                    Some((keys, want)) => Unit::Word { keys, want },
                    None => Unit::Sep(" ".into()),
                });
                word.clear();
            }
            if c != '\n' {
                out.push(Unit::Sep(c.to_string()));
            }
        }
        out
    }

    fn word(&mut self, w: &str) -> Option<(String, String)> {
        let std_keys = if w.is_ascii() { w.to_string() } else { keys_for(w, false)? };
        let mut keys = std_keys.clone();
        if !w.is_ascii() {
            let shown = telex(&std_keys, true);
            let toned = w.chars().any(|c| c.to_lowercase().next().is_some_and(|l| split_tone(l).1 != Tone::None));
            if self.chance(self.habits.late_w) {
                keys = late_w(&keys, toned).filter(|k| telex(k, true) == shown).unwrap_or(keys);
            }
            let low = |c: char| c.to_ascii_lowercase();
            if self.chance(self.habits.late_hat) {
                let ks: Vec<char> = keys.chars().collect();
                // phím mũ = nguyên âm a/e/o lặp liền (không có trong từ Việt nào ngoài mũ)
                let hat = (1..ks.len()).find(|&i| low(ks[i]) == low(ks[i - 1]) && "aeo".contains(low(ks[i])));
                if let Some(k) = hat.and_then(|i| self.move_later(&keys, i, toned)) {
                    keys = Some(k).filter(|k| telex(k, true) == shown).unwrap_or(keys);
                }
            }
            if self.chance(self.habits.late_d) && keys.len() > 1 && keys[..2].eq_ignore_ascii_case("dd") {
                if let Some(k) = self.move_later(&keys, 1, toned) {
                    keys = Some(k).filter(|k| telex(k, true) == shown).unwrap_or(keys);
                }
            }
            if toned && self.chance(self.habits.mid_tone) {
                keys = mid_tone(&keys).filter(|k| telex(k, true) == shown).unwrap_or(keys);
            }
        }
        // chỉ rút số ngẫu nhiên khi bật, để các cách gõ khác giữ nguyên chuỗi ngẫu nhiên cũ
        if !w.is_ascii() && self.habits.swap > 0.0 && self.chance(self.habits.swap) {
            let ks: Vec<char> = keys.chars().collect();
            if ks.len() > 1 {
                let p = self.pick(ks.len() - 1);
                if ks[p] != ks[p + 1] {
                    let mut s = ks.clone();
                    s.swap(p, p + 1);
                    return Some((s.into_iter().collect(), w.to_string()));
                }
            }
        }
        if self.chance(self.habits.typo) {
            // cách gõ nhầm được chọn có thể không áp được cho từ này (sót phím của từ một chữ): thử lại
            if let Some(k) = (0..8).find_map(|_| self.typo(&keys)) {
                return Some((k, SKIP.into()));
            }
        }
        if self.chance(self.habits.fixed_typo) {
            keys = self.fixed_typo(&keys).unwrap_or(keys);
        }
        Some((keys, w.to_string()))
    }

    /// Dời phím thứ `from` tới một chỗ ngẫu nhiên phía sau (sau ít nhất một phím, trước phím thanh cuối từ nếu có).
    fn move_later(&mut self, keys: &str, from: usize, toned: bool) -> Option<String> {
        let mut ks: Vec<char> = keys.chars().collect();
        let end = if toned { ks.len() - 1 } else { ks.len() };
        if end <= from + 1 {
            return None;
        }
        let c = ks.remove(from);
        ks.insert(from + 1 + self.pick(end - 1 - from), c);
        Some(ks.into_iter().collect())
    }

    fn fixed_typo(&mut self, keys: &str) -> Option<String> {
        let ks: Vec<char> = keys.chars().collect();
        let p = self.pick(ks.len());
        let near: Vec<char> = neighbors(ks[p].to_ascii_lowercase()).into_iter().filter(|k| INERT.contains(*k)).collect();
        if near.is_empty() {
            return None;
        }
        let k = same_case(near[self.pick(near.len())], ks[p]);
        let mut out: String = ks[..p].iter().collect();
        out.push(k);
        out.push(BS);
        out.extend(&ks[p..]);
        Some(out)
    }

    fn typo(&mut self, keys: &str) -> Option<String> {
        let mut ks: Vec<char> = keys.chars().collect();
        let p = self.pick(ks.len());
        match self.pick(4) {
            0 => {
                let near = neighbors(ks[p].to_ascii_lowercase());
                if near.is_empty() {
                    return None;
                }
                ks[p] = same_case(near[self.pick(near.len())], ks[p]);
            }
            1 if ks.len() > 1 => {
                ks.remove(p);
            }
            2 => ks.insert(p, ks[p]),
            3 if p + 1 < ks.len() && ks[p] != ks[p + 1] => ks.swap(p, p + 1),
            _ => return None,
        }
        Some(ks.into_iter().collect())
    }
}

/// Bỏ các w (móc/trăng) rồi gõ một w sau cùng, trước phím thanh nếu có.
fn late_w(keys: &str, toned: bool) -> Option<String> {
    let w = keys.chars().find(|c| c.eq_ignore_ascii_case(&'w'))?;
    let mut ks: Vec<char> = keys.chars().filter(|c| !c.eq_ignore_ascii_case(&'w')).collect();
    let at = if toned { ks.len() - 1 } else { ks.len() };
    ks.insert(at, w);
    Some(ks.into_iter().collect())
}

/// Chuyển phím thanh (cuối từ) tới ngay sau nguyên âm cuối (kèm w của nó), trước phụ âm cuối.
fn mid_tone(keys: &str) -> Option<String> {
    let mut ks: Vec<char> = keys.chars().collect();
    let tone = ks.pop()?;
    let last_vowel = ks.iter().rposition(|c| "aeiouy".contains(c.to_ascii_lowercase()))?;
    let mut at = last_vowel + 1;
    if ks.get(at).is_some_and(|c| c.eq_ignore_ascii_case(&'w')) {
        at += 1;
    }
    if at == ks.len() {
        return None;
    }
    ks.insert(at, tone);
    Some(ks.into_iter().collect())
}

/// Số từ được chấm trong chuỗi chữ đích (bỏ từ có `SKIP`).
pub fn scored_words(want: &str) -> usize {
    want.split_whitespace().filter(|w| !w.contains(SKIP)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_keys(t: &mut Typist, w: &str) -> String {
        match &t.line(w)[0] {
            Unit::Word { keys, .. } => keys.clone(),
            u => panic!("{u:?}"),
        }
    }

    #[test]
    fn habits_keep_the_same_text() {
        assert_eq!(mid_tone("tieengs").as_deref(), Some("tieesng"));
        assert_eq!(mid_tone("dduowcj").as_deref(), Some("dduowjc"));
        assert_eq!(mid_tone("hoaf"), None);
        assert_eq!(late_w("truwowngf", true).as_deref(), Some("truongwf"));
        let mut t = Typist::new(Habits { late_hat: 1.0, ..Default::default() }, 2);
        assert_eq!(t.move_later("thaays", 3, true).as_deref(), Some("thayas"));
        assert_eq!(t.move_later("caanf", 2, true).as_deref(), Some("canaf"));
        assert_eq!(t.move_later("mas", 1, true), None);
        let all = Habits { mid_tone: 1.0, late_w: 1.0, late_hat: 1.0, late_d: 1.0, ..Default::default() };
        for seed in 0..20 {
            let mut t = Typist::new(all, seed);
            for w in ["tiếng", "được", "trường", "Người", "VIỆT", "hoặc", "khuỷu", "thấy", "đâu", "cuối", "cần", "Điểm", "ok"] {
                assert_eq!(telex(&word_keys(&mut t, w), true), w, "{w}");
            }
        }
    }

    #[test]
    fn fixed_typo_is_erased() {
        let mut t = Typist::new(Habits { fixed_typo: 1.0, ..Default::default() }, 3);
        for _ in 0..20 {
            let k = word_keys(&mut t, "trường");
            let bs = k.find(BS).unwrap();
            let mut clean: Vec<char> = k.chars().collect();
            let i = k[..bs].chars().count();
            clean.drain(i - 1..=i);
            assert_eq!(clean.into_iter().collect::<String>(), "truwowngf");
        }
    }

    #[test]
    fn typos_are_not_scored() {
        let mut t = Typist::new(Habits { typo: 1.0, ..Default::default() }, 5);
        let units = t.line("mình check, ok");
        let want: String = units
            .iter()
            .map(|u| match u {
                Unit::Word { want, .. } => want.as_str(),
                Unit::Sep(s) => s.as_str(),
            })
            .collect();
        assert_eq!(scored_words(&want), 0);
        assert_eq!(scored_words("mình check, ok"), 3);
    }

    #[test]
    fn splits_words_and_separators() {
        let mut t = Typist::new(Habits::default(), 1);
        let u = t.line("Việt-Nam 4090!");
        assert_eq!(u[0], Unit::Word { keys: "Vieetj".into(), want: "Việt".into() });
        assert_eq!(u[1], Unit::Sep("-".into()));
        assert_eq!(u[2], Unit::Word { keys: "Nam".into(), want: "Nam".into() });
        assert_eq!(u.len(), 3 + 1 + 4 + 1);
    }
}
