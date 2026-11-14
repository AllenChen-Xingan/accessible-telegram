//! 执行方上走发送外壳的命令：L2 的 `send`、`resend`、`pending cancel`，和与它们同一外壳的 `pending` 列表（L0，只读）。
//! 解参数 → 执行方以直发启动时先出开场提示与 `status` 行（发起方要和直跑一样先看到）→ 与直跑同一段执行代码。
//! 从 `serve.rs` 挪出

use crate::args::Format;
use crate::chats::Feed;
use crate::out::{Fail, Sink, fail_with_to};
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use serde_json::Value;
use std::time::Instant;

/// 回（记日志用的命令名，退出码）；不认识的命令名不会到这里（调用方按名字分派）
#[allow(clippy::too_many_arguments)]
pub(super) fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    command: &str,
    params: Value,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> (&'static str, u8) {
    let bad = Fail::Core(CoreError::Internal {
        what: "ipc_bad_request",
    });
    let timeout = atg_core::send::RESULT_TIMEOUT;
    let direct = b.gate().direct();
    let banner = |sink: &mut dyn Sink| {
        if direct {
            crate::send::direct_banner(b, sink, format);
        }
    };
    match command {
        "send" => {
            let code = match serde_json::from_value::<crate::send::SendParams>(params) {
                Err(_) => fail_with_to(sink, format, &bad),
                Ok(p) => {
                    banner(sink);
                    crate::send::execute_send(b, stream, feed, &p, sink, format, started, timeout)
                }
            };
            ("send", code)
        }
        "resend" => {
            let code = match serde_json::from_value::<crate::send::ResendParams>(params) {
                Err(_) => fail_with_to(sink, format, &bad),
                Ok(p) => {
                    banner(sink);
                    crate::send::execute_resend(b, stream, feed, &p, sink, format, started, timeout)
                }
            };
            ("resend", code)
        }
        "pending_cancel" => {
            let code = match serde_json::from_value::<crate::pending::CancelParams>(params) {
                Err(_) => fail_with_to(sink, format, &bad),
                Ok(p) => {
                    banner(sink);
                    crate::pending::execute_cancel(b, stream, feed, &p, sink, format, started)
                }
            };
            ("pending_cancel", code)
        }
        // 列表只读：不出直发开场（直跑的 `pending` 也不出，`session::pending`）
        "pending" => {
            let code = match serde_json::from_value::<crate::pending::PendingParams>(params) {
                Err(_) => fail_with_to(sink, format, &bad),
                Ok(p) => crate::pending::execute_list(b, stream, feed, &p, sink, format, started),
            };
            ("pending", code)
        }
        _ => ("?", fail_with_to(sink, format, &bad)),
    }
}
