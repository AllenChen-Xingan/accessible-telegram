//! 还没挂订阅者时的结尾行：`error`，或已按了 Ctrl+C 时的 `result`
use super::{LineOut, Watcher};
use crate::args::Format;
use crate::out::Fail;
use std::sync::atomic::{AtomicBool, Ordering};

/// 出错时写一行 `error`，回退出码
pub(super) fn fail_line(mut out: Box<dyn LineOut>, format: Format, f: &Fail) -> u8 {
    let mut sink = LineSink(&mut *out);
    crate::out::fail_with_to(&mut sink, format, f)
}

/// 出 `status` 之前就失败了（如授权线还没定）：已按过 Ctrl+C 就照约定写一条 `result`、退 0（#95），否则照实报错
pub(super) fn before_status(
    mut out: Box<dyn LineOut>,
    format: Format,
    f: &Fail,
    stop: &AtomicBool,
    watcher: impl FnOnce() -> Watcher,
) -> u8 {
    if !stop.load(Ordering::SeqCst) {
        return fail_line(out, format, f);
    }
    let _ = out.line(&watcher().result_line());
    out.finish(0);
    0
}

/// 把 [`LineOut`] 当 [`crate::out::Sink`] 用（错误行走同一段渲染）
struct LineSink<'a>(&'a mut dyn LineOut);

impl crate::out::Sink for LineSink<'_> {
    fn out(&mut self, line: &str) {
        let _ = self.0.line(line);
    }
    fn err(&mut self, line: &str) {
        eprintln!("{line}");
    }
}
