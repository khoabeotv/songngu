//! Logic bộ gõ, độc lập hệ điều hành: nhận phím, trả về chữ cần đưa vào app. Lớp mỏng cho từng OS
//! (InputMethodKit, TSF, Fcitx) chỉ đổi sự kiện phím sang `Event` và áp `Action`.
//!
//! Không có vùng soạn (gạch chân): mỗi phím đưa chữ vào app ngay, engine đổi chữ thì thay đúng phần khác (`Action`:
//! xoá vài ký tự trước con trỏ rồi chèn). Lớp OS áp bằng một trong hai đường:
//! - **Thay chữ** (app cho đọc và thay chữ trước con trỏ): dùng API thay chữ của app, không bắn Backspace giả nên
//!   không tranh với phím thật. App không báo khi con trỏ bị click đi, nên đầu mỗi phím so chữ trước con trỏ với
//!   chữ bộ gõ đã để lại (`expected`). Khác sau một phím thường = con trỏ đã đi, bắt đầu lại. Ngay sau một lần thay
//!   mà chữ cũ còn nguyên và chữ mới bị chèn thêm ("taá") = app bỏ qua lệnh thay: app đó chuyển sang đường xoá lùi
//!   (`take_direct_broke`). Khác kiểu khác (macOS tự viết hoa/tự sửa chính tả đúng lúc đó, app cập nhật chậm) chỉ là
//!   mất dấu, phải `DIRECT_MISS_LIMIT` lần thay liền đều khác mới chuyển. So ở phím sau chứ không ngay sau khi thay
//!   vì Chrome/Electron cập nhật chữ đọc được hơi trễ. Ô không cho đọc chữ: từ đó không kiểm
//!   (`untracked`), lớp OS thay theo vị trí con trỏ (Zalo, vài app Flutter).
//! - **Xoá lùi** (`blind`: terminal, app hỏng thay chữ, app không đưa phím qua bộ gõ): lớp OS xoá `erase` ký tự
//!   bằng Backspace rồi gõ chữ mới. Không đọc được chữ trong app nên bộ gõ tin vào chữ nó tự theo dõi; click chuột,
//!   Enter, mũi tên… kết thúc từ.
//!
//! Từ sau có thể đổi từ trước còn phân vân (xin visa may mắn -> xin vía may mắn); gõ ngay sau một từ đã có trong
//! app thì mở lại từ đó ("tiêng" + s -> "tiếng", chỉ đường thay chữ); Enter, Tab, mũi tên, phím tắt luôn chốt trước.
//!
//! Lật Anh/Việt, theo thứ tự: từ đang gõ; lần từ sau vừa tự đổi từ trước (hoàn tác); từ vừa gõ; từ ngay trước
//! con trỏ (click quay lại rồi bấm, chỉ đường thay chữ).
//! - Backspace (mặc định, `backspace_flip`): bấm ngay sau một từ (có thể cách vài dấu ngăn cách) lật lần vừa tự đổi,
//!   hoặc từ vừa gõ mà hai phương án đều đáng kể (did/đi, theme/thêm). Còn lại Backspace xoá như thường. Esc trơn là của app.
//! - Esc (tắt `backspace_flip`, app thường): lật chữ bộ gõ vừa đưa vào; từ trước con trỏ chỉ khi lúc gõ bộ gõ còn phân
//!   vân. Không có gì để lật thì Esc là của app (huỷ sửa tin nhắn, đóng hộp thoại).
//! - Lỡ lật khi định dùng phím đó cho app (xoá, đóng hộp thoại) thì bấm lại ngay (`esc_undo_window`): chữ trả lại như cũ và
//!   app nhận phím đó. Giữ Backspace để xoá cả đoạn vẫn xoá đúng.
//! - Ctrl+Esc / Shift+Esc (`Flip`, mọi app; terminal/IDE chỉ có cách này vì Esc là của app): lật cả chữ bộ gõ không gõ ra.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::context::{canon, START};
use crate::engine::{Engine, Lang, View};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Ký tự gõ được, không kèm Cmd/Ctrl/Option.
    Char(char),
    Backspace,
    /// Esc: lật Anh/Việt, trừ app kiểu Terminal/IDE (ở đó Esc là của app).
    Escape,
    /// Ctrl+Esc, Shift+Esc: lật ở mọi app.
    Flip,
    /// Enter, Tab, mũi tên, phím tắt, click chuột…: app xử lý sau khi đã chốt.
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AppKind {
    #[default]
    Normal,
    /// Terminal, IDE: Esc thuộc về app, nghiêng tiếng Anh.
    Code,
    /// Bộ gõ đứng ngoài, phím đi thẳng vào app.
    Excluded,
}

/// Cách áp: xoá phần ngay trước con trỏ (`replace_before` đơn vị UTF-16 cho API thay chữ, cũng là `erase` ký tự cho
/// đường xoá lùi) rồi chèn `commit`.
#[derive(Debug, Default, PartialEq)]
pub struct Action {
    pub commit: String,
    pub replace_before: usize,
    pub erase: usize,
    /// Để app xử lý phím này (sau khi đã áp).
    pub pass: bool,
}

impl Action {
    /// Xoá `s` (chữ ngay trước con trỏ, hoặc trước phần đã xoá).
    fn cut(&mut self, s: &str) {
        self.replace_before += utf16(s);
        self.erase += s.chars().count();
    }
}

fn other(l: Lang) -> Lang {
    if l == Lang::Vn {
        Lang::En
    } else {
        Lang::Vn
    }
}

/// Từ vừa nằm trong app + dấu ngăn cách gõ sau nó: để lật (Esc) hoặc để từ sau đổi lại.
struct Tail {
    word: String,
    seps: String,
    /// (phương án kia, P đã chọn, P phương án kia, thứ tiếng đã chọn) nếu từ sau còn có thể đổi lại nó (`doubtful`).
    doubt: Option<(String, f64, f64, Lang)>,
    /// Hai phương án đều đáng kể (`uncertain`) hoặc người dùng đã lật nó: Backspace lật được (`backspace_flip`).
    unsure: bool,
    /// Phương án còn lại (để xoá cả từ rồi gõ lại thì ra nó, xem `Retype`).
    alt: Option<String>,
}

/// Từ vừa bị xoá hết mà hai phương án đều đáng kể: gõ lại đúng nó thì ra phương án kia (người ta xoá rồi gõ lại khi
/// thấy sai). Chỉ áp cho từ gõ ngay sau đó, ở đúng chỗ đó.
#[derive(Clone)]
struct Retype {
    shown: String,
    alt: String,
}

/// Đang xoá dần một từ đã chốt (đã xoá hết dấu ngăn cách sau nó): còn `left` chữ nữa là hết từ.
struct Deleting {
    retype: Retype,
    left: usize,
}

/// Một từ bộ gõ đã đưa vào app, để Esc sửa được cả khi con trỏ quay lại sau nó: biết đúng phương án còn lại
/// (từ phím thật đã gõ) và ngữ cảnh để ghi nhớ lựa chọn.
struct Recent {
    text: String,
    alt: String,
    /// Khoá `canon` của từ trước nó.
    prev: String,
    /// Esc lật được khi con trỏ quay lại sau nó: lúc gõ engine còn phân vân, hoặc người dùng đã lật nó.
    contested: bool,
}

const HISTORY: usize = 64;

/// Dấu câu chắc chắn kết câu ngay sau từ (dấu chấm còn nối từ: Nest.js).
const END_MARKS: &str = "?!";

/// Số lần thay liền mà chữ trước con trỏ khác chữ bộ gõ để lại (không rõ app bỏ qua lệnh thay) trước khi chuyển app
/// sang chặn phím. Một lần khác đơn lẻ thường do macOS tự sửa chữ cùng lúc.
const DIRECT_MISS_LIMIT: u32 = 3;

/// Lý do chuyển app sang chặn phím (nhật ký gõ).
pub const REASON_IGNORED: &str = "replace-ignored";
pub const REASON_MISMATCH: &str = "mismatch";

/// Lần Esc (không Shift) vừa lật, để Esc tiếp ngay sau đó trả lại.
struct EscUndo {
    at: Instant,
    /// Phím đã lật (Esc hoặc Backspace): bấm lại chính phím này ngay sau đó thì trả lại.
    key: Event,
    kind: EscKind,
}

enum EscKind {
    /// Từ đang gõ: bấm lật lại (chưa học gì, học lúc chốt).
    Current,
    /// Từ vừa gõ: engine lật ngược và bỏ điều vừa học.
    Tail,
    /// Chữ trong app: `now` (ngay trước `after`) đổi lại thành `back`, bỏ `learned` = (trước, bỏ, chọn).
    Text { now: String, back: String, after: String, learned: (String, String, String) },
}

/// Phím lật nào: mỗi phím lật tới đâu (xem `flip_inner`).
#[derive(Clone, Copy, PartialEq)]
enum Reach {
    /// Ctrl/Shift+Esc: mọi thứ, kể cả chữ bộ gõ không gõ ra.
    Any,
    /// Esc: chữ bộ gõ vừa đưa vào; từ trước con trỏ chỉ khi lúc gõ còn phân vân.
    Esc,
    /// Backspace ngay sau một từ: lần vừa tự đổi, hoặc từ vừa gõ mà hai phương án đều đáng kể.
    Backspace,
}

/// Lần từ sau vừa tự đổi từ trước: Esc ngay sau đó hoàn tác.
struct AutoFlip {
    /// Chữ engine đã chọn, và chữ nó bị đổi thành.
    orig: String,
    alt: String,
    /// Chữ bộ gõ đã đưa vào app sau `alt` (dấu ngăn cách, từ sau, dấu ngăn cách sau nó).
    after: String,
    prev: String,
}

#[derive(Debug, Default)]
pub struct Stats {
    /// Từ phân vân bị từ phía sau đổi lại.
    pub lookahead_flips: usize,
    pub words: usize,
    /// Từ gõ xong mà engine còn phân vân (Esc lật được).
    pub unsure: usize,
    /// Từ gõ xong mà Backspace ngay sau đó sẽ lật (`backspace_ratio`).
    pub backspace_flippable: usize,
    /// Lần thay chữ đã nằm trong app: người dùng thấy chữ đổi.
    pub rewrites: usize,
    /// Từ cuối câu đổi sang phương án hợp hết câu hơn (`end_check`).
    pub end_flips: usize,
}

pub struct Session<'a> {
    pub eng: Engine<'a>,
    pub app: AppKind,
    /// Đường xoá lùi: không đọc được chữ trước con trỏ, lớp OS xoá bằng Backspace (xem đầu file).
    pub blind: bool,
    /// Từ còn phân vân (hai phương án chênh dưới ngần này lần): Esc lật được nó.
    pub unsure_ratio: f64,
    /// Từ phía sau phải làm phương án kia hơn ngần này lần mới đổi từ trước.
    pub lookahead_ratio: f64,
    /// Từ sau xem lại từ trước nếu hai phương án của từ trước chênh dưới nghần này lần.
    pub lookahead_margin: f64,
    /// Bộ nhớ riêng vừa đổi (lớp ngoài nên ghi ra file).
    pub memory_dirty: bool,
    /// Bật thì mỗi từ chốt thêm một dòng "phím<TAB>chữ<TAB>tiếng", mỗi lần lật thêm "flip<TAB>cũ<TAB>mới".
    pub logging: bool,
    pub log: Vec<String>,
    pub stats: Stats,
    /// Chữ của từ đang gõ, đã nằm trong app.
    shown: String,
    /// Chữ trước con trỏ phải kết thúc bằng chuỗi này ở phím sau (đường thay chữ).
    expected: Option<String>,
    /// Phím vừa rồi đã thay chữ nằm trong app.
    rewrote: bool,
    /// App vừa bị phát hiện không thay chữ được, kèm lý do (lớp ngoài ghi nhớ cho app, xem `take_direct_broke`).
    direct_broke: Option<&'static str>,
    /// Chữ trước con trỏ sẽ kết thúc thế này nếu app bỏ qua lần thay vừa rồi (chèn chữ mới mà không xoá chữ cũ).
    ignored: Option<String>,
    /// Số lần thay liền khác chữ mà không rõ lý do (`DIRECT_MISS_LIMIT`).
    direct_misses: u32,
    /// Từ đang gõ bắt đầu ở chỗ không đọc được chữ: không kiểm con trỏ, tin vào chữ đã theo dõi như đường xoá lùi.
    untracked: bool,
    /// Từ sắp chốt đứng cuối câu (Enter, ? !): so thêm khả năng hết câu sau mỗi phương án (`end_check`).
    ending: bool,
    /// Phương án kia hết câu hợp hơn ngần này lần thì đổi ("người quay as" + Enter -> "người quay á"). 0 tắt.
    pub end_ratio: f64,
    /// Ngữ cảnh vừa mất (`commit_all`, `lose_track`): từ sau lấy lại ngữ cảnh từ chữ trước con trỏ nếu đọc được
    /// (`Engine::resume`).
    resync: bool,
    /// Tắt `resync` (để đo).
    pub resume_context: bool,
    /// Chữ đọc được ở phím trước, và phím trước có sửa chữ trong app không (phát hiện app chậm làm mới, `handle`).
    last_read: Option<String>,
    last_changed: bool,
    tail: Option<Tail>,
    keys: String,
    history: VecDeque<Recent>,
    auto_flip: Option<AutoFlip>,
    esc_undo: Option<EscUndo>,
    /// Esc trong khoảng này sau một lần Esc lật thì trả lại và chuyển Esc cho app.
    pub esc_undo_window: Duration,
    /// Backspace ngay sau một từ lật nó (mặc định), Esc trơn là của app. Tắt: Esc lật như cũ ở app thường.
    pub backspace_flip: bool,
    /// Backspace còn lật từ mà cả hai phương án đều có trong từ điển nếu chênh dưới ngần này lần ("làm visa" ~10^3,7).
    /// Dev: 10^3/10^4/10^5 lật được 83/88/92 từ sai lúc chốt, đổi lại 0,6-1,4% / 1,3-3% / 2,9-6% số từ Backspace sẽ
    /// lật thay vì xoá ("of" trong "the cost of" ~10^5,5).
    pub backspace_ratio: f64,
    /// Phương án kia có ít hơn ngần này lần gặp kèm từ theo sau, và chưa gặp từ sau sau phương án nào: so hai phương
    /// án chỉ bằng xác suất theo tiếng (xem `commit_word`). Chỉ áp khi phương án kia là từ Anh. Dev: 0/100 cho 340/339.
    pub lookahead_min_followers: f64,
    /// Gõ lại từ vừa xoá: ra phương án kia (`Retype`). `del_from`: từ đang gõ lúc bắt đầu xoá nó; `deleting`: đang xoá
    /// từ đã chốt; `retype_forced`: đã ép từ đang gõ theo `retype` (bỏ ép nếu gõ khác đi).
    retype: Option<Retype>,
    /// Phím vừa rồi là dấu ngăn cách sau một từ: chỉ Backspace đầu tiên sau đó mới lật (giữ Backspace thì chỉ xoá).
    bs_ready: bool,
    del_from: Option<Retype>,
    deleting: Option<Deleting>,
    retype_forced: bool,
}

/// `n` ký tự cuối của `s`.
fn tail_chars(s: &str, n: usize) -> String {
    let skip = s.chars().count().saturating_sub(n);
    s.chars().skip(skip).collect()
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Phần khác nhau giữa chữ cũ và chữ mới: (phần cuối chữ cũ cần xoá, phần chữ mới cần chèn).
fn diff(old: &str, new: &str) -> (String, String) {
    let common = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).count();
    (old.chars().skip(common).collect(), new.chars().skip(common).collect())
}

/// Từ dính ngay trước con trỏ và từ trước nó (None = đầu câu).
fn trailing_word(before: &str) -> Option<(String, Option<String>)> {
    let chars: Vec<char> = before.chars().collect();
    let end = chars.len();
    let start = chars.iter().rposition(|c| !c.is_alphabetic()).map_or(0, |i| i + 1);
    if start == end || end - start > 20 {
        return None;
    }
    let word: String = chars[start..end].iter().collect();
    let mut i = start;
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }
    let pstart = chars[..i].iter().rposition(|c| !c.is_alphabetic()).map_or(0, |j| j + 1);
    let prev = (pstart < i && i < start).then(|| chars[pstart..i].iter().collect());
    Some((word, prev))
}

/// Từ trước con trỏ, có thể cách con trỏ vài dấu ngăn cách: (từ, dấu ngăn cách sau nó, từ trước nó).
fn word_before(before: &str) -> Option<(String, String, Option<String>)> {
    let trimmed = before.trim_end_matches(|c: char| " ,.;:!?)\"'".contains(c));
    let seps = &before[trimmed.len()..];
    if seps.chars().count() > 4 {
        return None;
    }
    let (word, prev) = trailing_word(trimmed)?;
    Some((word, seps.to_string(), prev))
}

fn doubt_of(v: &View) -> Option<(String, f64, f64, Lang)> {
    let (p, alt_p) = if v.lang == Lang::Vn { (v.vn_p?, v.en_p?) } else { (v.en_p?, v.vn_p?) };
    let alt = if v.lang == Lang::Vn { v.en_text.clone() } else { v.vn_text.clone() };
    Some((alt, p, alt_p, v.lang))
}

impl<'a> Session<'a> {
    pub fn new(eng: Engine<'a>) -> Self {
        Session {
            eng,
            app: AppKind::Normal,
            blind: false,
            unsure_ratio: 1000.0,
            lookahead_ratio: 3.0,
            lookahead_margin: 1e4,
            memory_dirty: false,
            logging: false,
            log: Vec::new(),
            stats: Stats::default(),
            shown: String::new(),
            expected: None,
            rewrote: false,
            direct_broke: None,
            ignored: None,
            direct_misses: 0,
            untracked: false,
            resync: true,
            resume_context: true,
            last_read: None,
            last_changed: false,
            ending: false,
            end_ratio: 1.0,
            tail: None,
            keys: String::new(),
            history: VecDeque::new(),
            auto_flip: None,
            esc_undo: None,
            esc_undo_window: Duration::from_millis(1000),
            backspace_flip: true,
            backspace_ratio: 1e4,
            lookahead_min_followers: 100.0,
            retype: None,
            bs_ready: false,
            del_from: None,
            deleting: None,
            retype_forced: false,
        }
    }

    /// Quên bộ nhớ riêng (lớp ngoài lo file).
    pub fn forget(&mut self) {
        self.eng.forget();
        self.history.clear();
        self.auto_flip = None;
    }

    /// Sau mỗi lần engine chốt một từ: ghi nhật ký, nhớ từ (nếu có phương án khác) để Esc sửa lại sau này.
    fn committed(&mut self, v: &View) {
        self.log_word(v);
        if v.differs() {
            let alt = if v.lang == Lang::Vn { v.en_text.clone() } else { v.vn_text.clone() };
            let prev = self.eng.last_context().unwrap_or_else(|| START.into());
            let contested = self.uncertain(v);
            self.stats.unsure += usize::from(contested);
            self.history.push_back(Recent { text: v.text.clone(), alt, prev, contested });
            if self.history.len() > HISTORY {
                self.history.pop_front();
            }
        }
    }

    /// Một từ đã nằm trong app vừa đổi `old` -> `new`; trả ngữ cảnh của nó nếu bộ gõ còn nhớ từ đó.
    fn swapped(&mut self, old: &str, new: &str) -> Option<String> {
        let r = self.history.iter_mut().rev().find(|r| r.text == old)?;
        r.alt = std::mem::replace(&mut r.text, new.to_string());
        r.contested = true;
        Some(r.prev.clone())
    }

    /// Lớp OS cần đưa chữ trước con trỏ cho phím sắp tới không: đầu từ (mở lại từ), đang gõ (kiểm tra con trỏ).
    /// Esc luôn cần. Đọc chữ trong app tốn một lần gọi sang app nên chỉ đọc khi cần.
    pub fn wants_before(&self) -> bool {
        !self.blind && (self.eng.is_empty() || self.expected.is_some())
    }

    /// Không đọc được chữ trong app lúc này: tin vào chữ bộ gõ tự theo dõi.
    fn trusting(&self) -> bool {
        self.blind || self.untracked
    }

    /// App vừa bị phát hiện không thay chữ được (mỗi lần chỉ báo một lần), kèm lý do.
    pub fn take_direct_broke(&mut self) -> Option<&'static str> {
        self.direct_broke.take()
    }

    /// Đổi app: đếm lại số lần thay khác chữ.
    pub fn reset_direct_checks(&mut self) {
        self.direct_misses = 0;
        self.ignored = None;
    }

    /// Chữ bộ gõ đã để lại ngay trước con trỏ, nếu còn cần kiểm (để thay từ đang gõ hoặc từ trước).
    fn track(&self) -> Option<String> {
        if self.trusting() {
            return None;
        }
        let mut s = String::new();
        if let Some(t) = &self.tail {
            s += &t.word;
            s += &t.seps;
        }
        s += &self.shown;
        (!s.is_empty()).then_some(s)
    }

    /// Chữ đang theo dõi còn nằm ngay trước con trỏ không. Không đọc được: tin vào chữ đã theo dõi.
    fn before_ends_with(&self, before: Option<&str>, s: &str) -> bool {
        match before {
            Some(b) if !self.blind => b.ends_with(s),
            _ => self.trusting(),
        }
    }

    /// Mất dấu chữ đã để lại (con trỏ đã đi): kết thúc từ đang gõ, không thay gì nữa.
    fn lose_track(&mut self) {
        if !self.eng.is_empty() {
            let v = self.eng.commit();
            self.committed(&v);
        }
        self.auto_flip = None;
        self.shown.clear();
        self.tail = None;
        self.forget_retype();
        self.eng.separator('\n');
        self.resync = true;
    }

    fn forget_retype(&mut self) {
        self.retype = None;
        self.del_from = None;
        self.deleting = None;
        self.retype_forced = false;
    }

    fn word_key(&self, c: char) -> bool {
        c.is_ascii_alphabetic() || self.eng.cfg.telex.word_key(c, !self.eng.is_empty())
    }

    /// Từ sau còn được xem lại từ này không.
    fn doubtful(&self, v: &View) -> bool {
        match (v.vn_p, v.en_p) {
            (Some(a), Some(b)) => v.differs() && (a.ln() - b.ln()).abs() < self.lookahead_margin.ln(),
            _ => false,
        }
    }

    /// Engine còn phân vân về từ này (Esc lật được).
    fn uncertain(&self, v: &View) -> bool {
        self.within(v, self.unsure_ratio)
    }

    /// Hai phương án khác nhau và chênh dưới `ratio` lần.
    fn within(&self, v: &View, ratio: f64) -> bool {
        match (v.vn_p, v.en_p) {
            (Some(a), Some(b)) => v.differs() && (a.ln() - b.ln()).abs() < ratio.ln(),
            _ => false,
        }
    }

    fn log_word(&mut self, v: &View) {
        self.stats.words += 1;
        if self.logging {
            self.log.push(format!("{}\t{}\t{:?}", self.keys, v.text, v.lang));
        }
        self.keys.clear();
    }

    fn log_flip(&mut self, old: &str, new: &str) {
        if self.logging {
            self.log.push(format!("flip\t{old}\t{new}"));
        }
    }

    /// Đổi chữ của từ đang gõ trong app từ `shown` sang `new`.
    fn show(&mut self, a: &mut Action, new: String) {
        let (cut, add) = diff(&self.shown, &new);
        if !cut.is_empty() {
            self.stats.rewrites += 1;
            self.rewrote = true;
        }
        a.cut(&cut);
        a.commit += &add;
        self.shown = new;
    }

    /// Chốt từ đang gõ (chữ cuối cùng có thể khác chữ đang hiện), có thể đổi từ trước còn phân vân.
    fn commit_word(&mut self, a: &mut Action) {
        self.forget_retype();
        let v = self.eng.commit();
        let v = if std::mem::take(&mut self.ending) { self.end_check(v) } else { v };
        self.auto_flip = None;
        let next = canon(&v.text);
        let flip_prev = self.tail.as_ref().and_then(|t| {
            let (alt, p, alt_p, lang) = t.doubt.as_ref()?;
            let (word, alt_key) = (canon(&t.word), canon(alt));
            // Phương án kia là từ Anh hiếm trong corpus và chưa từng gặp `next` sau phương án nào: so bằng xác suất theo
            // tiếng cho cả hai. Nếu không, từ phổ biến ("nên") bị chiết khấu mạnh cho từ sau chưa gặp còn từ hiếm (ước
            // lượng từ vài mẫu) thì không, nên "nene model" thắng "nên model". Danh sách từ Anh lấy từ phụ đề nên lẫn
            // nhiều từ ngoại/hiếm (nene, mais, caf); âm tiết Việt hiếm (pải, ó) thì vẫn để từ sau kéo lại như cũ.
            let rare_en_alt = other(*lang) == Lang::En && self.eng.followers(&alt_key) < self.lookahead_min_followers;
            let backoff = rare_en_alt
                && !self.eng.next_attested(&word, &next)
                && !self.eng.next_attested(&alt_key, &next);
            let cur = p * self.eng.next_support(&word, Some(*lang), &next, backoff);
            let alt_score = alt_p * self.eng.next_support(&alt_key, Some(other(*lang)), &next, backoff);
            (alt_score > cur * self.lookahead_ratio).then(|| (t.word.clone(), t.seps.clone(), alt.clone()))
        });
        match flip_prev {
            Some((word, seps, alt)) => {
                let shown = std::mem::take(&mut self.shown);
                a.cut(&format!("{word}{seps}{shown}"));
                a.commit += &format!("{alt}{seps}{}", v.text);
                self.rewrote = true;
                self.stats.lookahead_flips += 1;
                self.stats.rewrites += 1;
                let prev = self.swapped(&word, &alt).unwrap_or_else(|| START.into());
                self.auto_flip = Some(AutoFlip { orig: word, alt, after: format!("{seps}{}", v.text), prev });
            }
            None => {
                let text = v.text.clone();
                self.show(a, text);
            }
        }
        self.committed(&v);
        self.shown.clear();
        let doubt = if self.doubtful(&v) { doubt_of(&v) } else { None };
        let unsure = self.uncertain(&v) || (self.eng.last_both_real() && self.within(&v, self.backspace_ratio));
        self.stats.backspace_flippable += usize::from(unsure);
        let alt = v.differs().then(|| if v.lang == Lang::Vn { v.en_text.clone() } else { v.vn_text.clone() });
        self.tail = Some(Tail { word: v.text.clone(), seps: String::new(), doubt, unsure, alt });
    }

    /// Từ vừa chốt đứng cuối câu: nhân mỗi phương án với khả năng hết câu sau nó. Từ Anh như "as", "of" gần như không
    /// bao giờ đứng cuối câu, còn "á", "nha" thì hay, nên câu tiếng Việt kết bằng "as" ra "á".
    fn end_check(&mut self, v: View) -> View {
        if self.end_ratio <= 0.0 || !self.doubtful(&v) {
            return v;
        }
        let Some((alt, p, alt_p, lang)) = doubt_of(&v) else { return v };
        let (Some(e), Some(alt_e)) = (self.eng.end_rate(&canon(&v.text)), self.eng.end_rate(&canon(&alt))) else {
            return v;
        };
        if alt_p * alt_e <= p * e * self.end_ratio || self.eng.prefer_last().is_none() {
            return v;
        }
        self.stats.end_flips += 1;
        View { text: alt, lang: other(lang), ..v }
    }

    /// Enter sau từ vừa gõ và vài dấu cách ("người quay as " + Enter): như `end_check` cho từ đó, đổi cả chữ trong app.
    fn end_tail(&mut self, a: &mut Action) {
        let Some(t) = self.tail.as_ref().filter(|t| t.seps.chars().all(|c| c == ' ')) else { return };
        let Some((alt, p, alt_p, _)) = t.doubt.clone() else { return };
        let (word, seps) = (t.word.clone(), t.seps.clone());
        let (Some(e), Some(alt_e)) = (self.eng.end_rate(&canon(&word)), self.eng.end_rate(&canon(&alt))) else { return };
        if self.end_ratio <= 0.0 || alt_p * alt_e <= p * e * self.end_ratio {
            return;
        }
        if self.eng.last_text() != Some(word.as_str()) || self.eng.prefer_last().is_none() {
            return;
        }
        a.cut(&format!("{word}{seps}"));
        a.commit += &format!("{alt}{seps}");
        self.rewrote = true;
        self.stats.rewrites += 1;
        self.stats.end_flips += 1;
        self.swapped(&word, &alt);
    }

    /// `before`: chữ ngay trước con trỏ trong app (None nếu app không cho đọc, hoặc đường xoá lùi). Cần ở đầu từ
    /// (`wants_before`) và khi Esc.
    pub fn handle(&mut self, ev: Event, before: Option<&str>) -> Action {
        // Chữ đọc được y hệt lần trước dù phím trước đã sửa chữ: app chưa làm mới phần đọc được (VS Code và app
        // Chromium khác chỉ làm mới khi con trỏ dời, nên thay "i" bằng "í" không đổi gì). Coi như không đọc được chữ,
        // tin vào chữ bộ gõ tự theo dõi, thay vì tưởng con trỏ đã bị click đi.
        let stale = before.is_some() && self.last_changed && before == self.last_read.as_deref();
        if stale {
            self.expected = None;
            self.untracked = true;
        }
        let a = self.handle_inner(ev, if stale { None } else { before });
        self.last_read = before.map(str::to_string);
        self.last_changed = !a.commit.is_empty() || a.erase > 0 || (a.pass && ev == Event::Backspace);
        a
    }

    fn handle_inner(&mut self, ev: Event, before: Option<&str>) -> Action {
        let mut a = Action::default();
        // chỉ Esc ngay sau (không phím nào xen giữa) mới trả lại được
        let esc_undo = self.esc_undo.take();
        let bs_ready = std::mem::take(&mut self.bs_ready);
        if self.app == AppKind::Excluded {
            self.commit_all();
            a.pass = true;
            return a;
        }
        // Mất dấu thì phím này bắt đầu từ mới, không mở lại từ trước con trỏ (chữ đọc được có thể chưa cập nhật).
        let mut reopen_ok = true;
        let ignored = self.ignored.take();
        if let Some(exp) = self.expected.take() {
            if !before.is_some_and(|b| b.ends_with(&exp)) {
                let lower = |s: &str| s.to_lowercase();
                if !self.rewrote {
                    self.lose_track();
                } else if ignored.is_some_and(|i| before.is_some_and(|b| b.ends_with(&i))) {
                    self.direct_failed(REASON_IGNORED);
                } else if before.is_some_and(|b| lower(b).ends_with(&lower(&exp))) {
                    // chỉ khác chữ hoa (macOS tự viết hoa đầu câu): app vẫn thay chữ đúng
                    self.lose_track();
                } else {
                    self.direct_misses += 1;
                    if self.direct_misses >= DIRECT_MISS_LIMIT {
                        self.direct_failed(REASON_MISMATCH);
                    } else {
                        self.lose_track();
                    }
                }
                reopen_ok = false;
            } else if self.rewrote {
                self.direct_misses = 0;
            }
        }
        self.rewrote = false;
        if let Some(u) = esc_undo.filter(|u| ev == u.key && u.at.elapsed() <= self.esc_undo_window) {
            // phím lật vừa rồi là ý dùng cho app: trả lại chữ, rồi để app xử lý phím này
            self.revert_esc(&mut a, u.kind, before);
            if ev == Event::Backspace {
                self.backspace(&mut a);
                self.expected = self.track();
            } else {
                self.commit_all();
            }
            a.pass = true;
            return a;
        }
        match ev {
            Event::Char(c) if self.word_key(c) => self.letter(&mut a, c, before, reopen_ok),
            Event::Char(c) => {
                if !self.eng.is_empty() {
                    self.ending = END_MARKS.contains(c);
                    self.commit_word(&mut a);
                }
                self.forget_retype();
                self.eng.separator(c);
                a.commit.push(c);
                if let Some(t) = self.tail.as_mut() {
                    t.seps.push(c);
                }
                if let Some(f) = self.auto_flip.as_mut() {
                    f.after.push(c);
                }
                // nhiều dấu ngăn cách: không còn là "từ vừa gõ"
                if self.tail.as_ref().is_some_and(|t| t.seps.chars().count() > 4) {
                    self.tail = None;
                }
                self.bs_ready = true;
            }
            Event::Backspace if self.backspace_flip && bs_ready && self.eng.is_empty() => {
                match self.flip_inner(&mut a, before, Reach::Backspace) {
                    Some(k) => self.esc_undo = Some(EscUndo { at: Instant::now(), key: ev, kind: k }),
                    None => self.backspace(&mut a),
                }
            }
            Event::Backspace => self.backspace(&mut a),
            Event::Escape if self.app == AppKind::Code || self.backspace_flip => self.finish(&mut a),
            Event::Escape => self.flip(&mut a, before, false),
            Event::Flip => self.flip(&mut a, before, true),
            Event::Other => self.finish(&mut a),
        }
        self.expected = self.track();
        // Thay chữ mà app bỏ qua vùng thay thì chữ mới bị chèn thêm sau chữ cũ. Chỉ xoá (không chèn) thì không phân
        // biệt được với app cập nhật chậm, bỏ qua.
        self.ignored = (self.rewrote && !a.pass && !a.commit.is_empty() && self.expected.is_some())
            .then(|| before.map(|b| tail_chars(&format!("{b}{}", a.commit), 40)))
            .flatten();
        a
    }

    fn letter(&mut self, a: &mut Action, c: char, before: Option<&str>, reopen_ok: bool) {
        if self.eng.is_empty() {
            self.auto_flip = None;
            self.untracked = !self.blind && before.is_none();
            let candidate = if self.trusting() {
                // không đọc được chữ: đã xoá hết dấu ngăn cách sau từ vừa gõ thì từ đó đang ngay trước con trỏ
                self.tail.as_ref().filter(|t| t.seps.is_empty()).map(|t| (t.word.clone(), self.eng.last_context()))
            } else {
                before.filter(|_| reopen_ok).and_then(trailing_word)
            };
            let reopened = candidate.filter(|(w, p)| self.eng.reopen(w, p.as_deref()));
            if let Some((w, _)) = reopened {
                // từ dính trước con trỏ đã nằm trong app: gõ tiếp vào chính nó
                self.shown = w;
                self.tail = None;
            } else if self.resync {
                match before.filter(|_| self.resume_context && !self.trusting()) {
                    Some(b) => self.eng.resume(b),
                    None => self.eng.leave_view(),
                }
            }
            self.resync = false;
        }
        self.del_from = None;
        self.deleting = None;
        self.keys.push(c);
        let mut v = self.eng.key(c);
        if let Some(r) = self.retype.clone() {
            // gõ lại đúng từ vừa xoá: ra phương án kia; gõ khác đi thì thôi
            if self.retype_forced {
                self.eng.set_force(None);
                self.retype_forced = false;
            }
            let n = self.eng.view();
            let alt = if n.lang == Lang::Vn { &n.en_text } else { &n.vn_text };
            if self.eng.forced().is_none() && n.differs() && n.text == r.shown && *alt == r.alt {
                self.eng.set_force(Some(other(n.lang)));
                self.retype_forced = true;
            }
            v = self.eng.view();
        }
        self.show(a, v.text);
    }

    fn backspace(&mut self, a: &mut Action) {
        self.auto_flip = None;
        if !self.eng.is_empty() {
            if self.del_from.is_none() {
                let v = self.eng.view();
                self.del_from = self.uncertain(&v).then(|| Retype {
                    alt: if v.lang == Lang::Vn { v.en_text.clone() } else { v.vn_text.clone() },
                    shown: v.text,
                });
            }
            self.keys.pop();
            let text = self.eng.backspace().map(|v| v.text).unwrap_or_default();
            if self.eng.is_empty() {
                self.retype = self.del_from.take();
                self.retype_forced = false;
            }
            // chỉ mất chữ cuối: để app tự xoá bằng chính phím Backspace này
            if text.chars().count() + 1 == self.shown.chars().count() && self.shown.starts_with(&text) {
                self.shown = text;
                a.pass = true;
            } else {
                self.show(a, text);
            }
        } else {
            match self.tail.take() {
                Some(mut t) if !t.seps.is_empty() => {
                    t.seps.pop();
                    self.tail = Some(t);
                }
                // xoá vào chính từ vừa gõ: đếm để biết lúc xoá hết
                Some(t) => {
                    self.deleting = t.alt.filter(|_| t.unsure).map(|alt| Deleting {
                        left: t.word.chars().count(),
                        retype: Retype { shown: t.word, alt },
                    });
                }
                None => {}
            }
            if let Some(d) = self.deleting.as_mut() {
                d.left -= 1;
                if d.left == 0 {
                    self.retype = self.deleting.take().map(|d| d.retype);
                }
            }
            a.pass = true;
        }
    }

    /// `any`: Ctrl/Shift+Esc, lật mọi thứ lật được, kể cả chữ bộ gõ không gõ ra. Esc lật chữ bộ gõ vừa đưa vào (xem đầu
    /// file); không có gì thì Esc là của app. Lật bằng Esc thì ghi lại để Esc tiếp ngay sau đó trả lại (`esc_undo`).
    fn flip(&mut self, a: &mut Action, before: Option<&str>, any: bool) {
        let reach = if any { Reach::Any } else { Reach::Esc };
        match self.flip_inner(a, before, reach) {
            Some(k) if !any => self.esc_undo = Some(EscUndo { at: Instant::now(), key: Event::Escape, kind: k }),
            Some(_) => {}
            None => self.finish(a),
        }
    }

    fn flip_inner(&mut self, a: &mut Action, before: Option<&str>, reach: Reach) -> Option<EscKind> {
        if !self.eng.is_empty() {
            // không có phương án khác ("ok"): Esc là của app
            if !self.eng.view().differs() {
                return None;
            }
            self.toggle_current(a);
            return Some(EscKind::Current);
        }
        // chỉ thay khi chữ đó còn nằm ngay trước con trỏ
        if let Some(f) = self.auto_flip.take() {
            if self.before_ends_with(before, &format!("{}{}", f.alt, f.after)) {
                let learned = (f.prev.clone(), canon(&f.alt), canon(&f.orig));
                self.replace_word(a, &f.alt, &f.orig, &f.after, Some(&learned));
                return Some(EscKind::Text { now: f.orig, back: f.alt, after: f.after, learned });
            }
        }
        if let Some(t) = self.tail.take() {
            // Backspace còn là phím xoá: chỉ lật từ mà hai phương án đều là từ có thật (did/đi, visa/vía), không lật
            // "tiêng"/"tieeng" (xoá dấu cách để gõ tiếp dấu vào từ)
            // đã xoá hết dấu ngăn cách thì Backspace tiếp là xoá chữ của từ
            let fits = reach != Reach::Backspace || (t.unsure && !t.seps.is_empty());
            if fits && self.before_ends_with(before, &format!("{}{}", t.word, t.seps)) {
                if self.flip_tail(a, t, true) {
                    return Some(EscKind::Tail);
                }
                return None;
            }
            self.tail = Some(t);
        }
        if reach == Reach::Backspace {
            return None;
        }
        before.filter(|_| !self.blind).and_then(|b| self.flip_before(a, b, reach == Reach::Any))
    }

    fn toggle_current(&mut self, a: &mut Action) {
        if let Some(v) = self.eng.toggle_current() {
            self.memory_dirty = true;
            self.show(a, v.text);
        }
    }

    /// Lật từ vừa gõ (đã kiểm là nằm ngay trước con trỏ).
    fn flip_tail(&mut self, a: &mut Action, t: Tail, learn: bool) -> bool {
        let flipped = if learn { self.eng.flip_last() } else { self.eng.unflip_last() };
        let Some((old, new)) = flipped else {
            self.tail = Some(t);
            return false;
        };
        a.cut(&format!("{}{}", t.word, t.seps));
        a.commit = format!("{new}{}", t.seps);
        self.rewrote = true;
        // người dùng đã chọn: từ sau không tự đổi lại nữa, nhưng Esc vẫn lật lại được
        self.tail = Some(Tail { word: new.clone(), seps: t.seps, doubt: None, unsure: true, alt: Some(old.clone()) });
        self.memory_dirty = true;
        self.swapped(&old, &new);
        self.log_flip(&old, &new);
        true
    }

    /// Thay từ `now` (ngay trước `after`, sát con trỏ) bằng `new` trong app; ghi nhớ `learned` = (trước, bỏ, chọn).
    fn replace_word(&mut self, a: &mut Action, now: &str, new: &str, after: &str, learned: Option<&(String, String, String)>) {
        a.cut(&format!("{now}{after}"));
        a.commit = format!("{new}{after}");
        self.rewrote = true;
        if let Some((p, r, c)) = learned {
            self.eng.learn(p, r, c);
            self.memory_dirty = true;
        }
        self.swapped(now, new);
        self.log_flip(now, new);
    }

    /// Lật từ ngay trước con trỏ (con trỏ đã quay lại sau nó). Từ bộ gõ gõ ra gần đây: biết đúng phương án kia,
    /// Esc chỉ lật nếu lúc gõ bộ gõ còn phân vân. Chữ có sẵn (chỉ khi `any`): engine đoán phương án kia.
    fn flip_before(&mut self, a: &mut Action, before: &str, any: bool) -> Option<EscKind> {
        let (word, seps, prev_word) = word_before(before)?;
        let known = self.history.iter().rev().find(|r| r.text == word).map(|r| (r.alt.clone(), r.prev.clone(), r.contested));
        let (alt, prev) = match known {
            Some((alt, prev, contested)) if any || contested => (alt, prev),
            Some(_) => return None,
            None => {
                let alt = self.eng.alternative(&word).filter(|_| any)?;
                (alt, prev_word.map_or_else(|| START.into(), |p| canon(&p)))
            }
        };
        let learned = (prev.clone(), canon(&word), canon(&alt));
        if self.history.iter().all(|r| r.text != word) {
            self.history.push_back(Recent { text: word.clone(), alt: alt.clone(), prev, contested: true });
        }
        self.replace_word(a, &word, &alt, &seps, Some(&learned));
        Some(EscKind::Text { now: alt, back: word, after: seps, learned })
    }

    /// Esc ngay sau một lần Esc lật: trả lại như cũ, bỏ điều vừa học. false nếu chữ trong app đã khác.
    fn revert_esc(&mut self, a: &mut Action, kind: EscKind, before: Option<&str>) -> bool {
        match kind {
            EscKind::Current => {
                if self.eng.is_empty() {
                    return false;
                }
                self.toggle_current(a);
            }
            EscKind::Tail => {
                let Some(t) = self.tail.take() else { return false };
                if !self.before_ends_with(before, &format!("{}{}", t.word, t.seps)) || !self.flip_tail(a, t, false) {
                    return false;
                }
            }
            EscKind::Text { now, back, after, learned } => {
                if !self.before_ends_with(before, &format!("{now}{after}")) {
                    return false;
                }
                self.eng.unlearn(&learned.0, &learned.1, &learned.2);
                self.memory_dirty = true;
                self.replace_word(a, &now, &back, &after, None);
            }
        }
        true
    }

    /// Phím app cần xử lý: chốt từ đang gõ (sửa nốt chữ trong app) rồi để app xử lý.
    fn finish(&mut self, a: &mut Action) {
        if !self.eng.is_empty() {
            self.ending = true;
            self.commit_word(a);
        } else {
            self.end_tail(a);
        }
        self.commit_all();
        a.pass = true;
    }

    /// App bỏ qua lệnh thay chữ: từ nay dùng đường xoá lùi.
    fn direct_failed(&mut self, reason: &'static str) {
        self.blind = true;
        self.direct_broke = Some(reason);
        self.direct_misses = 0;
        self.lose_track();
    }

    /// Kết thúc mọi thứ đang theo dõi (mất focus, click chuột): chữ đã nằm trong app, chỉ chốt từ. Con trỏ có thể đã
    /// sang chỗ khác, nên ngữ cảnh bắt đầu lại như đầu câu.
    pub fn commit_all(&mut self) {
        if !self.eng.is_empty() {
            let v = self.eng.commit();
            self.committed(&v);
        }
        self.auto_flip = None;
        self.shown.clear();
        self.expected = None;
        self.tail = None;
        self.forget_retype();
        self.eng.separator('\n');
        self.resync = true;
    }
}

#[cfg(test)]
mod tests {
    use super::diff;

    #[test]
    fn diff_replaces_only_the_changed_end() {
        let d = |a: &str, b: &str| {
            let (x, y) = diff(a, b);
            (x, y)
        };
        assert_eq!(d("hoa", "hoà"), ("a".into(), "à".into()));
        assert_eq!(d("tá", "task"), ("á".into(), "ask".into()));
        assert_eq!(d("tiêng", "tiếng"), ("êng".into(), "ếng".into()));
        assert_eq!(d("trườ", "trường"), (String::new(), "ng".into()));
        assert_eq!(d("tiến", "tiế"), ("n".into(), String::new()));
    }
}
