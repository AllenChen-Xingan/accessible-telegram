//! `stt` 的测试。假辅助程序在 `fixtures/stt/*.sh`，内容全是编的。
//! 选语种表逐行（design.md 第三节第 1 条）、协议完整性、生命周期（第 6 条）、隐私（第 7 条）。
//! 要真辅助程序的 `#[ignore]` 测试在 `tests/real.rs`：先 `bash src/atg-audio/stt-helper/build.sh`（要 macOS 26 起的 SDK），
//! 系统设置里装好中文、英文的听写语言模型（没装会得到 `Unavailable`，不是代码坏了），然后
//! `cargo test -p atg-audio stt::tests::real -- --ignored`。编辅助程序算重活，本项目要先领名额（CLAUDE.md 第〇节）

use super::*;
use std::path::PathBuf;
use std::time::Instant;

fn helper(name: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}/fixtures/stt/{name}.sh",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn both() -> Vec<String> {
    vec!["zh-CN".into(), "en-US".into()]
}

fn zh() -> Vec<String> {
    vec!["zh-CN".into()]
}

/// 一秒静音：内容无所谓，假辅助程序不看
fn pcm() -> Pcm {
    Pcm {
        samples: vec![0.0; SAMPLE_RATE as usize],
    }
}

/// 只给期限测试用的紧期限
const FAST: Limits = Limits {
    hello: Duration::from_millis(300),
    base: Duration::from_millis(600),
};

/// 其余用例用宽期限：机器忙、sh 起得慢也不会被握手期限误杀
/// 总期限测试用：握手期限远长于总期限，机器忙时也不会被握手期限抢先判成不可用
const SLOW_HELLO: Limits = Limits {
    hello: Duration::from_secs(20),
    base: Duration::from_millis(600),
};
const ROOMY: Limits = Limits {
    hello: Duration::from_secs(20),
    base: Duration::from_secs(20),
};

fn reaped() -> u32 {
    REAPED.with(|n| n.get())
}

/// 跑一个假辅助程序，并断言它被回收了恰好一次（顺带证明它真起来了：起不来的不回收，
/// 与「起来了、自己报不可用」区分开）
fn run_with(name: &str, locales: &[String], limits: Limits) -> Result<Recognition, SttError> {
    let before = reaped();
    let r = transcribe_with(
        &helper(name),
        &pcm(),
        locales,
        &AtomicBool::new(false),
        limits,
    );
    assert_eq!(
        reaped(),
        before + 1,
        "{name}：子进程没被回收（或根本没起来）"
    );
    r
}

fn run(name: &str, locales: &[String]) -> Result<Recognition, SttError> {
    run_with(name, locales, ROOMY)
}

fn seg(t: &str) -> Segment {
    Segment {
        text: t.into(),
        start_s: 0.0,
        end_s: 1.0,
    }
}

fn text(conf: f64) -> Outcome {
    Outcome::Text(conf, vec![seg("x")])
}

fn failed(unavailable: bool) -> Outcome {
    Outcome::Failed { unavailable }
}

fn locale_of(r: &Result<Recognition, SttError>) -> Option<&str> {
    match r {
        Ok(Recognition::Text { locale, .. }) => Some(locale),
        _ => None,
    }
}

// —— 选语种表（双跑、单跑各一组）——

#[test]
fn choose_table_both() {
    let l = both();
    let z = |o| ("zh-CN".to_string(), o);
    let e = |o| ("en-US".to_string(), o);
    // 至少一个有字 → 置信度高的；相等取先请求的 zh
    assert_eq!(
        locale_of(&choose(&l, vec![z(text(0.6)), e(text(0.9))])),
        Some("en-US")
    );
    assert_eq!(
        locale_of(&choose(&l, vec![z(text(0.9)), e(text(0.6))])),
        Some("zh-CN")
    );
    assert_eq!(
        locale_of(&choose(&l, vec![e(text(0.7)), z(text(0.7))])),
        Some("zh-CN")
    );
    assert_eq!(
        locale_of(&choose(&l, vec![z(failed(true)), e(text(0.1))])),
        Some("en-US")
    );
    assert_eq!(
        locale_of(&choose(&l, vec![z(Outcome::Empty), e(text(0.0))])),
        Some("en-US")
    );
    // 没有有字、至少一个空 → 没听出内容（其余败也算）
    assert_eq!(
        choose(&l, vec![z(Outcome::Empty), e(Outcome::Empty)]),
        Ok(Recognition::NoSpeech)
    );
    assert_eq!(
        choose(&l, vec![z(failed(false)), e(Outcome::Empty)]),
        Ok(Recognition::NoSpeech)
    );
    assert_eq!(
        choose(&l, vec![z(failed(true)), e(Outcome::Empty)]),
        Ok(Recognition::NoSpeech)
    );
    // 全败 → 任一不可用就 Unavailable，否则 HelperFailed
    assert_eq!(
        choose(&l, vec![z(failed(true)), e(failed(false))]),
        Err(SttError::Unavailable)
    );
    assert_eq!(
        choose(&l, vec![z(failed(false)), e(failed(false))]),
        Err(SttError::HelperFailed)
    );
}

#[test]
fn choose_table_single() {
    let l = zh();
    let z = |o| vec![("zh-CN".to_string(), o)];
    assert_eq!(locale_of(&choose(&l, z(text(0.0)))), Some("zh-CN"));
    assert_eq!(choose(&l, z(Outcome::Empty)), Ok(Recognition::NoSpeech));
    assert_eq!(choose(&l, z(failed(true))), Err(SttError::Unavailable));
    assert_eq!(choose(&l, z(failed(false))), Err(SttError::HelperFailed));
}

// —— 协议完整性（直接喂 read_protocol）——

fn proto(lines: &[&str], locales: &[String]) -> Result<usize, SttError> {
    let s = lines.join("\n");
    read_protocol(s.as_bytes(), locales, || {}).map(|v| v.len())
}

const HELLO: &str = r#"{"kind":"hello","proto":1}"#;
const ZH_EMPTY: &str = r#"{"kind":"locale","locale":"zh-CN","status":"empty"}"#;
const EN_EMPTY: &str = r#"{"kind":"locale","locale":"en-US","status":"empty"}"#;

#[test]
fn protocol_completeness() {
    let l = both();
    assert_eq!(
        proto(
            &[HELLO, ZH_EMPTY, EN_EMPTY, r#"{"kind":"done","count":2}"#],
            &l
        ),
        Ok(2)
    );
    // 没 done、count 不对、少一行、多一行（重复）、没请求的语种、hello 之前就出结果、未知 kind、坏状态、
    // 两个 hello、done 之后还有行
    let bad: &[&[&str]] = &[
        &[HELLO, ZH_EMPTY, EN_EMPTY],
        &[HELLO, ZH_EMPTY, EN_EMPTY, r#"{"kind":"done","count":3}"#],
        &[HELLO, ZH_EMPTY, r#"{"kind":"done","count":2}"#],
        &[HELLO, ZH_EMPTY, ZH_EMPTY, r#"{"kind":"done","count":2}"#],
        &[
            HELLO,
            ZH_EMPTY,
            r#"{"kind":"locale","locale":"ja-JP","status":"empty"}"#,
            r#"{"kind":"done","count":2}"#,
        ],
        &[ZH_EMPTY, HELLO, EN_EMPTY, r#"{"kind":"done","count":2}"#],
        &[HELLO, r#"{"kind":"progress"}"#],
        &[
            HELLO,
            r#"{"kind":"locale","locale":"zh-CN","status":"maybe"}"#,
        ],
        &[
            HELLO,
            HELLO,
            ZH_EMPTY,
            EN_EMPTY,
            r#"{"kind":"done","count":2}"#,
        ],
        &[
            HELLO,
            ZH_EMPTY,
            EN_EMPTY,
            r#"{"kind":"done","count":2}"#,
            ZH_EMPTY,
        ],
    ];
    for b in bad {
        assert_eq!(proto(b, &l), Err(SttError::HelperFailed), "{b:?}");
    }
    // 版本不对 → 不可用（提示跑 build.sh）
    assert_eq!(
        proto(&[r#"{"kind":"hello","proto":99}"#], &l),
        Err(SttError::Unavailable)
    );
}

/// 文字全是空白算空，不算有字
#[test]
fn whitespace_text_is_empty() {
    let l = zh();
    let s = [HELLO, r#"{"kind":"locale","locale":"zh-CN","status":"text","conf":0.9,"segments":[{"text":"  ","start_s":0,"end_s":1}]}"#, r#"{"kind":"done","count":1}"#].join("\n");
    let got = read_protocol(s.as_bytes(), &l, || {}).unwrap();
    assert_eq!(choose(&l, got), Ok(Recognition::NoSpeech));
}

// —— 子进程（假辅助程序）——

#[test]
fn fake_helper_paths() {
    let t = Instant::now();
    assert_eq!(locale_of(&run("ok-zh", &both())), Some("zh-CN"));
    assert_eq!(run("zh-fail-en-empty", &both()), Ok(Recognition::NoSpeech));
    assert_eq!(run("both-unavailable", &both()), Err(SttError::Unavailable));
    assert_eq!(run("bad-proto", &both()), Err(SttError::Unavailable));
    assert_eq!(run("missing-line", &both()), Err(SttError::HelperFailed));
    assert_eq!(run("garbage", &both()), Err(SttError::HelperFailed));
    // 协议完整但非零退出 → 进程失败
    assert_eq!(run("stderr-marker", &zh()), Err(SttError::HelperFailed));
    // 上面的 Unavailable 都是辅助程序自己报的，不是等到握手期限（20 秒）被杀
    assert!(t.elapsed() < Duration::from_secs(10));
    // 找不到：起不来，没有子进程可回收
    let before = reaped();
    let r = transcribe_with(
        &helper("no-such-helper"),
        &pcm(),
        &zh(),
        &AtomicBool::new(false),
        ROOMY,
    );
    assert_eq!(r, Err(SttError::Unavailable));
    assert_eq!(reaped(), before);
    // 没给语种：不起子进程
    let r = transcribe_with(
        &helper("ok-zh"),
        &pcm(),
        &[],
        &AtomicBool::new(false),
        ROOMY,
    );
    assert_eq!(r, Err(SttError::HelperFailed));
}

/// 进 `internal{what}` 的码只在这里定义；Unavailable、Cancelled 走自己的顶层码
#[test]
fn what_codes() {
    assert_eq!(SttError::Timeout.what(), Some("stt_timeout"));
    assert_eq!(SttError::HelperFailed.what(), Some("stt_helper_failed"));
    assert_eq!(SttError::Unavailable.what(), None);
    assert_eq!(SttError::Cancelled.what(), None);
}

#[test]
fn deadlines() {
    let t = Instant::now();
    assert_eq!(
        run_with("no-hello", &both(), FAST),
        Err(SttError::Unavailable)
    );
    assert!(t.elapsed() < Duration::from_secs(3), "握手期限没起作用");
    let t = Instant::now();
    // 总期限 = base + 音频 1 秒
    assert_eq!(
        run_with("hang", &both(), SLOW_HELLO),
        Err(SttError::Timeout)
    );
    assert!(t.elapsed() < Duration::from_secs(4), "总期限没起作用");
    // 不读输入：写线程堵在管道上，杀掉子进程后它也得返回（scope 才 join 得上）
    let t = Instant::now();
    assert_eq!(
        run_with("deaf", &both(), SLOW_HELLO),
        Err(SttError::Timeout)
    );
    assert!(t.elapsed() < Duration::from_secs(4), "写线程堵住时没收尾");
}

#[test]
fn cancel_kills_helper() {
    let cancel = AtomicBool::new(false);
    let slow = Limits {
        hello: Duration::from_secs(30),
        base: Duration::from_secs(30),
    };
    let t = Instant::now();
    let before = reaped();
    let r = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            cancel.store(true, Ordering::SeqCst);
        });
        transcribe_with(&helper("hang"), &pcm(), &both(), &cancel, slow)
    });
    assert_eq!(r, Err(SttError::Cancelled));
    assert!(t.elapsed() < Duration::from_secs(2));
    assert_eq!(reaped(), before + 1);
}

/// 隐私：坏 JSON、标准错误里的暗号，不出现在错误与 Debug 里（错误只带码）
#[test]
fn marker_never_leaks() {
    for name in ["garbage", "stderr-marker", "missing-line"] {
        let r = run(name, &both());
        assert!(!format!("{r:?}").contains("XYZZY"), "{name}");
    }
    let ok = Recognition::Text {
        locale: "zh-CN".into(),
        conf: 1.0,
        segments: vec![seg("暗号XYZZY")],
    };
    assert!(!format!("{ok:?}").contains("XYZZY"));
}

fn swift_src() -> String {
    std::fs::read_to_string(format!(
        "{}/stt-helper/main.swift",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// 协议版本只有 Rust 这一个正本；Swift 那边的写死值与它比对（specs/architecture-rules.md 第 2 条）
#[test]
fn proto_matches_swift() {
    let src = swift_src();
    let line = src
        .lines()
        .find(|l| l.starts_with("let PROTO = "))
        .expect("main.swift 里找不到 let PROTO = ");
    assert_eq!(
        line.trim_start_matches("let PROTO = ").trim(),
        PROTO.to_string()
    );
}

/// 「模型不可用」的错误码同上：Swift 报的就是 Rust 认的那个（改了一边、另一边会把它当普通失败，
/// 该提示装模型的变成进程失败）
#[test]
fn unavailable_code_matches_swift() {
    let want = format!(r#"let UNAVAILABLE = "{UNAVAILABLE}""#);
    assert!(
        swift_src().lines().any(|l| l == want),
        "main.swift 里没有 {want}"
    );
}

/// 终态只有一个赢家：收完之后看门线程再到点不算数；被杀之后主线程收完也改不回来
#[test]
fn phase_has_one_winner() {
    let mut p = Phase::Running;
    assert!(p.finish());
    assert!(!p.kill(SttError::Timeout));
    assert_eq!(p, Phase::Finished);
    let mut p = Phase::Running;
    assert!(p.kill(SttError::Cancelled));
    assert!(!p.finish());
    assert!(!p.kill(SttError::Timeout));
    assert_eq!(p, Phase::Killed(SttError::Cancelled));
}

/// 判据表：取消 > 握手期限 > 总期限
#[test]
fn overdue_table() {
    let s = Duration::from_secs;
    let l = Limits {
        hello: s(5),
        base: s(10),
    };
    let t = Some(s(1)); // 及时握手
    assert_eq!(overdue(false, t, s(3), l, s(11)), None);
    assert_eq!(
        overdue(true, None, s(99), l, s(11)),
        Some(SttError::Cancelled)
    );
    assert_eq!(
        overdue(false, None, s(6), l, s(11)),
        Some(SttError::Unavailable)
    );
    assert_eq!(overdue(false, None, s(4), l, s(11)), None);
    assert_eq!(overdue(false, t, s(6), l, s(11)), None);
    assert_eq!(overdue(false, t, s(12), l, s(11)), Some(SttError::Timeout));
}

/// 交错一：看门线程迟到，握手在期限之后才到（已记下）——迟到的 hello 改不了超期的事实
#[test]
fn late_hello_stays_late() {
    let ms = Duration::from_millis;
    let l = Limits {
        hello: ms(5000),
        base: ms(10000),
    };
    assert_eq!(
        overdue(false, Some(ms(5001)), ms(7000), l, ms(11000)),
        Some(SttError::Unavailable)
    );
}

/// 交错二：握手及时到了，看门线程过了期限才轮上判——按握手时刻判，不误杀
#[test]
fn timely_hello_judged_late_is_kept() {
    let ms = Duration::from_millis;
    let l = Limits {
        hello: ms(5000),
        base: ms(10000),
    };
    assert_eq!(overdue(false, Some(ms(4999)), ms(5001), l, ms(11000)), None);
}

/// 辅助程序到兜底期限以 3 退出、看门线程还没轮上：主线程（读到半截协议、或读完协议拿到退出码，两处都调 settle）
/// 用同一判据定终态，
/// 已到期就是超时；没到期才是进程失败；看门线程先定了的不被改
#[test]
fn settle_uses_deadline_when_watchdog_is_late() {
    let mut p = Phase::Running;
    p.settle(Some(SttError::Timeout));
    assert_eq!(p, Phase::Killed(SttError::Timeout));
    let mut p = Phase::Running;
    p.settle(None);
    assert_eq!(p, Phase::Finished);
    let mut p = Phase::Running;
    assert!(p.kill(SttError::Cancelled));
    p.settle(Some(SttError::Timeout));
    assert_eq!(p, Phase::Killed(SttError::Cancelled));
}

mod real;
