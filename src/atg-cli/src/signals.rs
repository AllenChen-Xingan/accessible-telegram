//! Ctrl+C / SIGTERM → 退出标志（`serve`、`watch` 共用）。第一次只立标志、正常收尾；第二次立刻以 130 退出、不出结尾行。

use std::sync::atomic::{AtomicBool, Ordering};

pub static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    // 两步都是异步信号安全的
    if STOP.swap(true, Ordering::SeqCst) {
        // SAFETY: _exit 是异步信号安全的
        unsafe { libc::_exit(130) }
    }
}

pub fn install() {
    let h = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: 处理函数只写一个原子量，是异步信号安全的
    unsafe {
        libc::signal(libc::SIGINT, h);
        libc::signal(libc::SIGTERM, h);
    }
}
