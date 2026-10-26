//! 跑真二进制，对照 `specs/contracts/cli.md`：标准输出只放 JSON 行、恰好一行结尾、退出码与结尾一致。

use serde_json::Value;
use std::process::{Command, Output};

/// 契约第一节第 2 小节表里的子命令（`version` 另测）
const PLACEHOLDERS: &[&str] = &[
    "login",
    "auth",
    "logout",
    "me",
    "pending",
    "status",
    "chats",
    "next-unread",
    "history",
    "links",
    "download",
    "open",
    "mute",
    "delete",
    "resend",
    "show",
    "search",
    "send",
    "watch",
    "draft",
    "play",
    "speech",
    "schema",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atg-cli"))
        .args(args)
        .output()
        .expect("跑得起来")
}

/// 标准输出逐行解析成 JSON，并检查每行外壳
fn lines(out: &Output) -> Vec<Value> {
    let s = String::from_utf8(out.stdout.clone()).expect("UTF-8");
    s.lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap_or_else(|_| panic!("不是 JSON：{l}"));
            assert_eq!(v["v"], 1, "{l}");
            assert!(v["kind"].is_string(), "{l}");
            assert!(v["at"].as_str().is_some_and(|a| a.ends_with('Z')), "{l}");
            assert!(v["data"].is_object(), "{l}");
            v
        })
        .collect()
}

#[test]
fn every_placeholder_ends_with_one_not_implemented_error() {
    for name in PLACEHOLDERS {
        let out = run(&[name, "123", "--limit", "5"]);
        let ls = lines(&out);
        assert_eq!(ls.len(), 1, "{name}");
        let last = &ls[0];
        assert_eq!(last["kind"], "error", "{name}");
        assert_eq!(last["data"]["code"], "not_implemented", "{name}");
        assert_eq!(last["data"]["command"], *name);
        assert_eq!(last["data"]["retryable"], false);
        assert!(last["data"]["message_zh"].is_string());
        assert_eq!(out.status.code(), Some(1), "{name}");
    }
}

#[test]
fn version_is_a_version_line_plus_result() {
    for args in [&["version"][..], &["--version"][..]] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(0));
        let ls = lines(&out);
        assert_eq!(ls.len(), 2, "{args:?}");
        assert_eq!(ls[0]["kind"], "version");
        assert_eq!(ls[0]["data"]["atg_cli"], env!("CARGO_PKG_VERSION"));
        assert_eq!(ls[0]["data"]["v"], 1);
        assert_eq!(ls[1]["kind"], "result");
        assert_eq!(ls[1]["data"]["command"], "version");
    }
}

#[test]
fn global_options_work_before_and_after_subcommand() {
    for args in [
        &["--format", "text", "chats"][..],
        &["chats", "--format", "text"][..],
        &["chats", "-n", "3", "--format=text"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let s = String::from_utf8(out.stdout).unwrap();
        assert!(s.contains("chats"), "{args:?}: {s}");
        assert!(!s.trim_start().starts_with('{'), "文字模式不出 JSON：{s}");
    }
}

#[test]
fn usage_error_is_one_json_line_exit_2() {
    for args in [
        &[][..],
        &["no-such-command"][..],
        &["--format", "xml", "me"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let ls = lines(&out);
        assert_eq!(ls.len(), 1, "{args:?}");
        assert_eq!(ls[0]["kind"], "error");
        assert_eq!(ls[0]["data"]["code"], "usage");
    }
}

#[test]
fn usage_error_does_not_echo_input_on_stdout() {
    let out = run(&["no-such-command", "秘密内容"]);
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(!s.contains("秘密内容"), "{s}");
}

#[test]
fn help_is_for_humans_exit_0() {
    let out = run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8(out.stdout).unwrap();
    for name in PLACEHOLDERS {
        assert!(s.contains(name), "--help 里缺 {name}");
    }
}
