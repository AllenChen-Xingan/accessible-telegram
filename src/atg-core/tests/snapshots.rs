//! 快照测试：TDLib JSON 夹具 → 树 → `brief` 读法 + JSON 行，一次把建树和读法一起测住。
//! 夹具在 `fixtures/speech/`，人名和内容全是编的（`.42cog/real.md`「隐私」；名字只取 `fixtures/names.txt`）。
//! - `basic`：各种内容与系统消息 · `entities`：文本实体 · `cases`：路线图 M1.5 列的读法情形 ·
//!   `chat_items`：会话项（`specs/style/chat-item.md` 的 C1–C15）
//!
//! 读法快照用 GUI 的按键（「按 M 收听」「按 D 展开」）；CLI 没有按键，`atg-cli` 的测试去掉按键后与这里逐行比对。
//! 改了读法或树：跑 `INSTA_UPDATE=always cargo test -p atg-core` 生成新快照，**逐行看过差异**再入库。

use atg_core::json;
use atg_core::speech::{SpeechCtx, brief};
use atg_core::tdjson::World;
use atg_core::tree::{Alert, AlertChat, Doc};

fn cx() -> SpeechCtx {
    SpeechCtx {
        utc_offset: 8 * 3600,
        listen_key: Some('m'),
        expand_key: Some('d'),
        offset_at: None,
    }
}

fn world(name: &str) -> World {
    let path = format!("{}/fixtures/speech/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("读夹具");
    World::from_json(&src).expect("解析夹具")
}

fn briefs(docs: &[Doc]) -> String {
    docs.iter()
        .map(|d| brief(d, &cx()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn json_pretty(docs: &[Doc]) -> String {
    docs.iter()
        .map(|d| serde_json::to_string_pretty(&json::value(d, &cx())).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn basic_messages_brief() {
    insta::assert_snapshot!(briefs(&world("basic").docs()));
}

#[test]
fn basic_messages_json() {
    insta::assert_snapshot!(json_pretty(&world("basic").docs()));
}

#[test]
fn basic_chats_brief() {
    insta::assert_snapshot!(briefs(&world("basic").chat_items()));
}

#[test]
fn basic_chats_json() {
    insta::assert_snapshot!(json_pretty(&world("basic").chat_items()));
}

#[test]
fn entities_brief() {
    insta::assert_snapshot!(briefs(&world("entities").docs()));
}

#[test]
fn entities_json() {
    insta::assert_snapshot!(json_pretty(&world("entities").docs()));
}

#[test]
fn cases_brief() {
    insta::assert_snapshot!(briefs(&world("cases").docs()));
}

#[test]
fn cases_json() {
    insta::assert_snapshot!(json_pretty(&world("cases").docs()));
}

#[test]
fn chat_items_brief() {
    insta::assert_snapshot!(briefs(&world("chat_items").chat_items()));
}

#[test]
fn chat_items_json() {
    insta::assert_snapshot!(json_pretty(&world("chat_items").chat_items()));
}

#[test]
fn alert_brief() {
    let one = Doc::Alert(Alert {
        total: 2,
        chats: vec![AlertChat {
            chat_id: 101,
            title: Some("林小雨".into()),
            count: 2,
        }],
    });
    let many = Doc::Alert(Alert {
        total: 7,
        chats: vec![
            AlertChat {
                chat_id: 101,
                title: Some("林小雨".into()),
                count: 2,
            },
            AlertChat {
                chat_id: -1001,
                title: None,
                count: 5,
            },
        ],
    });
    let no_key = SpeechCtx {
        listen_key: None,
        ..cx()
    };
    insta::assert_snapshot!(format!(
        "{}\n{}\n{}\n{}",
        brief(&one, &cx()),
        brief(&many, &cx()),
        brief(&one, &no_key),
        json::line(&one, &cx())
    ));
}

/// JSON 每行一条、不含换行；顶层四个字段齐全
#[test]
fn json_line_shape() {
    for d in world("basic").docs() {
        let line = json::line(&d, &cx());
        assert!(!line.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["v"], 1);
        assert!(v["kind"].is_string());
        assert!(v["data"]["type"].is_string());
        assert!(!v["speech"]["brief"].as_str().unwrap().is_empty());
    }
}

/// 夹具不是数组、不是 JSON：返回错误，不 panic
#[test]
fn bad_fixture_is_error_not_panic() {
    assert!(World::from_json("{}").is_err());
    assert!(World::from_json("not json").is_err());
    // 数组里什么怪东西都有：照样不崩
    let w =
        World::from_json(r#"[1, "x", null, {"@type": 5}, {"@type":"updateNewMessage"},
        {"@type":"updateNewChat","chat":1}, {"@type":"updateChatLastMessage"},
        {"@type":"updateChatPosition"}, {"@type":"updateNewMessage","message":{"date":"9223372036854775807"}}]"#)
        .unwrap();
    let _ = w.chat_items();
    for d in w.docs() {
        let _ = brief(&d, &cx());
        let _ = json::line(&d, &cx());
    }
}
