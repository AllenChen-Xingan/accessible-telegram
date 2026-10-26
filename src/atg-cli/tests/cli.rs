//! 跑真二进制，对照 `specs/contracts/cli.md`：标准输出只放 JSON 行、恰好一行结尾、退出码与结尾一致。

use serde_json::Value;
use std::process::{Command, Output};

/// 已做的子命令（`version` 另测）
const DONE: &[&str] = &["login", "auth", "logout", "speech"];

/// 契约第一节第 2 小节表里还没做的子命令
const PLACEHOLDERS: &[&str] = &[
    "me",
    "pending",
    "status",
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
        &["--format", "text", "history"][..],
        &["history", "--format", "text"][..],
        &["history", "-n", "3", "--format=text"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let s = String::from_utf8(out.stdout).unwrap();
        assert!(s.contains("history"), "{args:?}: {s}");
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
    for name in PLACEHOLDERS.iter().chain(DONE) {
        assert!(s.contains(name), "--help 里缺 {name}");
    }
}

/// 登录这组命令的外壳（真实后端要 TDLib 与终端，这里只测不联网就能判定的路径）
#[test]
fn login_commands_with_fake_are_not_implemented_yet() {
    for args in [
        &["--fake", "basic", "login"][..],
        &["--fake", "basic", "auth", "state"][..],
        &["logout", "--fake", "basic"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let ls = lines(&out);
        assert_eq!(ls.len(), 1, "{args:?}");
        assert_eq!(ls[0]["data"]["code"], "not_implemented");
        assert!(DONE.contains(&ls[0]["data"]["command"].as_str().unwrap()));
    }
}

#[test]
fn auth_needs_a_known_step() {
    for args in [
        &["auth"][..],
        &["auth", "nope"][..],
        &["login", "extra"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let ls = lines(&out);
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0]["data"]["code"], "usage");
    }
}

/// 秘密值不许进命令行参数：`auth code 12345`、`auth password x` 都是用法错误，而且不回显
#[test]
fn secrets_are_not_accepted_as_arguments() {
    for args in [
        &["auth", "code", "40419"][..],
        &["auth", "password", "hunter2"][..],
        &["auth", "phone", "+9996627731"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let s = String::from_utf8(out.stdout).unwrap();
        assert!(!s.contains(args[2]), "{s}");
    }
}

#[cfg(not(feature = "tdlib"))]
#[test]
fn without_tdlib_login_says_how_to_build() {
    let out = run(&["--test-dc", "auth", "state"]);
    assert_eq!(out.status.code(), Some(1));
    let ls = lines(&out);
    assert_eq!(ls[0]["data"]["code"], "not_implemented");
    assert!(
        ls[0]["data"]["message_zh"]
            .as_str()
            .unwrap()
            .contains("--features tdlib")
    );
}

#[test]
fn chats_bad_arguments_are_usage_errors() {
    for args in [
        &["chats", "--cursor", "abc"][..],
        &["chats", "--list", "folder"][..],
        &["chats", "-n", "0"][..],
        &["chats", "-n", "x"][..],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let ls = lines(&out);
        assert_eq!(ls.len(), 1, "{args:?}");
        assert_eq!(ls[0]["data"]["code"], "usage", "{args:?}");
    }
}

#[test]
fn chats_with_fake_scenario_is_not_implemented_yet() {
    // 场景目录还没做：说清楚，不碰档案、不联网
    let out = run(&["chats", "--fake", "basic", "--list", "archive", "--unread"]);
    assert_eq!(out.status.code(), Some(1));
    let ls = lines(&out);
    assert_eq!(ls.len(), 1);
    assert_eq!(ls[0]["data"]["code"], "not_implemented");
    assert_eq!(ls[0]["data"]["command"], "chats");
}

// ───── speech --fixture（#19） ─────

fn core_dir() -> String {
    format!("{}/../atg-core", env!("CARGO_MANIFEST_DIR"))
}

/// 快照文件的正文（去掉 insta 的抬头）
fn snapshot_body(name: &str) -> String {
    let path = format!("{}/tests/snapshots/snapshots__{name}.snap", core_dir());
    let s = std::fs::read_to_string(&path).expect("读快照");
    let body = s.splitn(3, "---\n").nth(2).expect("快照有抬头");
    body.trim_end().to_owned()
}

/// 文字模式每行一句，与 atg-core 的读法快照逐行一致（快照用 GUI 的按键，CLI 没有按键，比之前去掉；
/// V13 人定按键提示每条至多一次，在钟点前）
#[test]
fn speech_text_matches_core_snapshots() {
    for (fixture, level, snap) in [
        ("basic", "brief", "basic_messages_brief"),
        ("basic", "chat_brief", "basic_chats_brief"),
        ("entities", "brief", "entities_brief"),
        ("cases", "brief", "cases_brief"),
        ("chat_items", "chat_brief", "chat_items_brief"),
    ] {
        let path = format!("{}/fixtures/speech/{fixture}.json", core_dir());
        let out = run(&[
            "speech",
            "--fixture",
            &path,
            "--level",
            level,
            "--tz",
            "+08:00",
            "--format",
            "text",
        ]);
        assert_eq!(out.status.code(), Some(0), "{fixture}");
        let got = String::from_utf8(out.stdout).unwrap();
        // 「…，按 D 展开，19:33」与「…？按 D 展开，19:33」（V22：问号后不加逗号）都还原成没有按键的样子
        let want = snapshot_body(snap)
            .replace("，按 D 展开", "")
            .replace("按 D 展开，", "");
        assert_eq!(got.trim_end(), want, "{fixture} {level}");
    }
}

#[test]
fn speech_json_lines_end_with_result() {
    let path = format!("{}/fixtures/speech/cases.json", core_dir());
    let out = run(&["speech", "--fixture", &path, "--tz", "Z"]);
    assert_eq!(out.status.code(), Some(0));
    let ls = lines(&out);
    let (last, body) = ls.split_last().unwrap();
    assert!(!body.is_empty());
    for l in body {
        assert!(
            ["message", "event"].contains(&l["kind"].as_str().unwrap()),
            "{l}"
        );
        assert!(!l["speech"]["brief"].as_str().unwrap().is_empty(), "{l}");
        // CLI 没有按键
        assert!(
            !l["speech"]["brief"].as_str().unwrap().contains("按 D"),
            "{l}"
        );
    }
    assert_eq!(last["kind"], "result");
    assert_eq!(last["data"]["command"], "speech");
    assert_eq!(last["data"]["level"], "brief");
    assert_eq!(last["data"]["count"], body.len());
    assert_eq!(last["data"]["tz"], "fixed");
    assert_eq!(last["data"]["utc_offset_s"], 0);

    let path = format!("{}/fixtures/speech/chat_items.json", core_dir());
    let out = run(&["speech", "--fixture", &path, "--level", "chat_brief"]);
    let ls = lines(&out);
    let (last, body) = ls.split_last().unwrap();
    assert!(body.iter().all(|l| l["kind"] == "chat"));
    assert_eq!(last["data"]["level"], "chat_brief");
    // 不给 --tz：跟系统时区，没有对所有行都成立的固定偏移
    assert_eq!(last["data"]["tz"], "system");
    assert!(last["data"]["utc_offset_s"].is_null());
    assert_eq!(last["data"]["count"], body.len());
}

#[test]
fn speech_bad_input_is_usage_and_does_not_echo() {
    let dir = std::env::temp_dir().join(format!("atg-cli-speech-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let not_array = dir.join("暗号目录-not-array.json");
    std::fs::write(&not_array, r#"{"暗号":"内容"}"#).unwrap();
    let not_json = dir.join("not-json.json");
    std::fs::write(&not_json, "暗号 不是 JSON").unwrap();
    let missing = dir.join("missing.json");
    let good = format!("{}/fixtures/speech/basic.json", core_dir());
    for args in [
        vec!["speech"],
        vec!["speech", "--fixture", not_array.to_str().unwrap()],
        vec!["speech", "--fixture", not_json.to_str().unwrap()],
        vec!["speech", "--fixture", missing.to_str().unwrap()],
        vec!["speech", "--fixture", &good, "--level", "detail"],
        vec!["speech", "--fixture", &good, "--tz", "+25:00"],
        vec!["speech", "--fixture", &good, "--fake", "basic"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let ls = lines(&out);
        assert_eq!(ls.len(), 1, "{args:?}");
        assert_eq!(ls[0]["data"]["code"], "usage", "{args:?}");
        let s = String::from_utf8(out.stdout).unwrap();
        assert!(!s.contains("暗号"), "{s}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
