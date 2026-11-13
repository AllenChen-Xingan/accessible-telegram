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
//! 落盘两个文件，都在档案目录，都不记内容：
//! - [`ACCOUNT_LIMITED_FILE`]：账号被 Telegram 限制。人处理好、删掉它才恢复
//! - [`FLOOD_FILE`]：限流到什么时候（Unix 秒）之前不许再发；到点自然失效
//! - [`PENDING_FILE`]：交出未完成。每次交出前先写，交出这一步结束（正常返回、出错、调用方等不及丢掉、Ctrl+C
//!   走正常收尾）就删——常见情况都自己清，不留人工活；只有两种情况留着：进程在交出那一下（毫秒级：断网时 TDLib
//!   也是先放进本地待发队列、当场回应答）真的死掉，或某个受限 / 限流没写进盘（写失败时会补写这个标记）。
//!   新进程看到它就拒发，要人确认账号状态后删掉（codex 016 第 6 轮 #2：限制写盘失败只记日志时，下一个进程读到的
//!   是旧状态、照常放行）

use crate::backend::Backend;
use crate::error::{CoreError, SendRefusedReason};
use crate::gate::{unix_now, write_atomic};
use crate::logline;
use crate::send::{Answer, Sealed};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

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
}

#[derive(Default)]
pub struct SendLimits {
    state: Mutex<State>,
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
        // 上一个进程交出后没收尾（限制可能没写进去）：只有「不存在」才算干净
        match fs::metadata(dir.join(PENDING_FILE)) {
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            _ => s.unreadable = true,
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
        match e {
            CoreError::AccountLimited { .. } if !s.limited => {
                s.limited = true;
                if let Some(dir) = s.dir.clone()
                    && !persist_limited(&dir)
                {
                    s.unsaved = true;
                    mark_pending(&dir);
                }
            }
            CoreError::RateLimited { retry_after_s, .. } => {
                let until = unix_now() + i64::from(*retry_after_s);
                if until > s.flood_until {
                    s.flood_until = until;
                    if let Some(dir) = s.dir.clone()
                        && !persist_flood(&dir, until)
                    {
                        s.unsaved = true;
                        mark_pending(&dir);
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

    /// 交出这一步结束：没有别的正在交出、限制也都写进去了，就删标记
    fn end(&self) {
        let mut s = self.lock();
        s.inflight = s.inflight.saturating_sub(1);
        if s.inflight == 0
            && !s.unsaved
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
/// 先写交出未完成标记，交出这一步结束时由守卫收尾（见 [`SendLimits::begin`]）
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
struct Pending(Arc<SendLimits>);

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
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atg-limits-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn account_limited_is_written_at_once_and_read_by_the_next_process() {
        // codex 016 第 4 轮 #1：收到就落盘，不等下一次判开法；进程一退、重启照样停
        let dir = temp_dir("limited");
        let l = SendLimits::new();
        l.load(&dir);
        l.note(&CoreError::AccountLimited { td: None });
        assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
        let next = SendLimits::new();
        next.load(&dir);
        assert!(next.check(unix_now()).is_err());
    }

    #[test]
    fn noted_before_the_dir_is_known_is_written_when_it_is() {
        let dir = temp_dir("early");
        let l = SendLimits::new();
        l.note(&CoreError::AccountLimited { td: None });
        l.note(&CoreError::RateLimited {
            retry_after_s: 300,
            td: None,
        });
        l.load(&dir);
        assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
        assert!(dir.join(FLOOD_FILE).exists());
    }

    #[test]
    fn flood_wait_is_kept_across_processes_and_expires() {
        // codex 016 第 4 轮 #2：限流的等待期写进档案目录，新进程也遵守；到点自然失效
        let dir = temp_dir("flood");
        let l = SendLimits::new();
        l.load(&dir);
        l.note(&CoreError::RateLimited {
            retry_after_s: 120,
            td: None,
        });
        let next = SendLimits::new();
        next.load(&dir);
        let now = unix_now();
        match next.check(now) {
            Err(CoreError::RateLimited { retry_after_s, .. }) => assert!(retry_after_s > 100),
            other => panic!("应当限流：{other:?}"),
        }
        assert!(next.check(now + 121).is_ok());
        // 短的不会把长的冲掉
        next.note(&CoreError::RateLimited {
            retry_after_s: 5,
            td: None,
        });
        assert!(next.check(now + 60).is_err());
    }

    #[test]
    fn unreadable_marks_fail_closed() {
        // codex 016 第 5 轮 #2：截止文件内容坏了 → 不当成「没有记录」，拒发
        let dir = temp_dir("bad");
        fs::write(dir.join(FLOOD_FILE), b"not a number").unwrap();
        let l = SendLimits::new();
        l.load(&dir);
        assert!(matches!(
            l.check(unix_now()),
            Err(CoreError::Internal {
                what: "send_limits_unreadable"
            })
        ));
        // 读不了（这里是同名目录）也一样
        let dir2 = temp_dir("dir");
        fs::create_dir_all(dir2.join(FLOOD_FILE)).unwrap();
        let l2 = SendLimits::new();
        l2.load(&dir2);
        assert!(l2.check(unix_now()).is_err());
    }

    #[test]
    fn unsaved_limit_keeps_the_pending_mark_for_the_next_process() {
        // codex 016 第 6 轮 #2：限制写不进盘（这里用同名目录挡住）时，交出未完成标记留着，下一个进程照样停
        let dir = temp_dir("unsaved");
        let l = SendLimits::new();
        l.load(&dir);
        // 加载之后才把截止文件的位置占成非空目录：之后写截止时间（临时文件改名过去）必失败
        fs::create_dir_all(dir.join(FLOOD_FILE).join("x")).unwrap();
        let guard = l.begin().unwrap();
        l.note(&CoreError::RateLimited {
            retry_after_s: 60,
            td: None,
        });
        drop(guard);
        assert!(dir.join(PENDING_FILE).exists());
        let next = SendLimits::new();
        next.load(&dir);
        assert!(next.check(unix_now()).is_err());
        // 干净的一次交出：标记删掉
        let ok = temp_dir("clean");
        let c = SendLimits::new();
        c.load(&ok);
        drop(c.begin().unwrap());
        assert!(!ok.join(PENDING_FILE).exists());
        let after = SendLimits::new();
        after.load(&ok);
        assert!(after.check(unix_now()).is_ok());
    }

    #[test]
    fn abandoned_or_overlapping_submits_clean_up_themselves() {
        // 调用方等不及把交出丢掉（守卫随之 drop）也自己清；两次交错时最后一个收尾才删
        let dir = temp_dir("overlap");
        let l = SendLimits::new();
        l.load(&dir);
        let a = l.begin().unwrap();
        let b = l.begin().unwrap();
        drop(a);
        assert!(dir.join(PENDING_FILE).exists());
        drop(b);
        assert!(!dir.join(PENDING_FILE).exists());
    }
}
