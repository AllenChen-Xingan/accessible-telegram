//! `gate`：发送闸——L2 动作（发送、重发……）的唯一入口。规约正本：`specs/contracts/actions.md` 第一节第 4、5 小节。
//!
//! 等级（`actions.md` 第一节第 2 小节）：**L0** = 只读或只影响本机，随便调（发送前的准备、预览都是）；**L2** = 发给
//! 别人（发送、重发、删除、表情回应；除表情回应外都撤不回），只能凭一次性凭据交出，且绝不自动重试。
//!
//! - **凭据** [`ConfirmToken`] 只能由 [`Gate::issue`] 开出：构造函数不公开，不实现 `Clone` / `Copy` /
//!   `Serialize` / `Deserialize`，后端发送时按值吃掉它；60 秒（单调时钟：不受人改系统时间影响，改时间续不了期）；每张凭据带一个随机数，验过就记进本进程的「已用」表，同一张不能用两次；凭据本身**绝不交给
//!   别的进程**（经本机接口转来的命令由执行方自己的闸重开）
//! - **两种开法**（[`ConfirmMode`]）：测试服务器 + 收藏夹 → `test_exempt`（写死）；正式服务器 → `direct_mode`（不确认）。
//!   Touch ID / 系统密码确认已整个去掉（人定 2026-11-16「touch id 这个功能彻底关掉以及完全不用」，#113）
//! - **收藏夹**：Telegram 里自己与自己的私聊；私聊的会话 id 就是对方的用户 id，所以收藏夹的会话 id = 自己的用户 id，
//!   闸按这个等式认收藏夹，不认会话名字（别人可以把会话改名叫「收藏夹」）
//! - **测试服务器只许发收藏夹**：判定只认本进程**实际**传给 `setTdlibParameters` 的 `use_test_dc`（后端起来时
//!   [`Gate::set_server`] 交给闸）、当前登录号是 `99966` 开头、目标是自己的收藏夹（用户 id 判，不认名字）。
//!   开凭据时查一次，交出前（[`Gate::verify`]）再查一次
//! - **摘要**：交给 TDLib 那份请求的 JSON 字节 + 自己的账号 id + 服务器类别，SHA-256（[`digest`]）。凭据绑目标 + 摘要。
//!   账号与服务器也算进去：开凭据之后换了号或换了服务器，旧凭据就对不上，不会被拿去替另一个账号发
//! - **直发防重复**（`docs/research/003_send-receive-prior-art/decision.md` B17-2，人选 A）：见 [`GuardStore`]
//! - **直发留痕**：档案目录 `direct-send.log`，一条直发一行（时间、进程种类、进程号、会话 id、临时与最终消息 id、
//!   类型、结果；表情回应另记消息 id 与可能挤掉几个），不记内容与名字，保留 30 天；写不进去就拒绝直发
//!
//! 威胁边界同规约：闸防「走正规接口时出错或越权」，防不了能改本仓代码的程序。
//!
//! 代号：「001 拍板 N」= `docs/plan/001_arch-stack.md` 第七节第 N 条；「B17-2」「发N」= `docs/research/003_send-receive-prior-art/`
//! 的 `decision.md` 拍板卡与 `README.md` 第一节条目；「N7」「D1」= `docs/plan/005_decisions-b3.md` / `003_decisions-b2.md` 表里那一行。
//! 测试号段 `99966` 是 Telegram 给测试服务器留的号码前缀（[`crate::auth::TEST_PHONE_PREFIX`]）

use crate::config::ServerKind;
use crate::error::{CoreError, Holder, SendRefusedReason};
use crate::logline::{self, Field};
use crate::model::{ChatId, UserId};
use crate::secret::Secret;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod check;
mod display;

pub use crate::model::Me;
pub use check::Gate;
pub use display::{hidden_count, visible};

/// 凭据有效期（`actions.md` 第一节第 4 小节第 6 条）
pub const TOKEN_TTL: Duration = Duration::from_secs(60);
/// 直发防重复的窗口：要长过「结果未知」的 30 秒，又不至于把同一句话长久拦死（B17-2）
pub const DUP_WINDOW_S: i64 = 10 * 60;
/// 「结果未知」（含交出后没等到结果、进程中途死掉的「已提交」）的记录拦多久：这种消息可能还在 TDLib 的本地待发
/// 队列里（开着消息库，下次任何一条命令启动 TDLib 都会自动续发，003 发8），断网几小时后才发出也有可能。
/// 10 分钟太短；也不能永远拦（B17-2：到期作废）。取 24 小时，`--again` 随时放行（自审第 2 轮 P1）
pub const UNKNOWN_HOLD_S: i64 = 24 * 3600;
/// `direct-send.log` 保留多久
pub const DIRECT_LOG_KEEP_S: i64 = 30 * 24 * 3600;
/// 防重复记录的文件名（档案目录下，0600）
pub const GUARD_FILE: &str = "send-guard.json";
/// 直发留痕的文件名（档案目录下，0600）
pub const DIRECT_LOG_FILE: &str = "direct-send.log";
/// 账号被限制的标记文件（档案目录下，0600，内容只有写入时的 Unix 秒）。有它就停掉全部 L2，跨进程也停；
/// 只能由人在处理完之后**手动删掉**来解除（`errors.md` 第一节第 4 小节「同样写进档案目录，清除要人确认」）
pub use crate::send_limits::{ACCOUNT_LIMITED_FILE, FLOOD_FILE};
/// 指纹密钥的派生标签：不直接拿库钥匙当 HMAC 钥匙
const GUARD_KEY_LABEL: &[u8] = b"atg.send-guard.v1";

type HmacSha256 = Hmac<Sha256>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 现在的 Unix 秒
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

// ───────────────────────── 凭据 ─────────────────────────

/// 凭据是怎么开出来的
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmMode {
    /// 测试服务器 + 自己的收藏夹：闸自动开（001 拍板 10）
    TestExempt,
    /// 正式服务器：不确认直接发（001 拍板 11；人定 2026-11-16 为唯一方式）
    DirectMode,
}

/// 一次性确认凭据。故意不派生 `Clone` / `Copy` / `Serialize` / `Deserialize`；`Debug` 只打开法
pub struct ConfirmToken {
    target: ChatId,
    digest: [u8; 32],
    nonce: u64,
    issued: Instant,
    mode: ConfirmMode,
    /// 开凭据时带了 `--again`：交出前的查重放行这一次（#77）
    again: bool,
}

impl ConfirmMode {
    /// 机读名（与 serde 的 snake_case 同一份；有单测核对）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestExempt => "test_exempt",
            Self::DirectMode => "direct_mode",
        }
    }
}

impl ConfirmToken {
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }

    /// 只给本 crate 的单测拨时钟用
    #[cfg(test)]
    pub(crate) fn age_by(&mut self, d: Duration) {
        self.issued = self.issued.checked_sub(d).unwrap_or(self.issued);
    }
}

impl std::fmt::Debug for ConfirmToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmToken")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

/// 闸要核对的一份「已定稿的请求」：发送计划、重发计划都实现它
/// 封住 [`Gated`]：只有本 crate 里的计划能实现它（codex 016 第 4 轮 #3：crate 外的实现能沿用真计划的目标与摘要、
/// 却给人看另一段确认文字）
mod sealed {
    pub trait Sealed {}
}
pub(crate) use sealed::Sealed;

pub trait Gated: Sealed {
    /// 发给哪个会话
    fn target(&self) -> ChatId;
    /// 凭据绑的摘要（[`digest`]）
    fn digest(&self) -> [u8; 32];
    /// 防重复用的「意图摘要」：去掉每次都变的字段（`sending_id`）后的摘要
    fn intent(&self) -> [u8; 32];
    /// 防重复按哪几个指纹拦（#10）：默认就是 [`Gated::intent`] 一个。一次动好几条、每条都可能单独重来的（转发），
    /// 每条一个——换一批、取子集也拦得住其中结果未知的那条。有几个时与 [`Gated::log_items`] 一一对应、同序
    fn intents(&self) -> Vec<[u8; 32]> {
        vec![self.intent()]
    }
    /// 进不进直发防重复（B17-2）。重发不进：对象是某条失败消息的 id，重发后旧 id 就没了，同一条重发不了两次
    fn dedup(&self) -> bool;
    /// 留痕里的内容类型（`text` / `resend`……），不是内容本身
    fn kind(&self) -> &'static str;
    /// 留痕里另记的 `reaction`：动在哪条消息上、按最坏情况可能挤掉我原先点的几个回应（`None` = 拿不准）。只有表情回应有；只记 id 与个数
    fn log_target(&self) -> Option<(i64, Option<usize>)> {
        None
    }
    /// 留痕里另记的来源会话（`from_chat_id`）：只有转发有（#10）；只记 id
    fn log_source(&self) -> Option<ChatId> {
        None
    }
    /// 一次请求动好几条时（批量重发），各条的旧 id：留痕按条记，每条一行、带 `old_id`（#83）。单条的计划是空表
    fn log_items(&self) -> Vec<i64> {
        Vec::new()
    }
}

/// 摘要：请求 JSON 的字节（不做 Unicode 规范化）+ 自己的账号 id + 服务器类别，SHA-256
pub fn digest(request: &serde_json::Value, account: UserId, server: ServerKind) -> [u8; 32] {
    let server = if server.use_test_dc() { "test" } else { "main" };
    let mut h = Sha256::new();
    h.update(b"atg.send.v1\0");
    h.update(account.to_le_bytes());
    h.update(server.as_bytes());
    h.update(b"\0");
    h.update(serde_json::to_vec(request).unwrap_or_default());
    h.finalize().into()
}

/// 为什么防重复记录存的是**带钥匙**的指纹、不是普通哈希：`send-guard.json` 落在磁盘上，普通哈希谁都能算——拿「好的」
/// 「收到」这类候选短句逐条算一遍比对，就能猜出发过什么。带上只在内存里的钥匙，没有钥匙就算不出指纹，猜不了。
///
/// 指纹密钥：由本地库钥匙带标签派生（HMAC-SHA256(库钥匙, 标签 ‖ 档案名)）。只在内存里
pub fn derive_guard_key(db_key: &[u8], profile: &str) -> Secret {
    // HMAC 收任意长度的钥匙，new_from_slice 不会失败；失败时给全零（只会让指纹可猜，不会放行什么）
    let mut mac = HmacSha256::new_from_slice(db_key).unwrap_or_else(|_| {
        HmacSha256::new_from_slice(&[0u8; 32]).unwrap_or_else(|_| unreachable!())
    });
    mac.update(GUARD_KEY_LABEL);
    mac.update(b"\0");
    mac.update(profile.as_bytes());
    Secret::new(mac.finalize().into_bytes().to_vec())
}

fn fingerprint(key: &Secret, chat: ChatId, intent: &[u8; 32]) -> String {
    let Ok(mut mac) = HmacSha256::new_from_slice(key.expose()) else {
        return String::new();
    };
    mac.update(&chat.to_le_bytes());
    mac.update(intent);
    hex(&mac.finalize().into_bytes())
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 一次交出的结果（记账与留痕用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// 交给 TDLib 之前写下，等到结果再改
    Submitted,
    Succeeded,
    Failed,
    Unknown,
}

// ───────────────────────── 防重复记录 ─────────────────────────

/// 什么算命中：同会话同指纹、上次不是「明确失败」，取最近一条。开凭据时的早拒与交出前的认领共用这一份判定（#77）
fn latest_hit<'a>(entries: &'a [GuardEntry], chat: ChatId, fp: &str) -> Option<&'a GuardEntry> {
    entries
        .iter()
        .filter(|e| e.chat_id == chat && e.fp == fp && e.state != Outcome::Failed)
        .max_by_key(|e| e.at)
}

/// 一条防重复记录：时间、会话、指纹、状态。**不记内容、不记名字**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardEntry {
    pub at: i64,
    pub chat_id: ChatId,
    pub fp: String,
    pub state: Outcome,
}

/// 档案目录下的 `send-guard.json`（0600，整体重写：先写临时文件再改名）。同一档案同时只有一个进程（档案锁），
/// 不另加锁。每次写都删掉窗口以前的，文件不会越积越大
pub struct GuardStore {
    path: PathBuf,
}

impl GuardStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> std::io::Result<Vec<GuardEntry>> {
        match fs::read(&self.path) {
            Ok(b) => serde_json::from_slice(&b)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "guard")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    fn save(&self, entries: &[GuardEntry]) -> std::io::Result<()> {
        let body = serde_json::to_vec(entries).unwrap_or_default();
        write_atomic(&self.path, &body)
    }

    /// 还算数的记录：已发出 / 失败的留 10 分钟；结果未知 / 没收尾的留 24 小时
    fn live(now: i64, entries: Vec<GuardEntry>) -> Vec<GuardEntry> {
        entries
            .into_iter()
            .filter(|e| {
                let hold = match e.state {
                    Outcome::Unknown | Outcome::Submitted => UNKNOWN_HOLD_S,
                    Outcome::Succeeded | Outcome::Failed => DUP_WINDOW_S,
                };
                // 时间在「现在」之后的记录（本机时钟被往回拨过）照样算数、留到时钟追上再过期：丢掉就等于回拨
                // 时钟能解除防重复，刚提交、结果未知的那条能被再发一次（codex 016 第 1 轮 #1）。多拦了用 --again 放行
                e.at > now || now - e.at < hold
            })
            .collect()
    }

    /// 窗口内同会话同指纹、上次不是「明确失败」的最近一条（开凭据时早拒用；交出前那次见 [`Self::claim`]）
    pub fn hit(&self, now: i64, chat: ChatId, fp: &str) -> std::io::Result<Option<GuardEntry>> {
        Ok(latest_hit(&Self::live(now, self.load()?), chat, fp).cloned())
    }

    /// 交出前的查重与登记一步做完（#77）：读一次 → 不带 `again` 就查命中 → 记「已提交」→ 写一次。
    /// 命中返回 `Ok(Err(那条))`，什么都不写。调用方攥着闸的锁调（进程内不交错；跨进程有档案锁）
    pub fn claim(
        &self,
        now: i64,
        chat: ChatId,
        fp: &str,
        again: bool,
    ) -> std::io::Result<Result<(), GuardEntry>> {
        self.claim_all(now, chat, &[fp.to_owned()], again)
    }

    /// [`Self::claim`] 的多指纹版（#10）：先查全部，任一命中就整个不认领、什么都不写；都没命中才一起记「已提交」
    pub fn claim_all(
        &self,
        now: i64,
        chat: ChatId,
        fps: &[String],
        again: bool,
    ) -> std::io::Result<Result<(), GuardEntry>> {
        let mut entries = Self::live(now, self.load()?);
        if !again && let Some(hit) = fps.iter().find_map(|fp| latest_hit(&entries, chat, fp)) {
            return Ok(Err(hit.clone()));
        }
        for fp in fps {
            entries.push(GuardEntry {
                at: now,
                chat_id: chat,
                fp: fp.clone(),
                state: Outcome::Submitted,
            });
        }
        self.save(&entries).map(Ok)
    }

    pub fn record(&self, now: i64, chat: ChatId, fp: &str, state: Outcome) -> std::io::Result<()> {
        // 读不了（含文件坏了）就不写：拿空表覆盖会抹掉别的发送的防重复记录（codex 016 第 1 轮 #2）。
        // 调用方据此拒直发；文件坏了要人删掉 send-guard.json 才恢复
        let mut entries = Self::live(now, self.load()?);
        entries.push(GuardEntry {
            at: now,
            chat_id: chat,
            fp: fp.to_owned(),
            state,
        });
        self.save(&entries)
    }

    /// 把最近一条「已提交」的改成结果；找不到就补一条
    pub fn settle(&self, now: i64, chat: ChatId, fp: &str, state: Outcome) -> std::io::Result<()> {
        self.settle_all(now, chat, &[fp.to_owned()], state)
    }

    /// [`Self::settle`] 的多指纹版（#10）：读一次、逐个改、写一次
    pub fn settle_all(
        &self,
        now: i64,
        chat: ChatId,
        fps: &[String],
        state: Outcome,
    ) -> std::io::Result<()> {
        // 同 [`Self::record`]：读不了就不覆盖（「已提交」留着，照样按结果未知拦）
        let mut entries = Self::live(now, self.load()?);
        for fp in fps {
            match entries
                .iter_mut()
                .rev()
                .find(|e| e.chat_id == chat && &e.fp == fp && e.state == Outcome::Submitted)
            {
                Some(e) => e.state = state,
                None => entries.push(GuardEntry {
                    at: now,
                    chat_id: chat,
                    fp: fp.clone(),
                    state,
                }),
            }
        }
        self.save(&entries)
    }
}

/// 先写同目录的临时文件（0600）再改名：中途崩了也不会留下半个文件
pub fn write_atomic(path: &Path, body: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(body)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests;
