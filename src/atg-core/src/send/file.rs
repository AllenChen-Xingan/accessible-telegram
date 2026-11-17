//! 发文件 / 图片（#12，契约 `actions.md` `send_file_prepare` / `send_file`）：准备（[`prepare_file`]）→ 交出 → 等结果
//! （[`send_file_and_wait`]）。骨架同发文字（`send/text.rs`），多三件事：
//! 1. **许发哪些文件**（人定 2026-11-17）：只许「下载」「桌面」「文稿」里的（按解析符号链接后的真实路径），文件名像密钥的、
//!    本应用数据目录里的一律不许 → `file_not_allowed`；超 50 MB → `file_too_large`（人定 2026-11-17）。见 `file/local.rs`
//! 2. **交出的是快照**：准备时复制一份到档案目录 `outbox/<随机>/<原名>` 并算 SHA-256，摘要绑它；交出前重算，变了拒
//!    （`content_mismatch`）。原文件在确认后被换、被改都不影响发出去的内容。快照在成功、失败且不能重发后删；结果未知的
//!    （TDLib 下次启动续发要读它）、失败但能重发的（`resend` 要读它）留着，过 24 小时清
//! 3. **上传要等得久**（人定 2026-11-17）：有进度就接着等；连续 60 秒没进度按结果未知；总共最多 10 分钟（[`wait_upload`]）
//!
//! 按图片还是按文件发（人定 2026-11-17）：`jpg` `jpeg` `png` `webp` 按图片（会被压缩），按图片发不了的（超 10 MB、太大、太窄、
//! 读不出宽高）退回按文件发、计划行写明；其余按文件发。
//! 防重复：进；指纹 = 目标 + 文件 SHA-256 + 说明 + 回复（**不含路径**：同一文件换个地方再发也拦）。
//! 留痕：内容类型与字节数（[`Gated::log_file`]），不记路径、文件名、说明文字。

use super::text::{send_topic, text_request};
use super::*;
use crate::backend::Upload;

mod local;

pub use local::{ALLOWED_DIRS, FILE_MAX_BYTES, FileRoots, OUTBOX_KEEP_S, SendAs};

/// 说明文字最多几个字符（UTF-16 码元，Telegram 的算法；普通号的 `message_caption_length_max`）
pub const CAPTION_MAX: usize = 1024;

/// 连续多久没有上传进度就按结果未知（人定 2026-11-17）
pub const UPLOAD_IDLE: Duration = Duration::from_secs(60);
/// 交出后最多等多久（人定 2026-11-17）
pub const UPLOAD_CAP: Duration = Duration::from_secs(600);

/// 要发什么文件
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct FileSource {
    pub path: std::path::PathBuf,
    /// 说明文字（可空）
    pub caption: String,
    /// 回复同一会话里的哪条
    pub reply_to: Option<i64>,
}

/// 一份已定稿的文件发送。快照在它被丢掉时删（[`Drop`]），交出后结果未知、失败但能重发的除外（[`FilePlan::keep_snapshot`]）
#[cfg_attr(test, derive(Debug))]
pub struct FilePlan {
    target: ChatTarget,
    name: String,
    size: u64,
    sha256: [u8; 32],
    send_as: SendAs,
    caption: String,
    reply_to: Option<MsgId>,
    topic: Option<Topic>,
    snapshot_dir: std::path::PathBuf,
    snapshot: std::path::PathBuf,
    request: Value,
    digest: [u8; 32],
    intent: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
    keep: AtomicBool,
}

impl FilePlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    /// 原文件名（对方看到的也是它）
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn send_as(&self) -> SendAs {
        self.send_as
    }
    pub fn caption(&self) -> &str {
        &self.caption
    }
    pub fn reply_to(&self) -> Option<MsgId> {
        self.reply_to
    }
    pub fn topic(&self) -> Option<&Topic> {
        self.topic.as_ref()
    }
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    pub fn digest_hex(&self) -> String {
        gate::hex(&self.digest)
    }
    /// SHA-256 的前 8 个十六进制字符（给人核对是哪个文件）
    pub fn sha256_short(&self) -> String {
        gate::hex(&self.sha256)[..8].to_owned()
    }
    /// 交出后结果未知（TDLib 续发要读）、失败但能重发（`resend` 要读）：快照留着，不随计划删
    fn keep_snapshot(&self) {
        self.keep.store(true, Ordering::SeqCst);
    }
    #[cfg(test)]
    pub(crate) fn snapshot_path(&self) -> &std::path::Path {
        &self.snapshot
    }
}

impl Drop for FilePlan {
    fn drop(&mut self) {
        if !self.keep.load(Ordering::SeqCst) {
            let _ = std::fs::remove_dir_all(&self.snapshot_dir);
        }
    }
}

impl gate::Sealed for FilePlan {}
impl Gated for FilePlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.intent
    }
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "file"
    }
    fn log_file(&self) -> Option<(&'static str, u64)> {
        Some((self.send_as.kind(), self.size))
    }
}

fn caption_json(caption: &str) -> Value {
    json!({"@type": "formattedText", "text": caption, "entities": []})
}

/// 组 `sendMessage` 的请求：同发文字，内容换成本地文件（快照路径）
fn file_request(
    chat: ChatId,
    send_as: SendAs,
    path: &str,
    caption: &str,
    reply_to: Option<i64>,
    topic: Option<&Topic>,
    sending_id: i32,
) -> Value {
    let mut r = text_request(chat, "", reply_to, topic, sending_id);
    let file = json!({"@type": "inputFileLocal", "path": path});
    r["input_message_content"] = match send_as {
        SendAs::Photo { width, height } => json!({
            "@type": "inputMessagePhoto",
            "photo": {
                "@type": "inputPhoto",
                "photo": file,
                "thumbnail": null,
                "video": null,
                "added_sticker_file_ids": [],
                "width": width,
                "height": height,
            },
            "caption": caption_json(caption),
            "show_caption_above_media": false,
            "self_destruct_type": null,
            "has_spoiler": false,
        }),
        SendAs::Document { .. } => json!({
            "@type": "inputMessageDocument",
            "document": {
                "@type": "inputDocument",
                "document": file,
                "thumbnail": null,
                "disable_content_type_detection": false,
            },
            "caption": caption_json(caption),
        }),
    };
    r
}

/// 摘要与防重复都绑文件内容（SHA-256）；防重复那份再把每次都变的 `sending_id` 与快照路径清掉
fn digests(
    request: &Value,
    sha: &[u8; 32],
    me: crate::model::UserId,
    server: ServerKind,
) -> ([u8; 32], [u8; 32]) {
    let sha = gate::hex(sha);
    let digest = gate::digest(&json!({"request": request, "sha256": sha}), me, server);
    // 防重复只看「发给谁、哪个文件内容、说什么、回复哪条」：不装请求的媒体结构——同一个文件改个扩展名，
    // 按图片还是按文件发就变了，请求形状跟着变，但还是同一意图（codex 034 第 1 轮 #2）；路径与 `sending_id` 也不装
    let intent = gate::digest(
        &json!({
            "@type": "atg.send_file.intent.v1",
            "chat_id": request["chat_id"],
            "topic_id": request["topic_id"],
            "reply_to": request["reply_to"],
            "caption": request["input_message_content"]["caption"]["text"],
            "sha256": sha,
        }),
        me,
        server,
    );
    (digest, intent)
}

/// 只看本机的检查（说明长度、许不许发、大小），不碰后端：调用方在查会话之前先调它，会话不存在、网络不通时也先报文件的问题
/// （[`prepare_file`] 里还会再查一次，那次的结果才算数）
pub fn precheck(src: &FileSource, roots: &FileRoots) -> Result<(), CoreError> {
    check_caption(&src.caption)?;
    local::allowed(&src.path, roots).map(drop)
}

fn check_caption(caption: &str) -> Result<(), CoreError> {
    if caption.encode_utf16().count() > CAPTION_MAX {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::CaptionTooLong,
        });
    }
    Ok(())
}

/// `send_file_prepare`：只查不发——会话、回复、话题规则同发文字；再判文件许不许发、复制快照、定按图片还是按文件发
pub async fn prepare_file<B: Backend>(
    b: &B,
    chat: ChatId,
    src: &FileSource,
    roots: &FileRoots,
    origin: SendOrigin,
) -> Result<FilePlan, CoreError> {
    check_caption(&src.caption)?;
    // 只看本机就能拒的先拒（路径、大小）：会话不存在、网络不通时也先报文件的问题
    let file = local::allowed(&src.path, roots)?;
    let me = b.me().await?;
    let mut target = b.chat_target(chat).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let (reply_to, topic) = match src.reply_to {
        None => (None, None),
        Some(id) => {
            let mid = MsgId {
                chat_id: target.id,
                message_id: id,
            };
            let m = b.message_get(mid).await?;
            if !b.can_reply(mid).await? {
                return Err(CoreError::InvalidSend {
                    why: InvalidSendWhy::ReplyNotAllowed,
                });
            }
            (Some(mid), send_topic(m.topic.as_ref()))
        }
    };
    // 话题群里不回复任何一条：同发文字，第一阶段拒发
    if target.is_forum && reply_to.is_none() {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::ForumNeedsReply,
        });
    }
    let sending_id = super::text::random_sending_id()?;
    let snap = local::snapshot(file, &roots.outbox)?;
    // 从这里起出错要删快照：先装进计划，交给 Drop
    let send_as = local::send_as(&snap);
    let request = file_request(
        target.id,
        send_as,
        &snap.path.to_string_lossy(),
        &src.caption,
        reply_to.map(|r| r.message_id),
        topic.as_ref(),
        sending_id,
    );
    let mut plan = FilePlan {
        target,
        name: snap.name,
        size: snap.size,
        sha256: snap.sha256,
        send_as,
        caption: src.caption.clone(),
        reply_to,
        topic,
        snapshot_dir: snap.dir,
        snapshot: snap.path,
        request,
        digest: [0; 32],
        intent: [0; 32],
        origin,
        mode,
        keep: AtomicBool::new(false),
    };
    let server = server_of(b)?;
    (plan.digest, plan.intent) = digests(&plan.request, &plan.sha256, me.user_id, server);
    Ok(plan)
}

/// 交出：验凭据 → 被回复那条还在、还能回复 → 快照没被换 → 直发先记账 → 交给 TDLib
async fn hand_over<B: Backend>(
    b: &B,
    plan: &FilePlan,
    token: ConfirmToken,
    handed: &AtomicBool,
) -> Result<Submitted, CoreError> {
    let (mode, again) = b.gate().verify_again(token, plan)?;
    if let Some(r) = plan.reply_to {
        b.message_get(r).await?;
        if !b.can_reply(r).await? {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::ReplyNotAllowed,
            });
        }
    }
    if local::sha256_of(&plan.snapshot) != Some(plan.sha256) {
        return Err(CoreError::SendRefused {
            reason: SendRefusedReason::ContentMismatch,
            last: None,
        });
    }
    // 交出前的复核到此为止（同 `text::hand_over` 的说明：此后到交给 TDLib 之间没有 await）
    handed.store(true, Ordering::SeqCst);
    b.gate().before_submit_again(mode, plan, again)?;
    let sealed = Sealed {
        kind: SealedKind::Message {
            request: &plan.request,
            preview: false,
        },
    };
    match crate::send_limits::submit(b, sealed).await {
        Ok(Answer::Message(m)) => Ok(Submitted {
            mode,
            temp: m.id,
            old: None,
        }),
        Ok(Answer::Resent(_) | Answer::Done | Answer::Deleted) => Err(CoreError::Internal {
            what: "send_answer_shape",
        }),
        Err(e) => Err(submit_failed(b, mode, plan, e)),
    }
}

/// 发文件的全程：交出（最多 `hand_timeout`）→ 按上传进度等结果（[`wait_upload`]）→ 记账。结果未知时快照留着
#[allow(clippy::too_many_arguments)]
pub fn send_file_and_wait<B: Backend>(
    b: &B,
    plan: &FilePlan,
    token: ConfirmToken,
    stream: &mut EventStream,
    hand_timeout: Duration,
    idle: Duration,
    cap: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Result<(ConfirmMode, MsgId, Waited), CoreError> {
    let handed = AtomicBool::new(false);
    let sub = match submit_within(
        b,
        plan,
        plan.mode,
        hand_timeout,
        &handed,
        hand_over(b, plan, token, &handed),
    ) {
        Ok(s) => s,
        Err(e) => {
            if matches!(e, CoreError::SendOutcomeUnknown { .. }) {
                plan.keep_snapshot();
            }
            return Err(e);
        }
    };
    let (mode, temp) = (sub.mode, sub.temp);
    let w = wait_upload(b, stream, temp, idle, cap, on_event);
    // 结果未知（TDLib 续发要读）、失败但能重发（`resend` 要读）：快照留着
    if matches!(
        w,
        Waited::Unknown(_)
            | Waited::Failed {
                can_retry: true,
                ..
            }
    ) {
        plan.keep_snapshot();
    }
    finish(b, plan, sub, &w);
    Ok((mode, temp, w))
}

/// 等一条带文件的待发消息：结果事件同 [`wait`]；另隔一会儿问一次上传进度，有进度就把「没动静」的计时清零。
/// 连续 `idle` 没进度、或总共过了 `cap` → 结果未知（`timeout`）
pub fn wait_upload<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    temp: MsgId,
    idle: Duration,
    cap: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Waited {
    // 绝对截止时刻：事件等待与进度查询共用剩下的时间，不各算一份（codex 034 第 1 轮 #3）
    let cap_at = Instant::now() + cap;
    let mut idle_at = Instant::now() + idle;
    let mut last: Option<Upload> = None;
    let poll = (idle / 10).clamp(Duration::from_millis(10), Duration::from_secs(2));
    loop {
        let deadline = cap_at.min(idle_at);
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Waited::Unknown(UnknownWhy::Timeout);
        }
        match wait(stream, &[temp], left.min(poll), on_event).pop() {
            None | Some(Waited::Unknown(UnknownWhy::Timeout)) => {}
            Some(w) => return w,
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            continue;
        }
        let Some(Ok(Some(u))) = block_on_timeout(b.upload_progress(temp), left) else {
            continue;
        };
        // 查询回来时已过了截止：这次的进度不算，不再续等
        if Instant::now() >= deadline {
            continue;
        }
        let progressed = match last {
            None => u.uploaded > 0 || u.done,
            Some(l) => u.uploaded > l.uploaded || (u.done && !l.done),
        };
        if progressed {
            idle_at = Instant::now() + idle;
        }
        last = Some(u);
    }
}

/// 计划行（`kind:"plan"` 的 `data`，`actions.md` 第一节第 6 小节 `FilePlan`）。不带路径（快照路径、原路径都不给）
pub fn file_plan_json(plan: &FilePlan) -> Value {
    let (width, height, fallback) = match plan.send_as {
        SendAs::Photo { width, height } => (Some(width), Some(height), None),
        SendAs::Document { fallback } => (None, None, fallback),
    };
    json!({
        "action": "send_file",
        "target": target_json(&plan.target),
        "file_name": plan.name,
        "size": plan.size,
        "sha256_8": plan.sha256_short(),
        "send_as": plan.send_as.kind(),
        "photo_fallback": fallback,
        "width": width,
        "height": height,
        "caption": plan.caption,
        "hidden_chars": gate::hidden_count(&plan.caption) + gate::hidden_count(&plan.name),
        "reply_to": plan.reply_to,
        "topic": plan.topic,
        "origin": plan.origin,
        "confirm": plan.mode,
        "digest": plan.digest_hex(),
        "note_zh": QUEUE_NOTE_ZH,
    })
}

/// 退回按文件发的原因，说给人听
pub fn fallback_zh(why: &str) -> &'static str {
    match why {
        "photo_too_large" => "图片超过 10 MB",
        "photo_too_big" => "图片宽加高超过 10000 像素",
        "photo_too_narrow" => "图片太窄或太长（长宽比超过 20）",
        "photo_unreadable" => "读不出图片的宽高",
        _ => "按图片发不了",
    }
}

#[cfg(test)]
mod tests;
