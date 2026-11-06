//! C ABI cho lớp bộ gõ của từng OS. Một handle cho cả tiến trình; chuỗi trả về thuộc handle, hợp lệ tới lần gọi sau.
//! Engine lỗi (panic) thì handle tự tắt: mọi phím sau đó đi thẳng vào app, bộ gõ không chết theo.

use std::ffi::{c_char, CStr, CString};
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use crate::engine::{Config, Engine};
use crate::ime::{AppKind, Event, Session};
use crate::memory::Memory;
use crate::settings::Settings;
use crate::telex::TelexOpts;

pub struct Handle {
    session: Session<'static>,
    settings: Settings,
    app: String,
    commit: CString,
    memory_path: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    log_path: Option<PathBuf>,
    broken: bool,
}

/// Xoá phần ngay trước con trỏ rồi chèn `commit`: API thay chữ dùng `replace_before` (đơn vị UTF-16), đường chặn
/// phím bắn `erase` lần Backspace. `pass`: sau đó để app xử lý chính phím này.
#[repr(C)]
pub struct SnAction {
    pub commit: *const c_char,
    pub replace_before: u32,
    pub erase: u32,
    pub pass: bool,
}

unsafe fn string(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

fn cstring(s: String) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

fn write_file(path: &Option<PathBuf>, text: &str) {
    if let Some(p) = path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, text);
    }
}

impl Handle {
    /// Áp cài đặt lên engine/session (sau khi nạp hoặc đổi từ menu).
    fn apply_settings(&mut self) {
        let s = &self.settings;
        let cfg = &mut self.session.eng.cfg;
        cfg.modern = !s.option("classic_tone");
        cfg.telex = TelexOpts { vni: s.option("vni"), brackets: s.option("brackets") };
        self.session.logging = s.option("log") && self.log_path.is_some();
        self.session.app = s.app_kind(&self.app);
        self.session.blind = s.tap(&self.app);
        self.session.backspace_flip = !s.option("esc_flip");
        // Terminal/IDE: chủ yếu gõ lệnh, tên biến tiếng Anh.
        cfg.en_bias = if self.session.app == AppKind::Code { 5.0 } else { 1.0 };
    }

    fn after_event(&mut self) {
        if self.session.take_direct_broke() && !self.app.is_empty() {
            // app không thay chữ được: từ nay chặn phím và xoá bằng Backspace ở app này
            self.settings.set_tap(&self.app.clone(), true);
            write_file(&self.settings_path, &self.settings.save());
        }
        if self.session.memory_dirty {
            self.session.memory_dirty = false;
            write_file(&self.memory_path, &self.session.eng.memory().save());
        }
        if !self.session.log.is_empty() {
            let lines = std::mem::take(&mut self.session.log);
            if let Some(p) = &self.log_path {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                    for l in lines {
                        let _ = writeln!(f, "{l}");
                    }
                }
            }
        }
    }
}

/// Tạo bộ gõ. `data_dir`: từ điển; `model`: ngữ cảnh học sẵn; `memory`, `settings`, `log`: file của người dùng
/// (chưa có thì tạo khi cần). Chỉ `data_dir` bắt buộc. Trả NULL nếu không đọc được từ điển.
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ hoặc NULL.
#[no_mangle]
pub unsafe extern "C" fn sn_create(
    data_dir: *const c_char,
    model: *const c_char,
    memory: *const c_char,
    settings: *const c_char,
    log: *const c_char,
) -> *mut Handle {
    let (data_dir, model, memory, settings, log) = (string(data_dir), string(model), string(memory), string(settings), string(log));
    let made = catch_unwind(move || {
        // Có mô hình: từ điển chung nằm sẵn trong đó. Không có (hoặc hỏng): đọc data/, gõ không ngữ cảnh.
        let (vn, en, ctx) = match model.and_then(|m| crate::open_model(&PathBuf::from(m)).ok()) {
            Some((vn, en, c)) => (vn, en, Some(c)),
            None => {
                let (vn, en) = crate::load(&PathBuf::from(data_dir?)).ok()?;
                (vn, en, None)
            }
        };
        // Từ điển sống suốt tiến trình bộ gõ.
        let vn: &'static _ = Box::leak(Box::new(vn));
        let en: &'static _ = Box::leak(Box::new(en));
        let mut eng = Engine::new(vn, en, Config::default());
        if let Some(ctx) = ctx {
            eng = eng.with_context(Box::leak(Box::new(ctx)));
        }
        let memory_path = memory.map(PathBuf::from);
        if let Some(text) = memory_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            eng.set_memory(Memory::load(&text, vn));
        }
        let settings_path = settings.map(PathBuf::from);
        let s = settings_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Settings::default, |t| Settings::load(&t));
        let mut h = Handle {
            session: Session::new(eng),
            settings: s,
            app: String::new(),
            commit: CString::default(),
            memory_path,
            settings_path,
            log_path: log.map(PathBuf::from),
            broken: false,
        };
        h.apply_settings();
        Some(Box::into_raw(Box::new(h)))
    });
    made.ok().flatten().unwrap_or(std::ptr::null_mut())
}

/// App đang nhận phím (bundle id), gọi mỗi khi bộ gõ được kích hoạt cho một ô nhập.
///
/// # Safety
/// `h` từ `sn_create`, `bundle` là chuỗi C hợp lệ hoặc NULL.
#[no_mangle]
pub unsafe extern "C" fn sn_set_app(h: *mut Handle, bundle: *const c_char) {
    let h = &mut *h;
    h.app = string(bundle).unwrap_or_default();
    if !h.broken {
        h.apply_settings();
    }
}

/// `kind`: 0 ký tự (`ch` là mã Unicode), 1 Backspace, 2 Esc, 3 phím khác (Enter, Tab, mũi tên, phím tắt), 4 Shift+Esc.
/// `before`: chữ ngay trước con trỏ (NULL nếu app không cho đọc, hoặc đường chặn phím).
///
/// # Safety
/// `h` từ `sn_create`; `before` là chuỗi C hợp lệ hoặc NULL.
#[no_mangle]
pub unsafe extern "C" fn sn_event(h: *mut Handle, kind: u32, ch: u32, before: *const c_char) -> SnAction {
    let h = &mut *h;
    let ev = match kind {
        0 => char::from_u32(ch).map_or(Event::Other, Event::Char),
        1 => Event::Backspace,
        2 => Event::Escape,
        4 => Event::Flip,
        _ => Event::Other,
    };
    let before = string(before);
    let a = if h.broken {
        None
    } else {
        let r = catch_unwind(AssertUnwindSafe(|| h.session.handle(ev, before.as_deref())));
        if r.is_err() {
            h.broken = true;
        }
        r.ok()
    };
    // Lỗi: để phím đi thẳng vào app.
    let a = a.unwrap_or_else(|| crate::ime::Action { pass: true, ..Default::default() });
    if !h.broken {
        h.after_event();
    }
    h.commit = cstring(a.commit);
    SnAction { commit: h.commit.as_ptr(), replace_before: a.replace_before as u32, erase: a.erase as u32, pass: a.pass }
}

/// Phím sắp tới đi đường chặn phím dù app không có trong danh sách (app không mở phiên nhập liệu với bộ gõ): engine
/// không đọc chữ trong app (`blind`). Đổi đường giữa chừng thì kết thúc từ đang gõ.
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_set_blind(h: *mut Handle, on: bool) {
    let h = &mut *h;
    if h.broken {
        return;
    }
    let blind = on || h.settings.tap(&h.app);
    if h.session.blind != blind {
        if catch_unwind(AssertUnwindSafe(|| h.session.commit_all())).is_err() {
            h.broken = true;
            return;
        }
        h.session.blind = blind;
    }
}

/// App (bundle id) dùng đường chặn phím + Backspace thay vì API thay chữ.
///
/// # Safety
/// `h` từ `sn_create`, `bundle` là chuỗi C hợp lệ hoặc NULL.
#[no_mangle]
pub unsafe extern "C" fn sn_tap_app(h: *mut Handle, bundle: *const c_char) -> bool {
    let h = &*h;
    string(bundle).is_some_and(|b| h.settings.tap(&b))
}

/// Chuyển app sang (hoặc bỏ) đường chặn phím, lưu file cài đặt (lớp OS phát hiện app không đưa phím qua bộ gõ).
///
/// # Safety
/// `h` từ `sn_create`, `bundle` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_set_tap_app(h: *mut Handle, bundle: *const c_char, on: bool) {
    let h = &mut *h;
    let Some(b) = string(bundle).filter(|b| !b.is_empty()) else { return };
    h.settings.set_tap(&b, on);
    write_file(&h.settings_path, &h.settings.save());
    if !h.broken {
        h.apply_settings();
    }
}

/// Phím sắp tới có cần chữ trước con trỏ không (đầu từ, đang chèn thẳng). Esc luôn cần.
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_wants_before(h: *mut Handle) -> bool {
    let h = &*h;
    !h.broken && h.session.wants_before()
}


/// Kết thúc từ đang gõ (mất focus, click chuột): chữ đã nằm trong app, chỉ ngừng theo dõi.
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_commit_all(h: *mut Handle) {
    let h = &mut *h;
    if !h.broken && catch_unwind(AssertUnwindSafe(|| h.session.commit_all())).is_err() {
        h.broken = true;
    }
}

/// Ghi bộ nhớ riêng ra file (nếu đã học được gì).
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_save_memory(h: *mut Handle) -> bool {
    let h = &*h;
    if h.broken || h.session.eng.memory().is_empty() {
        return false;
    }
    write_file(&h.memory_path, &h.session.eng.memory().save());
    true
}

/// Số từ bộ nhớ riêng đã học (hiện trong menu).
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_memory_size(h: *mut Handle) -> u32 {
    let h = &*h;
    if h.broken {
        0
    } else {
        h.session.eng.memory().len() as u32
    }
}

/// Quên mọi thứ đã học từ các lần lật. File cũ đổi tên thành `memory-forgotten.tsv` cạnh đó (đổi tên lại là khôi phục
/// được), chỉ giữ một bản gần nhất.
///
/// # Safety
/// `h` từ `sn_create`.
#[no_mangle]
pub unsafe extern "C" fn sn_forget_memory(h: *mut Handle) {
    let h = &mut *h;
    if let Some(p) = &h.memory_path {
        if p.exists() {
            let _ = std::fs::rename(p, p.with_file_name("memory-forgotten.tsv"));
        }
    }
    if !h.broken {
        h.session.forget();
        h.session.memory_dirty = false;
    }
}

/// Tuỳ chọn trong menu: "classic_tone", "vni", "brackets", "log", "esc_flip", "manual_tap"; theo app hiện tại: "exclude".
/// Chặn phím theo app: `sn_tap_app` / `sn_set_tap_app` (app đang trước mặt, không phải app của bộ gõ).
///
/// # Safety
/// `h` từ `sn_create`, `name` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_option_get(h: *mut Handle, name: *const c_char) -> bool {
    let h = &*h;
    match string(name).as_deref() {
        Some("exclude") => h.settings.exclude.contains(&h.app),
        Some(n) => h.settings.option(n),
        None => false,
    }
}

/// Đổi một tuỳ chọn (xem `sn_option_get`), lưu file cài đặt và áp ngay.
///
/// # Safety
/// `h` từ `sn_create`, `name` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_option_toggle(h: *mut Handle, name: *const c_char) {
    let h = &mut *h;
    match string(name).as_deref() {
        Some("exclude") => {
            let app = h.app.clone();
            h.settings.toggle_exclude(&app);
        }
        Some(n) => h.settings.toggle_option(n),
        None => return,
    }
    write_file(&h.settings_path, &h.settings.save());
    if !h.broken {
        h.apply_settings();
    }
}
