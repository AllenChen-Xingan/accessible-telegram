//! 假后端的上传进度（#12）：`upload_progress` 按次回登记好的进度，测发文件「有进度就续等、没进度到点」

use super::{FakeBackend, lock};
use crate::backend::Upload;
use crate::error::CoreError;
use crate::model::MsgId;

impl FakeBackend {
    /// 接下来几次 `upload_progress` 回什么（按次取；只剩一个时一直回它——不再有进度）。没登记 → `None`
    pub fn set_uploads(&self, u: Vec<Upload>) {
        *lock(&self.uploads) = u.into();
    }

    /// 登记的进度还剩几个没取（测「问过几次」）
    pub fn uploads_left(&self) -> usize {
        lock(&self.uploads).len()
    }

    pub(super) async fn do_upload_progress(&self, _id: MsgId) -> Result<Option<Upload>, CoreError> {
        self.started()?;
        let mut q = lock(&self.uploads);
        Ok(if q.len() > 1 {
            q.pop_front()
        } else {
            q.front().copied()
        })
    }
}
