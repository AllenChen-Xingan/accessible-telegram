//! Store 查询 `pending_sends`（`actions.md`「待发出的消息」，#57）：待发的列出来；发出、失败、被删的不列

use atg_core::model::*;

#[allow(dead_code)]
mod common;
use common::*;

fn pending(chat_id: i64, message_id: i64) -> Message {
    let mut m = msg(chat_id, message_id, text("atg 测试"));
    m.outgoing = true;
    m.sender = SenderId::User(ME);
    m.sending = SendingState::Pending;
    m
}

#[test]
fn lists_only_pending_sorted() {
    let s = feed([
        new(pending(GROUP, 1_000_003)),
        new(msg(CHAT, 5, text("收到的"))),
        new(pending(CHAT, 1_000_002)),
        new(pending(CHAT, 1_000_001)),
    ]);
    assert_eq!(
        s.pending_sends(),
        vec![
            id(GROUP, 1_000_003),
            id(CHAT, 1_000_001),
            id(CHAT, 1_000_002)
        ]
    );
}

#[test]
fn resolved_ones_drop_out() {
    let p1 = pending(CHAT, 1_000_001);
    let p2 = pending(CHAT, 1_000_002);
    let p3 = pending(CHAT, 1_000_003);
    let mut s = feed([new(p1.clone()), new(p2.clone()), new(p3.clone())]);
    let mut sent = p1.clone();
    sent.id = id(CHAT, 42);
    sent.sending = SendingState::Sent;
    push(
        &mut s,
        CoreEvent::SendSucceeded {
            old_id: p1.id,
            message: sent,
        },
    );
    let mut failed = p2.clone();
    failed.id = id(CHAT, 1_000_009);
    failed.sending = SendingState::Failed;
    push(
        &mut s,
        CoreEvent::SendFailed {
            old_id: p2.id,
            message: failed,
            error: atg_core::error::map_td_error(400, ""),
            can_retry: true,
        },
    );
    assert_eq!(s.pending_sends(), vec![p3.id]);
    // 待发那条被删（自己撤销）：不再列出，也不留撤回占位
    push(&mut s, deleted(CHAT, &[1_000_003], true, false));
    assert!(s.pending_sends().is_empty());
}

/// 待发的不参与删旧：非当前会话超过 `keep_per_chat`（默认 50）条待发也一条不丢（codex 029 #1）
#[test]
fn pending_survive_pruning() {
    let s = feed((1..=60).map(|i| new(pending(CHAT, 1_000_000 + i))));
    assert_eq!(s.pending_sends().len(), 60);
}

/// 待发的也不占配额：60 条待发 + 普通消息，普通的照样留到 `keep_per_chat`（codex 029 第 2 轮 #2）
#[test]
fn pending_do_not_eat_the_quota() {
    let s = feed(
        (1..=60)
            .map(|i| new(pending(CHAT, 1_000_000 + i)))
            .chain((1..=10).map(|i| new(msg(CHAT, i, text("普通"))))),
    );
    assert_eq!(s.pending_sends().len(), 60);
    for i in 1..=10 {
        assert!(
            s.message(id(CHAT, i)).is_some(),
            "第 {i} 条普通消息被挤掉了"
        );
    }
}
