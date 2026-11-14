//! 发文字的测试（请求形状 + 假后端全流程）。夹具在 `../tests.rs`。

use super::*;

#[test]
fn request_keeps_draft_and_carries_sending_id() {
    let r = text_request(
        777,
        "好的",
        Some(5),
        Some(&Topic::Forum {
            topic_id: 3,
            name: None,
        }),
        42,
    );
    assert_eq!(r["input_message_content"]["clear_draft"], json!(false));
    assert_eq!(r["options"]["sending_id"], json!(42));
    assert_eq!(r["options"]["only_preview"], json!(false));
    assert_eq!(r["reply_to"]["message_id"], json!(5));
    assert_eq!(r["topic_id"]["forum_topic_id"], json!(3));
    // 文本不做 Unicode 规范化：组合字符原样
    let r = text_request(1, "e\u{301}", None, None, 1);
    assert_eq!(
        r["input_message_content"]["text"]["text"],
        json!("e\u{301}")
    );
}

#[test]
fn only_forum_and_thread_topics_are_sent() {
    assert!(send_topic(Some(&Topic::SavedMessages { topic_id: 1 })).is_none());
    assert!(send_topic(Some(&Topic::Thread { thread_id: 1 })).is_some());
}

#[test]
fn test_dc_saved_sends_without_asking_and_keeps_draft() {
    let mut r = rig("saved", false, true, Err(SendRefusedReason::AuthFailed));
    let w = send_once(&mut r, ME, &draft("测试", None), false).unwrap();
    assert!(matches!(w, Waited::Succeeded(ref m) if m.id.chat_id == ME));
    assert_eq!(r.asks.load(Ordering::SeqCst), 0);
    // 交给「TDLib」的就是摘要算过的那份：不清草稿、带 sending_id、不是预览
    let sent = r.b.submitted();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0]["input_message_content"]["clear_draft"],
        json!(false)
    );
    assert_ne!(sent[0]["options"]["sending_id"], json!(0));
    assert_eq!(sent[0]["options"]["only_preview"], json!(false));
}

#[test]
fn test_dc_other_chat_is_refused_before_anything_is_sent() {
    let mut r = rig("other", true, true, Ok(()));
    let e = send_once(&mut r, FRIEND, &draft("测试", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::TestDcSavedOnly);
    assert!(r.b.submitted().is_empty());
}

#[test]
fn main_server_asks_every_time_and_refusal_sends_nothing() {
    let mut r = rig("main", false, false, Ok(()));
    send_once(&mut r, ME, &draft("测试", None), false).unwrap();
    send_once(&mut r, ME, &draft("测试", None), false).unwrap();
    assert_eq!(r.asks.load(Ordering::SeqCst), 2);
    let mut r = rig("main-no", false, false, Err(SendRefusedReason::AuthFailed));
    let e = send_once(&mut r, ME, &draft("测试", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::AuthFailed);
    assert!(r.b.submitted().is_empty());
}

#[test]
fn direct_mode_blocks_second_identical_submit() {
    let mut r = rig("dup", true, false, Err(SendRefusedReason::AuthFailed));
    send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap();
    let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
    assert_eq!(r.b.submitted().len(), 1);
    // --again 放行；改一个字也不算重复
    send_once(&mut r, FRIEND, &draft("好的", None), true).unwrap();
    send_once(&mut r, FRIEND, &draft("好的！", None), false).unwrap();
    assert_eq!(r.b.submitted().len(), 3);
    assert_eq!(r.asks.load(Ordering::SeqCst), 0);
}

#[test]
fn direct_mode_blocks_rerun_after_unknown_outcome() {
    let mut r = rig("unknown", true, false, Ok(()));
    r.b.script_sends(vec![FakeSend::Silent]);
    let w = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap();
    assert_eq!(w, Waited::Unknown(UnknownWhy::Timeout));
    let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
}

#[test]
fn pending_deleted_is_unknown_not_failure() {
    let mut r = rig("deleted", false, true, Ok(()));
    r.b.script_sends(vec![FakeSend::Delete]);
    let w = send_once(&mut r, ME, &draft("测试", None), false).unwrap();
    assert_eq!(w, Waited::Unknown(UnknownWhy::PendingDeleted));
}

#[test]
fn failure_reports_reason_and_can_be_resent() {
    let mut r = rig("fail", false, true, Ok(()));
    r.b.script_sends(vec![FakeSend::Fail {
        code: 400,
        message: "MESSAGE_TOO_LONG".into(),
        can_retry: true,
    }]);
    let Waited::Failed {
        message,
        error,
        can_retry,
    } = send_once(&mut r, ME, &draft("测试", None), false).unwrap()
    else {
        panic!("应当失败");
    };
    assert!(can_retry);
    assert_eq!(error.code(), "invalid_input");
    // 重发：查失败态 → 一张凭据 → 新临时 id → 成功
    let plan = block_on(prepare_resend(
        &r.b,
        ME,
        &[message.id.message_id],
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = r.b.gate().issue(&plan, true).unwrap();
    let news = block_on(resend(&r.b, &plan, token)).unwrap();
    let subs: Vec<Submitted> = news.into_iter().flatten().collect();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].mode(), ConfirmMode::TestExempt);
    let temps: Vec<MsgId> = subs.iter().map(Submitted::temp).collect();
    let w = wait(&mut r.stream, &temps, Duration::from_secs(2), &mut |_| {});
    assert!(matches!(w[0], Waited::Succeeded(_)));
    for (sub, w) in subs.into_iter().zip(&w) {
        finish(&r.b, &plan, sub, w);
    }
    // 不是失败的那条不许重发
    let e = block_on(prepare_resend(&r.b, ME, &[5], SendOrigin::Cli)).unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::NotFailed
        }
    );
}

#[test]
fn reply_dropped_by_preview_is_refused_at_prepare_and_at_send() {
    // 准备时预览就去掉了回复
    let mut r = rig("drop1", false, true, Ok(()));
    r.b.script_previews(&[true]);
    let e = block_on(prepare_text(
        &r.b,
        ME,
        &draft("回你", Some(5)),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::ReplyDropped);
    // 准备时好好的，交出前那次预览去掉了（被回复那条在确认期间被删）：拒，什么都没交出去
    r.b.script_previews(&[false, true]);
    let plan = block_on(prepare_text(
        &r.b,
        ME,
        &draft("回你", Some(5)),
        SendOrigin::Cli,
    ))
    .unwrap();
    let token = r.b.gate().issue(&plan, false).unwrap();
    let e = block_on(send_text(&r.b, &plan, token)).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::ReplyDropped);
    assert!(r.b.submitted().is_empty());
    // 照常时回复保住了
    let w = send_once(&mut r, ME, &draft("回你", Some(5)), false).unwrap();
    let Waited::Succeeded(m) = w else { panic!() };
    assert!(check_kept(&plan, &m).is_ok());
}

#[test]
fn forum_needs_reply_and_takes_topic_from_replied_message() {
    let mut r = rig("forum", true, false, Ok(()));
    let e = block_on(prepare_text(
        &r.b,
        FORUM,
        &draft("大家好", None),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ForumNeedsReply
        }
    );
    let plan = block_on(prepare_text(
        &r.b,
        FORUM,
        &draft("收到", Some(9)),
        SendOrigin::Cli,
    ))
    .unwrap();
    assert!(matches!(
        plan.topic(),
        Some(Topic::Forum { topic_id: 3, .. })
    ));
    assert!(plan.target().is_forum);
    let _ = send_once(&mut r, FORUM, &draft("收到", Some(9)), false).unwrap();
    assert_eq!(r.b.submitted()[0]["topic_id"]["forum_topic_id"], json!(3));
}

#[test]
fn reply_not_allowed_and_empty_text_are_invalid() {
    let r = rig("noreply", false, true, Ok(()));
    r.b.set_no_reply(MsgId {
        chat_id: ME,
        message_id: 5,
    });
    let e = block_on(prepare_text(
        &r.b,
        ME,
        &draft("回你", Some(5)),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::ReplyNotAllowed
        }
    );
    let e = block_on(prepare_text(
        &r.b,
        ME,
        &draft("  \n", None),
        SendOrigin::Cli,
    ))
    .unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::EmptyText
        }
    );
}

#[test]
fn rejected_submit_is_recorded_as_failed_so_retry_is_not_blocked() {
    let mut r = rig("reject", true, false, Ok(()));
    r.b.script_sends(vec![FakeSend::Reject(CoreError::Forbidden { td: None })]);
    let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
    assert_eq!(e.code(), "forbidden");
    assert!(send_once(&mut r, FRIEND, &draft("好的", None), false).is_ok());
}

#[test]
fn error_after_handing_over_is_unknown_and_still_blocks_rerun() {
    // 交出途中断了（应答没等到）：不知道发没发出——按结果未知报，防重复照样拦重跑
    let mut r = rig("lost", true, false, Ok(()));
    r.b.script_sends(vec![FakeSend::Reject(CoreError::Disconnected)]);
    let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
    assert!(matches!(
        e,
        CoreError::SendOutcomeUnknown {
            why: UnknownWhy::Disconnected,
            ..
        }
    ));
    let e = send_once(&mut r, FRIEND, &draft("好的", None), false).unwrap_err();
    assert_eq!(refused_reason(e), SendRefusedReason::DuplicateRecent);
}
