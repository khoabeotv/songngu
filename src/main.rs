//! songngu type "<phím>"       : xem chữ hiện ra sau từng phím
//! songngu eval <văn bản>       : gõ cả file (mỗi dòng một tin nhắn) qua engine, đo từ sai / nhảy chữ
//! songngu eval-session <văn bản>: như eval nhưng qua lớp bộ gõ, chấm chữ cuối cùng nằm trong app
//! songngu keys <văn bản>       : in chuỗi phím (\x08 = Backspace, hết dòng = Enter) cho platform/macos/bench.c
//! songngu score <got> <want>, bench <văn bản>, ambig <văn bản> --out f, stats
//! Cách gõ (src/typing.rs): --realistic, --typos R, --seed N.
//! Tùy chọn chung: --data dir, --ctx văn_bản_song_ngữ, --mu x, --no-bigram, --flip x, --stick x, --classic

use std::path::PathBuf;
use std::time::Instant;

use unicode_normalization::UnicodeNormalization;

use songngu::engine::{Config, Engine, Lang, View};
use songngu::syllable::split_tone;
use songngu::typing::{scored_words, Habits, Typist, Unit, BS, SKIP};

const VALUE_OPTS: [&str; 27] = [
    "--clicks",
    "--bs-ratio",
    "--look-min",
    "--late-penalty",
    "--late-d-penalty",
    "--typos",
    "--seed",
    "--penalty",
    "--informative",
    "--unsure",
    "--lookahead",
    "--data",
    "--stick",
    "--flip",
    "--ctx",
    "--model",
    "--mu",
    "--show",
    "--dump",
    "--dump-want",
    "--cache",
    "--unseen-vn",
    "--word-switch",
    "--band",
    "--out",
    "--memory",
    "--save-memory",
];

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// Đối số không phải tùy chọn (bỏ cả giá trị đi kèm tùy chọn).
fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if VALUE_OPTS.contains(&a.as_str()) {
            skip = true;
        } else if !a.starts_with("--") {
            out.push(a.clone());
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let data = PathBuf::from(arg(&args, "--data").unwrap_or_else(|| "data".into()));
    let t = Instant::now();
    // --model: từ điển chung nằm sẵn trong file mô hình
    let (vn, en, model) = match (arg(&args, "--ctx"), arg(&args, "--model")) {
        (None, Some(p)) => {
            let (vn, en, c) = songngu::open_model(&PathBuf::from(p)).expect("cannot read --model");
            (vn, en, Some(c))
        }
        _ => {
            let (vn, en) = songngu::load(&data).expect("cannot read data dir");
            (vn, en, None)
        }
    };
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut cfg = Config::default();
    if let Some(v) = arg(&args, "--stick") {
        cfg.stick = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--flip") {
        cfg.flip_ratio = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--mu") {
        cfg.dom_weight = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--cache") {
        cfg.cache_weight = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--penalty") {
        cfg.odd_penalty = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--late-penalty") {
        cfg.late_penalty = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--late-d-penalty") {
        cfg.late_d_penalty = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--informative") {
        cfg.informative_ratio = v.parse().unwrap();
    }
    cfg.bigram = !args.iter().any(|a| a == "--no-bigram");
    cfg.lookahead_switch = !args.iter().any(|a| a == "--no-look-switch");
    cfg.join = !args.iter().any(|a| a == "--no-join");
    if let Some(v) = arg(&args, "--unseen-vn") {
        cfg.unseen_vn = v.parse().unwrap();
    }
    if let Some(v) = arg(&args, "--word-switch") {
        cfg.word_switch = v.parse().unwrap();
    }
    cfg.learn = !args.iter().any(|a| a == "--no-learn");
    cfg.modern = !args.iter().any(|a| a == "--classic");
    let t = Instant::now();
    let ctx = match arg(&args, "--ctx") {
        Some(p) => Some(songngu::load_context(&p, &vn, &en).expect("cannot read --ctx")),
        None => model,
    };
    let ctx_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut eng = Engine::new(&vn, &en, cfg);
    if let Some(c) = &ctx {
        eng = eng.with_context(c);
    }
    if let Some(p) = arg(&args, "--memory") {
        eng.set_memory(songngu::memory::Memory::load(&std::fs::read_to_string(p).unwrap_or_default(), &vn));
    }
    let pos = positional(&args);
    match pos.first().map(String::as_str) {
        Some("stats") => {
            println!("vn syllables {}  coverage of vi_50k mass {:.2}%", vn.len(), vn.coverage * 100.0);
            println!("en words {}  load {load_ms:.0} ms", en.len());
            if let Some(c) = &ctx {
                println!("{}  (train {ctx_ms:.0} ms)", c.stats());
            }
        }
        Some("type") => type_cmd(&mut eng, &pos[1..]),
        Some("save-model") => {
            let c = ctx.as_ref().expect("save-model needs --ctx");
            songngu::save_model(&PathBuf::from(&pos[1]), &vn, &en, c).unwrap();
            println!("{}  -> {}", c.stats(), pos[1]);
        }
        Some("eval") => eval_cmd(&mut eng, &pos[1], &mut typist(&args), &args),
        Some("ambig") => {
            // phiên song song (cùng model) để biết lựa chọn cuối sau lookahead của bộ gõ
            let mut e2 = Engine::new(&vn, &en, eng.cfg.clone());
            if let Some(c) = &ctx {
                e2 = e2.with_context(c);
            }
            let mut s = songngu::ime::Session::new(e2);
            ambig_cmd(&mut eng, &mut s, &pos[1], &mut typist(&args), &args)
        }
        Some("bench") => bench_cmd(&mut eng, &pos[1], &mut typist(&args)),
        Some("keys") => keys_cmd(&pos[1], &mut typist(&args)),
        Some("eval-session") => {
            let unsure = arg(&args, "--unsure").map_or(1000.0, |v| v.parse().unwrap());
            let look = arg(&args, "--lookahead").map_or(3.0, |v| v.parse().unwrap());
            let margin = arg(&args, "--look-margin").map_or(1e4, |v| v.parse().unwrap());
            let blind = args.iter().any(|a| a == "--blind");
            let bs_ratio = arg(&args, "--bs-ratio").map(|v| v.parse().unwrap());
            let look_min = arg(&args, "--look-min").map(|v| v.parse().unwrap());
            let dump = (arg(&args, "--dump"), arg(&args, "--dump-want"));
            let show = arg(&args, "--show").map_or(0, |v| v.parse().unwrap());
            let mut t = typist(&args);
            let opts = (unsure, look, blind, margin, bs_ratio, look_min);
            let sim = (arg(&args, "--clicks").map_or(0, |v| v.parse().unwrap()), !args.iter().any(|a| a == "--no-resume"));
            session_eval(eng, &pos[1], &mut t, opts, show, (dump.0.as_deref(), dump.1.as_deref()), sim)
        }
        Some("score") => score_cmd(&pos[1], &pos[2], arg(&args, "--show").map_or(0, |v| v.parse().unwrap())),
        _ => eprintln!(
            "usage: songngu stats | type \"<keys>\" | eval|eval-session|bench|keys <text> [--realistic] [--typos R] [--seed N] [--show N] [--dump got] [--dump-want want] | ambig <text> --out f | score <got> <want>"
        ),
    }
}

/// Người gõ giả lập theo tùy chọn: mặc định gõ chuẩn; --realistic thêm thói quen gõ và gõ nhầm có sửa; --typos R gõ nhầm không sửa.
fn typist(args: &[String]) -> Typist {
    let mut h = if args.iter().any(|a| a == "--realistic") { Habits::realistic() } else { Habits::default() };
    if let Some(v) = arg(args, "--typos") {
        h.typo = v.parse().unwrap();
    }
    Typist::new(h, arg(args, "--seed").map_or(1, |v| v.parse().unwrap()))
}

/// Các dòng không rỗng của file, chuẩn NFC.
fn read_lines(path: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines().map(|l| l.nfc().collect::<String>()).filter(|l| !l.trim().is_empty()).collect()
}

fn keys_cmd(path: &str, typist: &mut Typist) {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for line in read_lines(path) {
        let mut s: String = typist
            .line(&line)
            .into_iter()
            .map(|u| match u {
                Unit::Word { keys, .. } => keys,
                Unit::Sep(s) => s,
            })
            .collect();
        s.push('\n');
        // dừng êm khi bên đọc đóng ống (| head)
        if out.write_all(s.as_bytes()).is_err() {
            return;
        }
    }
}

fn fmt_p(p: Option<f64>) -> String {
    p.map_or("-".into(), |p| format!("{:.1}", p.log10()))
}

fn type_cmd(eng: &mut Engine, args: &[String]) {
    let text = args.join(" ");
    let mut out = String::new();
    let show = |k: &str, v: &View| {
        println!(
            "  {k:>6}  {:<12} {:?}  vn {:<10} log10p {:>5}   en {:<10} log10p {:>5}",
            v.text,
            v.lang,
            v.vn_text,
            fmt_p(v.vn_p),
            v.en_text,
            fmt_p(v.en_p)
        )
    };
    // `~` = người dùng bấm phím lật từ vừa chốt.
    let mut last_at: Option<usize> = None;
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            let v = eng.key(c);
            show(&c.to_string(), &v);
            continue;
        }
        if !eng.is_empty() {
            let v = eng.commit();
            show("commit", &v);
            last_at = Some(out.len());
            out += &v.text;
        }
        if c == '~' {
            if let (Some((old, new)), Some(at)) = (eng.flip_last(), last_at) {
                println!("    flip  {old} -> {new}");
                out.replace_range(at..at + old.len(), &new);
            }
            continue;
        }
        eng.separator(c);
        out.push(c);
    }
    if !eng.is_empty() {
        let v = eng.commit();
        show("commit", &v);
        out += &v.text;
    }
    println!("=> {out}");
}

/// So khớp bỏ qua vị trí dấu thanh (hoà == hòa).
fn canon(w: &str) -> String {
    let mut tone = 0;
    let mut out = String::new();
    for c in w.chars() {
        let lc = c.to_lowercase().next().unwrap();
        let (b, t) = split_tone(lc);
        if t.idx() != 0 {
            tone = t.idx();
        }
        if c.is_uppercase() {
            out.extend(b.to_uppercase());
        } else {
            out.push(b);
        }
    }
    if tone != 0 {
        out += &tone.to_string();
    }
    out
}

/// Số từ được chấm của `want` không khớp `got` (so theo dãy con chung dài nhất). Từ `SKIP` không khớp gì nên không đổi
/// dãy chung của các từ còn lại, chỉ cần bỏ khỏi số đếm.
fn wrong_words(got: &str, want: &str) -> usize {
    let x: Vec<String> = got.split_whitespace().map(canon).collect();
    let y: Vec<String> = want.split_whitespace().map(canon).collect();
    let skipped = y.iter().filter(|w| w.contains(SKIP)).count();
    let mut l = vec![vec![0usize; y.len() + 1]; x.len() + 1];
    for i in 1..=x.len() {
        for j in 1..=y.len() {
            l[i][j] = if x[i - 1] == y[j - 1] { l[i - 1][j - 1] + 1 } else { l[i - 1][j].max(l[i][j - 1]) };
        }
    }
    y.len() - skipped - l[x.len()][y.len()]
}

/// Gõ corpus qua đúng lớp bộ gõ (đường thay chữ, hoặc `--blind` đường xoá lùi), đo chữ cuối cùng nằm trong app.
fn session_eval(
    eng: Engine,
    path: &str,
    typist: &mut Typist,
    opts: (f64, f64, bool, f64, Option<f64>, Option<f64>),
    mut show: usize,
    dump: (Option<&str>, Option<&str>),
    sim: (usize, bool),
) {
    use songngu::ime::{Event, Session};
    use std::io::Write;
    // mỗi dòng một tin nhắn, để chấm lại bằng `score`
    let mut dump_got = dump.0.map(|p| std::fs::File::create(p).unwrap());
    let mut dump_want = dump.1.map(|p| std::fs::File::create(p).unwrap());
    let mut s = Session::new(eng);
    (s.unsure_ratio, s.lookahead_ratio, s.lookahead_margin) = (opts.0, opts.1, opts.3);
    s.blind = opts.2;
    // `--clicks N`: cứ N từ thì giả như người dùng click/đổi app rồi quay lại gõ tiếp (ngữ cảnh mất, chữ vẫn trong app)
    let (clicks, mut nword) = (sim.0, 0usize);
    s.resume_context = sim.1;
    let (mut wrong, mut words) = (0usize, 0usize);
    // phím mà Esc lật từ đang gõ (hai phương án chênh dưới unsure_ratio)
    let (mut keys_n, mut keys_unsure) = (0usize, 0usize);
    // từ sai lúc vừa chốt, và trong đó bao nhiêu từ Backspace ngay sau lật được
    let (mut wrong_at_commit, mut bs_fixable) = (0usize, 0usize);
    if let Some(r) = opts.4 {
        s.backspace_ratio = r;
    }
    if let Some(v) = opts.5 {
        s.lookahead_min_followers = v;
    }
    // Áp một Action lên dòng đang gõ như app thật (xoá rồi chèn trước con trỏ, Backspace đi thẳng vào app).
    let press = |s: &mut Session, doc: &mut String, ev: Event| {
        let before = s.wants_before().then(|| doc.clone());
        let a = s.handle(ev, before.as_deref());
        for _ in 0..a.erase {
            doc.pop();
        }
        *doc += &a.commit;
        if a.pass && ev == Event::Backspace {
            doc.pop();
        }
    };
    for line in read_lines(path) {
        let (mut doc, mut want) = (String::new(), String::new());
        let mut last_w: Option<String> = None;
        for u in typist.line(&line) {
            match u {
                Unit::Word { keys, want: w } => {
                    want += &w;
                    last_w = Some(w.clone());
                    nword += 1;
                    if clicks > 0 && nword % clicks == 0 && !doc.is_empty() {
                        s.commit_all();
                    }
                    for k in keys.chars() {
                        press(&mut s, &mut doc, if k == BS { Event::Backspace } else { Event::Char(k) });
                        let v = s.eng.view();
                        keys_n += 1;
                        if let (true, Some(a), Some(b)) = (v.differs(), v.vn_p, v.en_p) {
                            keys_unsure += usize::from((a.ln() - b.ln()).abs() < opts.0.ln());
                        }
                    }
                }
                Unit::Sep(sep) => {
                    for c in sep.chars() {
                        let flag = s.stats.backspace_flippable;
                        press(&mut s, &mut doc, Event::Char(c));
                        if let Some(w) = last_w.take() {
                            let body = &doc[..doc.len() - c.len_utf8()];
                            if !body.to_lowercase().ends_with(&w.to_lowercase()) {
                                wrong_at_commit += 1;
                                bs_fixable += usize::from(s.stats.backspace_flippable > flag);
                            }
                        }
                    }
                    want += &sep;
                }
            }
        }
        press(&mut s, &mut doc, Event::Other);
        if let Some(f) = dump_got.as_mut() {
            writeln!(f, "{}", doc.replace('\n', " ")).unwrap();
        }
        if let Some(f) = dump_want.as_mut() {
            writeln!(f, "{}", want.replace('\n', " ")).unwrap();
        }
        let n = wrong_words(&doc, &want);
        if n > 0 && show > 0 {
            show -= 1;
            eprintln!("GOT  {doc}\nWANT {want}\n");
        }
        wrong += n;
        words += scored_words(&want);
    }
    let st = &s.stats;
    println!(
        "words {words}  wrong {wrong} ({:.2}%)  unsure words {:.2}%  backspace-flippable {:.2}%  lookahead flips {} ({:.3}%)  replace ops {:.2}/word  unsure keys {:.2}%  wrong at commit {wrong_at_commit} (backspace fixes {bs_fixable}){}",
        100.0 * wrong as f64 / words as f64,
        100.0 * st.unsure as f64 / st.words.max(1) as f64,
        100.0 * st.backspace_flippable as f64 / st.words.max(1) as f64,
        st.lookahead_flips,
        100.0 * st.lookahead_flips as f64 / st.words.max(1) as f64,
        st.rewrites as f64 / st.words.max(1) as f64,
        100.0 * keys_unsure as f64 / keys_n.max(1) as f64,
        if s.blind == opts.2 { "" } else { "  (fell back to blind!)" }
    );
}

/// Thời gian mỗi phím chỉ ở engine (không qua lớp bộ gõ): phím chữ, Backspace, chốt từ + dấu ngăn cách.
fn bench_cmd(eng: &mut Engine, path: &str, typist: &mut Typist) {
    let mut ns: Vec<u64> = Vec::new();
    // theo loại: 0 phím đầu từ, 1 phím sau, 2 chốt từ/dấu ngăn cách
    let mut by: [Vec<u64>; 3] = Default::default();
    let mut time = |kind: usize, f: &mut dyn FnMut()| {
        let t = Instant::now();
        f();
        let d = t.elapsed().as_nanos() as u64;
        ns.push(d);
        by[kind].push(d);
    };
    let lines: Vec<Vec<Unit>> = read_lines(path).iter().map(|l| typist.line(l)).collect();
    let wall = Instant::now();
    for units in &lines {
        for u in units {
            match u {
                Unit::Word { keys, .. } => {
                    for k in keys.chars() {
                        let kind = usize::from(!eng.is_empty());
                        if k == BS {
                            time(kind, &mut || {
                                std::hint::black_box(eng.backspace());
                            });
                        } else {
                            time(kind, &mut || {
                                std::hint::black_box(eng.key(k));
                            });
                        }
                    }
                }
                Unit::Sep(s) => {
                    for c in s.chars() {
                        time(2, &mut || {
                            if !eng.is_empty() {
                                std::hint::black_box(eng.commit());
                            }
                            eng.separator(c);
                        });
                    }
                }
            }
        }
        time(2, &mut || {
            if !eng.is_empty() {
                std::hint::black_box(eng.commit());
            }
            eng.separator('\n');
        });
    }
    let secs = wall.elapsed().as_secs_f64();
    for (name, v) in ["first key", "next keys", "commit"].iter().zip(by.iter_mut()) {
        v.sort_unstable();
        let q = |x: f64| v[((v.len() - 1) as f64 * x) as usize];
        println!("  {name:<10} n {:>7}  p50 {:>6} ns  p99 {:>6} ns", v.len(), q(0.5), q(0.99));
    }
    ns.sort_unstable();
    let avg = ns.iter().sum::<u64>() as f64 / ns.len() as f64;
    let p = |q: f64| ns[((ns.len() - 1) as f64 * q) as usize];
    println!(
        "events {}  avg {avg:.0} ns  p50 {} ns  p95 {} ns  p99 {} ns  p99.9 {} ns  max {} ns  throughput {:.0} keys/s",
        ns.len(),
        p(0.5),
        p(0.95),
        p(0.99),
        p(0.999),
        ns.last().unwrap(),
        ns.len() as f64 / secs
    );
}

fn score_cmd(got: &str, want: &str, mut show: usize) {
    let g = std::fs::read_to_string(got).unwrap();
    let w = std::fs::read_to_string(want).unwrap();
    let (mut words, mut wrong, mut lines, mut perfect) = (0, 0, 0, 0);
    for (a, b) in g.lines().zip(w.lines()) {
        let n = wrong_words(a, b);
        words += b.split_whitespace().count();
        wrong += n;
        lines += 1;
        if n == 0 {
            perfect += 1;
        } else if show > 0 {
            show -= 1;
            eprintln!("GOT  {a}\nWANT {b}\n");
        }
    }
    println!(
        "lines {lines} (got {}, want {})  words {words}  wrong {wrong} ({:.2}%)  perfect lines {:.1}%",
        g.lines().count(),
        w.lines().count(),
        100.0 * wrong as f64 / words as f64,
        100.0 * perfect as f64 / lines as f64
    );
}

#[derive(Default)]
struct WordStat {
    last: Option<View>,
    shown: Option<Lang>,
    flips: usize,
    frames: Vec<String>,
    want: String,
}

/// Gõ câu qua engine; mỗi từ mà cả hai phương án đều có điểm và chênh dưới `band` lần ghi một dòng mẫu cho bộ chọn lại:
/// `nhãn(1 = Việt) <TAB> engine chọn <TAB> ln P(việt)/P(anh) <TAB> kiểu hoa <TAB> 4 từ trước <TAB> phương án Việt
/// <TAB> phương án Anh <TAB> 3 từ sau`. Ngữ cảnh lấy từ chữ gốc (khoá `canon`, `<s>` = hết câu, `<num>` = số).
/// Gõ cả dòng qua bộ gõ (như session_eval) rồi tách chữ cuối cùng theo từng từ; None nếu không khớp số từ.
fn session_words(s: &mut songngu::ime::Session, units: &[Unit]) -> Option<Vec<String>> {
    use songngu::ime::Event;
    let mut doc = String::new();
    let press = |s: &mut songngu::ime::Session, doc: &mut String, ev: Event| {
        let before = s.wants_before().then(|| doc.clone());
        let a = s.handle(ev, before.as_deref());
        for _ in 0..a.erase {
            doc.pop();
        }
        *doc += &a.commit;
        if a.pass && ev == Event::Backspace {
            doc.pop();
        }
    };
    for u in units {
        match u {
            Unit::Word { keys, .. } => {
                keys.chars().for_each(|c| press(s, &mut doc, if c == BS { Event::Backspace } else { Event::Char(c) }))
            }
            Unit::Sep(t) => t.chars().for_each(|c| press(s, &mut doc, Event::Char(c))),
        }
    }
    press(s, &mut doc, Event::Other);
    let got: Vec<String> = Typist::new(Habits::default(), 0)
        .line(&doc)
        .into_iter()
        .filter_map(|u| if let Unit::Word { want, .. } = u { Some(want) } else { None })
        .collect();
    (got.len() == units.iter().filter(|u| matches!(u, Unit::Word { .. })).count()).then_some(got)
}

fn ambig_line(
    eng: &mut Engine,
    sess: &mut songngu::ime::Session,
    units: &[Unit],
    band: f64,
    out: &mut impl std::io::Write,
    st: &mut [usize; 6],
) {
    use songngu::context::after_separator;
    let final_words = session_words(sess, units);
    st[5] += usize::from(final_words.is_none());
    let mut toks: Vec<String> = Vec::new();
    let mut at: Vec<usize> = Vec::new();
    for u in units {
        match u {
            Unit::Word { want: g, .. } => {
                at.push(toks.len());
                toks.push(canon(g));
            }
            Unit::Sep(s) => {
                if let Some(p) = s.chars().filter_map(after_separator).last() {
                    if toks.last().map(String::as_str) != Some(p) {
                        toks.push(p.to_string());
                    }
                }
            }
        }
    }
    let ctx = |i: isize| -> &str {
        if i < 0 {
            "<s>"
        } else {
            toks.get(i as usize).map_or("</s>", String::as_str)
        }
    };
    let mut w = 0;
    for u in units {
        match u {
            Unit::Word { keys, want: gold } => {
                keys.chars().for_each(|k| {
                    if k == BS {
                        eng.backspace();
                    } else {
                        eng.key(k);
                    }
                });
                let v = eng.commit();
                let ti = at[w] as isize;
                w += 1;
                let (Some(a), Some(b)) = (v.vn_p, v.en_p) else { continue };
                let lr = a.ln() - b.ln();
                if !v.differs() || lr.abs() >= band.ln() {
                    continue;
                }
                let (cv, ce, cg) = (canon(&v.vn_text), canon(&v.en_text), canon(gold));
                let y = if cg == cv {
                    1
                } else if cg == ce {
                    0
                } else {
                    st[3] += 1;
                    continue;
                };
                let pick = usize::from(v.lang == Lang::Vn);
                st[0] += 1;
                st[1] += usize::from(pick != y);
                st[2] += usize::from(lr.abs() < 100f64.ln());
                let case = songngu::context::case_class(keys, false, false);
                let left = (1..=4).rev().map(|d| ctx(ti - d)).collect::<Vec<_>>().join(" ");
                let right = (1..=3).map(|d| ctx(ti + d)).collect::<Vec<_>>().join(" ");
                // lựa chọn cuối của bộ gõ: 1 Việt, 0 Anh, 2 khác/không rõ
                let fin = final_words.as_ref().map_or(2, |f| {
                    let c = canon(&f[w - 1]);
                    if c == cv {
                        1
                    } else if c == ce {
                        0
                    } else {
                        2
                    }
                });
                st[4] += usize::from(fin != y);
                writeln!(out, "{y}\t{pick}\t{lr:.4}\t{case}\t{left}\t{cv}\t{ce}\t{right}\t{fin}").unwrap();
            }
            Unit::Sep(s) => s.chars().for_each(|c| eng.separator(c)),
        }
    }
    eng.separator('\n');
}

fn ambig_cmd(eng: &mut Engine, sess: &mut songngu::ime::Session, input: &str, typist: &mut Typist, args: &[String]) {
    use std::io::Write;
    let band: f64 = arg(args, "--band").map_or(1e4, |v| v.parse().unwrap());
    let out_path = arg(args, "--out").expect("--out file");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path).unwrap());
    let mut st = [0usize; 6];
    for l in read_lines(input) {
        ambig_line(eng, sess, &typist.line(&l), band, &mut out, &mut st);
    }
    out.flush().unwrap();
    eprintln!(
        "{} examples (band {band:e}), engine wrong on {}, session wrong on {}, {} within ratio 100; {} words where neither candidate is right; {} lines not aligned -> {out_path}",
        st[0], st[1], st[4], st[2], st[3], st[5]
    );
}

fn eval_cmd(eng: &mut Engine, path: &str, typist: &mut Typist, args: &[String]) {
    use std::io::Write;
    let mut show: usize = arg(args, "--show").map_or(0, |v| v.parse().unwrap());
    let mut dump = arg(args, "--dump").map(|p| std::fs::File::create(p).unwrap());
    let mut dump_want = arg(args, "--dump-want").map(|p| std::fs::File::create(p).unwrap());

    let (mut words, mut late, mut flip_words, mut flips, mut keys, mut differ_words) = (0, 0, 0, 0, 0usize, 0);
    let (mut lines, mut wrong, mut perfect, mut scored) = (0usize, 0usize, 0usize, 0usize);
    // --fix: người dùng lật ngay từ sai nếu phương án còn lại đúng, không thì tự gõ lại.
    let fix = args.iter().any(|a| a == "--fix");
    let (mut corrections, mut unfixable) = (0usize, 0usize);
    // (|ln P(việt)/P(anh)| lúc chốt, chọn sai mà phương án kia đúng) cho từng từ hai phương án khác nhau
    let mut margins: Vec<(f64, bool)> = Vec::new();
    let mut line = String::new();
    let mut want = String::new();
    let mut ws: Option<WordStat> = None;
    let mut late_examples: Vec<String> = Vec::new();
    let mut flip_examples: Vec<String> = Vec::new();
    let t = Instant::now();

    let mut finish = |eng: &mut Engine, ws: &mut Option<WordStat>, line: &mut String| {
        if let Some(s) = ws.take() {
            let mut v = eng.commit();
            let judged = s.want != SKIP;
            if v.differs() && judged {
                let m = match (v.vn_p, v.en_p) {
                    (Some(a), Some(b)) => (a.ln() - b.ln()).abs(),
                    _ => f64::INFINITY,
                };
                let other = if v.lang == Lang::Vn { &v.en_text } else { &v.vn_text };
                margins.push((m, canon(&v.text) != canon(&s.want) && canon(other) == canon(&s.want)));
            }
            if fix && judged && canon(&v.text) != canon(&s.want) {
                let other = if v.lang == Lang::Vn { &v.en_text } else { &v.vn_text };
                if canon(other) == canon(&s.want) {
                    corrections += 1;
                    v.text = eng.flip_last().unwrap().1;
                } else {
                    unfixable += 1;
                    v.text = s.want.clone();
                }
            }
            words += 1;
            if v.differs() {
                differ_words += 1;
            }
            if s.flips > 0 {
                flip_words += 1;
                flips += s.flips;
            }
            if let Some(last) = s.last {
                if last.text != v.text {
                    late += 1;
                    if late_examples.len() < 15 {
                        late_examples.push(format!("{} -> {}", last.text, v.text));
                    }
                }
            }
            line.push_str(&v.text);
        }
    };

    for text in read_lines(path) {
        for u in typist.line(&text) {
            match u {
                Unit::Word { keys: word_keys, want: word_want } => {
                    finish(eng, &mut ws, &mut line);
                    want += &word_want;
                    let mut s = WordStat { want: word_want, ..Default::default() };
                    for k in word_keys.chars() {
                        keys += 1;
                        let v = if k == BS {
                            match eng.backspace() {
                                Some(v) => v,
                                None => continue,
                            }
                        } else {
                            eng.key(k)
                        };
                        if v.differs() {
                            if s.shown.is_some_and(|l| l != v.lang) {
                                s.flips += 1;
                            }
                            s.shown = Some(v.lang);
                        }
                        s.frames.push(v.text.clone());
                        s.last = Some(v);
                    }
                    if s.flips > 0 && flip_examples.len() < 40 {
                        flip_examples.push(s.frames.join(" "));
                    }
                    ws = Some(s);
                }
                Unit::Sep(sep) => {
                    finish(eng, &mut ws, &mut line);
                    sep.chars().for_each(|c| eng.separator(c));
                    line.push_str(&sep);
                    want.push_str(&sep);
                }
            }
        }
        finish(eng, &mut ws, &mut line);
        eng.separator('\n');
        let n = wrong_words(&line, &want);
        wrong += n;
        scored += scored_words(&want);
        if n == 0 {
            perfect += 1;
        } else if show > 0 {
            show -= 1;
            eprintln!("GOT  {line}\nWANT {want}\n");
        }
        if let Some(f) = dump.as_mut() {
            writeln!(f, "{line}").unwrap();
        }
        if let Some(f) = dump_want.as_mut() {
            writeln!(f, "{want}").unwrap();
        }
        lines += 1;
        line.clear();
        want.clear();
    }
    let secs = t.elapsed().as_secs_f64();
    let pct = |n: usize| 100.0 * n as f64 / words.max(1) as f64;
    println!(
        "words {words}  wrong {wrong} ({:.2}% of {scored} scored)  perfect lines {:.1}%  late-fix {late} ({:.2}%)  mid-flip words {flip_words} ({:.2}%, {flips} flips)  vn!=en words {:.1}%  {:.0} ns/key",
        100.0 * wrong as f64 / scored.max(1) as f64,
        100.0 * perfect as f64 / lines.max(1) as f64,
        pct(late),
        pct(flip_words),
        pct(differ_words),
        secs * 1e9 / keys.max(1) as f64
    );
    if args.iter().any(|a| a == "--margins") {
        let errs = margins.iter().filter(|m| m.1).count().max(1);
        for t in [2.0f64, 5.0, 10.0, 30.0, 100.0, 1000.0] {
            let unsure = margins.iter().filter(|m| m.0 < t.ln()).count();
            let caught = margins.iter().filter(|m| m.1 && m.0 < t.ln()).count();
            println!("unsure if ratio < {t:>5}: {:.2}% of words, covers {:.0}% of flippable errors", pct(unsure), 100.0 * caught as f64 / errs as f64);
        }
    }
    if fix {
        println!(
            "user fixes: flip {corrections} ({:.3}%)  retype (neither candidate right) {unfixable} ({:.3}%)",
            pct(corrections),
            pct(unfixable)
        );
    }
    if let Some(p) = arg(args, "--save-memory") {
        std::fs::write(p, eng.memory().save()).unwrap();
    }
    if args.iter().any(|a| a == "--late") {
        for e in late_examples {
            eprintln!("  late: {e}");
        }
    }
    if args.iter().any(|a| a == "--flips") {
        for e in flip_examples {
            eprintln!("  flip: {e}");
        }
    }
}
