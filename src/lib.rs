pub mod context;
pub mod engine;
pub mod ffi;
pub mod ffi_settings;
pub mod ime;
pub mod lexicon;
pub mod memory;
pub mod settings;
pub mod store;
pub mod syllable;
pub mod telex;
pub mod typing;

use std::path::Path;

/// Nạp từ điển từ thư mục data/ (xem docs/data.md).
pub fn load(dir: &Path) -> std::io::Result<(lexicon::VnLex, lexicon::EnLex)> {
    let read = |f: &str| std::fs::read_to_string(dir.join(f));
    let vn = lexicon::VnLex::build(&(read("vi_50k.txt")? + &read("vi_extra.txt")?));
    let en = lexicon::EnLex::build(&read("en_words.txt")?, &read("en_dev.txt")?, 20_000.0);
    Ok((vn, en))
}

/// Học ngữ cảnh từ văn bản song ngữ (mỗi dòng một câu). `paths`: một hoặc nhiều file cách nhau bởi dấu phẩy;
/// `file*k` lặp file k lần để cân corpus nhỏ nhưng đúng miền với corpus lớn.
pub fn load_context(paths: &str, vn: &lexicon::VnLex, en: &lexicon::EnLex) -> std::io::Result<context::Context> {
    let mut text = String::new();
    for p in paths.split(',') {
        let (path, k) = p.rsplit_once('*').map_or((p, 1), |(f, k)| (f, k.parse().unwrap_or(1)));
        let t = std::fs::read_to_string(path)?;
        for _ in 0..k {
            text += &t;
        }
    }
    Ok(context::Context::train(&text, vn, en))
}

/// Mô hình nhị phân một file: từ điển chung + ngữ cảnh, để bộ gõ không cần thư mục data/ và không phải parse.
pub fn save_model(path: &Path, vn: &lexicon::VnLex, en: &lexicon::EnLex, ctx: &context::Context) -> std::io::Result<()> {
    let mut w = store::Writer::new();
    vn.write(&mut w);
    en.write(&mut w);
    ctx.write(&mut w);
    store::write_atomic(path, &w.into_bytes())
}

/// Nạp mô hình từ `save_model` bằng mmap (vài ms, dữ liệu nằm trong trang file chỉ đọc).
pub fn open_model(path: &Path) -> std::io::Result<(lexicon::VnLex, lexicon::EnLex, context::Context)> {
    let mut r = store::Reader::new(store::map_file(path)?)?;
    Ok((lexicon::VnLex::read(&mut r)?, lexicon::EnLex::read(&mut r)?, context::Context::read(&mut r)?))
}
