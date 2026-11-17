//! 媒体相关的节点：带文件的附件共用的小类型（`specs/content-tree.md` 第三节第 3 小节）、播放状态（`Doc::Playback`）、
//! 语音的转写（`Transcript`）、`transcribe` 这一次的结果（`Doc::Transcription`）。

use crate::model::MsgId;
use serde::Serialize;

/// 附件背后的那个文件。图片、视频、圆视频、音频、文件以后都挂这一个类型（M3.7），现在只有语音用。
///
/// `id` 是 TDLib 的文件 id：只在这次运行里有效的传输句柄，不是给人看的数据，**JSON 里不输出**。
/// 以后「下没下完」这类给人看的状态加在这里、照常输出；现在没有可输出的字段，整体不输出（快照不动，`v` 不加一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FileRef {
    #[serde(skip)]
    pub id: i32,
}

impl FileRef {
    /// 序列化时整体跳过：还没有给人看的字段
    pub fn nothing_to_show(&self) -> bool {
        true
    }
}

/// 过期的阅后即焚媒体是什么（`messageExpired*`，`td_api.tl:5752-5761`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expirable {
    Photo,
    Video,
    VideoNote,
    Voice,
}

/// 播放状态（`Doc::Playback`，`atg-cli play` 每次状态变化一行；arch 定 2026-11-13：要读出来的都先成树）。
/// 时间是秒（小数）；句子只由 `speech` 出
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Playback {
    pub state: PlayState,
    pub position_s: f64,
    pub duration_s: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayState {
    Playing,
    Paused,
}

/// 一条语音的转写（#59；`docs/experiments/exp008-local-stt/design.md` 第二节第 3 条）。
/// 不是消息本身的内容：TDLib 不知道它，存在 store 的转写表里、投影时挂到消息上，只在内存、不落盘。
/// 存原始片段，补标点只在读法里做。含转写原文：`Debug` 只在测试里派生，别处登记在 `redact.rs`
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Transcript {
    /// 识别用的语种（如 `zh-CN`）
    pub lang: String,
    pub source: TranscriptSource,
    pub segments: Vec<TranscriptSegment>,
}

/// 转写从哪来。以后接 Telegram 的 `speech_recognition_result` 也落同一个节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSource {
    /// 本机端侧模型（`atg-audio` 的 `stt`）
    Local,
}

/// `transcribe` 这一次的结果（`Doc::Transcription`；`docs/experiments/exp008-local-stt/design.md` 第三节第 1 条之后：
/// 是「本次结果」，不是转写表里存的那份——重转为空时本次是 `no_speech`，表里的旧转写不动）。
/// 含转写原文：`Debug` 只在测试里派生，别处登记在 `redact.rs`
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Transcription {
    /// 这条语音**现在的** id（提交时换过 id 的是新 id）
    pub id: MsgId,
    /// 语音被标为 18+ 遮挡：没展开时 [`crate::tree::mask`] 把 `outcome` 换成 `hidden`，读法也不读（大总管定 2026-11-14）
    pub sensitive: bool,
    pub outcome: Outcome,
}

impl Transcription {
    /// 这一次要不要遮住文字（18+ 遮挡、没展开）：遮挡（`mask`）、读法、CLI 的提示都只按它判
    pub fn hidden(&self, reveal: bool) -> bool {
        self.sensitive && !reveal
    }
}

/// 这一次转出了什么
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    /// 有字（这一份也写进了转写表）
    Text(Transcript),
    /// 跑成功了但没听出内容（不写表，不覆盖表里已有的）
    NoSpeech,
    /// 18+ 遮挡没展开：文字不输出（只由 `mask` 产出）
    Hidden,
}

/// 一段识别结果（时间是秒，从语音开头算）
#[derive(Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct TranscriptSegment {
    pub text: String,
    pub start_s: f64,
    pub end_s: f64,
}
