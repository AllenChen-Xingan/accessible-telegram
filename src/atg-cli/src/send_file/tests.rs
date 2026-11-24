//! 假后端：数据全是编的，不碰真实账号与本机档案目录（家目录、档案都在临时目录里）。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::Holder;
use atg_core::fake::{FakeAuth, FakeBackend};
use atg_core::gate::Gate;
// 别名留着原来的名字，测试体不用改：这里的 `chat(id, title)` 是私聊、通知设置写全
use atg_core::testkit::td_json::new_private_chat_notify_default as chat;
use serde_json::Value;

#[derive(Default)]
struct Cap {
    out: Vec<String>,
}

impl Sink for Cap {
    fn out(&mut self, line: &str) {
        self.out.push(line.to_owned());
    }
    fn err(&mut self, _: &str) {}
}

const ME: i64 = 4242;

fn backend() -> FakeBackend {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(ME, vec![chat(ME, "我自己")]);
    b.set_history(ME, vec![]);
    b.push_net("connectionStateReady");
    b
}

struct Env {
    profile: Profile,
    roots: FileRoots,
}

fn env(tag: &str) -> Env {
    let root = atg_core::testkit::temp_dir(&format!("cli-send-file-{tag}"));
    let profile = Profile::create(&root.join("data"), "p", ServerKind::Main).unwrap();
    let home = root.join("home");
    for d in file::ALLOWED_DIRS {
        std::fs::create_dir_all(home.join(d)).unwrap();
    }
    let roots = FileRoots {
        home,
        forbidden: vec![root.join("data")],
        outbox: profile.dir().join("outbox"),
    };
    Env { profile, roots }
}

fn sp(path: PathBuf, dry_run: bool) -> SendFileParams {
    SendFileParams {
        chat: "saved".into(),
        path,
        caption: String::new(),
        reply_to: None,
        dry_run,
        again: false,
        tz: Some("+08:00".into()),
    }
}

fn run(b: &FakeBackend, e: &Env, p: &SendFileParams) -> (u8, Vec<Value>) {
    crate::history::with_backend(b, &e.profile, Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let w = Wait {
            hand: Duration::from_secs(5),
            idle: Duration::from_secs(5),
            cap: Duration::from_secs(10),
        };
        let code = execute(
            b,
            st,
            fe,
            p,
            &e.roots,
            &mut cap,
            Format::Json,
            Instant::now(),
            w,
        );
        Ok((
            code,
            cap.out
                .iter()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect(),
        ))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

#[test]
fn dry_run_prints_plan_without_path_and_leaves_no_snapshot() {
    let e = env("dry");
    let p = e.roots.home.join("Downloads/报告.txt");
    std::fs::write(&p, b"hello").unwrap();
    let (code, ls) = run(&backend(), &e, &sp(p, true));
    assert_eq!(code, 0, "{ls:?}");
    assert_eq!(ls[0]["kind"], "plan");
    assert_eq!(ls[0]["data"]["action"], "send_file");
    assert_eq!(ls[0]["data"]["file_name"], "报告.txt");
    assert_eq!(ls[0]["data"]["size"], 5);
    assert!(!ls[0].to_string().contains("Downloads"));
    assert_eq!(ls[1]["data"]["dry_run"], true);
    let left = std::fs::read_dir(&e.roots.outbox)
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "只看计划不留快照");
}

#[test]
fn sends_to_saved_and_reports_kind_and_size() {
    let e = env("send");
    let p = e.roots.home.join("Desktop/a.txt");
    std::fs::write(&p, b"abc").unwrap();
    let (code, ls) = run(&backend(), &e, &sp(p, false));
    assert_eq!(code, 0, "{ls:?}");
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "result");
    assert_eq!(end["data"]["command"], "send-file");
    assert_eq!(end["data"]["send_as"], "document");
    assert_eq!(end["data"]["size"], 3);
}

#[test]
fn file_outside_allowed_folders_is_refused() {
    let e = env("refuse");
    let p = e.roots.home.join("notes.txt");
    std::fs::write(&p, b"x").unwrap();
    let (code, ls) = run(&backend(), &e, &sp(p, false));
    assert_ne!(code, 0);
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "error");
    assert_eq!(end["data"]["code"], "invalid_input");
    assert_eq!(end["data"]["why"], "file_not_allowed");
    assert_eq!(end["data"]["retryable"], false);
}

#[test]
fn caption_and_caption_file_are_exclusive() {
    use clap::Parser;
    let cli = crate::args::Cli::try_parse_from([
        "atg-cli",
        "send-file",
        "saved",
        "/tmp/x",
        "--caption",
        "a",
        "--caption-file",
        "/tmp/y",
    ])
    .unwrap_or_else(|e| panic!("{e}"));
    let Some(crate::args::Command::SendFile(a)) = cli.command else {
        panic!("不是 send-file");
    };
    assert!(matches!(params(&a, None), Err(Fail::Usage(_))));
}

/// 本机就能判的（路径、大小、说明）排在查会话之前：会话不存在时也先报文件的问题（自审第 2 轮 P2：CLI 入口先查会话）
#[test]
fn file_problem_is_reported_before_the_chat_is_looked_up() {
    let e = env("order");
    let p = e.roots.home.join("notes.txt");
    std::fs::write(&p, b"x").unwrap();
    let mut q = sp(p, false);
    q.chat = "999999".into();
    let (code, ls) = run(&backend(), &e, &q);
    assert_ne!(code, 0);
    let end = ls.last().unwrap();
    assert_eq!(end["data"]["code"], "invalid_input", "{end}");
    assert_eq!(end["data"]["why"], "file_not_allowed");
}
