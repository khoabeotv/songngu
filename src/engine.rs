//! Phiên gõ: chạy song song nhánh Việt (Telex) và nhánh Anh (phím gốc), chọn chữ hiển thị sau mỗi phím,
//! chốt ở ranh giới từ. Không có "chế độ": từ trước chỉ là ngữ cảnh.
//!
//! Điểm mỗi nhánh = P(chữ | từ trước), với
//!   P(w | prev) = λ·C(prev, w)/C(prev) + (1-λ)·P(tiếng | tiếng trước)·P(w | tiếng)
//!   P(w | tiếng) = μ·tần suất trong miền + (1-μ)·tần suất chung
//! Giữa từ dùng cùng công thức cho tiền tố (cộng mọi từ còn gõ tiếp thành được).

use crate::context::{after_separator, all_caps, canon, case_class, join_class, key_to_word, Context, Successors, START};
use crate::syllable::decompose;
use unicode_normalization::UnicodeNormalization;
use crate::lexicon::{compatible_prefix, EnLex, VnLex};
use crate::memory::Memory;
use crate::telex::{keys_for, Composer, TelexOpts};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Vn,
    En,
}

fn other(l: Lang) -> Lang {
    if l == Lang::Vn {
        Lang::En
    } else {
        Lang::Vn
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// hoà/khoẻ/thuỷ (true) hay hòa/khỏe/thủy.
    pub modern: bool,
    /// Không có ngữ cảnh học: P(từ này cùng thứ tiếng với từ trước).
    pub stick: f64,
    /// Giữa từ, chỉ đổi chữ đang hiện khi bên kia hơn ít nhất ngần này lần (giảm nhảy chữ).
    pub flip_ratio: f64,
    /// μ: trọng số tần suất trong miền so với tần suất chung.
    pub dom_weight: f64,
    /// Dùng bigram (false: chỉ tần suất + xác suất đổi tiếng).
    pub bigram: bool,
    /// γ: trọng số tỉ lệ Anh/Việt của chính những từ vừa gõ so với xác suất đổi tiếng học từ corpus.
    /// Người hay viết câu tiếng Anh (chat) và bài báo gần như thuần Việt cần tiên nghiệm khác nhau.
    pub cache_weight: f64,
    /// Ghi nhớ lựa chọn khi người dùng lật từ.
    pub learn: bool,
    /// Xem lại từ trước bằng từ sau có tính xác suất đổi tiếng (xem `next_support`).
    pub lookahead_switch: bool,
    /// Tính kiểu nối với từ trước (`join_class`).
    pub join: bool,
    /// Âm tiết đúng luật chưa gặp bao giờ tính như gặp nghần này lần trong danh sách tần suất (0 = không phải từ).
    pub unseen_vn: f64,
    /// Độ mạnh làm mượt xác suất đổi tiếng theo từng từ trước (`lang_after`); 0 = chỉ theo tiếng hai từ trước.
    pub word_switch: f64,
    pub telex: TelexOpts,
    /// Nhân xác suất tiếng Anh (Terminal, IDE: > 1).
    /// Nhân xác suất tiếng Anh (chỉ để đo; bộ gõ dùng 1 ở mọi app, kể cả terminal: xem `docs/evaluation.md`).
    pub en_bias: f64,
    /// Phạt mỗi thao tác người gõ Telex hiếm làm (xem `order_penalty`).
    pub odd_penalty: f64,
    /// Phạt mỗi lần đặt mũ muộn (Composer::late). Dev: 0,01 cân giữa từ Anh trùng dạng (theme, Cocos) và người quen gõ dấu sau.
    pub late_penalty: f64,
    /// Phạt đ đặt bằng d gõ muộn (Composer::late_d). Nhẹ hơn mũ: ít từ Anh trùng dạng, còn "did" cho "đi" là thói quen
    /// phổ biến. Dev: 0,01/0,03/0,1/0,3/1 cho 351/347/341/340/340 từ sai, cột gõ chuẩn không đổi.
    pub late_d_penalty: f64,
    /// Từ được tính là "nói rõ thứ tiếng" khi hai phương án chênh nhau hơn nghần này lần.
    pub informative_ratio: f64,
    /// Học thói quen gõ mũ/đ muộn của từng người (`Memory::late_rate`) thay vì chỉ dùng mức phạt chung.
    pub adapt: bool,
    /// Lúc chốt từ, hai phương án chênh dưới ngần này lần thì giữ phương án đang hiện (1 tắt). Dev: 1/1,5/2/2,5/3/4/10
    /// cho 317/316/307/307/309/317/328 từ sai.
    pub commit_stick: f64,
    /// Phạt từ Việt sửa từ hai phím gõ đảo (`swap_repair`); 0 tắt. Dev: 1e-3/1e-5/1e-7 cho 345/332/332 từ sai khi gõ
    /// thường (tắt: 334; mạnh tay thì itm -> tim, ib -> bi) và 733/789/999 khi 5% từ có dấu gõ đảo (tắt: 3659).
    pub swap_penalty: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modern: true,
            stick: 0.8,
            flip_ratio: 4.0,
            dom_weight: 0.5,
            bigram: true,
            cache_weight: 0.3,
            learn: true,
            lookahead_switch: true,
            join: true,
            unseen_vn: 0.0003,
            word_switch: 10.0,
            telex: TelexOpts::default(),
            en_bias: 1.0,
            odd_penalty: 0.05,
            late_penalty: 0.01,
            late_d_penalty: 0.3,
            informative_ratio: 10.0,
            adapt: true,
            swap_penalty: 1e-5,
            commit_stick: 2.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct View {
    pub text: String,
    pub lang: Lang,
    pub vn_text: String,
    pub en_text: String,
    pub vn_p: Option<f64>,
    pub en_p: Option<f64>,
}

impl View {
    pub fn differs(&self) -> bool {
        self.vn_text != self.en_text
    }
}

pub struct Engine<'a> {
    vn: &'a VnLex,
    en: &'a EnLex,
    ctx: Option<&'a Context>,
    pub cfg: Config,
    prev: String,
    /// Số hiệu của `prev` trong bảng cặp từ (tra một lần khi `prev` đổi, không phải mỗi phím).
    prev_id: Option<u32>,
    prev_lang: Option<Lang>,
    prev2_lang: Option<Lang>,
    /// Thứ tiếng của các từ gần đây (chỉ từ nói rõ thứ tiếng), mới nhất ở cuối.
    recent: std::collections::VecDeque<Lang>,
    keys: String,
    comp: Composer,
    shown: Option<Lang>,
    /// Người dùng đã ép thứ tiếng cho từ đang gõ (`toggle_current`).
    force: Option<Lang>,
    memory: Memory,
    /// Từ vừa chốt, còn lật được cho tới khi gõ chữ của từ sau.
    last: Option<Last>,
    /// Ký tự ngăn cách gõ sau từ vừa chốt, chưa áp vào ngữ cảnh: tới phím đầu của từ sau mới biết chúng dính liền hai
    /// từ (Nest.js: không phải hết câu) hay không. Khoảng trắng thì áp ngay.
    run: String,
    /// `run` bắt đầu ngay sau một từ.
    after_word: bool,
    /// Kiểu nối của từ đang gõ với từ trước (`join_class`).
    join: usize,
    /// Từ trước viết hoa toàn bộ.
    prev_caps: bool,
}

struct Last {
    vn_text: String,
    en_text: String,
    lang: Lang,
    /// Ngữ cảnh trước từ này (khoá `canon` của từ trước nữa).
    prev: String,
    /// Đã được tính vào `recent`.
    counted: bool,
    /// Thứ tiếng engine tự chọn, và bộ nhớ đang ghi người dùng chọn phương án kia cho từ này. Lật qua lại nhiều lần
    /// chỉ giữ trạng thái cuối: đang hiện khác `natural` thì học, quay về thì bỏ (không học cả hai chiều).
    natural: Lang,
    learned: bool,
    /// Cả hai phương án đều là từ có trong từ điển (không phải âm tiết/từ chưa gặp).
    both_real: bool,
    /// Cách gõ phương án Việt: (có â/ê/ô, mũ gõ muộn, có đ, đ gõ muộn). Tính vào thói quen khi từ đã thôi lật được.
    habit: (bool, bool, bool, bool),
    /// Phương án Việt do sửa gõ đảo phím (`swap_repair`): lật bỏ nó thì không trừ điểm từ Việt đó (`learn_pair`).
    repaired: bool,
}

/// Mỗi lần người dùng lật từ bằng ngang ngần này lần gặp cặp từ đó trong corpus: một lần sửa đã đủ để lần sau
/// cùng ngữ cảnh tự đúng, trừ khi corpus rất chắc chắn điều ngược lại.
const PERSONAL: f64 = 20.0;

/// Số từ gần nhất dùng để ước tỉ lệ Anh/Việt của người đang gõ.
const RECENT: usize = 20;

/// Số từ cuối của chữ trước con trỏ được tính vào tiếng gần đây khi lấy lại ngữ cảnh (`resume`).
const RESUME_WORDS: usize = 8;

/// Trộn hai nguồn tần suất; None khi cả hai không biết từ/tiền tố này.
fn mix(dom: Option<f64>, sub: Option<f64>, mu: f64) -> Option<f64> {
    (dom.is_some() || sub.is_some()).then(|| mu * dom.unwrap_or(0.0) + (1.0 - mu) * sub.unwrap_or(0.0))
}

impl<'a> Engine<'a> {
    pub fn new(vn: &'a VnLex, en: &'a EnLex, cfg: Config) -> Self {
        let cfg_telex = cfg.telex;
        Engine {
            vn,
            en,
            ctx: None,
            cfg,
            prev: START.into(),
            prev_id: None,
            prev_lang: None,
            prev2_lang: None,
            recent: std::collections::VecDeque::new(),
            keys: String::new(),
            comp: Composer::new(cfg_telex),
            shown: None,
            force: None,
            memory: Memory::default(),
            last: None,
            run: String::new(),
            after_word: false,
            join: 0,
            prev_caps: false,
        }
    }

    /// Áp các ký tự ngăn cách đang hoãn vào ngữ cảnh (kết câu -> START, số -> NUM).
    fn flush_run(&mut self) {
        for c in std::mem::take(&mut self.run).chars() {
            if let Some(p) = after_separator(c) {
                if self.prev != p {
                    self.set_prev(p.into());
                }
            }
        }
        self.after_word = false;
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn set_memory(&mut self, m: Memory) {
        self.memory = m;
    }

    pub fn with_context(mut self, ctx: &'a Context) -> Self {
        self.ctx = Some(ctx);
        let p = self.prev.clone();
        self.set_prev(p);
        self
    }

    /// Đổi từ trước (khoá `canon`), trả từ trước cũ.
    fn set_prev(&mut self, key: String) -> String {
        self.prev_id = self.ctx.and_then(|c| c.word_id(&key));
        std::mem::replace(&mut self.prev, key)
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn mu(&self) -> f64 {
        if self.ctx.is_some() {
            self.cfg.dom_weight
        } else {
            0.0
        }
    }

    // ---- P(w | tiếng) ----

    fn vn_word(&self) -> Option<f64> {
        if self.comp.undone {
            return None;
        }
        let (s, t) = (self.comp.toneless(), self.comp.tone);
        mix(self.ctx.and_then(|c| c.vn.word_prob(&s, t)), self.vn.word_prob(&s, t), self.mu())
            // âm tiết hiếm (đờ, giúm, xuê) có thật nhưng ngoài phụ đề và corpus: vẫn còn là phương án, để ngữ cảnh quyết.
            // Chỉ khi có ngữ cảnh: không có thì từ Anh lạ cũng không có xác suất, ROAS sẽ thành ROÁ.
            .or_else(|| self.ctx.and(self.vn.unseen_prob(&s, t, self.cfg.unseen_vn)))
            .or_else(|| self.personal_vn())
    }

    /// Dạng không phải âm tiết Việt mà người dùng đã lật sang (ừk, đzai, ák): ai đã viết kiểu đó thì hay viết lại,
    /// nên coi như một từ đã gặp `PERSONAL` lần mỗi lần lật. Chỉ khớp cả từ đang gõ (đang gõ dở "đz" thì chưa).
    fn personal_vn(&self) -> Option<f64> {
        let n = self.memory.chosen(&canon(&self.vn_text()));
        let total = self.ctx.map_or(self.vn.total(), |c| c.vn.total());
        (n > 0.0 && !self.comp.undone).then(|| PERSONAL * n / total)
    }

    /// Từ Việt đang gõ có trong từ điển (`vn_word` không tính phần âm tiết chưa gặp).
    fn vn_known(&self) -> bool {
        let (s, t) = (self.comp.toneless(), self.comp.tone);
        !self.comp.undone && (self.ctx.is_some_and(|c| c.vn.word_prob(&s, t).is_some()) || self.vn.word_prob(&s, t).is_some())
    }

    fn vn_prefix(&self, s: &[char], t: crate::syllable::Tone) -> Option<f64> {
        mix(self.ctx.and_then(|c| c.vn.prefix_prob(s, t)), self.vn.prefix_prob(s, t), self.mu())
    }

    fn en_word(&self, w: &str) -> Option<f64> {
        mix(self.ctx.and_then(|c| c.en.word_prob(w)), self.en.word_prob(w), self.mu())
    }

    fn en_prefix(&self, w: &str) -> Option<f64> {
        mix(self.ctx.and_then(|c| c.en.prefix_prob(w)), self.en.prefix_prob(w), self.mu())
    }

    /// Nhánh Anh: phím gốc; nếu đã gõ lặp để huỷ thì thử thêm bản đã gộp ("isssue" -> "issue").
    /// Không phải từ đã biết: giữ đúng phím đã gõ (Asus, mirrorless), xác suất theo mô hình từ chưa gặp.
    /// Trả (chữ hiện, chữ thường của nó, xác suất).
    fn en_side(&self, prob: impl Fn(&str) -> Option<f64>, whole: bool) -> (String, String, Option<f64>) {
        let low = self.keys.to_ascii_lowercase();
        if let Some(p) = prob(&low) {
            return (self.keys.clone(), low, Some(p));
        }
        if self.comp.undone {
            let lit = self.comp.literal();
            let lit_low = lit.to_ascii_lowercase();
            if let Some(p) = prob(&lit_low) {
                return (lit, lit_low, Some(p));
            }
        }
        let oov = self.ctx.map(|c| c.en_unseen).map(|u| u * self.en.chars.prob(&low, whole));
        (self.keys.clone(), low, oov.filter(|&p| p > 0.0))
    }

    /// P(phím | từ Việt) thô: người gõ Telex hiếm khi đổi dấu thanh giữa chừng hay thêm mũ/móc sau dấu thanh
    /// ("these" -> thế, "CORS" -> CÓ), nên chuỗi phím kiểu đó nghiêng về tiếng Anh.
    fn order_penalty(&self) -> f64 {
        self.order_penalty_of(&self.comp)
    }

    fn order_penalty_of(&self, comp: &Composer) -> f64 {
        let (late, late_d) = if self.cfg.adapt {
            (self.memory.late_rate(self.cfg.late_penalty), self.memory.late_d_rate(self.cfg.late_d_penalty))
        } else {
            (self.cfg.late_penalty, self.cfg.late_d_penalty)
        };
        let d = if comp.late_d { late_d } else { 1.0 };
        self.cfg.odd_penalty.powi(comp.odd as i32) * late.powi(comp.late as i32) * d
    }

    /// Phím gõ không thành âm tiết Việt: thử đảo hai phím liền nhau (nyaf -> nayf "này", ddwang -> ddawng "đăng"). Chỉ
    /// nhận từ có trong từ điển; trả (chữ, xác suất đã nhân `swap_penalty`) của cách đảo khá nhất. Gõ lặp để huỷ dấu
    /// (thiss) là cố ý, không sửa.
    fn swap_repair(&self) -> Option<(String, f64)> {
        // viết tắt toàn hoa (CSO) là gõ đúng như vậy
        if self.cfg.swap_penalty <= 0.0 || self.comp.undone || all_caps(&self.keys) {
            return None;
        }
        let ks: Vec<char> = self.keys.chars().collect();
        let mut best: Option<(String, f64)> = None;
        for i in 0..ks.len().saturating_sub(1) {
            if ks[i].eq_ignore_ascii_case(&ks[i + 1]) || !ks[i].is_alphabetic() || !ks[i + 1].is_alphabetic() {
                continue;
            }
            let mut c = Composer::new(self.cfg.telex);
            for (j, &k) in ks.iter().enumerate() {
                c.push(if j == i { ks[i + 1] } else if j == i + 1 { ks[i] } else { k });
            }
            let (s, t) = (c.toneless(), c.tone);
            let Some(u) = mix(self.ctx.and_then(|x| x.vn.word_prob(&s, t)), self.vn.word_prob(&s, t), self.mu()) else {
                continue;
            };
            // đổi dấu thanh giữa chừng (xof -> oxf "ò") không phải một từ gõ đúng bị đảo phím
            if c.undone || c.odd > 0 {
                continue;
            }
            let text = c.render(self.cfg.modern);
            let key = canon(&text);
            let p = self.cfg.swap_penalty
                * self.order_penalty_of(&c)
                * self.memory.preference(&key)
                * self.contextual(Lang::Vn, u, |f| f.count(&key));
            if best.as_ref().is_none_or(|b| p > b.1) {
                best = Some((text, p));
            }
        }
        best
    }

    /// Từ vừa chốt thôi lật được: nếu cuối cùng là từ Việt thì tính cách gõ của nó vào thói quen.
    fn settle_last(&mut self) {
        if let Some(l) = self.last.take() {
            if l.lang == Lang::Vn && self.cfg.learn && self.cfg.adapt {
                let (hat, late, d, late_d) = l.habit;
                self.memory.observe(hat, late, d, late_d);
            }
        }
    }

    // ---- ngữ cảnh ----

    /// P(từ sau là tiếng `lang` | từ trước `id`, tiếng hai từ trước): số đếm theo từng từ, làm mượt về xác suất theo
    /// tiếng với độ mạnh `word_switch`. Từ riêng có thói quen riêng: sau "deal", "company" người Việt hay viết tiếp
    /// tiếng Việt ("deal sẽ close"), sau "sẽ" hay là động từ Anh.
    fn lang_after(&self, c: &Context, id: Option<u32>, prev2: Option<Lang>, prev: Option<Lang>, lang: Lang) -> f64 {
        let base = c.switch(prev2, prev, lang);
        let k = self.cfg.word_switch;
        match id.filter(|_| k > 0.0) {
            Some(id) => {
                let n = c.next_lang(id);
                let l = if lang == Lang::Vn { 0 } else { 1 };
                (n[l] + k * base) / (n[0] + n[1] + k)
            }
            None => base,
        }
    }

    fn prior(&self, lang: Lang) -> f64 {
        let bias = if lang == Lang::En { self.cfg.en_bias } else { 1.0 };
        bias * match self.ctx {
            Some(c) => {
                let n = self.recent.iter().filter(|&&l| l == lang).count() as f64;
                let cache = (n + 1.0) / (self.recent.len() as f64 + 2.0);
                let g = self.cfg.cache_weight;
                (1.0 - g) * self.lang_after(c, self.prev_id, self.prev2_lang, self.prev_lang, lang) + g * cache
            }
            None => match self.prev_lang {
                None => 0.5,
                Some(l) if l == lang => self.cfg.stick,
                Some(_) => 1.0 - self.cfg.stick,
            },
        }
    }

    /// λ·(phần bigram) + (1-λ)·prior·P(w|tiếng). `bigram_mass` nhận bảng từ theo sau, trả tổng số đếm khớp.
    /// Bảng bigram = corpus + bộ nhớ riêng (nhân PERSONAL); λ Witten-Bell tính trên bảng gộp.
    fn contextual(&self, lang: Lang, uni: f64, bigram_mass: impl Fn(&dyn Successors) -> f64) -> f64 {
        let base = self.prior(lang) * uni;
        let (mut mass, mut total, mut types) = (0.0, 0.0, 0.0);
        let corpus = self.ctx.zip(self.prev_id).and_then(|(c, id)| c.follow_id(id));
        if let Some(f) = corpus.filter(|_| self.cfg.bigram) {
            mass += bigram_mass(&f);
            total += f.total();
            types += f.types();
        }
        if let Some(f) = self.memory.follow(&self.prev) {
            mass += PERSONAL * bigram_mass(f);
            total += PERSONAL * f.total();
            types += f.types();
        }
        let p = if total > 0.0 {
            let l = total / (total + types);
            l * mass / total + (1.0 - l) * base
        } else {
            base
        };
        // Chữ hoa là bằng chứng riêng về thứ tiếng: P(w, kiểu hoa | trước) ≈ P(w | trước) · P(tiếng | kiểu hoa) / P(tiếng).
        match self.ctx {
            Some(c) => {
                let case = case_class(&self.keys, self.prev == START, self.prev_caps);
                p * c.case_ratio(case, lang) * c.join_ratio(self.join, self.prev_lang, lang)
            }
            None => p,
        }
    }

    fn vn_text(&self) -> String {
        self.comp.render(self.cfg.modern)
    }

    pub fn key(&mut self, k: char) -> View {
        if self.keys.is_empty() {
            self.settle_last();
            self.join = if self.cfg.join { join_class(&self.run, self.after_word) } else { 0 };
            if self.join == 0 {
                self.flush_run();
            } else {
                self.run.clear();
                self.after_word = false;
            }
        }
        self.keys.push(k);
        self.comp.push(k);
        let v = self.view();
        if v.differs() {
            self.shown = Some(v.lang);
        }
        v
    }

    /// Xoá chữ cuối đang hiện của từ đang gõ. Đang hiện tiếng Việt thì xoá cả các phím tạo ra chữ đó và giữ dấu
    /// ("tiếng" -> "tiến"); đang hiện phím gốc thì xoá phím cuối. None khi từ đã rỗng.
    pub fn backspace(&mut self) -> Option<View> {
        if self.keys.is_empty() {
            return None;
        }
        let v = self.view();
        let keys = if v.lang == Lang::Vn && v.differs() && !self.comp.undone {
            self.comp.canonical_keys(1, self.cfg.modern)
        } else {
            let mut k = std::mem::take(&mut self.keys);
            k.pop();
            k
        };
        self.replay(&keys);
        if self.keys.is_empty() {
            self.force = None;
            return None;
        }
        let v = self.view();
        if v.differs() {
            self.shown = Some(v.lang);
        }
        Some(v)
    }

    fn replay(&mut self, keys: &str) {
        self.keys.clear();
        self.comp = Composer::new(self.cfg.telex);
        self.shown = None;
        for k in keys.chars() {
            self.keys.push(k);
            self.comp.push(k);
        }
    }

    /// Mở lại một từ đã nằm trong app (con trỏ ngay sau nó) để gõ tiếp: "tieng" + s -> "tiếng".
    /// `prev`: từ trước nó (None = đầu câu). false nếu từ có ký tự không gõ được.
    pub fn reopen(&mut self, word: &str, prev: Option<&str>) -> bool {
        let Some(keys) = keys_for(word, self.cfg.telex.vni) else { return false };
        self.run.clear();
        self.after_word = false;
        self.join = 0;
        self.replay(&keys);
        self.set_prev(prev.map_or(START.into(), |p| if p == START { p.into() } else { canon(p) }));
        self.last = None;
        self.force = None;
        let v = self.view();
        if v.differs() {
            self.shown = Some(v.lang);
        }
        true
    }

    /// Ghi nhớ một lần người dùng lật từ không còn trong engine (từ trước con trỏ, hoàn tác lần tự đổi).
    /// Cả ba ở dạng khoá `canon`.
    pub fn learn(&mut self, prev: &str, rejected: &str, chosen: &str) {
        if self.cfg.learn {
            self.memory.learn(prev, rejected, chosen, self.vn);
        }
    }

    /// Huỷ một lần `learn` cùng tham số.
    pub fn unlearn(&mut self, prev: &str, rejected: &str, chosen: &str) {
        if self.cfg.learn {
            self.memory.unlearn(prev, rejected, chosen);
        }
    }

    pub fn forget(&mut self) {
        self.memory = Memory::default();
    }

    /// Phương án còn lại của một chữ có sẵn trong app, khi không biết phím đã gõ: chữ Latin -> Telex của chính nó
    /// ("visa" -> "vía"); chữ Việt -> phím Telex, đặt phím dấu thanh ở chỗ cho ra từ Anh có thật ("vía" -> "visa",
    /// "tét" -> "test"). None nếu không có phương án nào khác.
    pub fn alternative(&self, word: &str) -> Option<String> {
        let render = |keys: &str| {
            let mut c = Composer::new(self.cfg.telex);
            keys.chars().for_each(|k| c.push(k));
            c.render(self.cfg.modern)
        };
        if word.is_ascii() {
            let vn = render(word);
            return (word.chars().all(|c| c.is_ascii_alphabetic()) && vn != word).then_some(vn);
        }
        if self.cfg.telex.vni {
            return None;
        }
        let keys = keys_for(word, false)?;
        let mut cands = vec![keys.clone()];
        // phím chuẩn để dấu thanh cuối; người gõ từ Anh thì chữ đó nằm giữa (vi-s-a)
        if let Some(t) = keys.chars().last().filter(|c| "sfrxjSFRXJ".contains(*c)) {
            let base = &keys[..keys.len() - 1];
            cands.extend((1..base.len()).map(|i| format!("{}{t}{}", &base[..i], &base[i..])));
        }
        let mut best: Option<(f64, String)> = None;
        for k in cands.into_iter().filter(|k| render(k) == word) {
            let p = self.en_word(&k.to_ascii_lowercase()).unwrap_or(0.0);
            if best.as_ref().is_none_or(|b| p > b.0) {
                best = Some((p, k));
            }
        }
        best.map(|b| b.1)
    }

    /// Ngữ cảnh (khoá `canon` của từ trước) của từ vừa chốt, để mở lại nó đúng ngữ cảnh.
    /// Từ vừa chốt có hai phương án đều là từ có trong từ điển (did/đi, visa/vía).
    pub fn last_both_real(&self) -> bool {
        self.last.as_ref().is_some_and(|l| l.both_real)
    }

    pub fn last_context(&self) -> Option<String> {
        self.last.as_ref().map(|l| l.prev.clone())
    }

    /// Thứ tiếng đang ép cho từ đang gõ (`toggle_current`, `set_force`).
    pub fn forced(&self) -> Option<Lang> {
        self.force
    }

    /// Ép (hoặc bỏ ép) thứ tiếng của từ đang gõ; như `toggle_current`, lựa chọn được ghi nhớ lúc chốt.
    pub fn set_force(&mut self, lang: Option<Lang>) {
        self.force = lang;
    }

    /// Ép từ đang gõ sang phương án còn lại (lật trước khi chốt). Lựa chọn được ghi nhớ lúc chốt nếu khác với engine.
    pub fn toggle_current(&mut self) -> Option<View> {
        let v = self.view();
        if self.keys.is_empty() || !v.differs() {
            return None;
        }
        self.force = Some(if v.lang == Lang::Vn { Lang::En } else { Lang::Vn });
        Some(self.view())
    }

    pub fn view(&self) -> View {
        let vn_text = self.vn_text();
        let (letters, tone) = (self.comp.toneless(), self.comp.tone);
        let stripped: String = letters.iter().map(|&c| crate::syllable::strip_mark(c)).collect();
        let penalty = self.order_penalty();
        let vn_p = self.vn_prefix(&letters, tone).map(|u| {
            penalty
                * self.contextual(Lang::Vn, u, |f| {
                    // một chữ cái chưa mũ/móc: mọi âm tiết cùng dạng không dấu đều còn gõ thành được (a -> ă, â; d -> đ)
                    if letters.len() == 1 && letters[0].is_ascii() && tone == crate::syllable::Tone::None {
                        return f.prefix_class_sum(&stripped, crate::context::VN);
                    }
                    f.prefix_sum(&stripped, &|w| {
                        w.vn
                            && (tone == crate::syllable::Tone::None || w.tone() == tone)
                            && compatible_prefix(&letters, w.toneless().chars())
                    })
                })
        });
        let vn_p = vn_p.or_else(|| self.personal_vn().map(|u| penalty * self.contextual(Lang::Vn, u, |f| f.count(&canon(&vn_text)))));
        let (en_text, en_low, en_u) = self.en_side(|w| self.en_prefix(w), false);
        // Khoá ASCII chỉ có số thanh ở cuối, nên khoá ASCII trong khoảng tiền tố không dấu luôn bắt đầu bằng `en_low`.
        let en_p = en_u.map(|u| self.contextual(Lang::En, u, |f| f.prefix_class_sum(&en_low, crate::context::ASCII)));
        let lang = match (vn_p, en_p) {
            (Some(_), None) => Lang::Vn,
            (None, _) => Lang::En,
            (Some(v), Some(e)) => {
                let margin = self.cfg.flip_ratio.ln();
                let d = v.ln() - e.ln();
                match self.shown {
                    Some(Lang::Vn) if -d < margin => Lang::Vn,
                    Some(Lang::En) if d < margin => Lang::En,
                    _ if d >= 0.0 => Lang::Vn,
                    _ => Lang::En,
                }
            }
        };
        let lang = self.force.unwrap_or(lang);
        let text = if lang == Lang::Vn { vn_text.clone() } else { en_text.clone() };
        View { text, lang, vn_text, en_text, vn_p, en_p }
    }

    /// Ngữ cảnh vừa mất (click, đổi app, phím tắt, dán) mà app cho đọc chữ trước con trỏ: lấy lại từ trước và tiếng
    /// của vài từ cuối từ chính chữ đó, thay vì coi như đầu câu. Gọi trước phím đầu của từ mới. Từ dính sát con trỏ là
    /// việc của `reopen`.
    pub fn resume(&mut self, before: &str) {
        if !self.keys.is_empty() {
            return;
        }
        let chars: Vec<char> = before.chars().collect();
        let mut words = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let s = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            if i > s {
                words.push((s, i));
            } else {
                i += 1;
            }
        }
        let Some(&(ls, le)) = words.last() else { return };
        if le == chars.len() {
            return;
        }
        // App chỉ đưa vài chục ký tự: từ đầu có thể bị cắt.
        let cut = usize::from(words.len() > 1 && words[0].0 == 0);
        for &(s, e) in words.iter().skip(cut).rev().take(RESUME_WORDS).rev() {
            let w: String = chars[s..e].iter().collect();
            if let Some(l) = self.visible_lang(&w) {
                self.prev2_lang = self.prev_lang;
                self.prev_lang = Some(l);
                self.recent.push_back(l);
                if self.recent.len() > RECENT {
                    self.recent.pop_front();
                }
            }
        }
        let last: String = chars[ls..le].iter().collect();
        self.settle_last();
        self.run.clear();
        self.after_word = false;
        self.set_prev(canon(&last));
        self.prev_caps = all_caps(&last);
        for &c in &chars[le..] {
            if let Some(p) = after_separator(c) {
                if self.prev != p {
                    self.set_prev(p.into());
                }
            }
        }
    }

    /// Tiếng của một từ đã nằm trong app: so xác suất chữ đó là từ Việt và từ Anh, chênh dưới `informative_ratio`
    /// thì không tính (như lúc chốt từ).
    fn visible_lang(&self, w: &str) -> Option<Lang> {
        let low: String = w.to_lowercase().nfc().collect();
        let vn = decompose(&low).and_then(|(s, t)| {
            let s: Vec<char> = s.chars().collect();
            mix(self.ctx.and_then(|c| c.vn.word_prob(&s, t)), self.vn.word_prob(&s, t), self.mu())
        });
        let en = low.is_ascii().then(|| self.en_word(&low)).flatten();
        match (vn, en) {
            (Some(_), None) => Some(Lang::Vn),
            (None, Some(_)) => Some(Lang::En),
            (Some(v), Some(e)) if (v.ln() - e.ln()).abs() > self.cfg.informative_ratio.ln() => {
                Some(if v > e { Lang::Vn } else { Lang::En })
            }
            _ => None,
        }
    }

    /// Chốt từ ở ranh giới (space, dấu câu, Enter).
    pub fn commit(&mut self) -> View {
        let (vn_text, vn_p, repaired) = match self.vn_word() {
            Some(u) => {
                let t = self.vn_text();
                let k = canon(&t);
                let p = self.order_penalty() * self.memory.preference(&k) * self.contextual(Lang::Vn, u, |f| f.count(&k));
                (t, Some(p), false)
            }
            None => self.swap_repair().map_or_else(|| (self.vn_text(), None, false), |(t, p)| (t, Some(p), true)),
        };
        let (en_text, en_low, en_u) = self.en_side(|w| self.en_word(w), true);
        let both_real = vn_text != en_text && self.vn_known() && self.en_word(&en_low).is_some();
        let en_key = canon(&en_text);
        let en_p = en_u.map(|u| self.memory.preference(&en_key) * self.contextual(Lang::En, u, |f| f.count(&en_key)));
        let lang = match (vn_p, en_p) {
            (Some(v), Some(e)) if v >= e => Lang::Vn,
            (Some(_), None) => Lang::Vn,
            _ => Lang::En,
        };
        // Chênh ít thì giữ chữ đang hiện: người gõ đã thấy "á" lúc gõ thì đổi thành "as" lúc bấm cách trông như lỗi
        // (cả từ còn tiền tố chung, cả từ chốt đều chỉ là ước lượng).
        let stick = self.cfg.commit_stick;
        let lang = match (self.shown, vn_p, en_p) {
            (Some(sh), Some(v), Some(e)) if sh != lang && vn_text != en_text && (v.ln() - e.ln()).abs() < stick.ln() => sh,
            _ => lang,
        };
        // Chỉ từ nói rõ thứ tiếng mới đổi "tiếng của từ trước".
        let mut informative = match (vn_p, en_p) {
            (Some(_), None) | (None, Some(_)) => true,
            (Some(v), Some(e)) => (v.ln() - e.ln()).abs() > self.cfg.informative_ratio.ln(),
            (None, None) => false,
        };
        let pick = |l: Lang| if l == Lang::Vn { &vn_text } else { &en_text };
        let natural = lang;
        let lang = match self.force.take() {
            Some(f) if f != lang && vn_text != en_text => {
                if self.cfg.learn {
                    if repaired && lang == Lang::Vn {
                        self.memory.learn_pair(&self.prev, &canon(pick(f)), self.vn);
                    } else {
                        self.memory.learn(&self.prev, &canon(pick(lang)), &canon(pick(f)), self.vn);
                    }
                }
                informative = true;
                f
            }
            _ => lang,
        };
        if informative {
            self.prev2_lang = self.prev_lang;
            self.prev_lang = Some(lang);
            self.recent.push_back(lang);
            if self.recent.len() > RECENT {
                self.recent.pop_front();
            }
        }
        let text = if lang == Lang::Vn { vn_text.clone() } else { en_text.clone() };
        let before = self.set_prev(canon(&text));
        self.prev_caps = all_caps(&self.keys);
        self.run.clear();
        self.after_word = true;
        let learned = lang != natural && self.cfg.learn;
        let low = vn_text.to_lowercase();
        let hat = low.chars().any(|c| matches!(crate::syllable::split_tone(c).0, 'â' | 'ê' | 'ô'));
        let habit = (hat, self.comp.late > 0, low.contains('đ'), self.comp.late_d);
        self.settle_last();
        self.last = Some(Last {
            vn_text: vn_text.clone(),
            en_text: en_text.clone(),
            lang,
            prev: before,
            counted: informative,
            natural,
            learned,
            both_real,
            habit,
            repaired,
        });
        self.keys.clear();
        self.comp = Composer::new(self.cfg.telex);
        self.shown = None;
        View { text, lang, vn_text, en_text, vn_p, en_p }
    }

    /// Số lần gặp `cand` có từ theo sau (corpus + bộ nhớ riêng): bảng ít mẫu thì phần chiết khấu cho từ sau chưa gặp
    /// ước lượng kém (xem `Session::lookahead_min_followers`).
    pub fn followers(&self, cand: &str) -> f64 {
        let corpus = self.ctx.and_then(|c| c.follow(cand)).filter(|_| self.cfg.bigram).map_or(0.0, |f| f.total());
        corpus + self.memory.follow(cand).map_or(0.0, |f| PERSONAL * f.total())
    }

    /// Đã gặp cặp (`cand`, `next`) trong corpus hoặc bộ nhớ riêng.
    pub fn next_attested(&self, cand: &str, next: &str) -> bool {
        let corpus = self.ctx.and_then(|c| c.follow(cand)).filter(|_| self.cfg.bigram).is_some_and(|f| f.count(next) > 0.0);
        corpus || self.memory.follow(cand).is_some_and(|f| f.count(next) > 0.0)
    }

    /// P(từ `next` | từ trước là `cand`), cả hai ở dạng khoá `canon`: dùng từ phía sau để xem lại một từ còn phân vân.
    /// Gọi ngay sau `commit` của `next`. `cand_lang`: thứ tiếng của `cand`; có thì phần không nhờ cặp từ tính cả xác suất đổi tiếng (từ Việt mà theo sau
    /// là từ chỉ có tiếng Anh thì đáng ngờ: "Tô subtle" -> "Too subtle").
    /// `backoff_only`: bỏ phần cặp từ, chỉ còn xác suất theo tiếng (dùng khi chưa từng gặp `next` sau cả hai phương án,
    /// xem `next_attested`).
    pub fn next_support(&self, cand: &str, cand_lang: Option<Lang>, next: &str, backoff_only: bool) -> f64 {
        let word = key_to_word(next);
        let vn = decompose(&word).and_then(|(s, t)| {
            let s: Vec<char> = s.chars().collect();
            mix(self.ctx.and_then(|c| c.vn.word_prob(&s, t)), self.vn.word_prob(&s, t), self.mu())
        });
        let en = next.is_ascii().then(|| self.en_word(next)).flatten();
        let (vn, en) = (vn.unwrap_or(0.0), en.unwrap_or(0.0));
        let base = match (self.ctx, cand_lang) {
            (Some(c), Some(l)) if self.cfg.lookahead_switch => {
                let id = c.word_id(cand);
                let after = |lang| self.lang_after(c, id, Some(l), Some(l), lang);
                after(Lang::Vn) * vn + after(Lang::En) * en
            }
            _ => vn.max(en),
        }
        .max(1e-9);
        let (mut count, mut total, mut types) = (0.0, 0.0, 0.0);
        if let Some(f) = self.ctx.and_then(|c| c.follow(cand)).filter(|_| self.cfg.bigram && !backoff_only) {
            count += f.count(next);
            total += f.total();
            types += f.types();
        }
        if let Some(f) = self.memory.follow(cand).filter(|_| !backoff_only) {
            count += PERSONAL * f.count(next);
            total += PERSONAL * f.total();
            types += f.types();
        }
        let p = if total > 0.0 {
            let l = total / (total + types);
            l * count / total + (1.0 - l) * base
        } else {
            base
        };
        // từ sau dính liền vào (`self.join` là của từ vừa chốt): low-fidelity, Max-SA, Assets.car
        match (self.ctx, cand_lang) {
            (Some(c), Some(l)) if self.cfg.join => p * c.join_next_ratio(self.join, l),
            _ => p,
        }
    }

    /// Lật từ vừa chốt sang phương án còn lại ("visa" <-> "vía") và ghi nhớ lựa chọn của người dùng.
    /// Trả về (chữ cũ, chữ mới) để bộ gõ thay trên màn hình; None nếu không có gì để lật
    /// (đã gõ sang từ khác, hoặc hai phương án giống nhau như "con"). Lật lần nữa thì quay lại.
    pub fn flip_last(&mut self) -> Option<(String, String)> {
        self.toggle_last()
    }

    /// Lật ngược lần `flip_last` vừa rồi và bỏ điều nó vừa học (lật nhầm, trả lại ngay).
    pub fn unflip_last(&mut self) -> Option<(String, String)> {
        self.toggle_last()
    }

    /// Chữ đang hiện của từ vừa chốt (còn lật được).
    pub fn last_text(&self) -> Option<&str> {
        self.last.as_ref().map(|l| if l.lang == Lang::Vn { l.vn_text.as_str() } else { l.en_text.as_str() })
    }

    /// Đổi từ vừa chốt sang phương án kia như thể engine tự chọn nó (không học, Backspace/Esc sau đó lật lại và học
    /// như thường): dùng khi biết thêm từ vừa chốt đứng cuối câu (`end_rate`).
    pub fn prefer_last(&mut self) -> Option<(String, String)> {
        let r = self.toggle_last_with(false);
        if let Some(l) = self.last.as_mut() {
            l.natural = l.lang;
        }
        r
    }

    /// P(hết câu ngay sau từ `key`, khoá `canon`), ước từ corpus: số lần gặp từ trừ số lần có từ theo sau trong cùng câu
    /// (hết câu, hết dòng, hay số thì không có cặp từ). None khi không có ngữ cảnh.
    pub fn end_rate(&self, key: &str) -> Option<f64> {
        let c = self.ctx.filter(|_| self.cfg.bigram)?;
        let word = key_to_word(key);
        let vn = decompose(&word)
            .and_then(|(s, t)| c.vn.word_prob(&s.chars().collect::<Vec<_>>(), t))
            .map_or(0.0, |p| p * c.vn.total());
        let en = word.is_ascii().then(|| c.en.word_prob(&word)).flatten().map_or(0.0, |p| p * c.en.total());
        let n = vn.max(en);
        let follow = c.follow(key).map_or(0.0, |f| f.total());
        Some(((n - follow).max(0.0) + 0.5) / (n + 1.0))
    }

    fn toggle_last(&mut self) -> Option<(String, String)> {
        self.toggle_last_with(true)
    }

    fn toggle_last_with(&mut self, learn: bool) -> Option<(String, String)> {
        let last = self.last.as_mut()?;
        if last.vn_text == last.en_text {
            return None;
        }
        let text = |l: &Last, lang: Lang| if lang == Lang::Vn { l.vn_text.clone() } else { l.en_text.clone() };
        let old = text(last, last.lang);
        last.lang = if last.lang == Lang::Vn { Lang::En } else { Lang::Vn };
        let new = text(last, last.lang);
        let (old_key, new_key) = (canon(&old), canon(&new));
        let chosen = last.lang != last.natural;
        if learn && self.cfg.learn && chosen != last.learned {
            let (nat, alt) = (canon(&text(last, last.natural)), canon(&text(last, other(last.natural))));
            // bỏ từ Việt bộ gõ tự sửa từ phím gõ đảo: chỉ nhớ cặp từ ở ngữ cảnh này
            let pair_only = last.repaired && last.natural == Lang::Vn;
            match (chosen, pair_only) {
                (true, false) => self.memory.learn(&last.prev, &nat, &alt, self.vn),
                (true, true) => self.memory.learn_pair(&last.prev, &alt, self.vn),
                (false, false) => self.memory.unlearn(&last.prev, &nat, &alt),
                (false, true) => self.memory.unlearn_pair(&last.prev, &alt),
            }
            last.learned = chosen;
        }
        if self.prev == old_key {
            self.prev_id = self.ctx.and_then(|c| c.word_id(&new_key));
            self.prev = new_key;
        }
        if last.counted {
            if let Some(r) = self.recent.back_mut() {
                *r = last.lang;
            }
        } else {
            self.recent.push_back(last.lang);
            last.counted = true;
        }
        self.prev_lang = Some(last.lang);
        Some((old, new))
    }

    /// Ký tự ngăn cách sau từ (gọi sau `commit`, hoặc khi chưa gõ chữ nào). Tiếng của từ trước vẫn giữ
    /// qua câu mới: người ta hay viết nhiều câu liền bằng một thứ tiếng.
    pub fn separator(&mut self, c: char) {
        self.run.push(c);
        if c.is_whitespace() || !self.cfg.join {
            self.flush_run();
        }
    }
}
