//! 正文与附件的读法（`docs/plan/010_complexity-budget.md` 定的 `speech/content.rs`；`speech.rs` 里其余正文、附件的读法
//! 以后照 010 搬进来）。现在有：时长的说法（视频、语音、音频、播放状态共用）、内容的全文、语音的转写（#59）。
//! **转写的句子是 AI 草稿，待人实听**。
//!
//! 转写（`docs/experiments/exp008-local-stt/design.md` 第二节第 4 条、第三节第 2 条）：
//! - 名称（`brief`）**不变**；「已转文字」是状态，进详情（`specs/content-tree.md` 第九节：名称只放谁发的、说了什么、几点）
//! - 全文：语音的说法后面接转写全文；没转过接「还没转文字」。按键提示（如「按 T 转」）留给 GUI 那一块定
//! - 补标点只在这里做，树里存原始片段：片段之间，前一段末尾没有标点就补「，」（v0 只做这一条；英文转写也补全角「，」——
//!   读屏只拿它当停顿，v0 不按语种分）
//! - 隐藏内容（18+ 遮挡）没展开时：全文不接转写、详情不说「已转文字」，同 JSON 不输出转写（`tree/mask.rs`）

use super::{HIDDEN, attachment, full_spans};
use crate::tree::{Attachment, Content, Message, Transcript};

/// 时长：「12 秒」「1 分 5 秒」「1 小时 2 分」
pub(super) fn dur(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s} 秒"),
        (0, _) if s == 0 => format!("{m} 分"),
        (0, _) => format!("{m} 分 {s} 秒"),
        _ if m == 0 => format!("{h} 小时"),
        _ => format!("{h} 小时 {m} 分"),
    }
}

const TRANSCRIBED: &str = "已转文字";
const NOT_YET: &str = "还没转文字";

/// 详情里的那一段：有转写、且没被 18+ 遮着才有
pub(super) fn transcript_detail(m: &Message, reveal: bool) -> Option<String> {
    let hidden = m.content.sensitive && !reveal;
    m.transcript
        .as_ref()
        .filter(|_| !hidden)
        .map(|_| TRANSCRIBED.to_owned())
}

/// 消息的全文：内容的全文，是语音再接转写；遮着的（18+ 没展开）不接
pub(super) fn full(m: &Message, reveal: bool) -> String {
    let full = full_content(&m.content, reveal);
    if !matches!(m.content.attachment, Some(Attachment::Voice { .. }))
        || (m.content.sensitive && !reveal)
    {
        return full;
    }
    let tail = m
        .transcript
        .as_ref()
        .map_or_else(|| NOT_YET.to_owned(), punctuate);
    format!("{full}，{tail}")
}

/// 内容的全文：附件照名称的说法放在前面，正文原样（`super::full` 的正文部分）
pub(super) fn full_content(c: &Content, reveal: bool) -> String {
    let body = full_spans(&c.body, reveal);
    let label = c.attachment.as_ref().map(|a| {
        let (l, spoiler) = attachment(a, reveal);
        if (spoiler || c.sensitive) && !reveal {
            format!("{l}，{HIDDEN}")
        } else {
            l
        }
    });
    match (label, body.trim().is_empty()) {
        (None, true) => "空消息".to_owned(),
        (None, false) => body,
        (Some(l), true) => l,
        (Some(l), false) => format!("{l}，{body}"),
    }
}

/// 片段拼成一句：前一段末尾没有标点就补「，」；全是空白的片段跳过
pub(super) fn punctuate(t: &Transcript) -> String {
    let mut out = String::new();
    for seg in t
        .segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
    {
        if !out.is_empty() && !out.ends_with(|c: char| is_punct(c)) {
            out.push('，');
        }
        out.push_str(seg);
    }
    out
}

fn is_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '。' | '？' | '！' | '、' | '；' | '：' | '…' | ',' | '.' | '?' | '!' | ';' | ':'
    )
}
