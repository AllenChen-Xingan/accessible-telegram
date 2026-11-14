//! 语音的组合动作（#4、#59）：找到这条语音 → 下载 → 解码、放（以后：转写）。
//!
//! 落层（arch 定 2026-11-13，arch 分支 `docs/plan/010_complexity-budget.md` 第四节）：这里是**库代码**——吃
//! `Backend` 和参数，吐结果；不碰 clap、不写标准输出，渲染交给调用方。以后 GUI 链接 `atg-cli` 的库目标直接用。
//! 设计：`docs/experiments/exp008-local-stt/design.md`。
//!
//! 下载是按需的（大总管定 2026-11-13）：不自动下，`download` / `play` / `transcribe` 要用时才下。

use crate::chats::{self, Feed};
use crate::deadlines::{DOWNLOAD_TIMEOUT, QUERY_TIMEOUT, bounded, within};
use crate::history::{self, ChatRef};
use crate::out::Fail;
use atg_audio::player::Output;
use atg_audio::{AudioError, Player};
use atg_core::backend::{Backend, Downloaded};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::model::{Body, MsgId};
use atg_core::tree::{Attachment, Content, FileRef};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// 要哪条语音（命令行参数解析之后的样子）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceQuery {
    pub chat: ChatRef,
    pub message_id: i64,
}

/// 下好的语音：身份、时长（秒，TDLib 给的）、本地文件、是不是阅后即焚
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceFile {
    pub id: MsgId,
    pub duration: u32,
    pub file: Downloaded,
    pub self_destruct: bool,
}

/// 命令行的「会话 + 消息 id」→ 查询（`download`、`play` 共用这一份）
pub fn query(m: &crate::args::MsgRef) -> Result<VoiceQuery, Fail> {
    Ok(VoiceQuery {
        chat: ChatRef::parse(&m.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?,
        message_id: m.message,
    })
}

/// 直跑：起后端、找到并下载这条语音、关后端
pub fn run_download<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &VoiceQuery,
    stop: &AtomicBool,
    started: Instant,
) -> Result<VoiceFile, Fail> {
    history::with_backend(b, profile, started, |b, stream, feed| {
        fetch(b, stream, feed, q, stop)
    })
}

/// 找到的语音（还没下载）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub id: MsgId,
    pub duration: u32,
    pub file: FileRef,
    pub self_destruct: bool,
}

/// 在已起好的后端上：取这条消息（`getMessage`，不标已读）、确认是语音。不是语音 → `invalid_input`
pub fn find<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
    stop: &AtomicBool,
) -> Result<Found, Fail> {
    chats::ensure_ready(b)?;
    let chat_id = history::resolve(b, stream, feed, q.chat)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: q.message_id,
    };
    let m =
        within(b, b.message_get(id), Instant::now() + QUERY_TIMEOUT, stop)?.map_err(Fail::Core)?;
    feed.drain(stream);
    // 有效内容（有临时内容就用它）；受限的存下来时已换成受限节点，不是语音
    match m.effective() {
        Body::Content(Content {
            attachment: Some(Attachment::Voice { duration, file, .. }),
            ..
        }) => Ok(Found {
            id,
            duration: *duration,
            file: *file,
            self_destruct: m.self_destruct,
        }),
        _ => Err(Fail::Core(CoreError::InvalidInput { td: None })),
    }
}

/// 下载找到的语音（同步等下完；只在下完时交路径）
pub fn download<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    f: Found,
    stop: &AtomicBool,
) -> Result<VoiceFile, Fail> {
    let file = within(
        b,
        b.file_download(f.file),
        Instant::now() + DOWNLOAD_TIMEOUT,
        stop,
    )?
    .map_err(Fail::Core)?;
    feed.drain(stream);
    Ok(VoiceFile {
        id: f.id,
        duration: f.duration,
        file,
        self_destruct: f.self_destruct,
    })
}

/// 找到并下载
pub fn fetch<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
    stop: &AtomicBool,
) -> Result<VoiceFile, Fail> {
    let f = find(b, stream, feed, q, stop)?;
    download(b, stream, feed, f, stop)
}

// ───────────────────────── 播放（M2.6） ─────────────────────────

/// `atg-audio` 的错误 → CLI 的失败。**这个映射只写这一处**（arch 定 2026-11-13）；码见 `AudioError::what`
pub fn audio_fail(e: AudioError) -> Fail {
    Fail::Core(CoreError::Internal { what: e.what() })
}

/// 播放前的准备（`design.md` 第三节第 5b 条），前一步失败后一步都不做：
/// 找到 →（要标而它是阅后即焚 → 拒绝，确认还没做）→ 下载 → 解码 → 出口**停着**打开 →（要标就）标已听。
/// 返回停着的播放器；调用方关掉后端后 `Cmd::Resume` 开始出声。
/// `cancel`（CLI 是 Ctrl+C 的标志）在每一步之间看，**尤其是标已听之前**：取消了就不标、不出声，报 `interrupted`
pub fn prepare_play<B: Backend, O: Output>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
    mark: bool,
    out: O,
    cancel: &AtomicBool,
) -> Result<(Player<O>, VoiceFile), Fail> {
    let f = after_step(cancel, find(b, stream, feed, q, cancel))?;
    if mark && f.self_destruct {
        return Err(Fail::NotImplemented {
            command: "play",
            message_zh: "阅后即焚的语音标已听会开始倒计时，要先确认（L2），确认还没做；不加 --mark-listened 可以照常听",
        });
    }
    let v = after_step(cancel, download(b, stream, feed, f, cancel))?;
    let pcm = Arc::new(after_step(
        cancel,
        atg_audio::decode(&v.file.path).map_err(audio_fail),
    )?);
    let player = after_step(cancel, Player::start_paused(out, pcm).map_err(audio_fail))?;
    if mark {
        // 失败时 player 随之释放出口，不出声（不说「已标」）
        // 等标记应答时按了 Ctrl+C、应答又失败：仍按 interrupted 报（同前几步）。已发出的标记不半途丢下等待
        // （改的是服务器上的状态），只按期限等，不看 Ctrl+C
        let r = bounded(b, b.mark_listened(v.id), QUERY_TIMEOUT).map_err(Fail::Core);
        after_step(cancel, r)?;
        feed.drain(stream);
    }
    Ok((player, v))
}

/// 每一步之后先看取消、再传播这一步的错误：取消期间某步失败，仍按 `interrupted` 报
fn after_step<T>(cancel: &AtomicBool, r: Result<T, Fail>) -> Result<T, Fail> {
    if cancel.load(Ordering::SeqCst) {
        Err(Fail::Interrupted)
    } else {
        r
    }
}

/// 直跑 `play` 的准备：起后端、准备好、关后端（声音不需要后端，关了再出声）
pub fn run_prepare_play<B: Backend, O: Output>(
    b: &B,
    profile: &Profile,
    q: &VoiceQuery,
    mark: bool,
    out: O,
    cancel: &AtomicBool,
    started: Instant,
) -> Result<(Player<O>, VoiceFile), Fail> {
    history::with_backend(b, profile, started, |b, stream, feed| {
        prepare_play(b, stream, feed, q, mark, out, cancel)
    })
}

// 测试数据全是编的
#[cfg(test)]
mod tests;
