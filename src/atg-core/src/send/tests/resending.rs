//! 重发与确认框的测试。夹具在 `../tests.rs`。

use super::*;

#[test]
fn resend_in_direct_mode_leaves_no_dedup_record() {
    let mut r = rig("rs-direct", false);
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
fn resend_binds_the_reply_target() {
    // codex 016 第 1 轮 #4：带回复的失败消息重发时，回复对象进摘要
    let mut r = rig("rs-reply", true);
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
    let mut other = plan.items.clone();
    other[0].reply = ResendReply::None;
    assert_ne!(
        resend_digest(&plan.request, &other, ME, ServerKind::Test),
        plan.digest
    );
}

#[test]
fn resend_binds_the_text() {
    let mut r = rig("rs-show", false);
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    // 正式服务器：第一次直发失败了
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
    let r = rig("recheck", true);
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

/// 造一条可以重发的失败消息，返回它的 id
fn failed_one(r: &mut Rig) -> i64 {
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "X".into(),
        can_retry: true,
    }]);
    let Waited::Failed { message, .. } = send_once(r, FRIEND, &draft("好的", None), false).unwrap()
    else {
        panic!("应当失败");
    };
    message.id.message_id
}

/// 重发卡在交出前的复核里就到点（#80）：没交出 → `network`，不算直发
#[test]
fn resend_timing_out_before_handing_over_is_network() {
    let mut r = rig("rs-pre-hang", false);
    let id = failed_one(&mut r);
    let plan = block_on(prepare_resend(&r.b, FRIEND, &[id], SendOrigin::Cli)).unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    let before = r.b.gate().direct_sent();
    r.b.fail_next(
        "failed_info",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let e = resend_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_millis(300),
        &mut |_| {},
    )
    .err()
    .unwrap();
    assert_eq!(e.code(), "network");
    assert_eq!(r.b.gate().direct_sent(), before);
}

/// 重发交出之后才到点：照旧结果未知
#[test]
fn resend_timing_out_after_handing_over_stays_unknown() {
    let mut r = rig("rs-post-hang", false);
    let id = failed_one(&mut r);
    let plan = block_on(prepare_resend(&r.b, FRIEND, &[id], SendOrigin::Cli)).unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    r.b.fail_next(
        "submit",
        CoreError::Internal {
            what: FakeBackend::HANG,
        },
    );
    let e = resend_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_millis(300),
        &mut |_| {},
    )
    .err()
    .unwrap();
    assert!(matches!(
        e,
        CoreError::SendOutcomeUnknown {
            why: UnknownWhy::Timeout,
            ..
        }
    ));
}

/// 重发的留痕按条记（#83）：每条重发一行「已提交」、一行结果，都带它的旧 id
#[test]
fn resend_log_has_one_line_per_message_with_old_id() {
    let mut r = rig("rs-log", false);
    let id = failed_one(&mut r);
    let plan = block_on(prepare_resend(&r.b, FRIEND, &[id], SendOrigin::Cli)).unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    resend_and_wait(
        &r.b,
        &plan,
        token,
        &mut r.stream,
        Duration::from_secs(2),
        &mut |_| {},
    )
    .unwrap();
    let log = std::fs::read_to_string(r.dir.join(crate::gate::DIRECT_LOG_FILE)).unwrap();
    let mine: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("\"kind\":\"resend\""))
        .collect();
    assert_eq!(mine.len(), 2, "{log}");
    let old = format!("\"old_id\":{id}");
    assert!(mine.iter().all(|l| l.contains(&old)), "{log}");
    assert!(mine[0].contains("\"result\":\"submitted\""));
    assert!(mine[1].contains("\"result\":\"succeeded\""));
}
