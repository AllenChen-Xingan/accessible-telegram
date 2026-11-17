//! 带文件的待发消息上传到哪了（`getMessage` 取那条，读它文件的 `remote`；L0，#12）。发文件等结果时按它续等

use super::TdBackend;
use crate::backend::Upload;
use crate::error::CoreError;
use crate::link;
use crate::model::MsgId;
use crate::tdjson;

impl TdBackend {
    pub(super) async fn upload_progress_impl(
        &self,
        mid: MsgId,
    ) -> Result<Option<Upload>, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(
            self.shared.clone(),
            tdlib_rs::functions::get_message(mid.chat_id, mid.message_id, id),
        )
        .await?;
        let tdlib_rs::enums::Message::Message(m) = r.map_err(|e| self.read_error("message", &e))?;
        let v = serde_json::to_value(&m).map_err(|_| CoreError::Internal {
            what: "td_message_reserialize_failed",
        })?;
        Ok(tdjson::file::upload(&v))
    }
}
