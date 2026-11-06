//! Hành vi bộ gõ ở mức phím: chữ đưa vào app, thay chữ trước con trỏ (API thay chữ hoặc xoá lùi).

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use songngu::context::Context;
use songngu::engine::{Config, Engine};
use songngu::ime::{AppKind, Event, Session};
use songngu::lexicon::{EnLex, VnLex};

fn lex() -> &'static (VnLex, EnLex) {
    static L: OnceLock<(VnLex, EnLex)> = OnceLock::new();
    L.get_or_init(|| songngu::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data")).unwrap())
}

fn session() -> Session<'static> {
    let (vn, en) = lex();
    Session::new(Engine::new(vn, en, Config::default()))
}

/// Đường xoá lùi (terminal, app chặn phím): không đọc chữ trong app.
fn blind() -> Session<'static> {
    let mut s = session();
    s.blind = true;
    s
}

/// Ngữ cảnh nhỏ: "xin visa" hay gặp, nhưng "may" chỉ đi sau "vía". Câu đệm để "may" không phổ biến quá mức.
fn ctx() -> &'static Context {
    static C: OnceLock<Context> = OnceLock::new();
    C.get_or_init(|| {
        let (vn, en) = lex();
        let text = "làm visa rồi\n".repeat(30)
            + &"anh xin visa\n".repeat(2)
            + &"cầu vía may mắn\n".repeat(20)
            + &"hôm nay trời đẹp quá đi chơi thôi\n".repeat(30)
            + &"please check the deploy log before the review today\n".repeat(30);
        Context::train(&text, vn, en)
    })
}

/// Giả lập app có ô nhập, áp `Action` như lớp OS: đường thay chữ dùng API thay chữ của app, đường xoá lùi bắn
/// Backspace (luôn có tác dụng).
#[derive(Default)]
struct App {
    doc: String,
    passed: Vec<char>,
    /// App không cho đọc chữ (cũng không cho thay).
    opaque: bool,
    /// App cho đọc nhưng bỏ qua vùng thay (chèn vào cuối).
    ignores_replace: bool,
    /// Số lần phải xoá chữ trong app.
    erases: usize,
}

impl App {
    /// `<` Backspace, `~` Esc, `^` Shift+Esc, `|` phím khác (Enter, click…).
    fn run(&mut self, s: &mut Session, keys: &str) -> &mut Self {
        for c in keys.chars() {
            let ev = match c {
                '<' => Event::Backspace,
                '~' => Event::Escape,
                '^' => Event::Flip,
                '|' => Event::Other,
                c => Event::Char(c),
            };
            let before = (!self.opaque && !s.blind).then(|| self.doc.clone());
            let a = s.handle(ev, before.as_deref());
            if a.erase > 0 {
                self.erases += 1;
            }
            if s.blind || !self.ignores_replace {
                for _ in 0..a.erase {
                    self.doc.pop();
                }
            }
            self.doc += &a.commit;
            if a.pass {
                self.passed.push(c);
                if c == '<' {
                    self.doc.pop();
                }
            }
        }
        self
    }
}

#[test]
fn typing_rewrites_the_word_in_place() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "truowfng deploy tas");
        assert_eq!(app.doc, "trường deploy tá");
        app.run(&mut s, "k ");
        assert_eq!(app.doc, "trường deploy task ");
        assert!(s.stats.rewrites > 0);
        assert!(app.erases > 0);
    }
}

#[test]
fn backspace_goes_to_the_app_when_only_the_last_letter_goes() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "tieengs<");
        assert_eq!(app.doc, "tiến");
        assert_eq!(app.passed, vec!['<']);
        app.run(&mut s, "g ");
        assert_eq!(app.doc, "tiếng ");
    }
}

#[test]
fn backspace_keeps_the_tone_and_the_engine_in_step() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        assert_eq!(app.run(&mut s, "tieengs<<<").doc, "ti");
        let mut app = App::default();
        assert_eq!(app.run(&mut s, " merges<").doc, " merge");
    }
}

#[test]
fn backspace_after_a_word_goes_to_the_app() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "truowfng <");
        assert_eq!(app.doc, "trường");
    }
}

#[test]
fn shift_escape_flips_the_current_and_the_last_word() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "khoong^");
        assert_eq!(app.doc, "khoong");
        app.run(&mut s, " truowfng ^");
        assert_eq!(app.doc, "khoong truowfng ");
        app.run(&mut s, "^");
        assert_eq!(app.doc, "khoong trường ");
    }
}

#[test]
fn escape_flips_what_was_just_typed_even_when_sure() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "truowfng ~");
        assert_eq!(app.doc, "truowfng ");
        s.esc_undo_window = Duration::ZERO;
        app.run(&mut s, "khoong~");
        assert_eq!(app.doc, "truowfng khoong");
        assert!(app.passed.is_empty());
    }
}

#[test]
fn escape_with_nothing_to_flip_belongs_to_the_app() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        // không có phương án khác, hoặc đã qua phím khác (Enter): Esc của app
        app.run(&mut s, "ok~");
        app.run(&mut s, " truowfng|~");
        assert_eq!(app.doc, "ok trường");
        assert_eq!(app.passed, vec!['~', '|', '~']);
    }
}

#[test]
fn second_escape_right_after_a_flip_undoes_it_and_reaches_the_app() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        // không ngữ cảnh, "visa" sau "xin" hai bên sát nhau: Esc lật
        app.run(&mut s, "xin visa ~");
        assert_eq!(app.doc, "xin visa ");
        assert!(!s.eng.memory().is_empty());
        assert!(app.passed.is_empty());
        // Esc ngay sau đó: ý là Esc cho app -> trả lại chữ, quên điều vừa học, app nhận Esc
        app.run(&mut s, "~");
        assert_eq!(app.doc, "xin vía ");
        assert!(s.eng.memory().is_empty());
        assert_eq!(app.passed, vec!['~']);
    }
}

#[test]
fn escape_belongs_to_code_apps_but_shift_escape_still_flips() {
    for mut s in [session(), blind()] {
        s.app = AppKind::Code;
        let mut app = App::default();
        app.run(&mut s, "khoong~");
        assert_eq!(app.doc, "không");
        assert_eq!(app.passed, vec!['~']);
        let mut app = App::default();
        app.run(&mut s, "khoong^");
        assert_eq!(app.doc, "khoong");
    }
}

#[test]
fn escape_undoes_an_automatic_change_of_the_previous_word() {
    let (vn, en) = lex();
    let fresh = |blind: bool| {
        let mut s = Session::new(Engine::new(vn, en, Config::default()).with_context(ctx()));
        s.blind = blind;
        s.unsure_ratio = 1e30; // mọi từ có hai phương án đều tính là còn phân vân
        s
    };
    for blind in [false, true] {
        let mut s = fresh(blind);
        let mut app = App::default();
        app.run(&mut s, "xin visa may ");
        assert_eq!(app.doc, "xin vía may ", "blind {blind}: từ sau đổi từ trước");
        app.run(&mut s, "~");
        assert_eq!(app.doc, "xin visa may ", "blind {blind}: Esc hoàn tác");
        assert!(!s.eng.memory().is_empty());
        // đã gõ sang từ khác thì Esc không còn đụng lần tự đổi đó
        let mut s = fresh(blind);
        let mut app = App::default();
        app.run(&mut s, "xin visa may mawns ~");
        assert!(app.doc.starts_with("xin vía may "), "blind {blind}: {}", app.doc);
    }
}

#[test]
fn other_keys_end_the_word() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "xin vi|");
        assert_eq!(app.doc, "xin vi");
        assert_eq!(app.passed, vec!['|']);
    }
    // xoá lùi không đọc được chữ: sau phím đó (con trỏ có thể đã đi) không đụng chữ trước nữa
    let mut s = blind();
    let mut app = App::default();
    app.run(&mut s, "xin vi|s");
    assert_eq!(app.doc, "xin vis");
}

#[test]
fn excluded_apps_get_raw_keys() {
    let mut s = session();
    s.app = AppKind::Excluded;
    let mut app = App::default();
    app.run(&mut s, "truowfng");
    assert_eq!(app.passed.len(), 8);
}

#[test]
fn forget_clears_what_was_learned() {
    let mut s = session();
    let mut app = App::default();
    app.run(&mut s, "truowfng ^");
    assert!(!s.eng.memory().is_empty());
    s.forget();
    assert!(s.eng.memory().is_empty());
}

// ---- đường thay chữ: đọc chữ trong app ----

#[test]
fn cursor_moved_by_a_click_starts_a_new_word_without_touching_old_text() {
    let mut s = session();
    let mut app = App::default();
    app.run(&mut s, "xin tie");
    // người dùng click về đầu dòng (bộ gõ không được báo): chữ trước con trỏ giờ khác hẳn
    let rest = std::mem::take(&mut app.doc);
    app.run(&mut s, "ees");
    assert_eq!(app.doc, "ế");
    assert_eq!(rest, "xin tie");
}

#[test]
fn one_cursor_move_is_not_a_lagging_app() {
    let mut s = session();
    let mut app = App::default();
    app.run(&mut s, "xin tie");
    app.doc.clear();
    app.run(&mut s, "ees khoong");
    assert!(!s.blind);
    assert_eq!(app.doc, "ế không");
}

#[test]
fn deleting_the_space_after_a_word_lets_typing_continue_into_it() {
    for mut s in [session(), blind()] {
        let mut app = App::default();
        app.run(&mut s, "xin chao tieeng <s ");
        assert!(app.doc.ends_with(" tiếng "), "{}", app.doc);
    }
    // ô không cho đọc chữ (Zalo, vài app Flutter): nhớ từ vừa gõ
    let mut s = session();
    let mut app = App { opaque: true, ..Default::default() };
    app.run(&mut s, "xin chao tieeng <s ");
    assert!(app.doc.ends_with(" tiếng "), "{}", app.doc);
    // chưa xoá hết dấu ngăn cách: từ mới
    let mut s = blind();
    let mut app = App::default();
    app.run(&mut s, "tieeng, <s");
    assert_eq!(app.doc, "tiêng,s");
}

#[test]
fn typing_right_after_a_word_reopens_it() {
    let mut s = session();
    let mut app = App { doc: "xin chào tiêng".into(), ..Default::default() };
    // con trỏ dính vào "tiêng" (quên dấu): gõ s thêm dấu cho chính từ đó
    app.run(&mut s, "s ");
    assert_eq!(app.doc, "xin chào tiếng ");
}

#[test]
fn escape_fixes_an_earlier_word_after_the_cursor_goes_back_behind_it() {
    let mut s = session();
    let mut app = App::default();
    app.run(&mut s, "xin visa may mawns ");
    assert_eq!(app.doc, "xin vía may mắn ");
    // click vào ngay sau "vía" (phần sau con trỏ không liên quan)
    app.doc.truncate("xin vía".len());
    s.esc_undo_window = Duration::ZERO; // bấm lại sau một lúc: lật lại chứ không chuyển Esc cho app
    app.run(&mut s, "|~");
    assert_eq!(app.doc, "xin visa");
    app.run(&mut s, "~");
    assert_eq!(app.doc, "xin vía");
    assert!(!s.eng.memory().is_empty());
}

#[test]
fn escape_leaves_text_typed_elsewhere_alone_but_shift_escape_flips_it() {
    let mut s = session();
    let mut app = App { doc: "hôm nay tét".into(), ..Default::default() };
    app.run(&mut s, "~");
    assert_eq!(app.doc, "hôm nay tét");
    assert_eq!(app.passed, vec!['~']);
    // không biết phím đã gõ: đặt phím dấu thanh ở chỗ cho ra từ Anh có thật (te-s-t, không phải tets)
    app.run(&mut s, "^");
    assert_eq!(app.doc, "hôm nay test");
    let mut app = App { doc: "xin visa ".into(), ..Default::default() };
    app.run(&mut s, "^");
    assert_eq!(app.doc, "xin vía ");
}

// ---- app không thay chữ được: chuyển sang xoá lùi ----

#[test]
fn app_that_ignores_replacement_switches_to_backspace() {
    let mut s = session();
    let mut app = App { ignores_replace: true, ..Default::default() };
    // "ta" -> "tá" là lần thay đầu tiên; app chèn vào cuối thành "taá", phím sau phát hiện
    app.run(&mut s, "tas");
    app.run(&mut s, "k");
    assert!(s.blind);
    assert!(s.take_direct_broke());
    app.run(&mut s, " truowfng duodwcj ");
    assert!(app.doc.ends_with(" trường được "), "{}", app.doc);
}

#[test]
fn app_that_hides_its_text_still_gets_rewrites_by_position() {
    // Zalo, vài app Flutter: không cho đọc chữ nhưng thay theo vị trí con trỏ được
    let mut s = session();
    let mut app = App { opaque: true, ..Default::default() };
    app.run(&mut s, "truowfng tas");
    assert_eq!(app.doc, "trường tá");
    app.run(&mut s, "k ddi duodwcj ");
    assert_eq!(app.doc, "trường task đi được ");
    assert!(!s.blind);
    assert!(!s.take_direct_broke());
}
