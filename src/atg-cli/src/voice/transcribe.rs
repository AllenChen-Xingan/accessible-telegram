//! `transcribe` 的组合动作（#59 3c）：本机把一条语音转成文字。库代码：吃后端和参数，吐树节点，不写标准输出。
//! 正本 `docs/experiments/exp008-local-stt/design.md` 第二节第 3 条、第三节第 1、4、6 条。顺序：
//! 1. 认会话 → **先要租约**（`lease::Guard`：超级群、频道只有打开的会话才推更新，提交前的核对才看得到撤回、替换；
//!    所有退出路径都放）
//! 2. 取消息、放进 store → **从 store 取转写凭据**（`store::TranscriptTicket` = 消息 id + 语音文件 id，判结果还作不作数；
//!    与发送闸的确认凭据 `ConfirmToken` 无关，没有时限、不是一次性），贯穿下载、解码、转写、提交
//! 3. 下载凭据里那个文件 → 解码 → 本机转写（`atg-audio` 的 `stt`）
//! 4. 提交前先把这期间收到的更新全部喂进 store，再用同一个裁决函数判：作数才输出；有字才写表
//!
//! 只在内存：转写文字等同消息内容，不落盘、不进日志。错误只带码

use super::{Found, VoiceQuery, after_step, audio_fail, download, find_in};
use crate::chats::Feed;
use crate::history;
use crate::lease::Guard;
use crate::out::Fail;
use atg_audio::stt::{self, Recognition, SttError};
use atg_core::backend::Backend;
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::model::{Body, MsgId};
use atg_core::store::{Input, Stale, Store, Transcribed};
use atg_core::tree::{Outcome, Transcript, TranscriptSegment, TranscriptSource, Transcription};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// 转写用什么：辅助程序在哪、跑哪几个语种（顺序有意义：置信度相等取排前的）
pub struct SttOpts {
    pub helper: PathBuf,
    pub locales: Vec<String>,
}

/// `atg-audio` 转写的错误 → CLI 的失败。**这个映射只写这一处**（同 [`super::audio_fail`]）
pub fn stt_fail(e: SttError) -> Fail {
    match (e, e.what()) {
        (SttError::Cancelled, _) => Fail::Interrupted,
        (_, Some(what)) => Fail::Core(CoreError::Internal { what }),
        (_, None) => Fail::Core(CoreError::SttUnavailable {
            what: stt::what::UNAVAILABLE,
        }),
    }
}

/// 找辅助程序找不到、不安全 → `stt_unavailable{what}`（码见 `atg_audio::stt::locate`）
pub fn helper_fail(what: &'static str) -> Fail {
    Fail::Core(CoreError::SttUnavailable { what })
}

/// 提交时作废 → 输出的错误（design 第二节第 3 条「提交的结果决定输出」）
fn stale_fail(s: Stale) -> Fail {
    Fail::Core(match s {
        Stale::Gone => CoreError::NotFound {
            what: "message",
            td: None,
        },
        Stale::Changed => CoreError::Internal {
            what: "transcript_stale",
        },
    })
}

/// 在已起好的后端上转一条
pub fn transcribe<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &VoiceQuery,
    opts: &SttOpts,
    cancel: &AtomicBool,
) -> Result<Transcription, Fail> {
    let id = super::chat(b, stream, feed, q)?;
    let leases = feed.hub.leases().clone();
    let _lease = Guard::take(b, leases, id.chat_id, "atg-cli transcribe").map_err(Fail::Core)?;
    let f = after_step(cancel, find_in(b, stream, feed, id, cancel))?;
    // 转写凭据在从 store 读出这条语音的那一刻取（find_in 已把它放进 store、喂完了之前到的更新）。
    // 取来时是语音、喂完更新就不是了：同提交时作废一样报（消息没了 / 内容变了），不报「不是语音」
    let ticket = feed.store().transcript_ticket(f.id).ok_or_else(|| {
        stale_fail(match feed.store().message(f.id) {
            None => Stale::Gone,
            Some(_) => Stale::Changed,
        })
    })?;
    let found = Found {
        file: ticket.file,
        ..f
    };
    let v = after_step(cancel, download(b, stream, feed, found, cancel))?;
    let pcm = after_step(cancel, atg_audio::decode(&v.file.path).map_err(audio_fail))?;
    let r = stt::transcribe(&opts.helper, &pcm, &opts.locales, cancel).map_err(stt_fail);
    let r = after_step(cancel, r)?;
    // 转写期间更新照常排进事件流、没人喂：提交前全部按序喂进 store，裁决看到的就是截至此刻的全部
    feed.drain(stream);
    let now = feed
        .store()
        .transcript_verdict(&ticket)
        .map_err(stale_fail)?;
    let sensitive = sensitive(feed.store(), now);
    let outcome = match r {
        Recognition::Text {
            locale, segments, ..
        } => {
            let t = Transcript {
                lang: locale,
                source: TranscriptSource::Local,
                segments: segments
                    .into_iter()
                    .map(|s| TranscriptSegment {
                        text: s.text,
                        start_s: s.start_s,
                        end_s: s.end_s,
                    })
                    .collect(),
            };
            // store 的 reducer（`apply`）里用同一个裁决（`transcript_verdict`）再判一次：两步之间插不进别的输入（store 只在本线程上一件件喂），结果一样
            let _ = feed.pump_mut().apply(Input::Transcribed(Transcribed {
                ticket,
                transcript: t.clone(),
            }));
            Outcome::Text(t)
        }
        // 没听出内容：不写表，不覆盖已有的成功结果
        Recognition::NoSpeech => Outcome::NoSpeech,
    };
    Ok(Transcription {
        id: now,
        sensitive,
        outcome,
    })
}

/// 这条语音现在被标为 18+ 遮挡没有
fn sensitive(store: &Store, id: MsgId) -> bool {
    matches!(store.message(id).map(|m| m.effective()), Some(Body::Content(c)) if c.sensitive)
}

/// 直跑：起后端、转、关后端（租约在关后端之前放）
pub fn run<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &VoiceQuery,
    opts: &SttOpts,
    cancel: &AtomicBool,
    started: Instant,
) -> Result<Transcription, Fail> {
    history::with_backend(b, profile, started, |b, stream, feed| {
        transcribe(b, stream, feed, q, opts, cancel)
    })
}
