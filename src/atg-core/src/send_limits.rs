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

#[derive(Default)]
struct State {
    /// 档案目录；后端起来之前还不知道，期间收到的先记在内存、知道后补写
    dir: Option<PathBuf>,
    limited: bool,
    /// Unix 秒；0 = 没有
    flood_until: i64,
    /// 标记文件读不准（除「不存在」以外的读错、内容坏了）：不知道该不该停，一律拒发
    unreadable: bool,
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
        match fs::metadata(dir.join(ACCOUNT_LIMITED_FILE)) {
            Ok(_) => s.limited = true,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                if s.limited {
                    persist_limited(dir);
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
        if saved < s.flood_until {
            persist_flood(dir, s.flood_until);
        }
        s.flood_until = s.flood_until.max(saved);
    }

    /// 一个发送错误（不论从哪条路来）：受限 / 限流就记下并当场落盘，别的不管
    pub fn note(&self, e: &CoreError) {
        let mut s = self.lock();
        match e {
            CoreError::AccountLimited { .. } if !s.limited => {
                s.limited = true;
                if let Some(dir) = &s.dir {
                    persist_limited(dir);
                }
            }
            CoreError::RateLimited { retry_after_s, .. } => {
                let until = unix_now() + i64::from(*retry_after_s);
                if until > s.flood_until {
                    s.flood_until = until;
                    if let Some(dir) = &s.dir {
                        persist_flood(dir, until);
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

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 交出口（`Backend::submit`）的唯一调用处：预览、发送、重发都经这里，回的错先登记进这一份限制
pub(crate) async fn submit<B: Backend>(b: &B, sealed: Sealed<'_>) -> Result<Answer, CoreError> {
    let r = b.submit(sealed).await;
    if let Err(e) = &r {
        b.gate().limits().note(e);
    }
    r
}

fn persist_limited(dir: &Path) {
    if write_atomic(
        &dir.join(ACCOUNT_LIMITED_FILE),
        unix_now().to_string().as_bytes(),
    )
    .is_err()
    {
        logline::warn("account_limited_mark_failed", &[]);
    }
}

fn persist_flood(dir: &Path, until: i64) {
    if write_atomic(&dir.join(FLOOD_FILE), until.to_string().as_bytes()).is_err() {
        logline::warn("flood_until_write_failed", &[]);
    }
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
}
