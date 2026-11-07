//! Bộ nhớ riêng của người gõ: học từ mỗi lần người dùng lật từ engine vừa chọn ("visa" -> "vía").
//! - Theo ngữ cảnh: cặp (từ trước, từ được chọn) cộng vào bigram với trọng số lớn, nên cùng ngữ cảnh lần sau tự đúng.
//! - Theo từ: tỉ lệ số lần được chọn / bị bỏ, đẩy nhẹ ở mọi ngữ cảnh khác.

use std::collections::HashMap;

use crate::context::Follow;
use crate::lexicon::VnLex;

#[derive(Default)]
pub struct Memory {
    follow: HashMap<String, Follow>,
    chosen: HashMap<String, f64>,
    rejected: HashMap<String, f64>,
}

impl Memory {
    /// Người dùng bỏ `rejected`, chọn `chosen` ngay sau `prev` (đều ở dạng khoá `canon`).
    pub fn learn(&mut self, prev: &str, rejected: &str, chosen: &str, vn: &VnLex) {
        self.follow.entry(prev.to_string()).or_default().add(chosen, 1.0, vn);
        *self.chosen.entry(chosen.to_string()).or_default() += 1.0;
        *self.rejected.entry(rejected.to_string()).or_default() += 1.0;
    }

    /// Huỷ một lần `learn` cùng tham số (người dùng lật nhầm rồi trả lại ngay).
    pub fn unlearn(&mut self, prev: &str, rejected: &str, chosen: &str) {
        if let Some(f) = self.follow.get_mut(prev) {
            f.remove(chosen, 1.0);
            if f.succ.is_empty() {
                self.follow.remove(prev);
            }
        }
        for (m, k) in [(&mut self.chosen, chosen), (&mut self.rejected, rejected)] {
            if let Some(n) = m.get_mut(k) {
                *n -= 1.0;
                if *n <= 0.0 {
                    m.remove(k);
                }
            }
        }
    }

    pub fn follow(&self, prev: &str) -> Option<&Follow> {
        self.follow.get(prev)
    }

    /// Hệ số ưu tiên cho từ ở mọi ngữ cảnh: (1 + số lần được chọn) / (1 + số lần bị bỏ).
    pub fn preference(&self, key: &str) -> f64 {
        (1.0 + self.chosen.get(key).copied().unwrap_or(0.0)) / (1.0 + self.rejected.get(key).copied().unwrap_or(0.0))
    }

    pub fn is_empty(&self) -> bool {
        self.chosen.is_empty()
    }

    /// Số từ đã học (được chọn ít nhất một lần khi lật).
    pub fn len(&self) -> usize {
        self.chosen.len()
    }

    /// Lưu dạng TSV: "pair<TAB>trước<TAB>từ<TAB>số" và "pref<TAB>từ<TAB>chọn<TAB>bỏ".
    pub fn save(&self) -> String {
        let mut out = String::new();
        for (p, f) in &self.follow {
            for (w, n) in &f.succ {
                out += &format!("pair\t{p}\t{}\t{n}\n", w.key);
            }
        }
        let keys: std::collections::BTreeSet<&String> = self.chosen.keys().chain(self.rejected.keys()).collect();
        for k in keys {
            let c = self.chosen.get(k).copied().unwrap_or(0.0);
            let r = self.rejected.get(k).copied().unwrap_or(0.0);
            out += &format!("pref\t{k}\t{c}\t{r}\n");
        }
        out
    }

    pub fn load(text: &str, vn: &VnLex) -> Memory {
        let mut m = Memory::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["pair", p, w, n] => m.follow.entry(p.to_string()).or_default().add(w, n.parse().unwrap_or(0.0), vn),
                ["pref", w, c, r] => {
                    m.chosen.insert(w.to_string(), c.parse().unwrap_or(0.0));
                    m.rejected.insert(w.to_string(), r.parse().unwrap_or(0.0));
                }
                _ => {}
            }
        }
        m.chosen.retain(|_, v| *v > 0.0);
        m.rejected.retain(|_, v| *v > 0.0);
        m
    }
}

/// Một từ trong bộ nhớ riêng, đọc thẳng từ file (cửa sổ cài đặt xem/xoá mà không cần từ điển).
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    /// Khoá `canon` ("via1"); hiện ra bằng `context::key_to_word`.
    pub key: String,
    pub chosen: f64,
    pub rejected: f64,
    /// Từ đứng trước (khoá `canon`, `<s>` là đầu câu) và số lần được chọn sau nó.
    pub contexts: Vec<(String, f64)>,
}

/// Các từ trong file bộ nhớ, từ được chọn nhiều nhất trước.
pub fn learned(text: &str) -> Vec<Learned> {
    let mut by_key: std::collections::BTreeMap<String, Learned> = Default::default();
    let entry = |m: &mut std::collections::BTreeMap<String, Learned>, k: &str| {
        m.entry(k.to_string()).or_insert_with(|| Learned { key: k.to_string(), chosen: 0.0, rejected: 0.0, contexts: vec![] }).clone()
    };
    for line in text.lines() {
        match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["pref", w, c, r] => {
                let mut e = entry(&mut by_key, w);
                (e.chosen, e.rejected) = (c.parse().unwrap_or(0.0), r.parse().unwrap_or(0.0));
                by_key.insert(w.to_string(), e);
            }
            ["pair", p, w, n] => {
                let mut e = entry(&mut by_key, w);
                e.contexts.push((p.to_string(), n.parse().unwrap_or(0.0)));
                by_key.insert(w.to_string(), e);
            }
            _ => {}
        }
    }
    let mut out: Vec<Learned> = by_key.into_values().filter(|e| e.chosen > 0.0 || e.rejected > 0.0 || !e.contexts.is_empty()).collect();
    for e in &mut out {
        e.contexts.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }
    out.sort_by(|a, b| b.chosen.total_cmp(&a.chosen).then_with(|| a.rejected.total_cmp(&b.rejected)).then_with(|| a.key.cmp(&b.key)));
    out
}

/// Bỏ mọi dòng về từ `key` (ưu tiên và các cặp ngữ cảnh chọn nó), giữ nguyên phần còn lại của file.
pub fn forget_key(text: &str, key: &str) -> String {
    text.lines()
        .filter(|line| !matches!(line.split('\t').collect::<Vec<_>>().as_slice(), ["pref", w, _, _] | ["pair", _, w, _] if *w == key))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "pair\t<s>\tvia1\t2\npair\tthôi\tvia1\t1\npair\t<s>\tvisa\t1\npref\tvia1\t3\t1\npref\tvisa\t1\t3\n";

    #[test]
    fn learned_groups_contexts_under_each_word() {
        let l = learned(FILE);
        assert_eq!(l.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["via1", "visa"]);
        assert_eq!((l[0].chosen, l[0].rejected), (3.0, 1.0));
        assert_eq!(l[0].contexts, vec![("<s>".to_string(), 2.0), ("thôi".to_string(), 1.0)]);
    }

    #[test]
    fn forgetting_a_word_keeps_the_others() {
        let rest = forget_key(FILE, "via1");
        assert_eq!(rest, "pair\t<s>\tvisa\t1\npref\tvisa\t1\t3\n");
        assert_eq!(learned(&rest).len(), 1);
    }
}
