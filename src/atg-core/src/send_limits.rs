//! 发送的硬限制：账号受限、限流冷却（`specs/contracts/errors.md` 第一节第 5 小节「限流怎么处理」；`.42cog/real.md`
//! 「必须遵守 FLOOD_WAIT」）。
//!
//! 一份，闸与收线程共用（`Arc`）。登记只有两个入口，都当场落盘（[`SendLimits::note`]）：
//! - **交出口**：预览、发送、重发都只经 [`submit`] 调 `Backend::submit`，它回的错在这里登记（codex 016 第 5 轮 #1：
//!   预览回的受限 / 限流曾直接返回、没登记）
//! - **收线程**：所有事件的唯一入口 `link::Shared::event`，发送失败事件（含等结果超时之后才到的）在那里登记
//!
//! 查也只有两处：闸判开法时（`Gate::mode_for`），和交出前最后一次复查（`Gate::before_submit` 开头——所有 await
//! 之后、交出之前的唯一位置，复核预览期间才到的受限也拦得住）。读不准就拒发（失败即关，codex 016 第 5 轮 #2）。新进程开在同一档案上时
//! 由 [`SendLimits::load`] 读回，所以 CLI 每条命令一个新进程、AI 循环调用也绕不过（codex 016 第 3 轮 #2、第 4 轮
//! #1 #2：先是只记在内存、后是只在下一次判开法时才落盘，进程一退就忘，按同类问题改成这一处）。
//!
//! 落盘三个文件，都在档案目录，都不记内容：
//! - [`ACCOUNT_LIMITED_FILE`]：账号被 Telegram 限制。人处理好、删掉它才恢复
//! - [`FLOOD_FILE`]：限流到什么时候（Unix 秒）之前不许再发；到点自然失效
//! - [`PENDING_FILE`]：交出未完成。每次交出前先写，**等到能登记受限 / 限流的最后一刻**才删（#88）：交出的应答
//!   回来并登记完（调用方等不及丢掉等待时，交出线程自己还攥着一份守卫，[`SendLimits::hold`]），交出成功的那几条
//!   各自的成败也到了（[`SendLimits::await_outcome`] 登记、收线程 [`SendLimits::outcome`] 销账）。进程正常关闭时
//!   还没回来的不再等（[`SendLimits::closed`]）：TDLib 下次启动续发，失败由那个进程登记（这一条暂定、待人定，题面
//!   记在 issue #88）。「正常关闭」= 后端 `shutdown` 走完（命令跑完、Ctrl+C 走正常收尾、停 `serve`）；被杀、崩溃不算。
//!   上一个进程留下的标记本进程从不自动删。常见情况都自己清，不留人工活；只有两种情况留着：进程没正常关闭就死掉，
//!   或某个受限 / 限流没写进盘（写失败时会补写这个标记；磁盘满时补写也可能失败，所以标记要一直留到这里，不能先删）。
//!   断网时消息会在待发队列里排着（最长 24 小时），这段时间里进程死掉，下一个进程就会拒发、要人删文件
//!   新进程看到它就拒发，要人确认账号状态后删掉（codex 016 第 6 轮 #2：限制写盘失败只记日志时，下一个进程读到的
//!   是旧状态、照常放行）

use crate::backend::Backend;
use crate::error::{CoreError, SendRefusedReason};
use crate::gate::{unix_now, write_atomic};
use crate::logline;
use crate::model::{CoreEvent, MsgId};
use crate::send::{Answer, Sealed};
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// 账号受限标记（`config-privacy.md` 落盘清单）
pub const ACCOUNT_LIMITED_FILE: &str = "account-limited";
/// 限流截止时刻（`errors.md` 里的 `flood_until`；第一阶段只有发送这一类 L2 动作，一个数就够）
pub const FLOOD_FILE: &str = "flood-until";
/// 交出未完成标记（见模块文档）
pub const PENDING_FILE: &str = "send-pending";

#[derive(Default)]
struct State {
    /// 档案目录；后端起来之前还不知道，期间收到的先记在内存、知道后补写
    dir: Option<PathBuf>,
    limited: bool,
    /// Unix 秒；0 = 没有
    flood_until: i64,
    /// 标记文件读不准（除「不存在」以外的读错、内容坏了），或上一个进程的交出没收尾：不知道该不该停，一律拒发
    unreadable: bool,
    /// 本进程有限制没写进盘：交出未完成标记留着不删，下一个进程照样停
    unsaved: bool,
    /// 正在交出的有几个（几次交出交错时，最后一个收尾才删标记）
    inflight: u32,
    /// 交出成功、成败还没回来的消息（临时 id）
    awaiting: HashSet<MsgId>,
    /// 交出在途期间到过的成败：登记时跳过。成败更新走收线程，`sendMessage` 的应答走交出线程，两条线程谁先到
    /// 不一定——先到的成败若不记下，之后登记的那条就永远等不到、标记永远删不掉。只在有交出在途时记（没有在途的
    /// 交出就不会再有登记，记了也没用），在途归零时清空
    early: HashSet<MsgId>,
    /// 标记是上一个进程留下的（`load` 时就在）：本进程绝不自动删，要人确认账号状态后手动删（codex 029 第 3 轮 #1）
    inherited: bool,
    /// 后端已正常关闭：之后不再登记、不再碰标记文件（档案锁已放，可能是新进程的标记，codex 029 第 3 轮 #2）
    closed: bool,
}

#[derive(Default)]
pub struct SendLimits {
    state: Mutex<State>,
    /// 在途交出归零时唤醒 [`SendLimits::closed`]
    idle: Condvar,
}

impl SendLimits {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 后端起来、知道档案目录时：读回上一个进程记下的，并把之前只在内存里的补写下来。
    /// 只有「文件不存在」算没有记录；别的读错、内容坏了都记成读不准，拒发（失败即关）
    pub(crate) fn load(&self, dir: &Path) {
        let mut s = self.lock();
        s.dir = Some(dir.to_path_buf());
        s.closed = false;
        // 上一个进程交出后没收尾（限制可能没写进去）：只有「不存在」才算干净
        s.inherited = !matches!(
            fs::metadata(dir.join(PENDING_FILE)),
            Err(e) if e.kind() == ErrorKind::NotFound
        );
        if s.inherited {
            s.unreadable = true;
        }
        match fs::metadata(dir.join(ACCOUNT_LIMITED_FILE)) {
            Ok(_) => s.limited = true,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                if s.limited && !persist_limited(dir) {
                    s.unsaved = true;
                }
            }
            Err(_) => s.unreadable = true,
        }
        let saved = match fs::read_to_string(dir.join(FLOOD_FILE)) {
            Ok(t) => t.trim().parse::<i64>().unwrap_or_else(|_| {
                s.unreadable = true;
                0
            }),
            Err(e) if e.kind() == ErrorKind::NotFound => 0,
            Err(_) => {
                s.unreadable = true;
                0
            }
        };
        if saved < s.flood_until && !persist_flood(dir, s.flood_until) {
            s.unsaved = true;
        }
        s.flood_until = s.flood_until.max(saved);
    }

    /// 一个发送错误（不论从哪条路来）：受限 / 限流就记下并当场落盘，别的不管
    pub fn note(&self, e: &CoreError) {
        let mut s = self.lock();
        // 后端已关闭（档案锁已放）：只记在内存，不写盘——目录里可能已是新进程的文件；没写进去的由留着的标记兜底
        // （codex 029 新身份第 2 轮 #1）。下次 `load` 时会补写
        let dir = if s.closed { None } else { s.dir.clone() };
        match e {
            CoreError::AccountLimited { .. } if !s.limited => {
                s.limited = true;
                if let Some(dir) = &dir
                    && !persist_limited(dir)
                {
                    s.unsaved = true;
                    mark_pending(dir);
                }
            }
            CoreError::RateLimited { retry_after_s, .. } => {
                let until = unix_now() + i64::from(*retry_after_s);
                if until > s.flood_until {
                    s.flood_until = until;
                    if let Some(dir) = &dir
                        && !persist_flood(dir, until)
                    {
                        s.unsaved = true;
                        mark_pending(dir);
                    }
                }
            }
            _ => {}
        }
    }

    /// 现在能不能发：受限 → `send_refused(account_limited)`；冷却中 → `rate_limited`（带还要等的秒数）；
    /// 标记文件读不准 → `internal{send_limits_unreadable}`
    pub fn check(&self, now: i64) -> Result<(), CoreError> {
        let s = self.lock();
        if s.unreadable {
            return Err(CoreError::Internal {
                what: "send_limits_unreadable",
            });
        }
        if s.limited {
            return Err(CoreError::SendRefused {
                reason: SendRefusedReason::AccountLimited,
                last: None,
            });
        }
        if s.flood_until > now {
            let left = (s.flood_until - now).clamp(1, i64::from(u32::MAX));
            return Err(CoreError::RateLimited {
                retry_after_s: u32::try_from(left).unwrap_or(u32::MAX),
                td: None,
            });
        }
        Ok(())
    }

    /// 交出前：写交出未完成标记；写不进去就拒（连这个都写不进，限制更写不进）。后端还没起来（不知道档案目录）不写。
    /// 回一个守卫：交出这一步不论怎么结束（含调用方等不及把整个等待丢掉）都经它的 `drop` 收尾
    fn begin(self: &Arc<Self>) -> Result<Pending, CoreError> {
        let mut s = self.lock();
        // 后端已关闭：不交出，也不写标记（codex 029 新身份第 2 轮 #1）
        if s.closed {
            return Err(CoreError::Internal {
                what: "not_started",
            });
        }
        if let Some(dir) = &s.dir {
            write_atomic(&dir.join(PENDING_FILE), unix_now().to_string().as_bytes()).map_err(
                |_| CoreError::Internal {
                    what: "send_limits_unwritable",
                },
            )?;
        }
        s.inflight += 1;
        Ok(Pending(self.clone()))
    }

    /// 再领一份守卫，不再写标记（调用方的 [`SendLimits::begin`] 已写）：交出线程等应答用，调用方先放也删不掉标记
    // 只有真实后端的交出线程用（不开 TDLib 时只有单测）
    #[cfg_attr(not(feature = "tdlib-api"), allow(dead_code))]
    pub(crate) fn hold(self: &Arc<Self>) -> Pending {
        self.lock().inflight += 1;
        Pending(self.clone())
    }

    /// 交出成功的这几条：成败回来之前标记留着
    pub(crate) fn await_outcome(&self, ids: &[MsgId]) {
        let mut s = self.lock();
        if s.closed {
            return;
        }
        for id in ids {
            if !s.early.contains(id) {
                s.awaiting.insert(*id);
            }
        }
    }

    /// 某条交出的成败到了（成功、失败或被永久删除；失败的先经 [`SendLimits::note`] 登记）
    pub(crate) fn outcome(&self, id: MsgId) {
        let mut s = self.lock();
        s.awaiting.remove(&id);
        if s.inflight > 0 {
            s.early.insert(id);
        }
        Self::settle(&mut s);
    }

    /// 收线程的每个事件：说的是交出的成败就销账（[`crate::link::Shared::event`] 调，在 `note` 之后）
    pub(crate) fn observe(&self, ev: &CoreEvent) {
        match ev {
            CoreEvent::SendSucceeded { old_id, .. } | CoreEvent::SendFailed { old_id, .. } => {
                self.outcome(*old_id);
            }
            // TDLib `updateDeleteMessages`：`message_ids` 就是交出时的临时 id；`from_cache`（只清了本机缓存）
            // 与非 `is_permanent`（暂时不可访问）的不算——请求还在
            CoreEvent::MessagesDeleted {
                chat_id,
                message_ids,
                is_permanent: true,
                from_cache: false,
            } => {
                for &message_id in message_ids {
                    self.outcome(MsgId {
                        chat_id: *chat_id,
                        message_id,
                    });
                }
            }
            _ => {}
        }
    }

    /// 后端正常关闭（收线程已停、**档案锁放掉之前**调）：先等在途的交出线程登记完应答（最多 `wait`；TDLib 关闭时
    /// 会回完所有请求），再清掉还没回来的成败（下次启动 TDLib 续发，由那个进程登记）、收尾标记。此后不再登记、
    /// 不再碰标记文件——锁一放，档案目录里的标记可能已是新进程的。到点还有在途的：标记留着（宁可要人看一眼）
    pub(crate) fn closed(&self, wait: Duration) {
        let deadline = Instant::now() + wait;
        let mut s = self.lock();
        while s.inflight > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                logline::warn("send_pending_inflight_at_close", &[]);
                break;
            }
            s = self
                .idle
                .wait_timeout(s, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        s.awaiting.clear();
        Self::settle(&mut s);
        s.closed = true;
    }

    /// 交出这一步结束
    fn end(&self) {
        let mut s = self.lock();
        s.inflight = s.inflight.saturating_sub(1);
        Self::settle(&mut s);
        if s.inflight == 0 {
            self.idle.notify_all();
        }
    }

    /// 没有在途的交出、没有待回的成败、限制也都写进去了，就删标记
    fn settle(s: &mut State) {
        if s.inflight > 0 || s.closed {
            return;
        }
        s.early.clear();
        if s.awaiting.is_empty()
            && !s.unsaved
            && !s.inherited
            && let Some(dir) = &s.dir
        {
            match fs::remove_file(dir.join(PENDING_FILE)) {
                Err(e) if e.kind() != ErrorKind::NotFound => {
                    logline::warn("send_pending_remove_failed", &[]);
                }
                _ => {}
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 交出口（`Backend::submit`）的唯一调用处：预览、发送、重发都经这里，回的错先登记进这一份限制
/// 先写交出未完成标记，交出这一步结束时由守卫收尾（见 [`SendLimits::begin`]）。
/// 交出成功、成败还要等的是哪几条，由后端在拿到应答的那一处登记（[`SendLimits::await_outcome`]，每条只登记一次）：
/// 真实后端在交出线程里（调用方丢掉等待后也照登，`td::requests::sent` / `resent`），假后端在它的 `submit` 里
pub(crate) async fn submit<B: Backend>(b: &B, sealed: Sealed<'_>) -> Result<Answer, CoreError> {
    let limits = b.gate().limits();
    let _pending = limits.begin()?;
    let r = b.submit(sealed).await;
    if let Err(e) = &r {
        limits.note(e);
    }
    r
}

/// 一次交出的收尾守卫：`drop` 时 [`SendLimits::end`]
pub(crate) struct Pending(Arc<SendLimits>);

impl Drop for Pending {
    fn drop(&mut self) {
        self.0.end();
    }
}

/// 某个限制没写进盘时补写交出未完成标记（尽力而为：盘坏了这个也可能写不进）
fn mark_pending(dir: &Path) {
    if write_atomic(&dir.join(PENDING_FILE), unix_now().to_string().as_bytes()).is_err() {
        logline::warn("send_pending_write_failed", &[]);
    }
}

/// 写进去了回真；写不进去记日志、回假（调用方据此留着交出未完成标记）
fn persist_limited(dir: &Path) -> bool {
    let ok = write_atomic(
        &dir.join(ACCOUNT_LIMITED_FILE),
        unix_now().to_string().as_bytes(),
    )
    .is_ok();
    if !ok {
        logline::warn("account_limited_mark_failed", &[]);
    }
    ok
}

fn persist_flood(dir: &Path, until: i64) -> bool {
    let ok = write_atomic(&dir.join(FLOOD_FILE), until.to_string().as_bytes()).is_ok();
    if !ok {
        logline::warn("flood_until_write_failed", &[]);
    }
    ok
}

#[cfg(test)]
mod tests;
