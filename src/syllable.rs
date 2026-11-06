//! Âm tiết tiếng Việt: thanh điệu, đặt dấu, và tập âm tiết hợp lệ sinh từ luật ghép vần.

use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Tone {
    #[default]
    None,
    Sac,
    Huyen,
    Hoi,
    Nga,
    Nang,
}

impl Tone {
    pub const ALL: [Tone; 6] = [Tone::None, Tone::Sac, Tone::Huyen, Tone::Hoi, Tone::Nga, Tone::Nang];

    pub fn from_key(c: char) -> Option<Tone> {
        match c {
            's' => Some(Tone::Sac),
            'f' => Some(Tone::Huyen),
            'r' => Some(Tone::Hoi),
            'x' => Some(Tone::Nga),
            'j' => Some(Tone::Nang),
            _ => None,
        }
    }

    pub fn idx(self) -> usize {
        self as usize
    }
}

// Mỗi hàng: nguyên âm không dấu thanh, rồi sắc, huyền, hỏi, ngã, nặng (đúng thứ tự Tone).
const VOWEL_ROWS: [&str; 12] = [
    "aáàảãạ", "ăắằẳẵặ", "âấầẩẫậ", "eéèẻẽẹ", "êếềểễệ", "iíìỉĩị", "oóòỏõọ", "ôốồổỗộ", "ơớờởỡợ", "uúùủũụ", "ưứừửữự",
    "yýỳỷỹỵ",
];

/// (hàng, thanh) của một nguyên âm; bảng xếp sẵn để tra nhị phân (gọi mỗi ký tự khi chốt từ).
fn row(c: char) -> Option<(usize, usize)> {
    if c.is_ascii() && !matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
        return None;
    }
    static TABLE: std::sync::OnceLock<Vec<(char, u8, u8)>> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t: Vec<(char, u8, u8)> = VOWEL_ROWS
            .iter()
            .enumerate()
            .flat_map(|(r, s)| s.chars().enumerate().map(move |(p, x)| (x, r as u8, p as u8)))
            .collect();
        t.sort_unstable();
        t
    });
    let i = t.binary_search_by(|e| e.0.cmp(&c)).ok()?;
    Some((t[i].1 as usize, t[i].2 as usize))
}

/// Nguyên âm (đã có mũ/móc, chưa có thanh).
pub fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'ă' | 'â' | 'e' | 'ê' | 'i' | 'o' | 'ô' | 'ơ' | 'u' | 'ư' | 'y')
}

pub fn with_tone(c: char, t: Tone) -> char {
    match row(c) {
        Some((r, 0)) => VOWEL_ROWS[r].chars().nth(t.idx()).unwrap(),
        _ => c,
    }
}

/// 'ắ' -> ('ă', Sac); ký tự không phải nguyên âm giữ nguyên.
pub fn split_tone(c: char) -> (char, Tone) {
    match row(c) {
        Some((r, p)) => (VOWEL_ROWS[r].chars().next().unwrap(), Tone::ALL[p]),
        None => (c, Tone::None),
    }
}

/// Bỏ mũ/móc/gạch: ă â -> a, ê -> e, ô ơ -> o, ư -> u, đ -> d.
pub fn strip_mark(c: char) -> char {
    match c {
        'ă' | 'â' => 'a',
        'ê' => 'e',
        'ô' | 'ơ' => 'o',
        'ư' => 'u',
        'đ' => 'd',
        _ => c,
    }
}

/// Vị trí đặt dấu thanh trong âm tiết (chưa thanh). `modern`: hoà/khoẻ/thuỷ thay vì hòa/khỏe/thủy.
pub fn tone_pos(s: &[char], modern: bool) -> Option<usize> {
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
    let ve = i;
    if vs == ve {
        return None;
    }
    let v = &s[vs..ve];
    if let Some(k) = v.iter().position(|&c| c == 'ơ') {
        return Some(vs + k);
    }
    if let Some(k) = v.iter().position(|&c| matches!(c, 'ă' | 'â' | 'ê' | 'ô' | 'ư')) {
        return Some(vs + k);
    }
    if ve < n {
        return Some(ve - 1);
    }
    Some(match v.len() {
        1 => vs,
        2 if modern && matches!(v, ['o', 'a'] | ['o', 'e'] | ['u', 'y']) => vs + 1,
        2 => vs,
        _ => vs + 1,
    })
}

/// Ghép thanh vào âm tiết chưa thanh (chữ thường).
pub fn apply_tone(s: &[char], t: Tone, modern: bool) -> Vec<char> {
    let mut out = s.to_vec();
    if t != Tone::None {
        if let Some(p) = tone_pos(s, modern) {
            out[p] = with_tone(out[p], t);
        }
    }
    out
}

/// Tách một từ (chữ thường, NFC) thành (âm tiết chưa thanh, thanh). None nếu có hơn một dấu thanh.
pub fn decompose(word: &str) -> Option<(String, Tone)> {
    let mut tone = Tone::None;
    let mut out = String::with_capacity(word.len());
    for c in word.chars() {
        let (b, t) = split_tone(c);
        if t != Tone::None {
            if tone != Tone::None {
                return None;
            }
            tone = t;
        }
        out.push(b);
    }
    Some((out, tone))
}

const ONSETS: [&str; 28] = [
    "", "b", "c", "ch", "d", "đ", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh", "p", "ph", "qu",
    "r", "s", "t", "th", "tr", "v", "x",
];

// Vần (chưa thanh). Vần "yê…" chỉ đứng không có âm đầu; có âm đầu viết "iê…".
const RHYMES: &str = "a ai ao au ay am an ang anh ap at ac ach \
    ăm ăn ăng ăp ăt ăc âm ân âng âp ât âc âu ây \
    e eo em en eng ep et ec ê êu êm ên ênh êp êt êch \
    i ia iu im in inh ip it ich iêu iêm iên iêng iêp iêt iêc y yêu yêm yên yêt \
    o oi om on ong op ot oc oong ooc oa oai oay oan oang oanh oat oac oach oam oăm oăn oăng oăt oăc \
    oe oeo oen oet ô ôi ôm ôn ông ôp ôt ôc ơ ơi ơm ơn ơp ơt \
    u ua ui um un ung up ut uc uân uâng uât uây uê uênh uêch uôi uôm uôn uông uôt uôc uơ \
    uy uya uyu uynh uyt uych uyên uyêt ư ưa ưi ưu ưm ưng ưt ưc ươi ươu ươm ươn ương ươp ươt ươc";

fn front(r: &str) -> bool {
    matches!(r.chars().next(), Some('e' | 'ê' | 'i' | 'y'))
}

fn medial_w(r: &str) -> bool {
    ["oa", "oă", "oe", "uy", "uâ", "uê", "uơ"].iter().any(|p| r.starts_with(p))
}

/// Vần kết thúc bằng phụ âm tắc (p t c ch): chỉ đi với sắc hoặc nặng.
pub fn stop_coda(s: &str) -> bool {
    s.ends_with('p') || s.ends_with('t') || s.ends_with('c') || s.ends_with("ch")
}

pub fn tone_allowed(syllable: &str, t: Tone) -> bool {
    !stop_coda(syllable) || matches!(t, Tone::Sac | Tone::Nang)
}

/// Mọi âm tiết chưa thanh hợp lệ theo luật ghép vần (dư một ít, tần suất sẽ lọc).
pub fn generate() -> HashMap<String, ()> {
    let mut out = HashMap::new();
    let rhymes: Vec<&str> = RHYMES.split_whitespace().collect();
    for &o in ONSETS.iter() {
        for &r in &rhymes {
            let yv = r.starts_with('y') && r.len() > 1; // yê… chỉ khi không có âm đầu
            let ok = match o {
                "" => !r.starts_with("iê"),
                "c" | "g" | "ng" => !yv && !front(r) && !(o == "c" && medial_w(r)),
                "k" | "gh" | "ngh" => !yv && front(r) && !medial_w(r),
                "qu" => !yv && !matches!(r.chars().next(), Some('o' | 'u' | 'ư')),
                "gi" => !yv && !r.starts_with('i') && !r.starts_with('y') && !medial_w(r),
                _ => !yv,
            };
            if ok {
                out.insert(format!("{o}{r}"), ());
            }
        }
    }
    for &r in &rhymes {
        if r.starts_with("uy") || r.starts_with("uâ") || r.starts_with("uê") || r.starts_with("uô") || r.starts_with("uơ") {
            out.insert(format!("q{r}"), ()); // quý, quân, quốc, quyết
        }
        if let Some(rest) = r.strip_prefix("iê") {
            out.insert(format!("giê{rest}"), ()); // giếng, giết
        }
        if r.starts_with('i') {
            out.insert(format!("g{r}"), ()); // gì, gìn
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(s: &str, t: Tone) -> String {
        apply_tone(&s.chars().collect::<Vec<_>>(), t, true).into_iter().collect()
    }

    #[test]
    fn places_tone() {
        assert_eq!(render("hoa", Tone::Huyen), "hoà");
        assert_eq!(render("truong", Tone::Huyen), "truòng");
        assert_eq!(render("trương", Tone::Huyen), "trường");
        assert_eq!(render("qua", Tone::Sac), "quá");
        assert_eq!(render("gia", Tone::Sac), "giá");
        assert_eq!(render("gi", Tone::Huyen), "gì");
        assert_eq!(render("khuya", Tone::Sac), "khuýa");
        assert_eq!(render("mua", Tone::Sac), "múa");
        assert_eq!(render("tiêng", Tone::Sac), "tiếng");
        assert_eq!(render("hoang", Tone::Huyen), "hoàng");
        assert_eq!(render("quyêt", Tone::Sac), "quyết");
    }

    #[test]
    fn generates_common_syllables() {
        let g = generate();
        for s in ["trương", "nghiêng", "quyêt", "giêng", "gin", "yêu", "khuya", "ngoai", "xoong", "đươc", "kê", "huơ"] {
            assert!(g.contains_key(s), "{s}");
        }
        assert!(!g.contains_key("ce"));
        assert!(!g.contains_key("ka"));
        assert!(!g.contains_key("iên"));
        assert!(g.contains_key("ghe"));
    }

    #[test]
    fn decomposes() {
        assert_eq!(decompose("trường"), Some(("trương".into(), Tone::Huyen)));
        assert_eq!(decompose("anh"), Some(("anh".into(), Tone::None)));
    }
}
