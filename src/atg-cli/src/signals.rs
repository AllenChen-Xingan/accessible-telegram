//! Ctrl+C / SIGTERM → 退出标志（`serve`、`watch`、`play`、登录这组命令共用）。第一次只立标志、正常收尾；第二次立刻以 130 退出、不出结尾行。
//!
//! 读秘密时终端被切成不回显（`tty.rs`）：第二次信号 `_exit` 会跳过恢复，所以切之前先把终端 fd 和原设置登记到这里
//! （[`arm_tty`]，放回时 [`disarm_tty`] 撤销登记），处理函数 `_exit` 之前先放回去（#98）。
//!
//! 信号可能落在任何一个线程（TDLib、收线程……）。「登记 + 切」与「放回 + 撤销」各是一段，攥着 [`TTY_LOCK`] 做、
//! 且做的时候本线程挡住 SIGINT / SIGTERM；第二次信号的处理函数也先抢这把锁再放回终端。这样别的线程上的处理函数
//! 要么等这一段做完（切好了再放回、不会被随后的「切」盖掉），要么先做完（这一段抢不到锁，进程随即退出）；
//! 本线程上的处理函数不会撞上自己攥着的锁（那时信号挡着）；处理函数之间也互不嵌套（[`install`]）。锁里只做不会阻塞的事
//! （`TCSANOW`，不等输出排空），否则终端输出被暂停时锁一直放不开，第二次信号也退不出去（codex 045 第 2 轮 #2）。处理函数抢到锁就不再放：马上 `_exit`（codex 045 第 1 轮 #2）

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

/// 登记、切换、放回、撤销与处理函数的放回互斥（见模块头）
static TTY_LOCK: AtomicBool = AtomicBool::new(false);

fn spin_lock() {
    // 只用原子量与 spin_loop：处理函数里也调，要异步信号安全
    while TTY_LOCK
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        std::hint::spin_loop();
    }
}

/// 本线程挡住 SIGINT / SIGTERM、攥着 [`TTY_LOCK`] 做 `f`，做完放锁、恢复信号屏蔽（挡着时来的信号随即送达）
fn locked<R>(f: impl FnOnce() -> R) -> R {
    // SAFETY: sigset_t 是纯数据，全零后由 sigemptyset 设好；pthread_sigmask 只改本线程
    let old = unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        let mut old: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut old);
        old
    };
    spin_lock();
    let r = f();
    TTY_LOCK.store(false, Ordering::SeqCst);
    // SAFETY: 放回进来时的屏蔽
    unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut()) };
    r
}

extern "C" fn on_signal(_: libc::c_int) {
    // 每一步都是异步信号安全的（原子量、tcsetattr、_exit）
    if STOP.swap(true, Ordering::SeqCst) {
        // 等正在做的「登记 + 切 / 放回 + 撤销」做完再看；抢到就不放了，下面直接退出
        spin_lock();
        let fd = TTY_FD.load(Ordering::SeqCst);
        if fd >= 0 {
            // 用 TCSANOW 立刻生效：马上就 `_exit` 了，不等输出排空（正常的切换与恢复也用 TCSANOW、另用 `tcflush` 丢输入，见 `tty.rs`）
            // SAFETY: fd 已发布，SAVED 在发布之前写好、之后不再写
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, (*SAVED.0.get()).as_ptr()) };
        }
        // SAFETY: _exit 是异步信号安全的
        unsafe { libc::_exit(130) }
    }
}

/// 用 `sigaction` 装，带 `SA_RESTART`（与 macOS 上 `signal` 的 BSD 语义相同）：阻塞中的 `read` 等被信号打断后由系统
/// 重启、接着等，所以读终端那边靠 `select` 切片看 [`STOP`]（`tty.rs`），不靠 `read` 被打断。
/// 处理期间 SIGINT、SIGTERM **两个都挡**：第二次信号的处理函数攥着 [`TTY_LOCK`] 时，另一个信号若落到同一线程、
/// 嵌套进来，会自旋等外层永远不放的锁——死锁、退不出去（codex 045 第 2 轮 #1）
pub fn install() {
    // SAFETY: sigaction 结构体全零后填好处理函数、屏蔽集与标志；处理函数只用异步信号安全的操作
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaddset(&mut sa.sa_mask, libc::SIGINT);
        libc::sigaddset(&mut sa.sa_mask, libc::SIGTERM);
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
    }
}

/// 登记恢复信息并切终端（`switch` 真切，成功回真），一段做完：第二次信号退出前把 `fd` 放回 `orig`。
/// 已登记着就不再登记（同一时刻只读一处输入）；`switch` 失败就撤销登记
pub fn arm_tty(fd: i32, orig: &libc::termios, switch: impl FnOnce() -> bool) -> bool {
    locked(|| {
        let fresh = fd >= 0 && TTY_FD.load(Ordering::SeqCst) < 0;
        if fresh {
            // SAFETY: 攥着锁且没发布（TTY_FD == -1）：处理函数此刻不读 SAVED
            unsafe { (*SAVED.0.get()).write(*orig) };
            TTY_FD.store(fd, Ordering::SeqCst);
        }
        let ok = switch();
        if !ok && fresh {
            TTY_FD.store(-1, Ordering::SeqCst);
        }
        ok
    })
}

/// 放回终端（`restore`）并撤销登记，一段做完
pub fn disarm_tty(restore: impl FnOnce()) {
    locked(|| {
        restore();
        TTY_FD.store(-1, Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 登记 / 撤销只改状态位；真发信号会让测试进程 `_exit`，不测
    #[test]
    fn arm_disarm_and_lock() {
        // SAFETY: termios 全零是合法值
        let t: libc::termios = unsafe { std::mem::zeroed() };
        assert!(arm_tty(7, &t, || true));
        assert_eq!(TTY_FD.load(Ordering::SeqCst), 7);
        assert!(arm_tty(8, &t, || true));
        assert_eq!(TTY_FD.load(Ordering::SeqCst), 7, "武装着不换");
        disarm_tty(|| {});
        assert_eq!(TTY_FD.load(Ordering::SeqCst), -1);
        assert!(!arm_tty(9, &t, || false), "切不成就撤销登记");
        assert_eq!(TTY_FD.load(Ordering::SeqCst), -1);

        // 锁被占着（处理函数正在放回）时，登记 + 切这一段等它放开才做，不会插进中间（codex 045 第 1 轮 #2）。
        // 和上面放在同一条测试里：几条测试并行跑会抢同一把静态锁
        use std::time::{Duration, Instant};
        spin_lock();
        let t0 = Instant::now();
        let h = std::thread::spawn(move || {
            // SAFETY: termios 全零是合法值
            let t: libc::termios = unsafe { std::mem::zeroed() };
            arm_tty(-1, &t, || true);
            t0.elapsed()
        });
        std::thread::sleep(Duration::from_millis(200));
        TTY_LOCK.store(false, Ordering::SeqCst);
        let waited = h.join().unwrap_or_default();
        assert!(waited >= Duration::from_millis(150), "{waited:?}");
    }
}
