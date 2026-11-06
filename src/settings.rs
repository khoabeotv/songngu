//! Cài đặt bộ gõ (file TSV trong thư mục người dùng): app bỏ qua, app kiểu code, tuỳ chọn gõ.
//!   exclude<TAB>bundle id      app bộ gõ đứng ngoài
//!   code<TAB>bundle id         thêm app kiểu Terminal/IDE (Esc của app, nghiêng tiếng Anh)
//!   tap<TAB>bundle id          app không thay chữ được hoặc không đưa phím qua bộ gõ (tự ghi khi kiểm tra thất bại, hoặc
//!                              chọn trong menu): chặn phím và xoá bằng Backspace. Dòng `marked` của bản cũ đọc như `tap`.
//!   option<TAB>tên<TAB>0|1     classic_tone, vni, brackets, log, esc_flip (lật bằng Esc thay cho Backspace)

use std::collections::BTreeSet;

use crate::ime::AppKind;

/// Terminal, IDE, trình soạn thảo code có sẵn: Esc là phím của app (vim), gõ chủ yếu tiếng Anh.
const CODE_APPS: &[&str] = &[
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.todesktop.230313mzl4w4u92",
    "com.exafunction.windsurf",
    "dev.zed.Zed",
    "com.sublimetext.4",
    "com.apple.dt.Xcode",
    "org.vim.MacVim",
    "com.neovide.neovide",
    "com.google.android.studio",
];

/// Luôn chặn phím và xoá bằng Backspace: terminal (chữ đã gửi vào shell không thay được bằng API thay chữ).
const TAP_APPS: &[&str] = &[
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
];

pub const OPTIONS: [&str; 5] = ["classic_tone", "vni", "brackets", "log", "esc_flip"];

#[derive(Default)]
pub struct Settings {
    pub exclude: BTreeSet<String>,
    pub code: BTreeSet<String>,
    pub tap: BTreeSet<String>,
    pub options: BTreeSet<String>,
}

impl Settings {
    pub fn load(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            match line.split('\t').collect::<Vec<_>>().as_slice() {
                ["exclude", b] => {
                    s.exclude.insert(b.to_string());
                }
                ["code", b] => {
                    s.code.insert(b.to_string());
                }
                ["tap" | "marked", b] => {
                    s.tap.insert(b.to_string());
                }
                ["option", name, "1"] => {
                    s.options.insert(name.to_string());
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) -> String {
        let mut out = String::new();
        for b in &self.exclude {
            out += &format!("exclude\t{b}\n");
        }
        for b in &self.code {
            out += &format!("code\t{b}\n");
        }
        for b in &self.tap {
            out += &format!("tap\t{b}\n");
        }
        for o in OPTIONS {
            out += &format!("option\t{o}\t{}\n", u8::from(self.options.contains(o)));
        }
        out
    }

    pub fn option(&self, name: &str) -> bool {
        self.options.contains(name)
    }

    pub fn toggle_option(&mut self, name: &str) {
        if !self.options.remove(name) {
            self.options.insert(name.to_string());
        }
    }

    pub fn toggle_exclude(&mut self, bundle: &str) {
        if !self.exclude.remove(bundle) {
            self.exclude.insert(bundle.to_string());
        }
    }

    /// App này dùng đường chặn phím + Backspace thay vì API thay chữ.
    pub fn tap(&self, bundle: &str) -> bool {
        self.tap.contains(bundle) || TAP_APPS.contains(&bundle)
    }

    /// Bật/tắt chặn phím ở app (menu). App có sẵn trong danh sách thì không tắt được.
    pub fn set_tap(&mut self, bundle: &str, on: bool) {
        if on {
            self.tap.insert(bundle.to_string());
        } else {
            self.tap.remove(bundle);
        }
    }

    pub fn app_kind(&self, bundle: &str) -> AppKind {
        if self.exclude.contains(bundle) {
            AppKind::Excluded
        } else if self.code.contains(bundle) || CODE_APPS.contains(&bundle) || bundle.starts_with("com.jetbrains.") {
            AppKind::Code
        } else {
            AppKind::Normal
        }
    }
}
