//! 闸本体 [`Gate`]：开凭据、交出前复核、记账与直发留痕。规则与威胁边界见模块根 `gate.rs` 的头注释。

use super::*;

// ───────────────────────── 闸 ─────────────────────────

#[derive(Default)]
struct Facts {
    /// 本进程实际传给 TDLib 的 `use_test_dc`（后端起来时设）
    use_test_dc: Option<bool>,
    me: Option<Me>,
    /// 档案目录（防重复记录、直发留痕都放这里）
    dir: Option<PathBuf>,
    guard_key: Option<Secret>,
}

/// 发送闸。每个后端持有一个；CLI / GUI 经 `Backend::gate` 拿到它开凭据
pub struct Gate {
    proc: Holder,
    used: Mutex<HashSet<u64>>,
    /// 验过的凭据带没带 `--again`，按计划摘要记，交出前查重时取（#77）。每次验凭据都覆盖：
    /// 同一份计划后来又开了一张不带的，不沿用前一张的放行
    again: Mutex<HashMap<[u8; 32], bool>>,
    facts: Mutex<Facts>,
    direct_sent: std::sync::atomic::AtomicU64,
    /// 账号受限、限流冷却（与收线程共用一份，见 [`crate::send_limits`]）
    limits: std::sync::Arc<crate::send_limits::SendLimits>,
}

impl Gate {
    /// 正式服务器一律直发（Touch ID 确认已去掉，人定 2026-11-16）
    pub fn new(proc: Holder) -> Self {
        Self {
            proc,
            used: Mutex::new(HashSet::new()),
            again: Mutex::new(HashMap::new()),
            facts: Mutex::new(Facts::default()),
            direct_sent: std::sync::atomic::AtomicU64::new(0),
            limits: crate::send_limits::SendLimits::new(),
        }
    }

    /// 交给收线程的那份发送限制（后端建好时 `Shared::watch_send_limits`）
    pub fn limits(&self) -> std::sync::Arc<crate::send_limits::SendLimits> {
        self.limits.clone()
    }

    /// 后端起来时：实际的 `use_test_dc`、档案目录、指纹密钥
    pub(crate) fn set_server(&self, use_test_dc: bool, dir: &Path, guard_key: Secret) {
        let mut f = lock(&self.facts);
        f.use_test_dc = Some(use_test_dc);
        f.dir = Some(dir.to_path_buf());
        f.guard_key = Some(guard_key);
        // 上一个进程记下的受限、限流：这一个照样遵守（CLI 每条命令都是新进程）
        self.limits.load(dir);
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
        self.limits.check(unix_now())?;
        let f = lock(&self.facts);
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
        Ok(ConfirmMode::DirectMode)
    }

    /// 开凭据。直发先查防重复（`again` 放行这一次）：这里只为早拒、人少等；以交出前 [`Self::before_submit`] 那次为准（#77）
    pub fn issue(&self, plan: &dyn Gated, again: bool) -> Result<ConfirmToken, CoreError> {
        let mode = self.mode_for(plan.target())?;
        match mode {
            ConfirmMode::TestExempt => {}
            ConfirmMode::DirectMode => {
                if plan.dedup() && !again {
                    self.check_duplicate(plan)?;
                }
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
            again,
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
        {
            let mut again = lock(&self.again);
            // 验过却没走到交出（复核被拒、超时……）的会留下；攒多了整个清掉。清掉的只会让那次交出按不带
            // `--again` 查重：宁可多拦
            if again.len() >= AGAIN_KEEP {
                again.clear();
            }
            again.insert(token.digest, token.again);
        }
        Ok(token.mode)
    }

    /// 交给 TDLib 之前：直发要先把「已提交」记下、留痕文件要写得进去，否则拒发
    pub(crate) fn before_submit(
        &self,
        mode: ConfirmMode,
        plan: &dyn Gated,
    ) -> Result<(), CoreError> {
        // 交出前最后一次复查限制：所有 await 之后的唯一位置（codex 016 第 5 轮 #1，见 `send_limits` 模块文档）
        self.limits.check(unix_now())?;
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
        let now = unix_now();
        // 查重与登记一步做完（#77）：凭据开出到交出之间别的同内容发送可能已经交出了。攥着 facts 锁做，
        // 进程内不交错；跨进程靠档案锁（同一档案同时只有一个进程）。命中就拒，什么都不写
        let guard = plan.dedup().then(|| {
            (
                GuardStore::new(dir.join(GUARD_FILE)),
                fingerprint(key, plan.target(), &plan.intent()),
            )
        });
        if let Some((store, fp)) = &guard {
            let again = lock(&self.again).remove(&plan.digest()).unwrap_or(false);
            let claimed =
                store
                    .claim(now, plan.target(), fp, again)
                    .map_err(|_| CoreError::Internal {
                        what: "send_guard_unwritable",
                    })?;
            if let Err(e) = claimed {
                return Err(duplicate(now, &e));
            }
        }
        // 留痕是条件：交出前先真写一行「已提交」（只打开文件不够：磁盘满时照样打得开，会先发后写不进去，
        // codex 016 第 1 轮 #5）。等到结果后再写一行结果。写不进去就拒，刚登记的「已提交」改成失败、不拦重跑
        let lines = self.log_lines(plan, now, 0, None, Outcome::Submitted, None);
        if append_direct_log(&dir.join(DIRECT_LOG_FILE), &lines, now).is_err() {
            if let Some((store, fp)) = &guard {
                let _ = store.settle(now, plan.target(), fp, Outcome::Failed);
            }
            return Err(CoreError::Internal {
                what: "direct_log_unwritable",
            });
        }
        // 前置条件都过了，接下来就交给 TDLib：算一条直发
        self.direct_sent
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// 等到结果后：直发改防重复记录、写一行留痕；受限 / 限流记进 [`crate::send_limits`]
    pub fn after_outcome(
        &self,
        mode: ConfirmMode,
        plan: &dyn Gated,
        temp_id: i64,
        final_id: Option<i64>,
        outcome: Outcome,
        error: Option<&CoreError>,
    ) {
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
        // 批量的计划没拿到临时 id（请求整个失败 / 到点）：每条各记一行（#83）
        let lines = self.log_lines(
            plan,
            now,
            temp_id,
            final_id,
            outcome,
            error.map(CoreError::code),
        );
        if append_direct_log(&dir.join(DIRECT_LOG_FILE), &lines, now).is_err() {
            logline::warn("direct_log_write_failed", &[]);
        }
    }

    /// 批量计划里一条的结果（#83）：`old_id` 是它重发前的 id；TDLib 说不能重发的那条记 `failed`、
    /// `error:"not_resendable"`。只写留痕（批量重发不进防重复）
    #[allow(clippy::too_many_arguments)]
    pub fn after_item(
        &self,
        mode: ConfirmMode,
        plan: &dyn Gated,
        old_id: i64,
        temp_id: i64,
        final_id: Option<i64>,
        outcome: Outcome,
        error: Option<&'static str>,
    ) {
        if mode != ConfirmMode::DirectMode {
            return;
        }
        let f = lock(&self.facts);
        let Some(dir) = f.dir.as_ref() else {
            return;
        };
        let now = unix_now();
        let mut line = self.log_line(plan, now, temp_id, final_id, outcome, error);
        line.old_id = Some(old_id);
        if append_direct_log(&dir.join(DIRECT_LOG_FILE), &[line], now).is_err() {
            logline::warn("direct_log_write_failed", &[]);
        }
    }

    fn log_line(
        &self,
        plan: &dyn Gated,
        at: i64,
        temp_id: i64,
        final_id: Option<i64>,
        result: Outcome,
        error: Option<&'static str>,
    ) -> DirectLogLine {
        DirectLogLine {
            at,
            proc: self.proc,
            pid: std::process::id(),
            chat_id: plan.target(),
            temp_id,
            final_id,
            kind: plan.kind(),
            result,
            error,
            reaction: plan.log_target(),
            old_id: None,
        }
    }

    /// 一次记账要写的几行：单条计划一行；批量计划（[`Gated::log_items`] 非空）且没有临时 id 时每条一行
    fn log_lines(
        &self,
        plan: &dyn Gated,
        at: i64,
        temp_id: i64,
        final_id: Option<i64>,
        result: Outcome,
        error: Option<&'static str>,
    ) -> Vec<DirectLogLine> {
        let items = plan.log_items();
        if items.is_empty() || temp_id != 0 {
            return vec![self.log_line(plan, at, temp_id, final_id, result, error)];
        }
        items
            .into_iter()
            .map(|old| {
                let mut l = self.log_line(plan, at, temp_id, final_id, result, error);
                l.old_id = Some(old);
                l
            })
            .collect()
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
            Some(e) => Err(duplicate(now, &e)),
        }
    }
}

/// 防重复命中：说明写清几秒前、什么结果
fn duplicate(now: i64, e: &GuardEntry) -> CoreError {
    CoreError::SendRefused {
        reason: SendRefusedReason::DuplicateRecent,
        last: Some(crate::error::LastSend {
            at: e.at,
            ago_s: now.saturating_sub(e.at).max(0),
            outcome: e.state,
        }),
    }
}

/// [`Gate`] 的 `again` 表最多留几条（见 `verify`）
const AGAIN_KEEP: usize = 256;

fn refused(reason: SendRefusedReason) -> CoreError {
    CoreError::SendRefused { reason, last: None }
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
    /// 表情回应动在哪条消息上、按最坏情况可能挤掉几个原先的回应（[`Gated::log_target`]；拿不准记 `null`）
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "reaction_fields"
    )]
    reaction: Option<(i64, Option<usize>)>,
    /// 批量重发里这一行说的是哪条（重发前的旧 id；[`Gated::log_items`]，#83）
    #[serde(skip_serializing_if = "Option::is_none")]
    old_id: Option<i64>,
}

fn reaction_fields<S: serde::Serializer>(
    t: &Option<(i64, Option<usize>)>,
    s: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut m = s.serialize_map(Some(2))?;
    if let Some((id, n)) = t {
        m.serialize_entry("message_id", id)?;
        m.serialize_entry("may_replace", n)?;
    }
    m.end()
}

/// 追加几行（一次写进去）；文件头一行已超过 30 天时先把过期的整理掉（整体重写）
fn append_direct_log(path: &Path, lines: &[DirectLogLine], now: i64) -> std::io::Result<()> {
    let text = lines
        .iter()
        .map(|l| serde_json::to_string(l).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
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
