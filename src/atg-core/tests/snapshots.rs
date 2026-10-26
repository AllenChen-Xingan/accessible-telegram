//! 快照测试：TDLib JSON 夹具 → 树 → `brief` 读法 + JSON 行，一次把建树和读法一起测住。
//! 夹具在 `fixtures/speech/`，人名和内容全是编的（`.42cog/real.md`「隐私」；名字只取 `fixtures/names.txt`）。
//! - `basic`：各种内容与系统消息 · `entities`：文本实体 · `cases`：路线图 M1.5 列的读法情形 ·
//!   `chat_items`：会话项（`specs/style/chat-item.md` 的 C1–C15）
//!
//! 读法快照用 GUI 的按键（「按 M 收听」「按 D 展开」）；CLI 没有按键，`atg-cli` 的测试去掉按键后与这里逐行比对。
//! 改了读法或树：跑 `INSTA_UPDATE=always cargo test -p atg-core` 生成新快照，**逐行看过差异**再入库。

use atg_core::json;
use atg_core::speech::{self, SpeechCtx, brief};
use atg_core::tdjson::World;
use atg_core::tree::{Alert, AlertChat, Doc};

fn cx() -> SpeechCtx {
    SpeechCtx {
        utc_offset: 8 * 3600,
        listen_key: Some('m'),
        expand_key: Some('d'),
        reveal: false,
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

// ───── 详情与全文（#21）。详情的句子是 AI 草稿，**待人实听**（`specs/style/` 只定了哪些进详情）─────

fn layer(docs: &[Doc], f: fn(&Doc, &SpeechCtx) -> String, cx: &SpeechCtx) -> String {
    docs.iter().map(|d| f(d, cx)).collect::<Vec<_>>().join("\n")
}

fn revealed() -> SpeechCtx {
    SpeechCtx {
        reveal: true,
        ..cx()
    }
}

#[test]
fn basic_messages_detail() {
    insta::assert_snapshot!(layer(&world("basic").docs(), speech::detail, &cx()));
}

#[test]
fn cases_detail() {
    insta::assert_snapshot!(layer(&world("cases").docs(), speech::detail, &cx()));
}

#[test]
fn basic_messages_full() {
    insta::assert_snapshot!(layer(&world("basic").docs(), speech::full, &cx()));
}

#[test]
fn cases_full_and_revealed() {
    let docs = world("cases").docs();
    insta::assert_snapshot!(format!(
        "# full\n{}\n# full --reveal\n{}\n# brief --reveal\n{}",
        layer(&docs, speech::full, &cx()),
        layer(&docs, speech::full, &revealed()),
        layer(&docs, brief, &revealed()),
    ));
}

#[test]
fn entities_full() {
    insta::assert_snapshot!(layer(&world("entities").docs(), speech::full, &cx()));
}

/// 不展开时，剧透里的字一个都不进 brief / detail / full（夹具里剧透的字逐个比对）
#[test]
fn hidden_content_never_leaks_without_reveal() {
    use atg_core::tree::{Span, text::plain_text};
    fn spoilers(spans: &[Span], out: &mut Vec<String>) {
        for s in spans {
            match s {
                Span::Spoiler { children } => out.push(plain_text(children)),
                Span::Bold { children }
                | Span::Italic { children }
                | Span::Underline { children }
                | Span::Strikethrough { children }
                | Span::Quote { children, .. } => spoilers(children, out),
                _ => {}
            }
        }
    }
    let mut checked = 0;
    for name in ["basic", "cases", "entities"] {
        for d in world(name).docs() {
            let Doc::Message(m) = &d else { continue };
            let mut secret = vec![];
            spoilers(&m.content.body, &mut secret);
            for s in secret.iter().filter(|s| s.trim().chars().count() >= 2) {
                for said in [
                    brief(&d, &cx()),
                    speech::detail(&d, &cx()),
                    speech::full(&d, &cx()),
                ] {
                    assert!(!said.contains(s.trim()), "{name}：剧透漏进读法");
                }
                // JSON 行的 data 树也遮住（大总管授权 2026-10-26）；展开后才带原文
                assert!(
                    !json::line(&d, &cx()).contains(s.trim()),
                    "{name}：剧透漏进 data"
                );
                assert!(
                    json::line(&d, &revealed()).contains(s.trim()),
                    "{name}：展开后 data 该带原文"
                );
                assert!(
                    speech::full(&d, &revealed()).contains(s.trim()),
                    "{name}：--reveal 该读原文"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "夹具里该有剧透");
}

/// 剧透里的链接、文字里套着剧透的链接：遮住后字和地址都不留；展开后照出
#[test]
fn spoiler_links_are_masked_in_data() {
    use atg_core::tree::{Content, Span, mask::masked};
    let mut docs = world("basic").docs();
    let Some(Doc::Message(m)) = docs.iter_mut().find(|d| matches!(d, Doc::Message(_))) else {
        panic!("夹具里该有消息")
    };
    m.content = Content {
        body: vec![
            Span::Text {
                text: "看 ".into()
            },
            Span::Spoiler {
                children: vec![Span::Link {
                    url: "https://example.invalid/secret".into(),
                    children: vec![Span::Text {
                        text: "秘密".into(),
                    }],
                }],
            },
            Span::Link {
                url: "https://example.invalid/secret2".into(),
                children: vec![Span::Spoiler {
                    children: vec![Span::Text {
                        text: "秘密".into(),
                    }],
                }],
            },
        ],
        attachment: None,
        sensitive: false,
    };
    let doc = Doc::Message(m.clone());
    let out = serde_json::to_string(&masked(&doc)).unwrap();
    assert!(
        !out.contains("secret") && !out.contains("秘密"),
        "遮住后不留地址和字"
    );
    assert!(!json::line(&doc, &cx()).contains("secret"));
    assert!(json::line(&doc, &revealed()).contains("secret2"));
}

/// 会话项的最后一条同样遮住：会话项夹具里剧透的字不进 data
#[test]
fn chat_item_last_message_is_masked() {
    for d in world("chat_items").chat_items() {
        let raw = serde_json::to_value(&d).unwrap().to_string();
        let shown = json::line(&d, &cx());
        if raw.contains("\"spoiler\"") {
            assert!(
                !shown.contains("\"spoiler\",\"children\":[{"),
                "会话项最后一条的剧透没遮"
            );
        }
    }
}

/// 不漏的总测试，按 JSON 形状走，不挑位置：把未遮的树里**每一个**剧透子树（不论在正文、投票题目、回复引用、
/// 置顶内容还是会话项的最后一条）的字和地址都收出来，逐个核：不展开时 brief / detail / full / JSON 行都不含；
/// 展开后 JSON 行含
#[test]
fn no_spoiler_text_anywhere_without_reveal() {
    use serde_json::Value;
    fn texts(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for k in ["text", "url"] {
                    if let Some(Value::String(s)) = o.get(k) {
                        out.push(s.clone());
                    }
                }
                o.values().for_each(|x| texts(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| texts(x, out)),
            _ => {}
        }
    }
    fn secrets(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) if o.get("type") == Some(&Value::from("spoiler")) => texts(v, out),
            Value::Object(o) => o.values().for_each(|x| secrets(x, out)),
            Value::Array(a) => a.iter().for_each(|x| secrets(x, out)),
            _ => {}
        }
    }
    let mut checked = 0;
    for name in ["basic", "cases", "entities", "chat_items", "hidden"] {
        let w = world(name);
        for d in w.docs().into_iter().chain(w.chat_items()) {
            let mut found = vec![];
            secrets(&serde_json::to_value(&d).unwrap(), &mut found);
            for s in found
                .iter()
                .map(|s| s.trim())
                .filter(|s| s.chars().count() >= 2)
            {
                let said = [
                    brief(&d, &cx()),
                    speech::detail(&d, &cx()),
                    speech::full(&d, &cx()),
                    json::line(&d, &cx()),
                ];
                assert!(said.iter().all(|x| !x.contains(s)), "{name}：剧透漏出来了");
                assert!(
                    json::line(&d, &revealed()).contains(s),
                    "{name}：展开后该有原文"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 5, "夹具里的剧透太少，测不住：{checked}");
}

/// 隐藏内容在各处（`fixtures/speech/hidden.json`）：读法、展开后的读法、遮住后的 JSON
#[test]
fn hidden_everywhere() {
    let w = world("hidden");
    let mut docs = w.docs();
    docs.extend(w.chat_items());
    insta::assert_snapshot!(format!(
        "# brief\n{}\n# brief --reveal\n{}\n# json\n{}",
        layer(&docs, brief, &cx()),
        layer(&docs, brief, &revealed()),
        docs.iter()
            .map(|d| json::line(d, &cx()))
            .collect::<Vec<_>>()
            .join("\n"),
    ));
}
