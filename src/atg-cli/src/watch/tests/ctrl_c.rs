//! `watch` 的 Ctrl+C 约定：按了就照常出 `result`、退 0
use super::*;

/// 开订阅前的请求没成（这里：会话找不到）：等的时候已按了 Ctrl+C → 照约定出 `result`、退 0；没按 → 照实报错
#[test]
fn failure_before_subscribing_after_ctrl_c_still_ends_with_result() {
    let q = || Query {
        chat: Some(ChatRef::Id(424_242)),
    };
    let (code, ls) = run_watch(&backend(), q(), false, |v| v["kind"] == "status");
    assert_eq!(code, 0);
    assert_eq!(ls.last().map(|l| &l["kind"]), Some(&json!("result")));
    let (code, ls) = run_watch(&backend(), q(), false, |_| false);
    assert_ne!(code, 0);
    assert_eq!(ls.last().map(|l| &l["kind"]), Some(&json!("error")));
}

/// 起好后端、授权线还没定时已按了 Ctrl+C（这里：没登录，`ensure_ready` 立刻报 `auth_required`）→ `result`、退 0；
/// 没按 → 照实报错（#95）
#[test]
fn ctrl_c_before_status_still_ends_with_result() {
    let not_logged_in = || {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth::default());
        b
    };
    let (code, ls) = run_watch_from(
        &not_logged_in(),
        Query { chat: None },
        false,
        |_| false,
        true,
    );
    assert_eq!(code, 0);
    assert_eq!(ls.len(), 1, "{ls:?}");
    assert_eq!(ls[0]["kind"], "result");
    let (code, ls) = run_watch(&not_logged_in(), Query { chat: None }, false, |_| false);
    assert_ne!(code, 0);
    assert_eq!(ls.last().unwrap()["data"]["code"], "auth_required");
}
