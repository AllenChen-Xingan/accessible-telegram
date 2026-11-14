//! 真实后端起停的状态机：没起 → 起步中 → 运行中（#62）。纯类型、不碰 tdlib-rs，不开特性也能单测。
//!
//! `start` 进来先在锁里占位（没起 → 起步中），放锁再做慢活（钥匙串可能弹窗，不能攥着锁挡住 `status`）；
//! 成功提交成运行中，中途失败由守卫退回没起，下次还能再起。

use std::sync::{Mutex, MutexGuard, PoisonError};

pub(super) enum Run<R> {
    Idle,
    Starting,
    Running(R),
}

impl<R> Run<R> {
    pub(super) fn running(&self) -> Option<&R> {
        match self {
            Run::Running(r) => Some(r),
            _ => None,
        }
    }

    pub(super) fn running_mut(&mut self) -> Option<&mut R> {
        match self {
            Run::Running(r) => Some(r),
            _ => None,
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 起步中的占位：[`Claim::commit`] 之前丢掉就退回没起
pub(super) struct Claim<'a, R> {
    run: &'a Mutex<Run<R>>,
    committed: bool,
}

impl<'a, R> Claim<'a, R> {
    /// 没起才占得到；起步中、运行中（含已关过的）→ `None`
    pub(super) fn take(run: &'a Mutex<Run<R>>) -> Option<Self> {
        let mut st = lock(run);
        if !matches!(*st, Run::Idle) {
            return None;
        }
        *st = Run::Starting;
        Some(Self {
            run,
            committed: false,
        })
    }

    pub(super) fn commit(mut self, r: R) {
        *lock(self.run) = Run::Running(r);
        self.committed = true;
    }
}

impl<R> Drop for Claim<'_, R> {
    fn drop(&mut self) {
        if !self.committed {
            *lock(self.run) = Run::Idle;
        }
    }
}

/// 关好之后放档案锁，放之前清运行标记。锁已经放过就什么都不做：
/// 那之后别的进程可能已起来、写了自己的运行标记，再清就抹掉了人家的（#62）
pub(super) fn release<L>(lock: &mut Option<L>, mark_clean: impl FnOnce()) {
    if lock.is_some() {
        mark_clean();
        *lock = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{Claim, Run, release};
    use std::sync::Mutex;

    #[test]
    fn a_second_start_is_refused_while_the_first_holds_the_claim() {
        let run = Mutex::new(Run::<u8>::Idle);
        let first = Claim::take(&run).expect("没起时占得到");
        assert!(Claim::take(&run).is_none(), "起步中再起");
        first.commit(7);
        assert!(Claim::take(&run).is_none(), "运行中再起");
        assert_eq!(run.lock().unwrap().running(), Some(&7));
    }

    #[test]
    fn a_failed_start_goes_back_to_idle_and_can_start_again() {
        let run = Mutex::new(Run::<u8>::Idle);
        drop(Claim::take(&run).unwrap());
        assert!(matches!(*run.lock().unwrap(), Run::Idle));
        Claim::take(&run).expect("失败后能再起").commit(1);
        assert!(run.lock().unwrap().running().is_some());
    }

    #[test]
    fn releasing_twice_clears_the_mark_once() {
        let mut held = Some(());
        let mut cleaned = 0;
        release(&mut held, || cleaned += 1);
        release(&mut held, || cleaned += 1);
        assert_eq!(cleaned, 1);
        assert!(held.is_none());
    }
}
