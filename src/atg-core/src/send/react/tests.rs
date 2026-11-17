//! `send::react` 的单测：假后端走全流程（数据全是编的）

use super::*;
use crate::backend::Backend;
use crate::config::{Profile, ServerKind};
use crate::error::{Holder, SendRefusedReason};
use crate::fake::{FakeAuth, FakeBackend, FakeSend};
use crate::gate::{DIRECT_LOG_FILE, GUARD_FILE, Gate};
use crate::link::{EventStream, block_on};
use crate::send::react_plan_json;
use crate::status::AuthStep;
use crate::tree::{Doc, Sender, Time};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const ME: i64 = 4242;
const FRIEND: i64 = 777;
const THUMB: &str = "👍";

struct Rig {
    b: FakeBackend,
    _stream: EventStream,
    dir: PathBuf,
}

fn new_chat(id: i64, title: &str) -> Value {
    json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
        "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [], "notification_settings": {}}})
}

fn msg(chat: i64, id: i64, extra: Value) -> Value {
    let mut m = json!({"@type": "message", "id": id, "chat_id": chat,
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false,
        "date": 1_792_755_000,
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "第5条", "entities": []}}});
    if let (Some(o), Value::Object(x)) = (m.as_object_mut(), extra) {
        o.extend(x);
    }
    m
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
    b.set_chat(ME, vec![new_chat(ME, "我自己")]);
    b.set_chat(FRIEND, vec![new_chat(FRIEND, "林小雨")]);
    b.set_history(ME, vec![msg(ME, 5, json!({}))]);
    b.set_history(FRIEND, vec![msg(FRIEND, 5, json!({}))]);
    let root = std::env::temp_dir().join(format!(
        "atg-react-{tag}-{}-{}",
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
    Rig {
        b,
        _stream: stream,
        dir: profile.dir().to_path_buf(),
    }
}

fn id(chat: i64) -> MsgId {
    MsgId {
        chat_id: chat,
        message_id: 5,
    }
}

/// 准备 → 开凭据 → 交出
fn once_with(
    r: &Rig,
    chat: i64,
    emoji: &str,
    remove: bool,
    replace: bool,
) -> Result<ConfirmMode, CoreError> {
    let plan = block_on(prepare_react(
        &r.b,
        id(chat),
        ask(emoji, remove, replace),
        SendOrigin::Cli,
    ))?;
    let token = r.b.gate().issue(&plan, false)?;
    react_within(&r.b, &plan, token, Duration::from_secs(2))
}

fn once(r: &Rig, chat: i64, emoji: &str, remove: bool) -> Result<ConfirmMode, CoreError> {
    once_with(r, chat, emoji, remove, false)
}

fn ask(emoji: &str, remove: bool, replace: bool) -> ReactAsk<'_> {
    ReactAsk {
        emoji,
        remove,
        replace,
        synced: true,
    }
}

fn chosen(r: &Rig, chat: i64) -> Vec<(String, u32, bool)> {
    let m = block_on(r.b.message_get(id(chat))).unwrap();
    m.reactions
        .iter()
        .map(|x| match &x.kind {
            ReactionKind::Emoji { emoji } => (emoji.clone(), x.count, x.chosen),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn log_lines(r: &Rig) -> Vec<Value> {
    std::fs::read_to_string(r.dir.join(DIRECT_LOG_FILE))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn request_shape_is_fixed() {
    let k = ReactionKind::Emoji {
        emoji: THUMB.into(),
    };
    let add = react_request(id(1), &k, false).unwrap();
    assert_eq!(add["@type"], "addMessageReaction");
    assert_eq!(add["reaction_type"]["@type"], "reactionTypeEmoji");
    assert_eq!(add["reaction_type"]["emoji"], THUMB);
    assert_eq!(add["is_big"], false);
    assert_eq!(add["update_recent_reactions"], false);
    let rm = react_request(id(1), &k, true).unwrap();
    assert_eq!(rm["@type"], "removeMessageReaction");
    assert!(rm.get("is_big").is_none());
    // 认不出的种类组不出请求
    let unknown = ReactionKind::Unknown {
        td_type: "reactionTypeX".into(),
    };
    assert!(react_request(id(1), &unknown, false).is_none());
}

#[test]
fn emoji_shape_check() {
    for ok in [THUMB, "❤️", "👍🏽", "👨‍👩‍👧", "1️⃣"] {
        assert!(check_emoji(ok).is_ok(), "{ok}");
    }
    for bad in [
        "👍\u{202E}",
        "\u{E0041}👍",
        "",
        "ok",
        "1",
        " 👍",
        "👍\n",
        "👍 👍",
        &"👍".repeat(17),
        "\u{7}",
    ] {
        let e = check_emoji(bad).unwrap_err();
        assert!(
            matches!(
                e,
                CoreError::InvalidSend {
                    why: InvalidSendWhy::ReactionInvalid
                }
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn test_dc_saved_reacts_without_asking() {
    let r = rig("tdc", true);
    assert_eq!(once(&r, ME, THUMB, false).unwrap(), ConfirmMode::TestExempt);
    assert_eq!(chosen(&r, ME), vec![(THUMB.to_owned(), 1, true)]);
    let sub = r.b.submitted();
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0]["@type"], "addMessageReaction");
    // 去掉：点过才能去；再去一次本地就拒，不交出
    once(&r, ME, THUMB, true).unwrap();
    assert!(chosen(&r, ME).is_empty());
    let e = once(&r, ME, THUMB, true).unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionNotChosen
        }
    ));
    assert_eq!(r.b.submitted().len(), 2);
    // 测试服务器上 test_exempt 不留痕
    assert!(log_lines(&r).is_empty());
}

#[test]
fn test_dc_other_chat_is_refused() {
    let r = rig("tdc-other", true);
    let e = block_on(prepare_react(
        &r.b,
        id(FRIEND),
        ask(THUMB, false, false),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::TestDcSavedOnly,
            ..
        }
    ));
    assert!(r.b.submitted().is_empty());
}

#[test]
fn main_server_reacts_directly_and_logs() {
    // 正式服务器：和发文字一样直发（#113 去掉 Touch ID）；照样写 direct-send.log，不进防重复
    let r = rig("main", false);
    assert_eq!(
        once(&r, FRIEND, THUMB, false).unwrap(),
        ConfirmMode::DirectMode
    );
    let log = log_lines(&r);
    assert_eq!(log.len(), 2, "{log:?}");
    for (l, want) in log.iter().zip(["submitted", "succeeded"]) {
        assert_eq!(l["target"]["message_id"], 5);
        assert_eq!(l["target"]["replaced"], 0);
        assert_eq!(l["kind"], "react");
        assert_eq!(l["result"], want);
        assert_eq!(l["chat_id"], FRIEND);
        // 不记 emoji 本身
        assert!(!l.to_string().contains(THUMB));
    }
    assert!(!r.dir.join(GUARD_FILE).exists());
    // 同一个回应再点一次不拦（不进防重复）
    once(&r, FRIEND, THUMB, true).unwrap();
    assert_eq!(log_lines(&r)[3]["kind"], "unreact");
}

#[test]
fn token_is_bound_to_the_plan() {
    let r = rig("bind", false);
    let add = block_on(prepare_react(
        &r.b,
        id(FRIEND),
        ask(THUMB, false, false),
        SendOrigin::Cli,
    ))
    .unwrap();
    let other = block_on(prepare_react(
        &r.b,
        id(FRIEND),
        ask("❤️", false, false),
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = r.b.gate().issue(&add, false).unwrap();
    let e = react_within(&r.b, &other, token, Duration::from_secs(2)).unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::ContentMismatch,
            ..
        }
    ));
    assert!(r.b.submitted().is_empty());
}

#[test]
fn rejected_submit_is_logged_as_failed() {
    let r = rig("reject", false);
    r.b.script_sends(vec![FakeSend::Reject(CoreError::InvalidInput { td: None })]);
    let e = once(&r, FRIEND, THUMB, false).unwrap_err();
    assert_eq!(e.code(), "invalid_input");
    let log = log_lines(&r);
    assert_eq!(log[1]["result"], "failed");
    assert_eq!(log[1]["error"], "invalid_input");
    assert!(chosen(&r, FRIEND).is_empty());
}

#[test]
fn added_reactions_lists_and_pages() {
    let r = rig("who", false);
    let other = json!({"@type": "addedReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"},
        "sender_id": {"@type": "messageSenderUser", "user_id": FRIEND}, "is_outgoing": false, "date": 1_792_755_100});
    r.b.set_history(FRIEND, vec![msg(FRIEND, 5, json!({"_added": [other]}))]);
    once(&r, FRIEND, THUMB, false).unwrap();
    let all = block_on(r.b.added_reactions(id(FRIEND), None, "", 10)).unwrap();
    assert_eq!(all.total, 2);
    assert!(all.next_offset.is_empty());
    assert!(all.items[0].outgoing);
    assert_eq!(all.items[1].sender, crate::model::SenderId::User(FRIEND));
    // 只要 ❤️ 的
    let heart = ReactionKind::Emoji {
        emoji: "❤️".into()
    };
    let only = block_on(r.b.added_reactions(id(FRIEND), Some(&heart), "", 10)).unwrap();
    assert_eq!(only.items.len(), 1);
    // 分页
    let p1 = block_on(r.b.added_reactions(id(FRIEND), None, "", 1)).unwrap();
    assert_eq!((p1.items.len(), p1.next_offset.as_str()), (1, "1"));
    let p2 = block_on(r.b.added_reactions(id(FRIEND), None, &p1.next_offset, 1)).unwrap();
    assert_eq!((p2.items.len(), p2.next_offset.as_str()), (1, ""));
}

#[test]
fn tdjson_maps_added_reactions() {
    let v = json!({"@type": "addedReactions", "total_count": 3, "next_offset": "abc", "reactions": [
        {"@type": "addedReaction", "type": {"@type": "reactionTypePaid"},
         "sender_id": {"@type": "messageSenderChat", "chat_id": -100}, "is_outgoing": false, "date": 7}]});
    let a = crate::tdjson::added_reactions(&v);
    assert_eq!((a.total, a.next_offset.as_str()), (3, "abc"));
    assert_eq!(a.items[0].kind, ReactionKind::Paid);
    assert_eq!(a.items[0].sender, crate::model::SenderId::Chat(-100));
    assert_eq!(a.items[0].date, 7);
}

#[test]
fn reactor_speech() {
    let cx = crate::speech::SpeechCtx {
        utc_offset: 8 * 3600,
        offset_at: None,
        listen_key: None,
        expand_key: None,
        reveal: false,
    };
    let doc = |sender, outgoing| {
        Doc::Reactor(crate::tree::Reactor {
            kind: ReactionKind::Emoji {
                emoji: THUMB.into(),
            },
            sender,
            outgoing,
            // 2026-10-20 19:33 +08:00
            date: Time(1_792_495_980),
        })
    };
    let named = Sender::User {
        user_id: FRIEND,
        name: Some("林小雨".into()),
        deleted: false,
    };
    let nameless = Sender::User {
        user_id: FRIEND,
        name: None,
        deleted: false,
    };
    assert_eq!(
        crate::speech::brief(&doc(named.clone(), false), &cx),
        "林小雨，👍，19:33"
    );
    assert_eq!(
        crate::speech::brief(&doc(named, true), &cx),
        "我，👍，19:33"
    );
    assert_eq!(
        crate::speech::brief(&doc(nameless, false), &cx),
        "未知发送人，👍，19:33"
    );
    assert_eq!(
        crate::json::kind(&doc(
            Sender::Unknown {
                td_type: "x".into()
            },
            false
        )),
        "reactor"
    );
}

#[test]
fn adding_over_my_other_reaction_needs_replace() {
    // TDLib 按上限把我原先点的回应挤掉（非会员 1 个）：不确认就没人看得到，所以默认拒，--replace 才放行
    let r = rig("replace", false);
    once(&r, FRIEND, "❤️", false).unwrap();
    let e = once(&r, FRIEND, THUMB, false).unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionWouldReplace
        }
    ));
    assert_eq!(r.b.submitted().len(), 1);
    assert_eq!(chosen(&r, FRIEND), vec![("❤️".to_owned(), 1, true)]);
    let plan = block_on(prepare_react(
        &r.b,
        id(FRIEND),
        ask(THUMB, false, true),
        SendOrigin::Cli,
    ))
    .unwrap();
    let heart = ReactionKind::Emoji {
        emoji: "❤️".into()
    };
    assert_eq!(plan.replaces(), std::slice::from_ref(&heart));
    assert_eq!(react_plan_json(&plan)["replaces"][0]["emoji"], "❤️");
    once_with(&r, FRIEND, THUMB, false, true).unwrap();
    // 正式服务器才留痕：留痕记下挤掉了 1 个
    assert_eq!(log_lines(&r).last().unwrap()["target"]["replaced"], 1);
    assert_eq!(chosen(&r, FRIEND), vec![(THUMB.to_owned(), 1, true)]);
    // 同一个回应再加一次不算挤掉别的
    assert!(
        block_on(prepare_react(
            &r.b,
            id(FRIEND),
            ask(THUMB, false, false),
            SendOrigin::Cli
        ))
        .is_ok()
    );
}

#[test]
fn unsynced_state_is_not_trusted() {
    // 没等到补发结束：本机库里「我点了哪些」可能是旧的（人刚在手机上点了），不带 --replace 就拒
    let r = rig("unsynced", false);
    let mut a = ask(THUMB, false, false);
    a.synced = false;
    let e = block_on(prepare_react(&r.b, id(FRIEND), a, SendOrigin::Cli)).unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionStateUnknown
        }
    ));
    a.replace = true;
    assert!(block_on(prepare_react(&r.b, id(FRIEND), a, SendOrigin::Cli)).is_ok());
    // 去掉不受影响（点过没点过只会误报 reaction_not_chosen，后果轻）
    assert!(r.b.submitted().is_empty());
}

#[test]
fn supergroup_asks_the_server_who_reacted() {
    // 超级群没打开时回应变化不一定推来：本机缓存里没有我点的 ❤️，服务器名单里有 → 仍按会挤掉拒
    let r = rig("sg", false);
    const SG: i64 = -100_777;
    r.b.set_chat(SG, vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": SG, "title": "周末爬山群",
        "type": {"@type": "chatTypeSupergroup", "supergroup_id": 777, "is_channel": false},
        "positions": [], "notification_settings": {}}})]);
    let mine = json!({"@type": "addedReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"},
        "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true, "date": 1_792_755_100});
    r.b.set_history(SG, vec![msg(SG, 5, json!({"_added": [mine]}))]);
    let e = block_on(prepare_react(
        &r.b,
        id(SG),
        ask(THUMB, false, false),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionWouldReplace
        }
    ));
    let plan = block_on(prepare_react(
        &r.b,
        id(SG),
        ask(THUMB, false, true),
        SendOrigin::Cli,
    ))
    .unwrap();
    assert_eq!(plan.replaces().len(), 1);
    // 服务器名单里没有我的：拿得准，照常放行
    r.b.set_history(SG, vec![msg(SG, 5, json!({}))]);
    assert!(
        block_on(prepare_react(
            &r.b,
            id(SG),
            ask(THUMB, false, false),
            SendOrigin::Cli
        ))
        .is_ok()
    );
}

#[test]
fn basic_group_asks_the_server_who_reacted() {
    // 自审第 4 轮 P1：普通群的回应变化 TDLib 也不一定推（只有私聊、加密聊天推），本机缓存不能信，要问服务器
    let r = rig("bg", false);
    const BG: i64 = -777;
    r.b.set_chat(
        BG,
        vec![
            json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": BG, "title": "读书会",
        "type": {"@type": "chatTypeBasicGroup", "basic_group_id": 777},
        "positions": [], "notification_settings": {}}}),
        ],
    );
    let mine = json!({"@type": "addedReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"},
        "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true, "date": 1_792_755_100});
    r.b.set_history(BG, vec![msg(BG, 5, json!({"_added": [mine]}))]);
    let e = block_on(prepare_react(
        &r.b,
        id(BG),
        ask(THUMB, false, false),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionWouldReplace
        }
    ));
}

#[test]
fn unknown_state_with_replace_logs_null() {
    // 拿不准我点过哪些还用 --replace 放行：留痕的 replaced 记 null，和「确知 0 个」分开
    let r = rig("null", false);
    let mut a = ask(THUMB, false, true);
    a.synced = false;
    let plan = block_on(prepare_react(&r.b, id(FRIEND), a, SendOrigin::Cli)).unwrap();
    assert!(!plan.replaces_known());
    assert_eq!(react_plan_json(&plan)["replaces_known"], false);
    let token = r.b.gate().issue(&plan, false).unwrap();
    react_within(&r.b, &plan, token, Duration::from_secs(2)).unwrap();
    let last = log_lines(&r).pop().unwrap();
    assert!(last["target"]["replaced"].is_null(), "{last}");
    assert_eq!(last["target"]["message_id"], 5);
}
