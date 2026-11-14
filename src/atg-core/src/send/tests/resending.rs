//! 重发与确认框的测试。夹具在 `../tests.rs`。

use super::*;

#[test]
fn resend_in_direct_mode_leaves_no_dedup_record() {
    let mut r = rig("rs-direct", true, false, Ok(()));
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    let Waited::Failed { message, .. } =
        send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap()
    else {
        panic!("应当失败");
    };
    let plan = block_on(prepare_resend(
        &r.b,
        FRIEND,
        &[message.id.message_id],
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    let news = block_on(resend(&r.b, &plan, token)).unwrap();
    let subs: Vec<Submitted> = news.into_iter().flatten().collect();
    assert_eq!(subs.len(), 1);
    let temps: Vec<MsgId> = subs.iter().map(Submitted::temp).collect();
    let w = wait(&mut r.stream, &temps, Duration::from_secs(2), &mut |_| {});
    for (sub, w) in subs.into_iter().zip(&w) {
        finish(&r.b, &plan, sub, w);
    }
    // 同一句话（第一次失败了，不拦）现在照样能直发：重发没往防重复记录里写东西
    assert!(send_once(&mut r, FRIEND, &draft("好的", None), false).is_ok());
}

#[test]
fn confirm_body_cannot_hide_text_behind_blank_lines() {
    let padded = format!("好的{}真正要发的话", "\n".repeat(40));
    let b = confirm_body(&padded);
    // 一段，不靠换行排版；行数与字数在最前面
    assert_eq!(b.lines().count(), 2);
    assert!(b.starts_with("共 "));
    assert!(b.contains("41 行"));
    assert!(b.contains("真正要发的话"));
    // ⏎ 是换行的意思：正文里本来的换行一个不剩
    assert!(
        !confirm_body("一\n二")
            .lines()
            .nth(1)
            .unwrap()
            .contains('\n')
    );
}

#[test]
fn main_server_long_text_is_refused_without_asking() {
    let mut r = rig("long", false, false, Ok(()));
    let long = "字".repeat(gate::CONFIRM_MAX_CHARS);
    let e = send_once(&mut r, ME, &draft(&long, None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
    assert_eq!(r.asks.load(Ordering::SeqCst), 0);
    assert!(r.b.submitted().is_empty());
    // 放得下的照常问人；直发与测试豁免不受这条管
    let ok = "字".repeat(30);
    send_once(&mut r, ME, &draft(&ok, None), false).unwrap();
    assert_eq!(r.asks.load(Ordering::SeqCst), 1);
    let mut d = rig("long-direct", true, false, Ok(()));
    send_once(&mut d, FRIEND, &draft(&long, None), false).unwrap();
}

#[test]
fn resend_confirm_shows_and_binds_the_reply_target() {
    // codex 016 第 1 轮 #4：带回复的失败消息重发时，人要知道它回复谁；回复对象也进摘要
    let mut r = rig("rs-reply", false, true, Ok(()));
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "X".into(),
        can_retry: true,
    }]);
    let Waited::Failed { message, .. } =
        send_once(&mut r, ME, &draft("回你", Some(5)), false).unwrap()
    else {
        panic!("应当失败");
    };
    let plan = block_on(prepare_resend(
        &r.b,
        ME,
        &[message.id.message_id],
        SendOrigin::Cli,
    ))
    .unwrap();
    assert_eq!(
        plan.items()[0].reply,
        ResendReply::Message(MsgId {
            chat_id: ME,
            message_id: 5
        })
    );
    assert!(
        plan.confirm_text().contains("（回复 id 5 那条）"),
        "{}",
        plan.confirm_text()
    );
    let mut other = plan.items.clone();
    other[0].reply = ResendReply::None;
    assert_ne!(
        resend_digest(&plan.request, &other, ME, ServerKind::Test),
        plan.digest
    );
}

#[test]
fn resend_confirm_shows_the_text_and_binds_it() {
    let mut r = rig("rs-show", false, false, Ok(()));
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    // 正式服务器 + 默认模式：发第一次时人按了（假认证器放行），失败了
    let Waited::Failed { message, .. } =
        send_once(&mut r, ME, &draft("周末北门停车场见", None), false).unwrap()
    else {
        panic!("应当失败");
    };
    let plan = block_on(prepare_resend(
        &r.b,
        ME,
        &[message.id.message_id],
        SendOrigin::Cli,
    ))
    .unwrap();
    assert_eq!(plan.items()[0].text.as_deref(), Some("周末北门停车场见"));
    let t = plan.confirm_text();
    assert!(
        t.starts_with("重发到 ") && t.contains("周末北门停车场见"),
        "{t}"
    );
    assert!(plan.confirm_fits());
    // 正文进了摘要：同 id、不同正文 → 摘要不同；准备时算的就是这个函数
    assert_eq!(
        resend_digest(&plan.request, &plan.items, ME, ServerKind::Main),
        plan.digest
    );
    let mut other = plan.items.clone();
    other[0].text = Some("别的".into());
    assert_ne!(
        resend_digest(&plan.request, &other, ME, ServerKind::Main),
        plan.digest
    );
}

#[test]
fn long_chat_title_counts_toward_what_must_fit() {
    // 正文很短，但会话名长到把整段说明撑出确认框：照样拒，不弹框
    let mut r = rig("long-title", false, false, Ok(()));
    let title = "周末爬山群".repeat(50);
    r.b.set_chat(
        888,
        vec![new_chat(
            888,
            &title,
            json!({"@type": "chatTypePrivate", "user_id": 888}),
        )],
    );
    let e = send_once(&mut r, 888, &draft("好的", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
    assert_eq!(r.asks.load(Ordering::SeqCst), 0);
}

#[test]
fn resend_rechecks_before_submitting() {
    // codex 016 第 2 轮 #2：准备到交出之间那条被改了 → 拒、什么都不交出；没变 → 照常交出
    let failed = |text: &str| {
        let mut m = msg(ME, 100, text, Value::Null);
        m["sending_state"] = json!({"@type": "messageSendingStateFailed",
            "error": {"@type": "error", "code": 400, "message": "X"}, "can_retry": true,
            "need_another_sender": false, "need_another_reply_quote": false, "need_drop_reply": false,
            "required_paid_message_star_count": 0, "retry_after": 0.0});
        m
    };
    let r = rig("recheck", false, true, Ok(()));
    r.b.set_history(ME, vec![failed("周末见")]);
    let plan = block_on(prepare_resend(&r.b, ME, &[100], SendOrigin::Cli)).unwrap();
    r.b.set_history(ME, vec![failed("改过的话")]);
    let t = r.b.gate().issue(&plan, false).unwrap();
    let e = block_on(resend(&r.b, &plan, t)).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::ContentMismatch);
    assert!(r.b.submitted().is_empty());
    // 没变：真的交出了重发
    r.b.set_history(ME, vec![failed("周末见")]);
    let plan = block_on(prepare_resend(&r.b, ME, &[100], SendOrigin::Cli)).unwrap();
    let t = r.b.gate().issue(&plan, false).unwrap();
    block_on(resend(&r.b, &plan, t)).unwrap();
    assert_eq!(r.b.submitted().len(), 1);
}

#[test]
fn many_resend_items_do_not_fit() {
    let r = rig("many", false, false, Ok(()));
    let failed = |id: i64| {
        let mut m = msg(ME, id, "一", Value::Null);
        m["sending_state"] = json!({"@type": "messageSendingStateFailed",
            "error": {"@type": "error", "code": 400, "message": "X"}, "can_retry": true,
            "need_another_sender": false, "need_another_reply_quote": false, "need_drop_reply": false,
            "required_paid_message_star_count": 0, "retry_after": 0.0});
        m
    };
    let ids: Vec<i64> = (100..110).collect();
    r.b.set_history(ME, ids.iter().map(|&i| failed(i)).collect());
    let plan = block_on(prepare_resend(&r.b, ME, &ids, SendOrigin::Cli)).unwrap();
    // 每条正文只有一个字，合起来很短，但十条的行数放不下
    assert!(!plan.confirm_fits());
    let e = r.b.gate().issue(&plan, false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::TooLongToConfirm);
}
