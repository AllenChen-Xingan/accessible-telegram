//! 极小的执行器：在当前线程上等一个 future。
//! 被禁的 [`block_on`]（没有期限）和它的替代品 [`block_on_until`] / [`block_on_timeout`] 放在一处（#78）。
//! 期限取多少是产品取舍，不放这里（`atg-cli/src/deadlines.rs`）；这里只收截止时刻

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

/// 每片最多睡这么久再看一眼停止标志：信号处理函数只立标志、叫不醒 park，只能切片。
/// 100 毫秒：按 Ctrl+C 后人感觉不到延迟，空转开销可忽略
const STOP_POLL: Duration = Duration::from_millis(100);

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// 在当前线程上跑完一个 future，**没有期限**。只给测试和有意一直等的地方用（标 `allow` 写理由）；
/// 产品代码一律用 [`block_on_until`]（根目录 `clippy.toml` 的 `disallowed-methods` 拦着，#78）
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        thread::park();
    }
}

/// 有期限地等的结果
#[derive(Debug, PartialEq, Eq)]
pub enum Until<T> {
    Done(T),
    /// 到了截止时刻还没完
    TimedOut,
    /// 停止标志立起来了（Ctrl+C / SIGTERM）
    Stopped,
}

/// 在当前线程上跑一个 future，最多等到 `deadline`，`stop` 立起就不再等。
/// 没完的 future 直接丢掉（迟到的应答由 link 那边丢弃）。每次轮询之前先看停止：立起了就不再轮询——
/// 请求在第一次轮询时才交出去，叫停之后不会再发新请求。轮询完才看期限：已经完成的不算超时
pub fn block_on_until<F: Future>(fut: F, deadline: Instant, stop: &AtomicBool) -> Until<F::Output> {
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if stop.load(Ordering::SeqCst) {
            return Until::Stopped;
        }
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return Until::Done(v);
        }
        let now = Instant::now();
        if now >= deadline {
            return Until::TimedOut;
        }
        thread::park_timeout((deadline - now).min(STOP_POLL));
    }
}

/// 只要期限、不管停止的薄包装（退出路径用，免得卡死）；超时返回 `None`
pub fn block_on_timeout<F: Future>(fut: F, timeout: Duration) -> Option<F::Output> {
    static NEVER: AtomicBool = AtomicBool::new(false);
    match block_on_until(fut, Instant::now() + timeout, &NEVER) {
        Until::Done(v) => Some(v),
        Until::TimedOut | Until::Stopped => None,
    }
}

#[cfg(test)]
mod tests;
