//! `--fake <场景>`：跑真二进制，`chats` / `history` 在场景 `basic-4-chats` 上的输出；缺场景报 `internal{fixture_bad}`；
//! 不留一次性档案目录。另有「这条路不碰钥匙串」的文本门槛（arch 审定的三层证明之一；另两层是类型与调用路径，见 `src/fake_run.rs` 文件头）。

use serde_json::Value;
use std::process::{Command, Output};
use std::sync::Mutex;

/// 本文件里起进程的测试串行跑：「前后比一比临时目录」不能看到别的测试同时建的目录
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atg-cli"))
        .args(args)
        .output()
        .expect("跑得起来")
}

fn lines(out: &Output) -> Vec<Value> {
    String::from_utf8(out.stdout.clone())
        .expect("UTF-8")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("不是 JSON：{l}")))
        .collect()
}

/// 自带场景 basic-4-chats 的目录。用路径不用名字：`cargo test --release` 跑的是发布构建，它不认裸场景名
fn basic4() -> String {
    format!(
        "{}/../atg-core/fixtures/scenarios/basic-4-chats",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// 系统临时目录里 `atg-fake-` 开头的目录
fn fake_dirs() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(std::env::temp_dir())
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("atg-fake-"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn chats_on_scenario_lists_four_chats_in_order() {
    let _s = serial();
    let before = fake_dirs();
    let out = run(&["chats", "--fake", &basic4()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let ls = lines(&out);
    let ids: Vec<i64> = ls
        .iter()
        .filter(|l| l["data"]["type"] == "chat_item")
        .filter_map(|l| l["data"]["chat_id"].as_i64())
        .collect();
    assert_eq!(ids, vec![101, -1001, -1002, 9]);
    let end = ls.last().unwrap();
    assert_eq!(end["kind"], "result");
    assert_eq!(
        end["data"]["synced"], true,
        "场景是已连接的，不该等到同步超时"
    );
    assert_eq!(fake_dirs(), before, "一次性档案目录没删干净");
}

#[test]
fn history_on_scenario_takes_negative_chat_id() {
    // 群「周末爬山群」已读到 2001：落在第一条未读 2002（F2 落点 = 进会话落在第一条未读，`specs/contracts/cli.md`
    // `history` 行、`docs/plan/003_decisions-b2.md` F2），比它新的没有了
    let _s = serial();
    let before = fake_dirs();
    let out = run(&["history", "-1001", "--fake", &basic4()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let ls = lines(&out);
    let ids: Vec<i64> = ls
        .iter()
        .filter(|l| l["data"]["type"] == "message")
        .filter_map(|l| l["data"]["id"]["message_id"].as_i64())
        .collect();
    assert_eq!(ids, vec![2002]);
    let end = ls.last().unwrap();
    assert_eq!(end["data"]["chat_id"], -1001);
    assert_eq!(end["data"]["first_unread"], 2002);
    assert_eq!(
        end["data"]["synced"], true,
        "场景是已连接的，不该等到同步超时"
    );
    assert_eq!(fake_dirs(), before, "一次性档案目录没删干净");
}

#[test]
fn history_saved_resolves_to_my_own_chat() {
    // `saved` 换成自己的用户 id（R4：问后端 me()）；场景里收藏夹是会话 9「我自己」
    let _s = serial();
    let before = fake_dirs();
    let out = run(&["history", "saved", "--fake", &basic4()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let ls = lines(&out);
    let ids: Vec<i64> = ls
        .iter()
        .filter(|l| l["data"]["type"] == "message")
        .filter_map(|l| l["data"]["id"]["message_id"].as_i64())
        .collect();
    assert!(ids.contains(&5001) && ids.contains(&5002), "{ids:?}");
    let end = ls.last().unwrap();
    assert_eq!(end["data"]["chat_id"], 9);
    assert_eq!(fake_dirs(), before, "一次性档案目录没删干净");
}

/// 别的测试都按路径给场景；按名字找只在开发构建里有，这里单测这一条路
#[cfg(debug_assertions)]
#[test]
fn scenario_by_name_works_in_debug_builds() {
    let _s = serial();
    let before = fake_dirs();
    let out = run(&["chats", "--fake", "basic-4-chats"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(fake_dirs(), before, "一次性档案目录没删干净");
}

#[test]
fn missing_scenario_is_fixture_bad_and_leaves_nothing() {
    let _s = serial();
    let before = fake_dirs();
    let out = run(&["chats", "--fake", "/nonexistent/atg-scenario"]);
    // internal 的退出码是 1（cli.md 第一节第 4 小节）
    assert_eq!(out.status.code(), Some(1));
    let ls = lines(&out);
    assert_eq!(ls.len(), 1);
    assert_eq!(ls[0]["data"]["code"], "internal");
    assert_eq!(ls[0]["data"]["what"], "fixture_bad");
    assert_eq!(fake_dirs(), before);
}

// ───── 不碰钥匙串：文本门槛 ─────

const FORBIDDEN: &[&str] = &["keystore", "Keychain", "td_backend", "TdBackend"];

fn hits(src: &str) -> Vec<&'static str> {
    FORBIDDEN
        .iter()
        .copied()
        .filter(|w| src.contains(w))
        .collect()
}

/// `--fake` 这条路上的源码：假后端全部、`config.rs` 里私有临时目录那一段、`fake_run.rs`
fn fake_path_sources() -> Vec<(String, String)> {
    let core = format!("{}/../atg-core/src", env!("CARGO_MANIFEST_DIR"));
    let mut v = Vec::new();
    let mut push = |p: String| {
        let s = std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("读不了 {p}"));
        v.push((p, s));
    };
    push(format!("{core}/fake.rs"));
    let mut sub: Vec<String> = std::fs::read_dir(format!("{core}/fake"))
        .expect("fake/ 目录")
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.ends_with(".rs"))
        .collect();
    sub.sort();
    for p in sub {
        push(p);
    }
    push(format!("{}/src/fake_run.rs", env!("CARGO_MANIFEST_DIR")));
    let cfg = std::fs::read_to_string(format!("{core}/config.rs")).expect("config.rs");
    let start = cfg
        .find("pub fn private_temp_dir")
        .expect("config.rs 里有 private_temp_dir：改名要同步改这条测试");
    let end = cfg[start..]
        .find("#[cfg(test)]")
        .map_or(cfg.len(), |i| start + i);
    v.push((
        "config.rs（private_temp_dir 一段）".into(),
        cfg[start..end].to_owned(),
    ));
    v
}

#[test]
fn fake_path_never_mentions_the_keychain_or_real_backend() {
    let srcs = fake_path_sources();
    assert!(srcs.len() >= 4, "扫到的文件太少：路径变了要同步改这条测试");
    for (p, s) in &srcs {
        assert!(
            hits(s).is_empty(),
            "{p} 里出现了 {:?}：--fake 这条路不许碰钥匙串或真实后端",
            hits(s)
        );
    }
}

#[test]
fn keychain_gate_goes_red_on_a_planted_mention() {
    // 改坏会红：往扫描内容里塞一个钥匙串的名字
    assert_eq!(
        hits("let k = atg_core::keystore::Keychain;"),
        vec!["keystore", "Keychain"]
    );
    assert!(hits("FakeBackend::new()").is_empty());
}
