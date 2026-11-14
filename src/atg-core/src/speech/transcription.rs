//! `transcribe` 这一次结果的读法（`Doc::Transcription`，#59）。**句子是 AI 草稿，待人实听**。
//!
//! - 有字：读转写全文，片段间补「，」（同消息全文里的转写，[`super::content::punctuate`]）
//! - 没听出内容：「没有听出内容」（只在这里出现，`docs/experiments/exp008-local-stt/design.md` 第三节第 1 条之后）
//! - 18+ 遮挡没展开：只说「隐藏内容」，同消息里的隐藏内容（R1）；怎么展开由调用方提示（CLI 在标准错误提示 `--reveal`）

use super::HIDDEN;
use super::content::punctuate;
use crate::tree::{Outcome, Transcription};

const NO_SPEECH: &str = "没有听出内容";

pub(super) fn brief(t: &Transcription, reveal: bool) -> String {
    if t.sensitive && !reveal {
        return HIDDEN.to_owned();
    }
    match &t.outcome {
        Outcome::Text(tr) => {
            let said = punctuate(tr);
            if said.is_empty() {
                NO_SPEECH.to_owned()
            } else {
                said
            }
        }
        Outcome::NoSpeech => NO_SPEECH.to_owned(),
        Outcome::Hidden => HIDDEN.to_owned(),
    }
}
