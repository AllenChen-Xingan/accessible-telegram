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

/// 一条消息（TDLib `message` 的 JSON）里那个文件的上传进度（#12）：文件消息看 `document.document`，图片把各尺寸
/// 的已上传量加起来（只用来看「有没有在涨」），都传完才算传完。不带文件 → `None`
pub fn upload(m: &Value) -> Option<crate::backend::Upload> {
    let c = &m["content"];
    let files: Vec<&Value> = match c["@type"].as_str()? {
        "messageDocument" => vec![&c["document"]["document"]],
        "messagePhoto" => c["photo"]["sizes"]
            .as_array()?
            .iter()
            .map(|s| &s["photo"])
            .collect(),
        _ => return None,
    };
    let files: Vec<&Value> = files.into_iter().filter(|f| f.is_object()).collect();
    if files.is_empty() {
        return None;
    }
    Some(crate::backend::Upload {
        uploaded: files
            .iter()
            .map(|f| u64::try_from(i(&f["remote"], "uploaded_size")).unwrap_or(0))
            .sum(),
        done: files
            .iter()
            .all(|f| b(&f["remote"], "is_uploading_completed")),
    })
}
