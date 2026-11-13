//! `gate`：发送闸——L2 动作（发送、重发……）的唯一入口。规约正本：`specs/contracts/actions.md` 第一节第 4、5 小节。
//!
//! 等级（`actions.md` 第一节第 2 小节）：**L0** = 只读或只影响本机，随便调（发送前的准备、预览都是）；**L2** = 发给
//! 别人、撤不回（发送、重发、删除），只能凭一次性凭据交出，且绝不自动重试。
//!
//! - **凭据** [`ConfirmToken`] 只能由 [`Gate::issue`] 开出：构造函数不公开，不实现 `Clone` / `Copy` /
//!   `Serialize` / `Deserialize`，后端发送时按值吃掉它；60 秒（单调时钟：不受人改系统时间影响，改时间续不了期）；每张凭据带一个随机数，验过就记进本进程的「已用」表，同一张不能用两次；凭据本身**绝不交给
//!   别的进程**（经本机接口转来的命令由执行方自己的闸重开）
//! - **三种开法**（[`ConfirmMode`]）：测试服务器 + 收藏夹 → `test_exempt`（写死）；以直发参数启动 → `direct_mode`；
//!   其余（正式服务器的一切目标，含自己的收藏夹）→ `human_auth`（Touch ID / 系统密码，[`Authenticator`]）
//! - **收藏夹**：Telegram 里自己与自己的私聊；私聊的会话 id 就是对方的用户 id，所以收藏夹的会话 id = 自己的用户 id，
//!   闸按这个等式认收藏夹，不认会话名字（别人可以把会话改名叫「收藏夹」）
//! - **测试服务器只许发收藏夹**：判定只认本进程**实际**传给 `setTdlibParameters` 的 `use_test_dc`（后端起来时
//!   [`Gate::set_server`] 交给闸）、当前登录号是 `99966` 开头、目标是自己的收藏夹（用户 id 判，不认名字）。
//!   开凭据时查一次，交出前（[`Gate::verify`]）再查一次
//! - **摘要**：交给 TDLib 那份请求的 JSON 字节 + 自己的账号 id + 服务器类别，SHA-256（[`digest`]）。凭据绑目标 + 摘要。
//!   账号与服务器也算进去：开凭据之后换了号或换了服务器，旧凭据就对不上，不会被拿去替另一个账号发
//! - **直发防重复**（`docs/research/003_send-receive-prior-art/decision.md` B17-2，人选 A）：见 [`GuardStore`]
//! - **直发留痕**：档案目录 `direct-send.log`，一条直发一行（时间、进程种类、进程号、会话 id、临时与最终消息 id、
//!   类型、结果），不记内容与名字，保留 30 天；写不进去就拒绝直发
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
pub const ACCOUNT_LIMITED_FILE: &str = "account-limited";
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
    /// Touch ID / 系统密码（001 拍板 9）
    HumanAuth,
    /// 以 `--unsafe-direct-send` 启动的这次运行（001 拍板 11）
    DirectMode,
}

/// 一次性确认凭据。故意不派生 `Clone` / `Copy` / `Serialize` / `Deserialize`；`Debug` 只打开法
pub struct ConfirmToken {
    target: ChatId,
    digest: [u8; 32],
    nonce: u64,
    issued: Instant,
    mode: ConfirmMode,
}

impl ConfirmMode {
    /// 机读名（与 serde 的 snake_case 同一份；有单测核对）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestExempt => "test_exempt",
            Self::HumanAuth => "human_auth",
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
pub trait Gated {
    /// 发给哪个会话
    fn target(&self) -> ChatId;
    /// 凭据绑的摘要（[`digest`]）
    fn digest(&self) -> [u8; 32];
    /// 防重复用的「意图摘要」：去掉每次都变的字段（`sending_id`）后的摘要
    fn intent(&self) -> [u8; 32];
    /// 给人确认时看的说明（Touch ID 弹框的文字）；已转义控制字符
    fn confirm_text(&self) -> String;
    /// 进不进直发防重复（B17-2）。重发不进：对象是某条失败消息的 id，重发后旧 id 就没了，同一条重发不了两次
    fn dedup(&self) -> bool;
    /// 留痕里的内容类型（`text` / `resend`……），不是内容本身
    fn kind(&self) -> &'static str;
    /// 要人确认的整段说明（[`Gated::confirm_text`]，含会话名、来源、每条正文）能不能在确认框里放下。放不下就不弹框
    /// （人看不到全文，按下去等于没确认；自审第 5、7 轮 P1）
    fn confirm_fits(&self) -> bool {
        fits_confirm_box(&self.confirm_text())
    }
}

/// Touch ID 确认框的说明区最多放几个字、几行（系统面板高度固定，放不下的被截掉）。**估计值，没在真弹框上量过**
/// （待实测，`state/handoff/send.md`）；取得保守些，宁可多拒
/// macOS 还会在说明前面加一句「“atg-cli”想要……」，系统密码兜底框的版式也不同——量出来之前取得很紧（自审第 8 轮 P1）。
/// 量了以后改这两个数，并把实测记进契约
pub const CONFIRM_MAX_CHARS: usize = 120;
pub const CONFIRM_MAX_LINES: usize = 4;

/// 整段确认说明放不放得下：行数与字数都不超（会话名、来源那几行也算，不只算正文）
pub fn fits_confirm_box(text: &str) -> bool {
    text.lines().count() <= CONFIRM_MAX_LINES && text.chars().count() <= CONFIRM_MAX_CHARS
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

// ───────────────────────── 人的确认 ─────────────────────────

/// Touch ID / 系统密码。macOS 上是 [`crate::localauth::LocalAuth`]；单测给假的
pub trait Authenticator: Send + Sync {
    /// 弹框让人确认；`reason` 是框里的说明。没过 → `auth_failed`，弹不出 → `not_interactive`
    fn authenticate(&self, reason: &str) -> Result<(), SendRefusedReason>;
}

/// 一律拒绝（没有本地认证的平台）
pub struct NoAuth;

impl Authenticator for NoAuth {
    fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
        Err(SendRefusedReason::NotInteractive)
    }
}

/// 本机默认的认证器：macOS 用 LocalAuthentication，别的平台一律拒
pub fn system_authenticator() -> Box<dyn Authenticator> {
    #[cfg(target_os = "macos")]
    {
        Box::new(crate::localauth::LocalAuth)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NoAuth)
    }
}

// ───────────────────────── 闸 ─────────────────────────

pub use crate::model::Me;

#[derive(Default)]
struct Facts {
    /// 本进程实际传给 TDLib 的 `use_test_dc`（后端起来时设）
    use_test_dc: Option<bool>,
    me: Option<Me>,
    /// 档案目录（防重复记录、直发留痕都放这里）
    dir: Option<PathBuf>,
    guard_key: Option<Secret>,
    /// 收到过 `account_limited`（本进程收到，或启动时发现档案目录里的 [`ACCOUNT_LIMITED_FILE`]）：停掉全部 L2，
    /// 直到人删掉那个文件（`errors.md`）
    account_limited: bool,
}

/// 发送闸。每个后端持有一个；CLI / GUI 经 `Backend::gate` 拿到它开凭据
pub struct Gate {
    direct: bool,
    proc: Holder,
    auth: Box<dyn Authenticator>,
    used: Mutex<HashSet<u64>>,
    facts: Mutex<Facts>,
    direct_sent: std::sync::atomic::AtomicU64,
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

impl Gate {
    /// `direct`：本进程是不是以 `--unsafe-direct-send` 启动的（只认启动参数）
    pub fn new(direct: bool, proc: Holder, auth: Box<dyn Authenticator>) -> Self {
        Self {
            direct,
            proc,
            auth,
            used: Mutex::new(HashSet::new()),
            facts: Mutex::new(Facts::default()),
            direct_sent: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn direct(&self) -> bool {
        self.direct
    }

    /// 后端起来时：实际的 `use_test_dc`、档案目录、指纹密钥
    pub(crate) fn set_server(&self, use_test_dc: bool, dir: &Path, guard_key: Secret) {
        let mut f = lock(&self.facts);
        f.use_test_dc = Some(use_test_dc);
        f.dir = Some(dir.to_path_buf());
        f.guard_key = Some(guard_key);
        // 上一个进程收到过账号被限：这一个照样停（CLI 每条命令都是新进程）
        if dir.join(ACCOUNT_LIMITED_FILE).exists() {
            f.account_limited = true;
        }
    }

    /// 本进程已直发（交给 TDLib）了几条（`status` 行的 `direct_sent`）
    pub fn direct_sent(&self) -> u64 {
        self.direct_sent.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 每次查到「自己是谁」都更新（测试服务器清空后重注册，id 会变）
    pub(crate) fn set_me(&self, me: Me) {
        lock(&self.facts).me = Some(me);
    }

    /// 本进程实际传给 TDLib 的 `use_test_dc`（后端还没起是 `None`）
    pub fn use_test_dc(&self) -> Option<bool> {
        lock(&self.facts).use_test_dc
    }

    pub fn me(&self) -> Option<Me> {
        lock(&self.facts).me
    }

    /// 这份计划会用哪种方式确认（不弹框、不记账）。测试服务器上目标不是收藏夹 → `test_dc_saved_only`
    pub fn mode_for(&self, target: ChatId) -> Result<ConfirmMode, CoreError> {
        let f = lock(&self.facts);
        if f.account_limited {
            return Err(refused(SendRefusedReason::AccountLimited));
        }
        let use_test_dc = f.use_test_dc.ok_or(CoreError::Internal {
            what: "gate_not_started",
        })?;
        if use_test_dc {
            let me = f.me.ok_or(CoreError::Internal { what: "gate_no_me" })?;
            return if me.test_number && target == me.saved_chat_id {
                Ok(ConfirmMode::TestExempt)
            } else {
                Err(refused(SendRefusedReason::TestDcSavedOnly))
            };
        }
        Ok(if self.direct {
            ConfirmMode::DirectMode
        } else {
            ConfirmMode::HumanAuth
        })
    }

    /// 开凭据。直发模式先查防重复（`again` 放行这一次）；默认模式弹 Touch ID / 系统密码
    pub fn issue(&self, plan: &dyn Gated, again: bool) -> Result<ConfirmToken, CoreError> {
        let mode = self.mode_for(plan.target())?;
        match mode {
            ConfirmMode::TestExempt => {}
            ConfirmMode::DirectMode => {
                if plan.dedup() && !again {
                    self.check_duplicate(plan)?;
                }
            }
            ConfirmMode::HumanAuth => {
                if !plan.confirm_fits() {
                    return Err(refused(SendRefusedReason::TooLongToConfirm));
                }
                self.auth
                    .authenticate(&plan.confirm_text())
                    .map_err(refused)?;
            }
        }
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).map_err(|_| CoreError::Internal { what: "random" })?;
        logline::info("gate_issue", &[Field::Name("mode", mode.as_str())]);
        Ok(ConfirmToken {
            target: plan.target(),
            digest: plan.digest(),
            nonce: u64::from_le_bytes(nonce),
            issued: Instant::now(),
            mode,
        })
    }

    /// 交出前验凭据（后端的发送动作里调，调用方碰不到）：目标对、摘要对、60 秒内、没用过、测试服务器规则仍成立。
    /// 凭据在这里被吃掉；过了才登记随机数
    pub(crate) fn verify(
        &self,
        token: ConfirmToken,
        plan: &dyn Gated,
    ) -> Result<ConfirmMode, CoreError> {
        // 测试服务器规则每次交出前重查（`actions.md` 第一节第 4 小节第 3 条）
        let now_mode = self.mode_for(plan.target())?;
        if token.target != plan.target() {
            return Err(refused(SendRefusedReason::TargetMismatch));
        }
        if token.digest != plan.digest() {
            return Err(refused(SendRefusedReason::ContentMismatch));
        }
        if token.issued.elapsed() > TOKEN_TTL {
            return Err(refused(SendRefusedReason::TokenExpired));
        }
        // 开法只由服务器类别与启动参数定，进程内不会变（换号后不再是测试号已在上面的 `mode_for` 拒掉）；
        // 真对不上是本项目的 bug，不当成某种可读的拒绝原因
        if token.mode != now_mode {
            return Err(CoreError::Internal {
                what: "confirm_mode_changed",
            });
        }
        if !lock(&self.used).insert(token.nonce) {
            return Err(refused(SendRefusedReason::TokenReused));
        }
        Ok(token.mode)
    }

    /// 交给 TDLib 之前：直发要先把「已提交」记下、留痕文件要写得进去，否则拒发
    pub(crate) fn before_submit(
        &self,
        mode: ConfirmMode,
        plan: &dyn Gated,
    ) -> Result<(), CoreError> {
        if mode != ConfirmMode::DirectMode {
            return Ok(());
        }
        let f = lock(&self.facts);
        let dir = f.dir.as_ref().ok_or(CoreError::Internal {
            what: "gate_not_started",
        })?;
        let key = f.guard_key.as_ref().ok_or(CoreError::Internal {
            what: "gate_not_started",
        })?;
        // 留痕是条件：先确认能追加
        open_append(&dir.join(DIRECT_LOG_FILE)).map_err(|_| CoreError::Internal {
            what: "direct_log_unwritable",
        })?;
        if plan.dedup() {
            let store = GuardStore::new(dir.join(GUARD_FILE));
            let fp = fingerprint(key, plan.target(), &plan.intent());
            store
                .record(unix_now(), plan.target(), &fp, Outcome::Submitted)
                .map_err(|_| CoreError::Internal {
                    what: "send_guard_unwritable",
                })?;
        }
        // 前置条件都过了，接下来就交给 TDLib：算一条直发
        self.direct_sent
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// 等到结果后：直发改防重复记录、写一行留痕；`account_limited` 记下（之后 L2 全停）
    pub fn after_outcome(
        &self,
        mode: ConfirmMode,
        plan: &dyn Gated,
        temp_id: i64,
        final_id: Option<i64>,
        outcome: Outcome,
        error: Option<&CoreError>,
    ) {
        if matches!(error, Some(CoreError::AccountLimited { .. })) {
            let mut f = lock(&self.facts);
            f.account_limited = true;
            // 写进档案目录：之后的进程也停，直到人删掉它
            if let Some(dir) = f.dir.as_ref()
                && write_atomic(
                    &dir.join(ACCOUNT_LIMITED_FILE),
                    unix_now().to_string().as_bytes(),
                )
                .is_err()
            {
                logline::warn("account_limited_mark_failed", &[]);
            }
        }
        if mode != ConfirmMode::DirectMode {
            return;
        }
        let f = lock(&self.facts);
        let (Some(dir), Some(key)) = (f.dir.as_ref(), f.guard_key.as_ref()) else {
            return;
        };
        let now = unix_now();
        if plan.dedup() {
            let fp = fingerprint(key, plan.target(), &plan.intent());
            if GuardStore::new(dir.join(GUARD_FILE))
                .settle(now, plan.target(), &fp, outcome)
                .is_err()
            {
                logline::warn("send_guard_update_failed", &[]);
            }
        }
        let line = DirectLogLine {
            at: now,
            proc: self.proc,
            pid: std::process::id(),
            chat_id: plan.target(),
            temp_id,
            final_id,
            kind: plan.kind(),
            result: outcome,
            error: error.map(CoreError::code),
        };
        if append_direct_log(&dir.join(DIRECT_LOG_FILE), &line, now).is_err() {
            logline::warn("direct_log_write_failed", &[]);
        }
    }

    fn check_duplicate(&self, plan: &dyn Gated) -> Result<(), CoreError> {
        let f = lock(&self.facts);
        let (Some(dir), Some(key)) = (f.dir.as_ref(), f.guard_key.as_ref()) else {
            return Err(CoreError::Internal {
                what: "gate_not_started",
            });
        };
        let fp = fingerprint(key, plan.target(), &plan.intent());
        let store = GuardStore::new(dir.join(GUARD_FILE));
        let now = unix_now();
        // 读不了记录：宁可拒（直发撤不回）
        let hit = store
            .hit(now, plan.target(), &fp)
            .map_err(|_| CoreError::Internal {
                what: "send_guard_unreadable",
            })?;
        match hit {
            None => Ok(()),
            Some(e) => Err(CoreError::SendRefused {
                reason: SendRefusedReason::DuplicateRecent,
                last: Some(crate::error::LastSend {
                    at: e.at,
                    ago_s: now.saturating_sub(e.at).max(0),
                    outcome: e.state,
                }),
            }),
        }
    }
}

fn refused(reason: SendRefusedReason) -> CoreError {
    CoreError::SendRefused { reason, last: None }
}

// ───────────────────────── 防重复记录 ─────────────────────────

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
                // 时间在「现在」之后一分钟以上的记录不认：本机时钟被往回拨过，或文件被改过。容一分钟是给
                // 同一秒内前后写读的进程留的余量；丢掉只会少拦，不会误拦
                now.saturating_sub(e.at) < hold && e.at <= now + 60
            })
            .collect()
    }

    /// 窗口内同会话同指纹、上次不是「明确失败」的最近一条
    pub fn hit(&self, now: i64, chat: ChatId, fp: &str) -> std::io::Result<Option<GuardEntry>> {
        Ok(Self::live(now, self.load()?)
            .into_iter()
            .filter(|e| e.chat_id == chat && e.fp == fp && e.state != Outcome::Failed)
            .max_by_key(|e| e.at))
    }

    pub fn record(&self, now: i64, chat: ChatId, fp: &str, state: Outcome) -> std::io::Result<()> {
        // 旧文件坏了：丢掉重来（只影响防重复，不影响别的）
        let mut entries = Self::live(now, self.load().unwrap_or_default());
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
        let mut entries = Self::live(now, self.load().unwrap_or_default());
        match entries
            .iter_mut()
            .rev()
            .find(|e| e.chat_id == chat && e.fp == fp && e.state == Outcome::Submitted)
        {
            Some(e) => e.state = state,
            None => entries.push(GuardEntry {
                at: now,
                chat_id: chat,
                fp: fp.to_owned(),
                state,
            }),
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

fn open_append(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
}

#[derive(Serialize)]
struct DirectLogLine {
    at: i64,
    proc: Holder,
    pid: u32,
    chat_id: ChatId,
    temp_id: i64,
    final_id: Option<i64>,
    kind: &'static str,
    result: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

/// 追加一行；文件头一行已超过 30 天时先把过期的整理掉（整体重写）
fn append_direct_log(path: &Path, line: &DirectLogLine, now: i64) -> std::io::Result<()> {
    let text = serde_json::to_string(line).unwrap_or_default();
    if let Ok(old) = fs::read_to_string(path) {
        let stale = |l: &str| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v["at"].as_i64())
                .is_some_and(|at| now.saturating_sub(at) >= DIRECT_LOG_KEEP_S)
        };
        if old.lines().next().is_some_and(stale) {
            let mut keep: String = old
                .lines()
                .filter(|l| !stale(l))
                .flat_map(|l| [l, "\n"])
                .collect();
            keep.push_str(&text);
            keep.push('\n');
            return write_atomic(path, keep.as_bytes());
        }
    }
    let mut f = open_append(path)?;
    writeln!(f, "{text}")
}

// ───────────────────────── 给人看的文字 ─────────────────────────

/// 把会改写终端显示、读屏也不读的字符换成可见的 `\u{…}`：控制字符（换行、制表符除外）与 [`invisible`] 列的
/// 全部看不见的字符（零宽、双向控制、标签字符、变体选择符……；`actions.md` 第一节第 4 小节第 4 条）
pub fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if hides(c) {
            out.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// [`visible`] 会把几个字符换成 `\u{…}`（计划行里报给 AI：正文里藏了几个看不见的字符）
pub fn hidden_count(s: &str) -> usize {
    s.chars().filter(|&c| hides(c)).count()
}

fn hides(c: char) -> bool {
    c != '\n' && c != '\t' && (c.is_control() || invisible(c))
}

/// 看不见、读屏也不读、却能藏东西的字符：Unicode 的 Default_Ignorable_Code_Point 全集（Unicode 15.1
/// `DerivedCoreProperties.txt`），外加格式类（Cf）与行 / 段分隔符（Zl / Zp）里不在其中的那几个。
/// 包括零宽字符、双向控制、软连字符、蒙古文变体选择符、变体选择符（含补充平面）、标签字符（U+E0000–E007F，
/// 能藏一整段只有程序读得出的文字）。宁可多转（emoji 的 U+FE0F、U+200D 也会显示成 `\u{…}`），不可漏
/// （自审第 3 轮 P1：人按 Touch ID 时看到的必须就是要发出去的全文）
fn invisible(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x034F
            | 0x061C
            | 0x0600..=0x0605
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Plan {
        target: ChatId,
        digest: [u8; 32],
        intent: [u8; 32],
    }

    impl Gated for Plan {
        fn target(&self) -> ChatId {
            self.target
        }
        fn digest(&self) -> [u8; 32] {
            self.digest
        }
        fn intent(&self) -> [u8; 32] {
            self.intent
        }
        fn confirm_text(&self) -> String {
            "发给：收藏夹".into()
        }
        fn dedup(&self) -> bool {
            true
        }
        fn kind(&self) -> &'static str {
            "text"
        }
    }

    /// 不进防重复的计划（重发）：直发时不查、不写防重复记录，留痕照写
    struct Resend(Plan);
    impl Gated for Resend {
        fn target(&self) -> ChatId {
            self.0.target
        }
        fn digest(&self) -> [u8; 32] {
            self.0.digest
        }
        fn intent(&self) -> [u8; 32] {
            self.0.intent
        }
        fn confirm_text(&self) -> String {
            String::new()
        }
        fn dedup(&self) -> bool {
            false
        }
        fn kind(&self) -> &'static str {
            "resend"
        }
    }

    #[test]
    fn non_dedup_plans_skip_guard_but_keep_log() {
        let (g, _, dir) = gate(true, false, Ok(()));
        let p = Resend(plan(777, 4));
        for _ in 0..2 {
            let t = g.issue(&p, false).unwrap();
            let mode = g.verify(t, &p).unwrap();
            g.before_submit(mode, &p).unwrap();
            g.after_outcome(mode, &p, -1, Some(9), Outcome::Succeeded, None);
        }
        assert!(!dir.join(GUARD_FILE).exists());
        let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
        assert_eq!(log.lines().count(), 2);
        assert!(log.contains("\"kind\":\"resend\""));
    }

    #[test]
    fn mode_names_match_serde() {
        for m in [
            ConfirmMode::TestExempt,
            ConfirmMode::HumanAuth,
            ConfirmMode::DirectMode,
        ] {
            assert_eq!(
                serde_json::to_value(m).unwrap(),
                serde_json::json!(m.as_str())
            );
        }
    }

    fn plan(target: ChatId, d: u8) -> Plan {
        Plan {
            target,
            digest: [d; 32],
            intent: [d; 32],
        }
    }

    /// 记下被问了几次；按设定放行或拒
    struct CountingAuth(Arc<AtomicU32>, Result<(), SendRefusedReason>);

    impl Authenticator for CountingAuth {
        fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
            self.0.fetch_add(1, Ordering::SeqCst);
            self.1
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "atg-gate-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    const ME: Me = Me {
        user_id: 4242,
        saved_chat_id: 4242,
        test_number: true,
    };

    fn gate(
        direct: bool,
        test_dc: bool,
        auth: Result<(), SendRefusedReason>,
    ) -> (Gate, Arc<AtomicU32>, PathBuf) {
        let n = Arc::new(AtomicU32::new(0));
        let g = Gate::new(direct, Holder::Cli, Box::new(CountingAuth(n.clone(), auth)));
        let dir = temp_dir(&format!("{direct}{test_dc}"));
        g.set_server(test_dc, &dir, derive_guard_key(b"k", "t"));
        g.set_me(ME);
        (g, n, dir)
    }

    fn reason(e: CoreError) -> SendRefusedReason {
        match e {
            CoreError::SendRefused { reason, .. } => reason,
            other => panic!("不是 send_refused：{other:?}"),
        }
    }

    #[test]
    fn test_dc_saved_is_exempt_and_never_asks() {
        let (g, n, _) = gate(false, true, Err(SendRefusedReason::AuthFailed));
        let p = plan(ME.saved_chat_id, 1);
        let t = g.issue(&p, false).unwrap();
        assert_eq!(t.mode(), ConfirmMode::TestExempt);
        assert_eq!(g.verify(t, &p).unwrap(), ConfirmMode::TestExempt);
        assert_eq!(n.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_dc_other_chat_refused_even_in_direct_mode() {
        for direct in [false, true] {
            let (g, _, _) = gate(direct, true, Ok(()));
            let e = g.issue(&plan(777, 1), false).unwrap_err();
            assert_eq!(reason(e), SendRefusedReason::TestDcSavedOnly);
        }
    }

    #[test]
    fn test_dc_saved_needs_test_number() {
        let (g, _, _) = gate(false, true, Ok(()));
        g.set_me(Me {
            test_number: false,
            ..ME
        });
        let e = g.issue(&plan(ME.saved_chat_id, 1), false).unwrap_err();
        assert_eq!(reason(e), SendRefusedReason::TestDcSavedOnly);
    }

    #[test]
    fn main_server_saved_still_needs_human_auth() {
        let (g, n, _) = gate(false, false, Ok(()));
        let p = plan(ME.saved_chat_id, 1);
        let t = g.issue(&p, false).unwrap();
        assert_eq!(t.mode(), ConfirmMode::HumanAuth);
        assert_eq!(n.load(Ordering::SeqCst), 1);
        let (g, _, _) = gate(false, false, Err(SendRefusedReason::AuthFailed));
        assert_eq!(
            reason(g.issue(&p, false).unwrap_err()),
            SendRefusedReason::AuthFailed
        );
        let (g, _, _) = gate(false, false, Err(SendRefusedReason::NotInteractive));
        assert_eq!(
            reason(g.issue(&p, false).unwrap_err()),
            SendRefusedReason::NotInteractive
        );
    }

    #[test]
    fn token_binds_target_and_digest_once_within_ttl() {
        let (g, _, _) = gate(true, false, Ok(()));
        let p = plan(777, 1);
        // 目标不对
        let t = g.issue(&p, false).unwrap();
        assert_eq!(
            reason(g.verify(t, &plan(778, 1)).unwrap_err()),
            SendRefusedReason::TargetMismatch
        );
        // 内容不对
        let t = g.issue(&p, true).unwrap();
        assert_eq!(
            reason(g.verify(t, &plan(777, 2)).unwrap_err()),
            SendRefusedReason::ContentMismatch
        );
        // 过期
        let mut t = g.issue(&p, true).unwrap();
        t.age_by(TOKEN_TTL + Duration::from_secs(1));
        assert_eq!(
            reason(g.verify(t, &p).unwrap_err()),
            SendRefusedReason::TokenExpired
        );
        // 用过：同一个随机数第二次（凭据本身不可复制，只能在 crate 内伪造来测）
        let t = g.issue(&p, true).unwrap();
        let nonce = t.nonce;
        assert!(g.verify(t, &p).is_ok());
        let forged = ConfirmToken {
            target: 777,
            digest: [1; 32],
            nonce,
            issued: Instant::now(),
            mode: ConfirmMode::DirectMode,
        };
        assert_eq!(
            reason(g.verify(forged, &p).unwrap_err()),
            SendRefusedReason::TokenReused
        );
    }

    #[test]
    fn direct_mode_blocks_duplicate_until_again_or_failure() {
        let (g, n, dir) = gate(true, false, Ok(()));
        let p = plan(777, 9);
        let t = g.issue(&p, false).unwrap();
        let mode = g.verify(t, &p).unwrap();
        g.before_submit(mode, &p).unwrap();
        // 还没结果（进程中途死掉也是这样）：拦
        let e = g.issue(&p, false).unwrap_err();
        match &e {
            CoreError::SendRefused {
                reason: SendRefusedReason::DuplicateRecent,
                last: Some(l),
            } => assert_eq!(l.outcome, Outcome::Submitted),
            other => panic!("{other:?}"),
        }
        g.after_outcome(mode, &p, -1, Some(5), Outcome::Succeeded, None);
        // 已发出：拦；--again 放行
        assert_eq!(
            reason(g.issue(&p, false).unwrap_err()),
            SendRefusedReason::DuplicateRecent
        );
        assert!(g.issue(&p, true).is_ok());
        // 别的会话、别的内容不受影响
        assert!(g.issue(&plan(778, 9), false).is_ok());
        assert!(g.issue(&plan(777, 8), false).is_ok());
        // 直发从不弹框
        assert_eq!(n.load(Ordering::SeqCst), 0);
        // 记录里没有内容：只有时间、会话、指纹、状态；权限 0600
        let raw = fs::read_to_string(dir.join(GUARD_FILE)).unwrap();
        let v: Vec<GuardEntry> = serde_json::from_str(&raw).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].state, Outcome::Succeeded);
        use std::os::unix::fs::PermissionsExt;
        let mode_bits = fs::metadata(dir.join(GUARD_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode_bits & 0o777, 0o600);
        let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
        assert_eq!(log.lines().count(), 1);
        assert!(log.contains("\"result\":\"succeeded\""));

        // 明确失败的不拦
        let p2 = plan(777, 7);
        let t = g.issue(&p2, false).unwrap();
        let mode = g.verify(t, &p2).unwrap();
        g.before_submit(mode, &p2).unwrap();
        g.after_outcome(mode, &p2, -2, Some(-3), Outcome::Failed, None);
        assert!(g.issue(&p2, false).is_ok());
    }

    #[test]
    fn duplicate_record_expires_after_window() {
        let dir = temp_dir("expire");
        let s = GuardStore::new(dir.join(GUARD_FILE));
        s.record(1000, 777, "ab", Outcome::Succeeded).unwrap();
        assert!(s.hit(1000 + DUP_WINDOW_S - 1, 777, "ab").unwrap().is_some());
        assert!(s.hit(1000 + DUP_WINDOW_S, 777, "ab").unwrap().is_none());
        // 结果未知的拦 24 小时：消息可能还在 TDLib 的待发队列里，下次启动会续发
        s.record(2000, 778, "ab", Outcome::Unknown).unwrap();
        assert!(s.hit(2000 + DUP_WINDOW_S + 1, 778, "ab").unwrap().is_some());
        assert!(
            s.hit(2000 + UNKNOWN_HOLD_S - 1, 778, "ab")
                .unwrap()
                .is_some()
        );
        assert!(s.hit(2000 + UNKNOWN_HOLD_S, 778, "ab").unwrap().is_none());
        // 写新的时把过期的清掉
        s.record(2000 + UNKNOWN_HOLD_S + 5, 777, "cd", Outcome::Submitted)
            .unwrap();
        assert_eq!(s.load().unwrap().len(), 1);
    }

    #[test]
    fn fingerprint_depends_on_key_and_hides_content() {
        let a = derive_guard_key(b"key-a", "main");
        let b = derive_guard_key(b"key-b", "main");
        let c = derive_guard_key(b"key-a", "test");
        let i = [3u8; 32];
        assert_ne!(fingerprint(&a, 1, &i), fingerprint(&b, 1, &i));
        assert_ne!(fingerprint(&a, 1, &i), fingerprint(&c, 1, &i));
        assert_ne!(fingerprint(&a, 1, &i), fingerprint(&a, 2, &i));
        // 派生出的钥匙不是库钥匙本身
        assert_ne!(a.expose(), b"key-a");
    }

    #[test]
    fn account_limited_stops_everything() {
        let (g, _, _) = gate(true, false, Ok(()));
        let p = plan(777, 1);
        g.after_outcome(
            ConfirmMode::DirectMode,
            &p,
            -1,
            None,
            Outcome::Failed,
            Some(&CoreError::AccountLimited { td: None }),
        );
        assert_eq!(
            reason(g.issue(&plan(778, 2), false).unwrap_err()),
            SendRefusedReason::AccountLimited
        );
    }

    #[test]
    fn account_limited_survives_into_the_next_process_until_removed() {
        let (g, _, dir) = gate(true, false, Ok(()));
        g.after_outcome(
            ConfirmMode::DirectMode,
            &plan(777, 1),
            -1,
            None,
            Outcome::Failed,
            Some(&CoreError::AccountLimited { td: None }),
        );
        assert!(dir.join(ACCOUNT_LIMITED_FILE).exists());
        // 下一个进程（新的闸）开在同一个档案目录上：照样停
        let next = Gate::new(true, Holder::Cli, Box::new(NoAuth));
        next.set_server(false, &dir, derive_guard_key(b"k", "t"));
        next.set_me(ME);
        assert_eq!(
            reason(next.issue(&plan(778, 2), false).unwrap_err()),
            SendRefusedReason::AccountLimited
        );
        // 人处理完、删掉标记：再开就恢复
        fs::remove_file(dir.join(ACCOUNT_LIMITED_FILE)).unwrap();
        let after = Gate::new(true, Holder::Cli, Box::new(NoAuth));
        after.set_server(false, &dir, derive_guard_key(b"k", "t"));
        after.set_me(ME);
        assert!(after.issue(&plan(778, 2), false).is_ok());
    }

    #[test]
    fn direct_sent_counts_handed_over_sends() {
        let (g, _, _) = gate(true, false, Ok(()));
        let p = plan(777, 3);
        assert_eq!(g.direct_sent(), 0);
        let t = g.issue(&p, false).unwrap();
        let mode = g.verify(t, &p).unwrap();
        g.before_submit(mode, &p).unwrap();
        assert_eq!(g.direct_sent(), 1);
    }

    #[test]
    fn visible_escapes_tags_separators_and_variation_selectors() {
        // 标签字符藏的一段话、行分隔符、蒙古文元音分隔符、软连字符、变体选择符（含补充平面）
        assert_eq!(
            visible("好的\u{E0041}\u{E0042}"),
            "好的\\u{E0041}\\u{E0042}"
        );
        assert_eq!(
            visible("a\u{2028}b\u{180E}c\u{00AD}d"),
            "a\\u{2028}b\\u{180E}c\\u{00AD}d"
        );
        assert_eq!(visible("x\u{FE0F}\u{E0100}"), "x\\u{FE0F}\\u{E0100}");
        // 普通中文、标点、emoji 本身不动
        assert_eq!(visible("好的！👍"), "好的！👍");
        assert_eq!(hidden_count("好的\u{E0041}\u{200B}\n"), 2);
    }

    #[test]
    fn visible_escapes_bidi_zero_width_and_controls() {
        assert_eq!(visible("a\u{202E}b"), "a\\u{202E}b");
        assert_eq!(visible("x\u{200B}y\u{1b}[31m"), "x\\u{200B}y\\u{001B}[31m");
        assert_eq!(visible("第一行\n第二行"), "第一行\n第二行");
    }
}
