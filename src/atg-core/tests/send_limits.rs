//! 发送限制（`atg_core::send_limits`）走完整发送流程的回归测试：codex 016 第 5 轮 #1。
//! 假后端、测试服务器、发自己的收藏夹；数据全是编的，不碰真实账号与本机档案目录。

use atg_core::backend::Backend;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::{CoreError, Holder, SendRefusedReason};
use atg_core::fake::{FakeAuth, FakeBackend, FakeSend};
use atg_core::gate::{Gate, NoAuth};
use atg_core::link::block_on;
use atg_core::send::{Draft, SendOrigin, prepare_text, send_and_wait, send_text};
use atg_core::status::AuthStep;
use serde_json::json;
use std::time::Duration;

/// 自己（测试号）的 id，收藏夹就是与自己的私聊
const ME: i64 = 4242;

fn rig(tag: &str) -> (FakeBackend, atg_core::link::EventStream) {
    let b = FakeBackend::with_gate(Gate::new(false, Holder::Cli, Box::new(NoAuth))).unwrap();
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
    let root = std::env::temp_dir().join(format!("atg-limits-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let profile = Profile::create(&root, "p", ServerKind::Test).unwrap();
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
