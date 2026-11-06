//! Hành vi quyết định ngôn ngữ trên dữ liệu thật trong data/.

use std::path::Path;
use std::sync::OnceLock;

use songngu::engine::{Config, Engine, Lang};
use songngu::lexicon::{EnLex, VnLex};

fn lex() -> &'static (VnLex, EnLex) {
    static L: OnceLock<(VnLex, EnLex)> = OnceLock::new();
    L.get_or_init(|| songngu::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data")).unwrap())
}

/// Gõ một câu (từ cách nhau bởi space), trả về chữ đã chốt và phím mà từ cuối được chốt ngôn ngữ.
fn type_text(text: &str) -> String {
    let (vn, en) = lex();
    let mut e = Engine::new(vn, en, Config::default());
    let mut out = Vec::new();
    for w in text.split(' ') {
        w.chars().for_each(|k| {
            e.key(k);
        });
        out.push(e.commit().text);
    }
    out.join(" ")
}

/// Phím thứ mấy thì chữ hiển thị đã đúng ngôn ngữ và không đổi nữa, khi gõ ngay sau `context`.
fn settled_at(context: &str, keys: &str, lang: Lang) -> usize {
    let (vn, en) = lex();
    let mut e = Engine::new(vn, en, Config::default());
    for w in context.split_whitespace() {
        w.chars().for_each(|k| {
            e.key(k);
        });
        e.commit();
    }
    let views: Vec<_> = keys.chars().map(|k| e.key(k)).collect();
    let last_wrong = views.iter().rposition(|v| v.differs() && v.lang != lang);
    last_wrong.map_or(1, |i| i + 2)
}

#[test]
fn converts_and_keeps() {
    assert_eq!(type_text("truowfng"), "trường");
    assert_eq!(type_text("truongwf"), "trường");
    assert_eq!(type_text("khoong"), "không");
    assert_eq!(type_text("merge"), "merge");
    assert_eq!(type_text("deploy"), "deploy");
    assert_eq!(type_text("issue"), "issue");
    assert_eq!(type_text("version"), "version");
    // từ lạ: giữ đúng phím đã gõ, không lấy bản Telex đã gộp
    assert_eq!(type_text("Asus"), "Asus");
    // âm tiết đúng luật nhưng không ai dùng không được thắng phím gốc
    assert_eq!(type_text("ROAS"), "ROAS");
}

#[test]
fn decides_before_the_word_ends() {
    // "mẻ" hiếm nên "mer" đã hiện tiếng Anh; muộn nhất tới "g" (hết đường tiếng Việt)
    assert!(settled_at("khoong", "merge", Lang::En) <= 4);
    // sau từ tiếng Anh: "tru" hai bên giống nhau, "truo" hết đường tiếng Anh -> không lúc nào hiện sai
    assert_eq!(settled_at("this", "truowfng", Lang::Vn), 1);
    assert!(settled_at("khoong", "deploy", Lang::En) <= 4);
    assert!(settled_at("khoong", "version", Lang::En) <= 5);
}

#[test]
fn learned_pairs_break_ties() {
    let (vn, en) = lex();
    let text = "mình xin visa du lịch\nanh đã xin visa chưa\nxin visa mất bao lâu\n";
    let ctx = songngu::context::Context::train(text, vn, en);
    let run = |e: &mut Engine| {
        let mut out = Vec::new();
        for w in "xin visa".split(' ') {
            w.chars().for_each(|k| {
                e.key(k);
            });
            out.push(e.commit().text);
            e.separator(' ');
        }
        out.join(" ")
    };
    assert_eq!(run(&mut Engine::new(vn, en, Config::default())), "xin vía");
    assert_eq!(run(&mut Engine::new(vn, en, Config::default()).with_context(&ctx)), "xin visa");
}

/// Gõ nguyên văn `text`: chữ cái vào từ, ký tự khác là dấu ngăn cách (như bộ gõ nhận từ bàn phím).
fn type_raw(e: &mut Engine, text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            e.key(c);
        } else {
            if !e.is_empty() {
                out += &e.commit().text;
            }
            e.separator(c);
            out.push(c);
        }
    }
    if !e.is_empty() {
        out += &e.commit().text;
    }
    out
}

#[test]
fn valid_syllables_missing_from_the_frequency_list_are_still_vietnamese() {
    // đờ, xuê, giúm: đúng luật nhưng danh sách tần suất (phụ đề) và corpus đều không có; sai luật (khă) thì giữ phím
    let (vn, en) = lex();
    let ctx = songngu::context::Context::train(&"hôm nay trời đẹp\nplease check the log\n".repeat(5), vn, en);
    let mut e = Engine::new(vn, en, Config::default()).with_context(&ctx);
    assert_eq!(type_raw(&mut e, "ddownf xuee giums khaw"), "đờn xuê giúm khaw");
    // không ngữ cảnh: không đoán (từ Anh lạ cũng không có xác suất)
    assert_eq!(type_text("ddownf"), "ddownf");
}

#[test]
fn corpus_adds_rare_but_valid_syllables() {
    let (vn, en) = lex();
    // "xò" đúng luật ghép vần nhưng thiếu trong danh sách tần suất; "xọz" sai luật thì không thêm
    let ctx = songngu::context::Context::train(&"công nghệ xịn xò lắm\n".repeat(3), vn, en);
    let mut plain = Engine::new(vn, en, Config::default());
    assert_eq!(type_raw(&mut plain, "xinj xof"), "xịn xof");
    let mut e = Engine::new(vn, en, Config::default()).with_context(&ctx);
    assert_eq!(type_raw(&mut e, "xinj xof"), "xịn xò");
}

#[test]
fn a_glued_dot_is_not_a_sentence_end() {
    let (vn, en) = lex();
    let ctx = songngu::context::Context::train(
        &("mở file Assets.car ra xem\n".repeat(5) + &"cả nhà đều vui. Cả lớp cũng vậy\n".repeat(20)),
        vn,
        en,
    );
    let mut e = Engine::new(vn, en, Config::default()).with_context(&ctx);
    assert_eq!(type_raw(&mut e, "file Assets.car ok"), "file Assets.car ok");
    // có khoảng trắng thì vẫn là hết câu
    let mut e = Engine::new(vn, en, Config::default()).with_context(&ctx);
    assert_eq!(type_raw(&mut e, "vui. Car"), "vui. Cả");
}

#[test]
fn binary_model_gives_the_same_decisions() {
    let (vn, en) = lex();
    let text = "mình xin visa du lịch\nanh đã xin visa chưa\nxin visa mất bao lâu\ndeploy lên server trước\n";
    let ctx = songngu::context::Context::train(text, vn, en);
    let path = std::env::temp_dir().join(format!("songngu-test-{}.bin", std::process::id()));
    songngu::save_model(&path, vn, en, &ctx).unwrap();
    let (vn2, en2, ctx2) = songngu::open_model(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let run = |e: &mut Engine| {
        let mut out = Vec::new();
        for w in "xin visa deploy leen server truowcs".split(' ') {
            let mut shown = Vec::new();
            w.chars().for_each(|k| shown.push(e.key(k).text));
            let v = e.commit();
            out.push(format!("{shown:?} {} {:?} {:?}", v.text, v.vn_p, v.en_p));
            e.separator(' ');
        }
        out
    };
    let a = run(&mut Engine::new(vn, en, Config::default()).with_context(&ctx));
    let b = run(&mut Engine::new(&vn2, &en2, Config::default()).with_context(&ctx2));
    assert_eq!(a, b);
    assert!(songngu::open_model(std::path::Path::new("/nonexistent/model.bin")).is_err());
}

/// Gõ từng từ; từ kết thúc bằng `~` thì lật ngay sau khi chốt.
fn type_with_flips(e: &mut Engine, text: &str) -> String {
    let mut out = Vec::new();
    for w in text.split(' ') {
        let (keys, flip) = w.strip_suffix('~').map_or((w, false), |k| (k, true));
        keys.chars().for_each(|k| {
            e.key(k);
        });
        let mut t = e.commit().text;
        if flip {
            t = e.flip_last().expect("flip").1;
        }
        e.separator(' ');
        out.push(t);
    }
    out.join(" ")
}

#[test]
fn flip_is_remembered() {
    let (vn, en) = lex();
    let mut e = Engine::new(vn, en, Config::default());
    // không ngữ cảnh học sẵn, "xin visa" ra "xin vía"; người dùng lật một lần thì lần sau cùng ngữ cảnh tự đúng
    assert_eq!(type_with_flips(&mut e, "xin visa~"), "xin visa");
    assert_eq!(type_with_flips(&mut e, "xin visa"), "xin visa");
    // lật lại là quay về phương án cũ
    "visa".chars().for_each(|k| {
        e.key(k);
    });
    e.commit();
    let (a, b) = e.flip_last().unwrap();
    assert_eq!(e.flip_last().unwrap(), (b, a));
    // hai phương án giống nhau: không có gì để lật
    "con".chars().for_each(|k| {
        e.key(k);
    });
    e.commit();
    assert!(e.flip_last().is_none());
}

#[test]
fn memory_survives_save_and_load() {
    let (vn, en) = lex();
    let mut e = Engine::new(vn, en, Config::default());
    type_with_flips(&mut e, "xin visa~");
    let mut fresh = Engine::new(vn, en, Config::default());
    fresh.set_memory(songngu::memory::Memory::load(&e.memory().save(), vn));
    assert_eq!(type_with_flips(&mut fresh, "xin visa"), "xin visa");
}

#[test]
fn context_breaks_ties() {
    assert_eq!(type_text("cos"), "có");
    assert_eq!(type_text("thif"), "thì");
    assert_eq!(type_text("I think this"), "I think this");
}

#[test]
#[ignore]
fn print_uncovered_syllables() {
    use songngu::syllable::{decompose, generate, tone_allowed};
    let g = generate();
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vi_50k.txt")).unwrap();
    let mut n = 0;
    for line in text.lines() {
        use unicode_normalization::UnicodeNormalization;
        let mut it = line.split_whitespace();
        let (w, c) = (it.next().unwrap().nfc().collect::<String>(), it.next().unwrap());
        let w = w.as_str();
        let vn_chars = w.chars().any(|ch| !ch.is_ascii());
        if let Some((s, t)) = decompose(w) {
            if vn_chars && !(g.contains_key(&s) && tone_allowed(&s, t)) && n < 60 {
                n += 1;
                print!("{w}:{c} ");
            }
        }
    }
    println!();
}
