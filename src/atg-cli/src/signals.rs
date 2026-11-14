//! Ctrl+C / SIGTERM → 退出标志（`serve`、`watch`、`play`、登录这组命令共用）。第一次只立标志、正常收尾；第二次立刻以 130 退出、不出结尾行。
//!
//! 读秘密时终端被切成不回显（`tty.rs`）：第二次信号 `_exit` 会跳过恢复，所以读之前先把原设置交到这里
//! （[`arm_tty`]），处理函数 `_exit` 之前先放回去（#98）。

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

pub static STOP: AtomicBool = AtomicBool::new(false);

/// 正切着的终端 fd；`-1` = 没有
static TTY_FD: AtomicI32 = AtomicI32::new(-1);

/// 那个终端原来的设置。只在 `TTY_FD == -1` 时写、写完才发布 fd；处理函数只在 fd 已发布时读
struct Saved(UnsafeCell<MaybeUninit<libc::termios>>);

// SAFETY: 写只发生在没发布时（处理函数不会读），读只发生在发布之后（不会再写），见 [`arm_tty`]
unsafe impl Sync for Saved {}

static SAVED: Saved = Saved(UnsafeCell::new(MaybeUninit::uninit()));

extern "C" fn on_signal(_: libc::c_int) {
    // 每一步都是异步信号安全的（原子量、tcsetattr、_exit）
    if STOP.swap(true, Ordering::SeqCst) {
        let fd = TTY_FD.load(Ordering::SeqCst);
        if fd >= 0 {
            // SAFETY: fd 已发布，SAVED 在发布之前写好、之后不再写
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, (*SAVED.0.get()).as_ptr()) };
        }
        // SAFETY: _exit 是异步信号安全的
        unsafe { libc::_exit(130) }
    }
}

pub fn install() {
    let h = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: 处理函数只用异步信号安全的操作
    unsafe {
        libc::signal(libc::SIGINT, h);
        libc::signal(libc::SIGTERM, h);
    }
}

/// 终端切成不回显之后调：第二次信号退出前把它放回 `orig`。已经武装着就不动（同一时刻只读一处输入）
pub fn arm_tty(fd: i32, orig: &libc::termios) {
    if fd < 0 || TTY_FD.load(Ordering::SeqCst) >= 0 {
        return;
    }
    // SAFETY: 还没发布（TTY_FD == -1），处理函数不读 SAVED；读输入只在主线程，没有并发的写
    unsafe { (*SAVED.0.get()).write(*orig) };
    TTY_FD.store(fd, Ordering::SeqCst);
}

/// 终端已放回：解除
pub fn disarm_tty() {
    TTY_FD.store(-1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 武装 / 解除只改状态位；真发信号会让测试进程 `_exit`，不测
    #[test]
    fn arm_and_disarm() {
        // SAFETY: termios 全零是合法值
        let t: libc::termios = unsafe { std::mem::zeroed() };
        arm_tty(7, &t);
        assert_eq!(TTY_FD.load(Ordering::SeqCst), 7);
        arm_tty(8, &t);
        assert_eq!(TTY_FD.load(Ordering::SeqCst), 7, "武装着不换");
        disarm_tty();
        assert_eq!(TTY_FD.load(Ordering::SeqCst), -1);
    }
}
