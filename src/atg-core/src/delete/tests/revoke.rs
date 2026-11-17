//! 假后端走「给所有人删除」的全流程（#8，M2.7；数据全是编的，名字取自 fixtures/names.txt）
use super::*;
use crate::fake::Cap;

fn mid(chat: i64, id: i64) -> MsgId {
    MsgId {
        chat_id: chat,
        message_id: id,
    }
}

fn refused(why: DeleteRefusedWhy) -> CoreError {
    CoreError::DeleteRefused { why }
}

#[test]
fn own_sent_message_is_deleted_for_everyone_directly() {
    let b = rig(false);
    let plan = block_on(prepare_delete(&b, FRIEND, &[5, 5], SendOrigin::Cli)).unwrap();
    assert_eq!(plan.action(), Action::Delete);
    assert_eq!(plan.mode(), ConfirmMode::DirectMode);
    let j = plan.json();
    assert_eq!(j["action"], "delete");
    assert_eq!(
        j["items"],
        json!([{"message_id": 5, "text": "atg 测试 已发", "hidden_chars": 0}])
    );
    assert_eq!(j["note_zh"], DELETE_NOTE_ZH);
    let token = b.gate().issue(&plan, false).unwrap();
    assert_eq!(
        block_on(delete(&b, &plan, token)).unwrap(),
        ConfirmMode::DirectMode
    );
    // 重复的 id 只删一次；`revoke=true` = 给所有人
    assert_eq!(
        deletes(&b),
        vec![Call::Delete {
            chat: FRIEND,
            ids: vec![5],
            revoke: true
        }]
    );
}

#[test]
fn saved_messages_are_refused_on_both_servers() {
    for test_dc in [false, true] {
        let b = rig(test_dc);
        let e = block_on(prepare_delete(&b, ME, &[5], SendOrigin::Cli)).unwrap_err();
        assert_eq!(e, refused(DeleteRefusedWhy::Saved));
        assert_eq!(e.code(), "forbidden");
        assert!(deletes(&b).is_empty());
    }
}

#[test]
fn someone_elses_message_is_refused() {
    let b = rig(false);
    let mut m = own(FRIEND, 6, "atg 测试 对方发的", Value::Null);
    m["is_outgoing"] = json!(false);
    m["sender_id"] = json!({"@type": "messageSenderUser", "user_id": FRIEND});
    b.set_history(
        FRIEND,
        vec![own(FRIEND, 5, "atg 测试 已发", Value::Null), m],
    );
    let e = block_on(prepare_delete(&b, FRIEND, &[5, 6], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e, refused(DeleteRefusedWhy::NotOwn));
}

#[test]
fn what_tdlib_will_not_revoke_is_refused() {
    let b = rig(false);
    // 还在待发的：TDLib 不许给所有人删（该用 `pending cancel`）
    let e = block_on(prepare_delete(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e, refused(DeleteRefusedWhy::NotRevocable));
    // 过了时限等：TDLib 说不行就不行，本项目不自己算
    b.deny(mid(FRIEND, 5), Cap::DeleteForAll);
    let e = block_on(prepare_delete(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e, refused(DeleteRefusedWhy::NotRevocable));
    let e = block_on(prepare_delete(&b, FRIEND, &[99], SendOrigin::Cli)).unwrap_err();
    assert_eq!(e.code(), "not_found");
    assert!(deletes(&b).is_empty());
}

#[test]
fn non_text_message_can_be_deleted_without_text() {
    let b = rig(false);
    let mut m = own(FRIEND, 7, "", Value::Null);
    m["content"] = json!({"@type": "messagePhoto", "caption": {"@type": "formattedText", "text": "atg 测试 图", "entities": []},
        "photo": {"@type": "photo", "has_stickers": false, "sizes": []}});
    b.set_history(FRIEND, vec![m]);
    let plan = block_on(prepare_delete(&b, FRIEND, &[7], SendOrigin::Cli)).unwrap();
    assert_eq!(plan.items()[0].text, None);
    let token = b.gate().issue(&plan, false).unwrap();
    block_on(delete(&b, &plan, token)).unwrap();
    assert_eq!(deletes(&b).len(), 1);
}

#[test]
fn edited_after_prepare_is_refused_before_submit() {
    let b = rig(false);
    let plan = block_on(prepare_delete(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.set_history(FRIEND, vec![own(FRIEND, 5, "atg 测试 改过", Value::Null)]);
    let e = block_on(delete(&b, &plan, token)).unwrap_err();
    assert_eq!(
        e,
        CoreError::InvalidSend {
            why: InvalidSendWhy::Changed
        }
    );
    assert!(deletes(&b).is_empty());
}

#[test]
fn no_longer_revocable_at_submit_is_refused() {
    let b = rig(false);
    let plan = block_on(prepare_delete(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.deny(mid(FRIEND, 5), Cap::DeleteForAll);
    let e = block_on(delete(&b, &plan, token)).unwrap_err();
    assert_eq!(e, refused(DeleteRefusedWhy::NotRevocable));
    assert!(deletes(&b).is_empty());
}

#[test]
fn plans_of_the_other_kind_are_not_interchangeable() {
    let b = rig(false);
    let c = block_on(prepare_cancel(&b, FRIEND, &[1_048_577], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&c, false).unwrap();
    let e = block_on(delete(&b, &c, token)).unwrap_err();
    assert_eq!(e.code(), "internal");
    // 删除的凭据拿去撤销：同样拒，闸没被耗掉也什么都没删
    let d = block_on(prepare_delete(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&d, false).unwrap();
    assert_eq!(
        block_on(cancel(&b, &d, token)).unwrap_err().code(),
        "internal"
    );
    assert!(deletes(&b).is_empty());
}

#[test]
fn disconnect_while_submitting_is_outcome_unknown() {
    let b = rig(false);
    let plan = block_on(prepare_delete(&b, FRIEND, &[5], SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    b.fail_next("delete", CoreError::Disconnected);
    let e = block_on(delete(&b, &plan, token)).unwrap_err();
    assert_eq!(e.code(), "send_outcome_unknown");
}
