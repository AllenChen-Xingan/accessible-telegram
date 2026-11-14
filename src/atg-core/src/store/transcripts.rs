//! 转写表：消息现在的 id → 一条语音的转写（`docs/experiments/exp008-local-stt/design.md` 第二节第 3 条）。
//!
//! 一条规则管写、删、迁、读：**转写只对「同一个音频文件」作数**（凭据见 [`TranscriptTicket`]）。
//! - 写：只经 [`Input::Transcribed`](super::Input::Transcribed)，凭据过得了 [`Store::transcript_verdict`] 才写；
//!   同一条后到的覆盖先到的（都要过裁决）。失败不进表，所以不会盖掉已有的成功结果
//! - 删：每次 `apply` 末尾整理一遍，消息不是活的了、或有效内容不再是那个语音文件的就删——
//!   撤回、删除、删旧、内容替换、临时内容出现或消失、受限（有效内容换成受限节点）一处判完。
//!   删了就是删了：之后有效内容又变回那段语音（如临时内容消失），要再转一次才有
//! - 迁：键是消息**现在的** id（[`Store::current_id`]：沿换 id 链往新走到底）。发送成功换 id 后，整理时把键挪到新 id。
//!   不按稳定键（[`Store::stable_key`]：往旧走到最初那个 id）存：重发（`Input::Resent`）让失败那条与新那条在一段时间里
//!   同时是活消息、共用一个稳定键，而且重发应答可能晚于新那条的换 id 才到、事后改掉它的稳定键
//! - 读：投影时 [`Store::transcript`] 再核一次同一条规则
//!
//! 转写是人一条条按的，表很小，每次整理全表扫一遍的代价可忽略。只在内存，不落盘：转写文字等同消息内容

use super::{Store, TranscriptTicket};
use crate::model::*;
use crate::tree::{Attachment, FileRef, Transcript};
use std::collections::HashMap;

/// 一条：现在的 id（与表键相同，整理时一起挪）、对应的音频文件、转写（`Transcript` 的 `Debug` 已脱敏，见 `redact.rs`）
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Entry {
    id: MsgId,
    file: FileRef,
    transcript: Transcript,
}

pub(super) type Transcripts = HashMap<MsgId, Entry>;

/// 一条转写完的结果，带着取凭据时的凭据回来（[`Input::Transcribed`](super::Input::Transcribed)）
#[derive(Debug, Clone, PartialEq)]
pub struct Transcribed {
    pub ticket: TranscriptTicket,
    pub transcript: Transcript,
}

/// 提交的转写为什么不作数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stale {
    /// 消息没了（撤回、删除、删旧）
    Gone,
    /// 有效内容不再是那个语音文件（内容替换、临时内容、受限）
    Changed,
}

/// 消息的有效内容是语音时，它的文件
pub(super) fn voice_file(m: &Message) -> Option<FileRef> {
    match m.effective() {
        Body::Content(c) => match &c.attachment {
            Some(Attachment::Voice { file, .. }) => Some(*file),
            _ => None,
        },
        Body::Service(_) => None,
    }
}

impl Store {
    /// 裁决：凭据现在还作不作数。作数给现在的 id（换过 id 的是新 id）。
    /// 调用方（`transcribe`）提交前用它决定输出；reducer 收 [`Input::Transcribed`](super::Input::Transcribed) 时用同一个判。
    /// 两次之间没有别的输入能插进来：store 只在调用方自己的线程上、一件一件地喂
    pub fn transcript_verdict(&self, t: &TranscriptTicket) -> Result<MsgId, Stale> {
        let id = self.current_id(t.id);
        let m = self.message(id).ok_or(Stale::Gone)?;
        if voice_file(m) == Some(t.file) {
            Ok(id)
        } else {
            Err(Stale::Changed)
        }
    }

    /// 这条消息的转写（投影时填进 [`crate::tree::Message::transcript`]）。`id` 要是现在的 id
    pub fn transcript(&self, id: MsgId) -> Option<&Transcript> {
        let e = self.transcripts.get(&id)?;
        self.still_valid(e).then_some(&e.transcript)
    }

    pub(super) fn commit_transcript(&mut self, r: Transcribed) {
        let Transcribed {
            ticket: t,
            transcript,
        } = r;
        if let Ok(id) = self.transcript_verdict(&t) {
            let e = Entry {
                id,
                file: t.file,
                transcript,
            };
            self.transcripts.insert(id, e);
        }
    }

    /// 删不再作数的，键挪到现在的 id
    pub(super) fn tidy_transcripts(&mut self) {
        if self.transcripts.is_empty() {
            return;
        }
        // 已经在现在 id 上的先放：两条挪到同一个 id（换 id 链合流，极少见）时留它——它是换 id 之后写的，更新
        let (stay, moved): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut self.transcripts)
            .into_values()
            .partition(|e| self.current_id(e.id) == e.id);
        for mut e in stay.into_iter().chain(moved) {
            e.id = self.current_id(e.id);
            if self.still_valid(&e) {
                self.transcripts.entry(e.id).or_insert(e);
            }
        }
    }

    fn still_valid(&self, e: &Entry) -> bool {
        self.message(e.id).and_then(voice_file) == Some(e.file)
    }
}
