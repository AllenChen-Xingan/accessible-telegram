//! `block_on_until`：完成、超时、叫停三种下落

use super::{Until, block_on_timeout, block_on_until};
use std::future::{pending, ready};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[test]
fn ready_future_is_done_even_past_deadline() {
    let stop = AtomicBool::new(false);
    let past = Instant::now();
    assert_eq!(block_on_until(ready(7), past, &stop), Until::Done(7));
}

/// 已经叫停：连第一次轮询都不做（请求在第一次轮询时才交出去，叫停之后不发新请求）
#[test]
fn stopped_before_start_never_polls() {
    let stop = AtomicBool::new(true);
    let polled = std::cell::Cell::new(false);
    let fut = std::future::poll_fn(|_| {
        polled.set(true);
        std::task::Poll::Ready(())
    });
    let r = block_on_until(fut, Instant::now() + Duration::from_secs(5), &stop);
    assert_eq!(r, Until::Stopped);
    assert!(!polled.get());
}

#[test]
fn pending_future_times_out_near_deadline() {
    let stop = AtomicBool::new(false);
    let t0 = Instant::now();
    let r = block_on_until(pending::<()>(), t0 + Duration::from_millis(250), &stop);
    assert_eq!(r, Until::TimedOut);
    let took = t0.elapsed();
    assert!(took >= Duration::from_millis(250), "{took:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// 停止标志由别的线程立起（像信号处理函数那样，不唤醒等的线程）：一片之内就停
#[test]
fn stop_flag_from_another_thread_ends_wait_within_a_slice() {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        s.store(true, Ordering::SeqCst);
    });
    let t0 = Instant::now();
    let r = block_on_until(pending::<()>(), t0 + Duration::from_secs(30), &stop);
    setter.join().unwrap();
    assert_eq!(r, Until::Stopped);
    assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
}

#[test]
fn timeout_wrapper_keeps_old_contract() {
    assert_eq!(block_on_timeout(ready(1), Duration::ZERO), Some(1));
    assert_eq!(
        block_on_timeout(pending::<()>(), Duration::from_millis(50)),
        None
    );
}
