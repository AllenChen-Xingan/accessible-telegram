//! 会话租约：同一进程里「替谁开着会话」的引用计数（`docs/experiments/exp008-local-stt/design.md` 第二节第 3 条）。
//!
//! 超级群、频道只有打开的会话才推新消息，所以 `watch --chat`、将来的 `transcribe` 都要 `chat_open`。
//! 同一会话可能同时有几个使用者（几个 `watch` 订阅者、几项转写）：**第一份要的真开，最后一份放的真关**。
//! 计数 [`Leases`] 只数，不碰后端（同 [`crate::watch::Hub::reap`] 的形状：回要关的，调用方去关）；
//! 「第一份真开、开失败退掉」「最后一份真关、出错只记」写在旁边的 [`open`] / [`close`] 一处。
//! 后端故障时不关，由调用方决定（故障时不调 [`close`]）。
//!
//! 内部带锁、方法都吃 `&self`：执行方是单线程的，但转写这类长动作握着一份租约的同时，
//! `watch` 订阅者还要照常开关，所以持有者只拿共享引用（`&Leases` / `Arc<Leases>`），不独占整张表
//! （架构负责会话 arch 2026-11-14 批）。转写用的守卫（放手走 [`Leases::give`]）随转写命令那个 PR 加。
//! 表的实例暂时寄放在 [`crate::watch::Hub`] 上（执行方里只有一个），以后 atg-cli 有了 lib 目标，随它一起挪

use crate::deadlines::{QUERY_TIMEOUT, bounded};
use crate::out::Sink;
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::model::ChatId;
use atg_core::status::CoreState;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// 各会话现在有几份租约。没有一份的会话不在表里
#[derive(Debug, Default)]
pub struct Leases {
    held: Mutex<HashMap<ChatId, usize>>,
}

impl Leases {
    /// 要一份。回 `true` = 这是第一份，调用方去 `chat_open`；开失败调 [`Leases::undo`] 退掉
    pub fn take(&self, chat: ChatId) -> bool {
        let mut held = self.lock();
        let n = held.entry(chat).or_insert(0);
        *n += 1;
        *n == 1
    }

    /// 开失败时退掉刚要的那一份。不回「要关」：根本没开成
    pub fn undo(&self, chat: ChatId) {
        self.release(chat);
    }

    /// 放一份。回 `true` = 这是最后一份，调用方去 `chat_close`（后端故障时调用方不关）。
    /// 放一个没要过的：不回关（配对错了，宁可不关也不替别人关掉）；debug 构建（含测试）里 `debug_assert` 直接 panic
    pub fn give(&self, chat: ChatId) -> bool {
        self.release(chat)
    }

    /// 现在几份（只给测试看）
    #[cfg(test)]
    pub fn held(&self, chat: ChatId) -> usize {
        self.lock().get(&chat).copied().unwrap_or(0)
    }

    /// 减一份；减到零回 `true` 并移出表
    fn release(&self, chat: ChatId) -> bool {
        let mut held = self.lock();
        debug_assert!(held.contains_key(&chat), "放了一份没要过的会话租约");
        let Some(n) = held.get_mut(&chat) else {
            return false;
        };
        *n -= 1;
        if *n > 0 {
            return false;
        }
        held.remove(&chat);
        true
    }

    /// 锁坏了（别处持锁时 panic）照样用：表里只有计数，没有半改的不变量
    fn lock(&self) -> MutexGuard<'_, HashMap<ChatId, usize>> {
        self.held.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// 要一份租约，是第一份就真开；开失败退掉那份（`serve`、直跑的 `watch` 共用这一处）。
///
/// 「要」和「开」不是一步：前提是从 `take` 到 `chat_open` 应答之间没有别人在同一张表上要同一个会话
/// （否则后来的那个拿到「已有人开着」，而第一个开失败退掉后会话其实没开）。现在所有使用者都在执行方线程上
/// 一件一件跑，成立；以后有了跨线程的使用者，要先把「开着没开成」也记进表里再放开
pub fn open<B: Backend>(b: &B, leases: &Leases, chat: ChatId) -> Result<(), CoreError> {
    if leases.take(chat) {
        bounded(b, b.chat_open(chat), QUERY_TIMEOUT).inspect_err(|_| leases.undo(chat))?;
    }
    Ok(())
}

/// 关掉放完租约、已没人开着的会话（[`Leases::give`] 回 `true` 的那些）；出错只记一句码，`who` 是日志前缀。
/// 用 [`bounded`] 不用 `within`：收尾时 Ctrl+C 往往已经按了，关会话的请求照发，只受期限管。
/// 后端已故障就一个都不关（请求发不出去）：判在这一处，读方断开与故障同时到时调用方先 `reap` 再判故障也不会关
pub fn close<B: Backend>(b: &B, chats: Vec<ChatId>, who: &str, own: &mut dyn Sink) {
    if b.status().lines.core == CoreState::Faulted {
        return;
    }
    for c in chats {
        if let Err(e) = bounded(b, b.chat_close(c), QUERY_TIMEOUT) {
            own.err(&format!("{who}: 关闭会话时出错（{}）", e.code()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_take_opens_and_last_give_closes() {
        let l = Leases::default();
        assert!(l.take(1));
        assert!(!l.take(1));
        assert!(!l.take(1));
        assert_eq!(l.held(1), 3);
        assert!(!l.give(1));
        assert!(!l.give(1));
        assert!(l.give(1));
        assert_eq!(l.held(1), 0);
        // 关过之后再要，又是第一份
        assert!(l.take(1));
    }

    #[test]
    fn chats_are_counted_apart() {
        let l = Leases::default();
        assert!(l.take(1));
        assert!(l.take(2));
        assert!(l.give(1));
        assert_eq!(l.held(2), 1);
        assert!(l.give(2));
    }

    #[test]
    fn undo_after_failed_open_leaves_nothing_held() {
        let l = Leases::default();
        assert!(l.take(1));
        l.undo(1);
        assert_eq!(l.held(1), 0);
        // 退掉的那份不留痕：下一个要的仍是第一份、要去开
        assert!(l.take(1));
    }

    #[test]
    fn undo_with_others_holding_keeps_their_share() {
        let l = Leases::default();
        assert!(l.take(1));
        assert!(!l.take(1));
        l.undo(1);
        assert_eq!(l.held(1), 1);
        assert!(l.give(1));
    }

    /// 配对错了（放了没要过的）：debug 构建里 `debug_assert` 抓出来
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "没要过")]
    fn give_unheld_is_caught_in_debug() {
        Leases::default().give(7);
    }

    /// 发布构建里：不 panic、不回关、表里不留东西
    #[cfg(not(debug_assertions))]
    #[test]
    fn give_unheld_does_not_close() {
        let l = Leases::default();
        assert!(!l.give(7));
        assert_eq!(l.held(7), 0);
    }

    #[test]
    fn shared_across_threads() {
        let l = std::sync::Arc::new(Leases::default());
        let opened: usize = (0..8)
            .map(|_| {
                let l = l.clone();
                std::thread::spawn(move || usize::from(l.take(1)))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap_or(0))
            .sum();
        assert_eq!(opened, 1);
        assert_eq!(l.held(1), 8);
    }
}
