//! 「谁点了回应」（`getMessageAddedReactions`，L0 只读；#9）。从 `td` 拆出（#60 行数线）

use super::TdBackend;
use crate::error::CoreError;
use crate::link;
use crate::model::{AddedReactions, MsgId};
use crate::tdjson;
use crate::tree::ReactionKind;

impl TdBackend {
    pub(super) async fn added_reactions_impl(
        &self,
        mid: MsgId,
        kind: Option<&ReactionKind>,
        offset: &str,
        limit: u32,
    ) -> Result<AddedReactions, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let bad = || CoreError::InvalidSend {
            why: crate::error::InvalidSendWhy::ReactionInvalid,
        };
        let ty = match kind {
            None => None,
            Some(k) => {
                let v = tdjson::reaction_type(k).ok_or_else(bad)?;
                Some(serde_json::from_value(v).map_err(|_| bad())?)
            }
        };
        let limit = i32::try_from(limit).map_err(|_| bad())?;
        let r = link::guard(
            self.shared.clone(),
            tdlib_rs::functions::get_message_added_reactions(
                mid.chat_id,
                mid.message_id,
                ty,
                offset.to_owned(),
                limit,
                id,
            ),
        )
        .await?;
        let tdlib_rs::enums::AddedReactions::AddedReactions(a) =
            r.map_err(|e| self.read_error("message", &e))?;
        let v = serde_json::to_value(&a).map_err(|_| CoreError::Internal {
            what: "td_reactions_reserialize_failed",
        })?;
        Ok(tdjson::added_reactions(&v))
    }
}
