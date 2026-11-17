//! `send` 的测试：本文件放共用的假后端夹具（`rig`、`draft`、`send_once`……，数据全是编的）；
//! 发文字的测试在 `tests/text.rs`，重发与确认框的测试在 `tests/resending.rs`。

use super::*;
// 下面几项只为测试放宽成 pub(super)：测试原是它们的子模块、看得到私有项，拆开后成了兄弟
use super::resending::resend_digest;
use super::text::send_topic;

// ───── 假后端走全流程（数据全是编的；名字取自 fixtures/names.txt）─────

use crate::backend::Backend;
use crate::config::Profile;
use crate::error::Holder;
use crate::fake::{FakeAuth, FakeBackend, FakeSend};
use crate::gate::Gate;
use crate::link::block_on;
use crate::status::AuthStep;
use std::sync::atomic::{AtomicU32, Ordering};

const ME: i64 = 4242;
const FRIEND: i64 = 777;
const FORUM: i64 = -100_555;

fn new_chat(id: i64, title: &str, ty: Value) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title, "type": ty,
        "positions": [], "notification_settings": {}}})
}

fn msg(chat: i64, id: i64, text: &str, topic: Value) -> Value {
    json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false,
        "date": 1_792_755_000, "topic_id": topic,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}}})
}

struct Rig {
    b: FakeBackend,
    stream: EventStream,
}

fn rig(tag: &str, test_dc: bool) -> Rig {
    static N: AtomicU32 = AtomicU32::new(0);
    let gate = Gate::new(Holder::Cli);
    let b = FakeBackend::with_gate(gate).unwrap();
    b.set_auth(FakeAuth {
        phone: if test_dc {
            "+9996612345"
        } else {
            "+15550001234"
        }
        .into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(
        ME,
        vec![new_chat(
            ME,
            "我自己",
            json!({"@type": "chatTypePrivate", "user_id": ME}),
        )],
    );
    b.set_chat(
        FRIEND,
        vec![new_chat(
            FRIEND,
            "林小雨",
            json!({"@type": "chatTypePrivate", "user_id": FRIEND}),
        )],
    );
    b.set_chat(
        FORUM,
        vec![new_chat(
            FORUM,
            "周末爬山群",
            json!({"@type": "chatTypeSupergroup", "supergroup_id": 555, "is_channel": false}),
        )],
    );
    b.set_forum(FORUM);
    b.set_history(ME, vec![msg(ME, 5, "第5条", Value::Null)]);
    b.set_history(FRIEND, vec![msg(FRIEND, 5, "第5条", Value::Null)]);
    b.set_history(
        FORUM,
        vec![msg(
            FORUM,
            9,
            "第9条",
            json!({"@type": "messageTopicForum", "forum_topic_id": 3}),
        )],
    );
    let root = std::env::temp_dir().join(format!(
        "atg-send-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let kind = if test_dc {
        ServerKind::Test
    } else {
        ServerKind::Main
    };
    let profile = Profile::create(&root, "p", kind).unwrap();
    let stream = b.subscribe().unwrap();
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    Rig { b, stream }
}

fn draft(text: &str, reply_to: Option<i64>) -> Draft {
    Draft {
        text: text.into(),
        reply_to,
    }
}

/// 准备 → 开凭据 → 交出 → 等结果，返回下落
fn send_once(r: &mut Rig, chat: i64, d: &Draft, again: bool) -> Result<Waited, CoreError> {
    let plan = block_on(prepare_text(&r.b, chat, d, SendOrigin::Cli))?;
    let token = r.b.gate().issue(&plan, again)?;
    let (_, _, w) = send_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_secs(2),
        &mut |_| {},
    )?;
    Ok(w)
}

fn refused_reason(e: CoreError) -> SendRefusedReason {
    match e {
        CoreError::SendRefused { reason, .. } => reason,
        other => panic!("不是 send_refused：{other:?}"),
    }
}

mod resending;
mod text;
