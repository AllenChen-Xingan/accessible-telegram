//! 加上回应会不会挤掉我原先点的（`reaction_would_replace` / `reaction_state_unknown`，按 A：只有私聊、加密聊天信本机）。数据全是编的

use super::*;

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
    assert_eq!(log_lines(&r).last().unwrap()["reaction"]["may_replace"], 1);
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
    // 拿不准我点过哪些还用 --replace 放行：留痕的 reaction.may_replace 记 null，和「确知 0 个」分开
    let r = rig("null", false);
    let mut a = ask(THUMB, false, true);
    a.synced = false;
    let plan = block_on(prepare_react(&r.b, id(FRIEND), a, SendOrigin::Cli)).unwrap();
    assert!(!plan.replaces_known());
    assert_eq!(react_plan_json(&plan)["replaces_known"], false);
    let token = r.b.gate().issue(&plan, false).unwrap();
    react_within(&r.b, &plan, token, Duration::from_secs(2)).unwrap();
    let last = log_lines(&r).pop().unwrap();
    assert!(last["reaction"]["may_replace"].is_null(), "{last}");
    assert_eq!(last["reaction"]["message_id"], 5);
}

/// 一个非私聊会话（普通群 / 频道），消息 5 带 `extra`
fn group_rig(tag: &str, chat: i64, ty: Value, extra: Value) -> Rig {
    let r = rig(tag, false);
    r.b.set_chat(
        chat,
        vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": chat, "title": "读书会",
            "type": ty, "positions": [], "notification_settings": {}}})],
    );
    r.b.set_history(chat, vec![msg(chat, 5, extra)]);
    r
}

fn state_unknown(r: &Rig, chat: i64) -> bool {
    matches!(
        block_on(prepare_react(
            &r.b,
            id(chat),
            ask(THUMB, false, false),
            SendOrigin::Cli
        )),
        Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionStateUnknown
        })
    )
}

#[test]
fn server_list_failure_or_partial_page_is_unknown() {
    // 按 A：非私聊问服务器；取不到、一页没取全都算拿不准（不退回信缓存）
    const BG: i64 = -778;
    let basic = json!({"@type": "chatTypeBasicGroup", "basic_group_id": 778});
    let r = group_rig("fail", BG, basic.clone(), json!({"_added_fail": true}));
    assert!(state_unknown(&r, BG));
    // 别人点的超过一页（我的可能在下一页）
    let other = |i: i64| {
        json!({"@type": "addedReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"},
        "sender_id": {"@type": "messageSenderUser", "user_id": 10_000 + i}, "is_outgoing": false, "date": 1_792_755_100})
    };
    let many: Vec<Value> = (0..=i64::from(SERVER_PAGE)).map(other).collect();
    let r = group_rig("partial", BG, basic, json!({"_added": many}));
    assert!(state_unknown(&r, BG));
    // 拿不准还 --replace：放行，计划标拿不准
    let plan = block_on(prepare_react(
        &r.b,
        id(BG),
        ask(THUMB, false, true),
        SendOrigin::Cli,
    ))
    .unwrap();
    assert!(!plan.replaces_known());
}

#[test]
fn server_full_list_still_needs_sync() {
    // 服务器名单取全了，没等到补发结束照样按拿不准
    const BG: i64 = -779;
    let r = group_rig(
        "unsynced-bg",
        BG,
        json!({"@type": "chatTypeBasicGroup", "basic_group_id": 779}),
        json!({}),
    );
    let mut a = ask(THUMB, false, false);
    assert!(block_on(prepare_react(&r.b, id(BG), a, SendOrigin::Cli)).is_ok());
    a.synced = false;
    assert!(matches!(
        block_on(prepare_react(&r.b, id(BG), a, SendOrigin::Cli)),
        Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionStateUnknown
        })
    ));
}

#[test]
fn channel_asks_the_server_too() {
    const CH: i64 = -100_780;
    let ty = json!({"@type": "chatTypeSupergroup", "supergroup_id": 780, "is_channel": true});
    let r = group_rig("channel", CH, ty, json!({"_added_fail": true}));
    assert!(state_unknown(&r, CH));
}

#[test]
fn stale_plan_is_rechecked_before_submit() {
    // codex 032 第 1 轮 #1：先后准备 👍、❤️ 两份计划（当时都没点过），再依次交出——第二份交出前要复核，不许挤掉第一份
    let r = rig("stale", false);
    let plan = |e| {
        block_on(prepare_react(
            &r.b,
            id(FRIEND),
            ask(e, false, false),
            SendOrigin::Cli,
        ))
        .unwrap()
    };
    let (thumb, heart) = (plan(THUMB), plan("❤️"));
    let (t1, t2) = (
        r.b.gate().issue(&thumb, false).unwrap(),
        r.b.gate().issue(&heart, false).unwrap(),
    );
    react_within(&r.b, &thumb, t1, Duration::from_secs(2)).unwrap();
    let e = react_within(&r.b, &heart, t2, Duration::from_secs(2)).unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionWouldReplace
        }
    ));
    assert_eq!(r.b.submitted().len(), 1);
    assert_eq!(chosen(&r, FRIEND), vec![(THUMB.to_owned(), 1, true)]);
}

#[test]
fn concurrent_react_on_same_message_is_refused() {
    // 同一条消息上已有一个回应在交出途中：第二个不复核、直接按拿不准拒
    let r = rig("inflight", false);
    let p = block_on(prepare_react(
        &r.b,
        id(FRIEND),
        ask(THUMB, false, false),
        SendOrigin::Cli,
    ))
    .unwrap();
    let t = r.b.gate().issue(&p, false).unwrap();
    let busy = super::super::InFlight::take(r.b.gate(), id(FRIEND)).unwrap();
    let e = react_within(&r.b, &p, t, Duration::from_secs(2)).unwrap_err();
    assert!(matches!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReactionStateUnknown
        }
    ));
    drop(busy);
    assert!(r.b.submitted().is_empty());
}
