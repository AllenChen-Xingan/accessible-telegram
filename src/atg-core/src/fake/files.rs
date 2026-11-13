//! 假后端的文件与语音动作（M2.5 M2.6）：`file_download`、`mark_listened`，以及登记文件的 `set_file`。

use super::{Call, FakeBackend, lock};
use crate::backend::Downloaded;
use crate::error::CoreError;
use crate::model::MsgId;
use crate::tree::FileRef;

impl FakeBackend {
    /// 登记一个文件：`file_download` 回它（`None` = 回来了但没下完）；没登记的 → 找不到
    pub fn set_file(&self, id: i32, file: Option<Downloaded>) {
        lock(&self.files).insert(id, file);
    }

    pub(super) async fn do_file_download(&self, file: FileRef) -> Result<Downloaded, CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::FileDownload(file.id));
        self.ready_or_401("file_download").await?;
        match lock(&self.files).get(&file.id) {
            Some(Some(d)) => Ok(d.clone()),
            Some(None) => Err(CoreError::Internal {
                what: "download_incomplete",
            }),
            None => Err(CoreError::NotFound {
                what: "file",
                td: None,
            }),
        }
    }

    pub(super) async fn do_mark_listened(&self, id: MsgId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::MarkListened(id));
        self.ready_or_401("mark_listened").await
    }
}
