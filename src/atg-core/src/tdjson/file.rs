//! 带文件的附件的映射（M2.5）：语音节点与它的 `FileRef`。以后图片、文件（M3.7）的文件句柄也在这里取。

use super::{b, i, u};
use crate::tree::{Attachment, FileRef};
use serde_json::Value;

/// `messageVoiceNote`（`td_api.tl` `messageVoiceNote`）→ 语音附件
pub(super) fn voice(c: &Value) -> Attachment {
    Attachment::Voice {
        duration: u(&c["voice_note"], "duration"),
        listened: b(c, "is_listened"),
        file: FileRef {
            id: file_id(&c["voice_note"]["voice"]),
        },
    }
}

/// TDLib `file.id`（int32）；缺了或越界为 0（TDLib 不发 0 号文件，下载时按找不到处理）
fn file_id(file: &Value) -> i32 {
    i32::try_from(i(file, "id")).unwrap_or(0)
}
