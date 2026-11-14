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
