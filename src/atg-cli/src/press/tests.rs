//! 假后端：数据全是编的，不碰真实账号与本机档案目录；**不真的打开任何网址**（`open` 出口换成只记录）。
use super::*;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::Holder;
use atg_core::fake::{FakeAuth, FakeBackend};
use atg_core::gate::Gate;

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
const BOT: i64 = 900;
const DATA: &str = "c2VjcmV0LWRhdGE=";

fn backend(keyboard_at: i64) -> FakeBackend {
    backend_in(keyboard_at, false)
}

/// `group`：把机器人放进一个普通群（群里按键盘按钮要回复键盘那条，机器人才收得到）
fn backend_in(keyboard_at: i64, group: bool) -> FakeBackend {
    let chat_type = if group {
        json!({"@type": "chatTypeBasicGroup", "basic_group_id": 31})
    } else {
        json!({"@type": "chatTypePrivate", "user_id": BOT})
    };
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+15550001234".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(
        BOT,
        vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": BOT, "title": "订票助手",
            "type": chat_type, "positions": [], "reply_markup_message_id": keyboard_at,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}})],
    );
    let text = |t: &str| json!({"@type": "formattedText", "text": t, "entities": []});
    let ib = |t: &str, ty: Value| json!({"@type": "inlineKeyboardButton", "text": t, "type": ty});
    b.set_history(
        BOT,
        vec![
            json!({"@type": "message", "id": 5, "chat_id": BOT,
                "sender_id": {"@type": "messageSenderUser", "user_id": BOT}, "is_outgoing": false, "date": 1_792_755_000,
                "content": {"@type": "messageText", "text": text("选一个")},
                "reply_markup": {"@type": "replyMarkupInlineKeyboard", "rows": [[
                    ib("确认", json!({"@type": "inlineKeyboardButtonTypeCallback", "data": DATA})),
                    ib("官网", json!({"@type": "inlineKeyboardButtonTypeUrl", "url": "https://example.org/a"}))]]}}),
            json!({"@type": "message", "id": 6, "chat_id": BOT,
                "sender_id": {"@type": "messageSenderUser", "user_id": BOT}, "is_outgoing": false, "date": 1_792_755_001,
                "content": {"@type": "messageText", "text": text("请回答")},
                "reply_markup": {"@type": "replyMarkupShowKeyboard", "rows": [[
                    {"@type": "keyboardButton", "text": "好的", "type": {"@type": "keyboardButtonTypeText"}}]]}}),
        ],
    );
    b.push_net("connectionStateReady");
    b
}

fn profile() -> Profile {
    let root = atg_core::testkit::temp_dir("cli-press");
    Profile::create(&root, "p", ServerKind::Main).unwrap()
}

fn press_p(msg: i64, n: usize, dry_run: bool) -> PressParams {
    PressParams {
        chat: BOT.to_string(),
        message_id: msg,
        button: n,
        again: false,
        dry_run,
        tz: Some("+08:00".into()),
    }
}

fn lines(cap: &Cap) -> Vec<Value> {
    cap.out
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// 跑一次，回（退出码，输出，打开过的网址）
fn run(b: &FakeBackend, p: &PressParams, format: Format) -> (u8, Cap, Vec<String>) {
    crate::history::with_backend(b, &profile(), Instant::now(), |b, st, fe| {
        let mut cap = Cap::default();
        let mut opened = Vec::new();
        let t = Duration::from_millis(500);
        let code = execute(
            b,
            st,
            fe,
            p,
            &mut cap,
            format,
            Instant::now(),
            t,
            &mut |u| {
                opened.push(u.to_owned());
            },
        );
        Ok((code, cap, opened))
    })
    .unwrap_or_else(|_| panic!("起不来"))
}

#[test]
fn callback_presses_and_opens_only_web_urls() {
    let b = backend(0);
    b.script_callback(Some(CallbackReply {
        text: "去这里付款".into(),
        alert: false,
        url: "https://example.org/pay".into(),
    }));
    let (code, cap, opened) = run(&b, &press_p(5, 1, false), Format::Json);
    assert_eq!(code, 0, "{:?}", cap.out);
    let ls = lines(&cap);
    assert_eq!(ls[0]["kind"], "plan");
    assert_eq!(ls[0]["data"]["action"], "press");
    assert_eq!(ls[0]["data"]["text"], "确认");
    let end = ls.last().unwrap();
    assert_eq!(end["data"]["pressed"], true);
    assert_eq!(end["data"]["reply"]["text"], "去这里付款");
    assert_eq!(end["data"]["opened"], true);
    assert_eq!(opened, vec!["https://example.org/pay".to_owned()]);
    // 回调数据不出现在任何输出行里
    assert!(cap.out.iter().all(|l| !l.contains(DATA)));
}

/// 人定 B 的约束：只开 http / https，别的只读出
#[test]
fn non_web_url_is_not_opened() {
    let b = backend(0);
    b.script_callback(Some(CallbackReply {
        text: "好".into(),
        alert: true,
        url: "tg://resolve?domain=x".into(),
    }));
    let (_, cap, opened) = run(&b, &press_p(5, 1, false), Format::Text);
    assert!(opened.is_empty());
    let end = cap.out.last().unwrap();
    assert!(end.contains("弹窗") && end.contains("没打开"), "{end}");
    for bad in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://a b",
        "HTTPS://ok.example",
    ] {
        assert_eq!(openable(bad), bad == "HTTPS://ok.example", "{bad}");
    }
}

#[test]
fn no_reply_is_reported_as_success() {
    let b = backend(0);
    let (code, cap, opened) = run(&b, &press_p(5, 1, false), Format::Text);
    assert_eq!(code, 0);
    assert!(cap.out.last().unwrap().contains("机器人没回话"));
    assert!(opened.is_empty());
}

#[test]
fn dry_run_does_not_press() {
    let b = backend(0);
    let (code, cap, _) = run(&b, &press_p(5, 1, true), Format::Json);
    assert_eq!(code, 0);
    assert_eq!(lines(&cap).last().unwrap()["data"]["dry_run"], true);
    assert!(b.submitted().is_empty());
}

#[test]
fn url_button_is_read_only() {
    let b = backend(0);
    let (code, cap, opened) = run(&b, &press_p(5, 2, false), Format::Json);
    assert_eq!(code, 0);
    let end = lines(&cap).pop().unwrap();
    assert_eq!(end["data"]["pressed"], false);
    assert_eq!(end["data"]["read_only"]["type"], "url");
    assert_eq!(end["data"]["read_only"]["value"], "https://example.org/a");
    assert!(opened.is_empty());
    assert!(b.submitted().is_empty());
}

/// 键盘文字按钮转给 send；键盘已换过的照样发，计划里说一句（人定 B）
#[test]
fn keyboard_button_goes_through_send_and_says_replaced() {
    let b = backend(99);
    let (_, cap, _) = run(&b, &press_p(6, 1, true), Format::Text);
    let plan = &cap.out[0];
    assert!(plan.contains("键盘按钮"), "{plan}");
    assert!(plan.contains("键盘已更换，照原文字发"), "{plan}");
    assert!(plan.contains("好的"), "{plan}");
    let b = backend(6);
    let (_, cap, _) = run(&b, &press_p(6, 1, true), Format::Json);
    let plan = &lines(&cap)[0];
    assert_eq!(plan["data"]["press"]["keyboard"], "here");
    assert_eq!(plan["data"]["text"], "好的");
}

#[test]
fn missing_button_is_invalid_input() {
    let b = backend(0);
    let (code, cap, _) = run(&b, &press_p(5, 9, false), Format::Json);
    let end = lines(&cap).pop().unwrap();
    assert_eq!(end["data"]["why"], "button_not_found");
    assert_eq!(end["data"]["retryable"], false);
    assert_ne!(code, 0);
}

/// 自审 13b-2 第 1 轮 P2：群里按键盘文字按钮，发出的要回复键盘所在那条（同官方客户端）——机器人默认开隐私模式，
/// 群里只收得到命令与回复它的消息，不回复就等于没按
#[test]
fn keyboard_button_in_group_replies_to_the_keyboard_message() {
    let b = backend_in(6, true);
    let (code, cap, _) = run(&b, &press_p(6, 1, true), Format::Json);
    assert_eq!(code, 0, "{:?}", cap.out);
    let plan = &lines(&cap)[0];
    assert_eq!(plan["data"]["reply_to"]["message_id"], 6, "{plan}");
    // 私聊里不用回复
    let b = backend(6);
    let (_, cap, _) = run(&b, &press_p(6, 1, true), Format::Json);
    assert!(lines(&cap)[0]["data"]["reply_to"].is_null());
}
