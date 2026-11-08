//! C ABI cho cửa sổ cài đặt (tiến trình riêng, SwiftUI trên macOS): đọc và sửa thẳng file người dùng theo đường dẫn.
//! Bộ gõ tự nạp lại khi thấy file đổi (`ffi.rs`), nên mỗi lần sửa ghi trọn file (file tạm rồi đổi tên) để bộ gõ không
//! đọc phải file ghi dở. Chuỗi trả về do Rust cấp, trả lại bằng `sn_str_free`.

use std::ffi::{c_char, CStr, CString};
use std::path::Path;

use crate::memory;
use crate::settings::{builtin_tap, Settings};

unsafe fn arg(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

fn give(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).unwrap_or_default().into_raw()
}

fn write_atomic(p: &Path, text: &str) -> bool {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension("tsv.tmp");
    std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, p)).is_ok()
}

fn load(p: &Path) -> Settings {
    std::fs::read_to_string(p).map_or_else(|_| Settings::default(), |t| Settings::load(&t))
}

/// Khoá bộ nhớ ("via1", "<s>") -> chữ hiện cho người dùng.
pub fn show_key(key: &str) -> String {
    if key == "<s>" {
        "(đầu câu)".into()
    } else {
        crate::context::key_to_word(key)
    }
}

/// # Safety
/// `s` do hàm `sn_*` ở đây trả về, hoặc NULL.
#[no_mangle]
pub unsafe extern "C" fn sn_str_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Tuỳ chọn `name` trong settings.tsv ở `path`.
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_cfg_option(path: *const c_char, name: *const c_char) -> bool {
    let (Some(p), Some(n)) = (arg(path), arg(name)) else { return false };
    load(Path::new(&p)).option(&n)
}

/// Đổi tuỳ chọn `name`; false nếu không ghi được file.
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_cfg_toggle_option(path: *const c_char, name: *const c_char) -> bool {
    let (Some(p), Some(n)) = (arg(path), arg(name)) else { return false };
    let p = Path::new(&p);
    let mut s = load(p);
    s.toggle_option(&n);
    write_atomic(p, &s.save())
}

/// Danh sách app, mỗi dòng một bundle id. `list`: "exclude", "tap", "tap_auto" (phần của tap do bộ gõ tự ghi),
/// "builtin_tap" (luôn chặn phím, không gỡ được).
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_cfg_list(path: *const c_char, list: *const c_char) -> *mut c_char {
    let (Some(p), Some(l)) = (arg(path), arg(list)) else { return give(String::new()) };
    let s = load(Path::new(&p));
    let items: Vec<String> = match l.as_str() {
        "exclude" => s.exclude.into_iter().collect(),
        "tap" => s.tap.into_iter().collect(),
        "tap_auto" => s.tap_auto.into_iter().collect(),
        "builtin_tap" => builtin_tap().iter().map(|b| b.to_string()).collect(),
        _ => vec![],
    };
    give(items.join("\n"))
}

/// Thêm/gỡ `bundle` trong danh sách `list` ("exclude" hoặc "tap"; tap thêm ở đây là người dùng chọn). False nếu không
/// ghi được file.
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_cfg_set(path: *const c_char, list: *const c_char, bundle: *const c_char, on: bool) -> bool {
    let (Some(p), Some(l), Some(b)) = (arg(path), arg(list), arg(bundle)) else { return false };
    let p = Path::new(&p);
    let mut s = load(p);
    match l.as_str() {
        "exclude" if on => {
            s.exclude.insert(b);
        }
        "exclude" => {
            s.exclude.remove(&b);
        }
        "tap" => s.set_tap(&b, on),
        _ => return false,
    }
    write_atomic(p, &s.save())
}

/// Các từ trong memory.tsv, được chọn nhiều nhất trước. Mỗi dòng: khoá, chữ, số lần chọn, số lần bỏ, ngữ cảnh (chữ của
/// từ đứng trước, "×n" nếu nhiều lần, cách nhau ", "), cách nhau bằng tab.
///
/// # Safety
/// `path` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_mem_learned(path: *const c_char) -> *mut c_char {
    let text = arg(path).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let lines: Vec<String> = memory::learned(&text)
        .into_iter()
        .map(|e| {
            let ctx: Vec<String> =
                e.contexts.iter().map(|(p, n)| if *n > 1.0 { format!("{} ×{n}", show_key(p)) } else { show_key(p) }).collect();
            format!("{}\t{}\t{}\t{}\t{}", e.key, show_key(&e.key), e.chosen, e.rejected, ctx.join(", "))
        })
        .collect();
    give(lines.join("\n"))
}

/// Thói quen gõ trong memory.tsv, mỗi dòng: tên ("late" mũ, "late_d" đ), số từ có thể gõ muộn, số từ gõ muộn.
///
/// # Safety
/// `path` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_mem_habits(path: *const c_char) -> *mut c_char {
    let text = arg(path).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let lines: Vec<String> = memory::habits(&text).into_iter().map(|(n, o, u)| format!("{n}\t{o}\t{u}")).collect();
    give(lines.join("\n"))
}

/// Quên một từ (khoá `canon`). False nếu không ghi được file.
///
/// # Safety
/// Các con trỏ là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_mem_forget(path: *const c_char, key: *const c_char) -> bool {
    let (Some(p), Some(k)) = (arg(path), arg(key)) else { return false };
    let p = Path::new(&p);
    let text = std::fs::read_to_string(p).unwrap_or_default();
    write_atomic(p, &memory::forget_key(&text, &k))
}

/// Quên hết: như bộ gõ, file đổi tên thành memory-forgotten.tsv (đổi tên lại là khôi phục), giữ một bản gần nhất.
///
/// # Safety
/// `path` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn sn_mem_forget_all(path: *const c_char) -> bool {
    let Some(p) = arg(path) else { return false };
    let p = Path::new(&p);
    !p.exists() || std::fs::rename(p, p.with_file_name("memory-forgotten.tsv")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    unsafe fn take(p: *mut c_char) -> String {
        let s = CStr::from_ptr(p).to_string_lossy().into_owned();
        sn_str_free(p);
        s
    }

    #[test]
    fn settings_and_memory_round_trip_through_the_files() {
        let dir = std::env::temp_dir().join(format!("sn-ffi-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (cfg, mem) = (c(dir.join("settings.tsv").to_str().unwrap()), c(dir.join("memory.tsv").to_str().unwrap()));
        unsafe {
            assert!(!sn_cfg_option(cfg.as_ptr(), c("vni").as_ptr()));
            assert!(sn_cfg_toggle_option(cfg.as_ptr(), c("vni").as_ptr()));
            assert!(sn_cfg_option(cfg.as_ptr(), c("vni").as_ptr()));
            assert!(sn_cfg_set(cfg.as_ptr(), c("tap").as_ptr(), c("com.a").as_ptr(), true));
            assert_eq!(take(sn_cfg_list(cfg.as_ptr(), c("tap").as_ptr())), "com.a");
            assert_eq!(take(sn_cfg_list(cfg.as_ptr(), c("tap_auto").as_ptr())), "");
            assert!(take(sn_cfg_list(cfg.as_ptr(), c("builtin_tap").as_ptr())).contains("com.apple.Terminal"));

            std::fs::write(dir.join("memory.tsv"), "pair\t<s>\tvia1\t2\npref\tvia1\t3\t1\npref\tvisa\t1\t3\n").unwrap();
            let learned = take(sn_mem_learned(mem.as_ptr()));
            assert_eq!(learned.lines().next(), Some("via1\tvía\t3\t1\t(đầu câu) ×2"));
            assert!(sn_mem_forget(mem.as_ptr(), c("via1").as_ptr()));
            assert_eq!(take(sn_mem_learned(mem.as_ptr())), "visa\tvisa\t1\t3\t");
            assert!(sn_mem_forget_all(mem.as_ptr()));
            assert!(dir.join("memory-forgotten.tsv").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
