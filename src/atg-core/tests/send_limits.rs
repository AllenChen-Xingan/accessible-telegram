//! 发送限制（`atg_core::send_limits`）走完整发送流程的回归测试：codex 016 第 5 轮 #1。
//! 假后端、测试服务器、发自己的收藏夹；数据全是编的，不碰真实账号与本机档案目录。
#![allow(
    clippy::disallowed_methods,
    reason = "测试里等假后端，卡住就是测试失败"
)]

use atg_core::backend::Backend;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::{CoreError, Holder, SendRefusedReason};
use atg_core::fake::{FakeAuth, FakeBackend, FakeSend};
use atg_core::gate::Gate;
use atg_core::link::block_on;
use atg_core::send::{Draft, SendOrigin, prepare_text, send_and_wait, send_text};
use atg_core::status::AuthStep;
use serde_json::json;
use std::time::Duration;

/// 自己（测试号）的 id，收藏夹就是与自己的私聊
const ME: i64 = 4242;

fn root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("atg-limits-it-{tag}-{}", std::process::id()))
}

fn rig(tag: &str) -> (FakeBackend, atg_core::link::EventStream) {
    let root = root(tag);
    let _ = std::fs::remove_dir_all(&root);
    let profile = Profile::create(&root, "p", ServerKind::Test).unwrap();
    boot(profile)
}

/// 起一个假后端开在这份档案上（「下一个进程」也用它）
fn boot(profile: Profile) -> (FakeBackend, atg_core::link::EventStream) {
    let b = FakeBackend::with_gate(Gate::new(Holder::Cli)).unwrap();
    b.set_auth(FakeAuth {
        phone: "+9996612345".into(),
        logged_in: true,
        ..FakeAuth::default()
    });
    b.set_me(ME);
    b.set_chat(
        ME,
        vec![json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": ME, "title": "我自己",
            "type": {"@type": "chatTypePrivate", "user_id": ME}, "positions": [], "notification_settings": {}}})],
    );
    let stream = b.subscribe().unwrap();
    block_on(b.start(&profile)).unwrap();
    b.wait_auth(Duration::from_secs(5), &|s| s == AuthStep::Ready)
        .unwrap();
    (b, stream)
}

fn draft(text: &str) -> Draft {
    Draft {
        text: text.into(),
        reply_to: None,
    }
}

#[test]
fn rate_limit_from_the_submit_exit_is_registered() {
    // 交出口当场回了限流：经唯一的交出口登记，下一次准备就被拒，不再交出
    let (b, mut stream) = rig("exit");
    b.script_sends(vec![FakeSend::Reject(CoreError::RateLimited {
        retry_after_s: 120,
        td: None,
    })]);
    let plan = block_on(prepare_text(&b, ME, &draft("测试一"), SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    let e = send_and_wait(
        &b,
        &plan,
        token,
        &mut stream,
        Duration::from_secs(2),
        &mut |_| {},
    )
    .err()
    .unwrap();
    assert!(matches!(e, CoreError::RateLimited { .. }), "{}", e.code());
    let before = b.submitted().len();
    let again = block_on(prepare_text(&b, ME, &draft("测试二"), SendOrigin::Cli));
    assert!(matches!(again, Err(CoreError::RateLimited { .. })));
    assert_eq!(b.submitted().len(), before);
}

#[test]
fn limit_arriving_during_the_recheck_preview_is_caught_before_submit() {
    // 凭据已验过（verify 在复核预览之前）、复核预览还没回时才到的受限：只有交出前最后一次复查拦得住，
    // 什么都不交出（codex 016 第 6 轮 #3：原先在 send_text 之前就登记，入口的 verify 已会拒，测不到最后复查）
    let (b, _stream) = rig("late");
    let plan = block_on(prepare_text(&b, ME, &draft("测试三"), SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    let limits = b.gate().limits();
    b.on_next_preview(move || limits.note(&CoreError::AccountLimited { td: None }));
    let before = b.submitted().len();
    let e = block_on(send_text(&b, &plan, token)).err().unwrap();
    assert!(matches!(
        e,
        CoreError::SendRefused {
            reason: SendRefusedReason::AccountLimited,
            ..
        }
    ));
    // 真正的发送没有交出（`submitted` 只记真发，不记预览）
    assert_eq!(b.submitted().len(), before);
}

/// 模拟磁盘满：在 `write_atomic` 的临时文件位置放一个目录，写就失败，而目标文件本身不存在
fn jam(dir: &std::path::Path, name: &str) {
    let tmp = dir
        .join(name)
        .with_extension(format!("tmp{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("x")).unwrap();
}

/// 磁盘又有空间了（下一个进程开始前；否则它交出前写标记就失败，测不到它读回了什么）
fn unjam(dir: &std::path::Path, name: &str) {
    let tmp = dir
        .join(name)
        .with_extension(format!("tmp{}", std::process::id()));
    std::fs::remove_dir_all(tmp).unwrap();
}

#[test]
fn late_limit_with_a_full_disk_still_stops_the_next_process() {
    // #88：交出成功（进了待发队列）后，限流才以发送失败晚到；这时磁盘满，截止时间与补写的交出未完成标记都写不进。
    // 交出未完成标记不能在交出返回时就删——否则下一个进程什么都看不到，照常放行
    let (b, mut stream) = rig("late-full");
    b.script_sends(vec![FakeSend::Silent]);
    let plan = block_on(prepare_text(&b, ME, &draft("测试四"), SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    let sub = block_on(send_text(&b, &plan, token)).unwrap();
    let temp = sub.temp();
    let dir = Profile::open(&root("late-full"), "p")
        .unwrap()
        .dir()
        .to_path_buf();
    jam(&dir, atg_core::send_limits::FLOOD_FILE);
    jam(&dir, atg_core::send_limits::PENDING_FILE);
    let err = json!({"@type": "error", "code": 429, "message": "FLOOD_WAIT_60"});
    b.push(json!({"@type": "updateMessageSendFailed", "old_message_id": temp.message_id, "error": err,
        "message": {"@type": "message", "id": temp.message_id + 1, "chat_id": ME,
            "sender_id": {"@type": "messageSenderUser", "user_id": ME}, "is_outgoing": true, "date": 1_792_755_000,
            "sending_state": {"@type": "messageSendingStateFailed", "error": err, "can_retry": true,
                "need_another_sender": false, "need_another_reply_quote": false, "need_drop_reply": false,
                "required_paid_message_star_count": 0, "retry_after": 60.0},
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": "测试四", "entities": []}}}}));
    let w = atg_core::send::wait(&mut stream, &[temp], Duration::from_secs(2), &mut |_| {});
    assert!(matches!(w[0], atg_core::send::Waited::Failed { .. }));
    atg_core::send::finish(&b, &plan, sub, &w[0]);
    let _ = b.shutdown(Duration::from_secs(2));
    drop(stream);
    unjam(&dir, atg_core::send_limits::FLOOD_FILE);
    unjam(&dir, atg_core::send_limits::PENDING_FILE);
    // 下一个进程：同一份档案
    let (next, _s) = boot(Profile::open(&root("late-full"), "p").unwrap());
    let again = block_on(prepare_text(&next, ME, &draft("测试五"), SendOrigin::Cli));
    assert!(
        matches!(
            again,
            Err(CoreError::Internal {
                what: "send_limits_unreadable"
            })
        ),
        "限流没写进盘，下一个进程却放行了：{:?}",
        again.as_ref().err().map(|e| e.code())
    );
}

#[test]
fn accepted_send_left_open_at_a_clean_close_leaves_no_manual_work() {
    // #88 暂定（待人定，题面在 issue #88）：交出成功、结果没回来就正常关闭——标记删掉，下一个进程照常能发（失败由它登记）
    let (b, stream) = rig("open-close");
    b.script_sends(vec![FakeSend::Silent]);
    let plan = block_on(prepare_text(&b, ME, &draft("测试六"), SendOrigin::Cli)).unwrap();
    let token = b.gate().issue(&plan, false).unwrap();
    let sub = block_on(send_text(&b, &plan, token)).unwrap();
    let w = atg_core::send::Waited::Unknown(atg_core::error::UnknownWhy::Timeout);
    atg_core::send::finish(&b, &plan, sub, &w);
    b.shutdown(Duration::from_secs(2)).unwrap();
    drop(stream);
    let (next, _s) = boot(Profile::open(&root("open-close"), "p").unwrap());
    assert!(block_on(prepare_text(&next, ME, &draft("测试七"), SendOrigin::Cli)).is_ok());
}
