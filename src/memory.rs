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
