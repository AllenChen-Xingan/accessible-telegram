//! `delete`：经发送闸的 `deleteMessages`。现在只有「撤销待发消息」（`pending_cancel`，#57）；给所有人删除
//! （M2.7，`delete_prepare` / `delete`）接着加在这里，共用同一份计划、同一张凭据、同一个交出口。
//! 规约：`specs/contracts/actions.md`「待发出的消息」、第一节第 4 小节（闸）。
//!
//! **撤销只是尽力而为**（TDLib 1.8.69 `MessagesManager.cpp`）：删掉待发消息只停本机的网络请求（`:21330-21345`）；
//! 请求已经到了服务器的，成功应答回来时 TDLib 自己以 `revoke=true` 在服务器上删掉它（`:27848-27851` →
//! `:8484-8490`），对方可能已经看到或收到通知；会话变得不可访问时连这一步都没有（`:8466-8469`）。所以它按 L2 过闸
//! （大总管定 2026-11-14 Q1：赶上已送达会给所有人撤回，属对外影响）。
//!
//! 词：**撤销** = 在本机停掉还没发出的请求、删掉这条待发消息；**撤回** = 消息已在服务器上，给会话里所有人删掉
//! （对方的设备上也没了，但对方可能已经看到或收到过通知）。
//!
//! **撤销用 `revoke=false`**（自审第 3 轮 P1）：TDLib 删除时先把临时 id 换成它发出后的正式 id（`:8415`
//! `get_persistent_message_id`，`:21075-21087`；发出成功时登记，`:26949`、`:28347`）。交出前的复查离 TDLib 处理
//! 删除隔着一次往返，这段空隙里恰好发出的那条，删除请求会被悄悄换成那条已送达的消息——用 `revoke=true` 就给所有人
//! 删了，正是要拒的「给所有人删除」。`revoke=false` 对还在待发的消息没有区别（`:21330-21345` 只停本机请求），TDLib
//! 事后的自动撤回也固定用 `true`（`:8484-8490`），不受它影响；赶上恰好发出的，只删掉自己这边，对方那边留着。
//!
//! 流程：[`prepare_cancel`]（L0，逐条查是不是自己的待发消息、取正文给人确认）→ 闸开凭据 → [`cancel`]（验凭据 →
//! 交出前逐条再查一遍，变了就拒 → 交出）。成没成看之后的事件：临时 id 出现在 `messages_deleted` 里 = 撤销了；
//! 先来了这条的 `send_succeeded` = 没赶上，消息已发出——随后若它的正式 id 出现在 `messages_deleted` 里，就是上面
//! 那种被换成正式 id、只从自己这边删掉了（调用方要接着看一小会儿，见 CLI `pending`）。

use crate::backend::Backend;
use crate::error::{CoreError, InvalidSendWhy};
use crate::gate::{self, ConfirmMode, ConfirmToken, Gated, Outcome};
use crate::model::{ChatId, ChatTarget, MsgId, SendingState};
use crate::send::{self, Answer, Sealed, SendOrigin};
use serde_json::{Value, json};

/// 计划行与确认时说给人的「只是尽力撤销」（`actions.md`「待发出的消息」：不许写成「可以撤销」）
pub const CANCEL_NOTE_ZH: &str = "只是尽力撤销：请求已到服务器的照样送达，之后会给会话里所有人撤回（对方可能已经看到）；恰好刚发出的只从自己这边删掉，对方那边还在";

/// 一次最多几条（TDLib 的 `deleteMessages` 不设上限，这里是「明显给错了」的防线，也让确认说明放得下）
pub const MAX_IDS: usize = 100;

/// 计划里的一条：消息 id + 能给人确认的正文（不是纯文字的给不出，`None`）
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct DeleteItem {
    pub id: i64,
    pub text: Option<String>,
}

/// 一份已定稿的删除：确认给人看的、交给 TDLib 的都是它（摘要绑请求 + 每条正文）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct DeletePlan {
    target: ChatTarget,
    items: Vec<DeleteItem>,
    request: Value,
    digest: [u8; 32],
    origin: SendOrigin,
    mode: ConfirmMode,
}

impl DeletePlan {
    pub fn target(&self) -> &ChatTarget {
        &self.target
    }
    pub fn items(&self) -> &[DeleteItem] {
        &self.items
    }
    pub fn mode(&self) -> ConfirmMode {
        self.mode
    }
    pub fn origin(&self) -> SendOrigin {
        self.origin
    }
    pub fn digest_hex(&self) -> String {
        gate::hex(&self.digest)
    }
    /// 计划行（JSON，`kind:"plan"` 的 `data`）
    pub fn json(&self) -> Value {
        json!({
            "action": "pending_cancel",
            "target": send::target_json(&self.target),
            // 撤的是待发消息：id 是临时 id，与结尾行同名（`temp_message_id`；结尾行的 `message_id` 是发出后的正式 id）
            "items": self.items.iter().map(|i| json!({"temp_message_id": i.id, "text": i.text,
                "hidden_chars": i.text.as_deref().map(gate::hidden_count)})).collect::<Vec<_>>(),
            "note_zh": CANCEL_NOTE_ZH,
            "origin": self.origin,
            "confirm": self.mode.as_str(),
            "digest": self.digest_hex(),
        })
    }
}

impl gate::Sealed for DeletePlan {}
impl Gated for DeletePlan {
    fn target(&self) -> ChatId {
        self.target.id
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// 不进直发防重复（[`Gated::dedup`] 为假），这个值不用
    fn intent(&self) -> [u8; 32] {
        self.digest
    }
    /// 不拦：撤销的对象是某条待发消息的临时 id，撤下之后那个 id 就没了，同一条撤不了两次
    fn dedup(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "cancel"
    }
}

/// `pending_cancel` 的准备（L0，只查不删）：每条都得是自己发的、还在「待发」；取正文给人确认
pub async fn prepare_cancel<B: Backend>(
    b: &B,
    chat: ChatId,
    ids: &[i64],
    origin: SendOrigin,
) -> Result<DeletePlan, CoreError> {
    let mut ids = ids.to_vec();
    ids.dedup();
    if ids.is_empty() || ids.len() > MAX_IDS {
        return Err(CoreError::InvalidInput { td: None });
    }
    let me = b.me().await?;
    let mut target = b.chat_target(chat).await?;
    target.saved = target.id == me.saved_chat_id;
    let mode = b.gate().mode_for(target.id)?;
    let items = load_pending(b, target.id, &ids).await?;
    let request = json!({
        "@type": "deleteMessages",
        "chat_id": target.id,
        "message_ids": ids,
        // 见模块文档：恰好在交出前发出的那条，TDLib 会把它换成正式 id，`true` 就给所有人删了
        "revoke": false,
    });
    let texts: Vec<Option<&str>> = items.iter().map(|i| i.text.as_deref()).collect();
    let digest = gate::digest(
        &json!({"request": request, "texts": texts}),
        me.user_id,
        send::server_of(b)?,
    );
    Ok(DeletePlan {
        target,
        items,
        request,
        digest,
        origin,
        mode,
    })
}

/// 逐条查：自己发的、还在「待发」，取正文（准备时用；交出前再取一次与计划比对）
async fn load_pending<B: Backend>(
    b: &B,
    chat: ChatId,
    ids: &[i64],
) -> Result<Vec<DeleteItem>, CoreError> {
    let mut items = Vec::new();
    for &id in ids {
        let m = b
            .message_get(MsgId {
                chat_id: chat,
                message_id: id,
            })
            .await?;
        if !m.outgoing || m.sending != SendingState::Pending {
            return Err(CoreError::InvalidSend {
                why: InvalidSendWhy::NotPending,
            });
        }
        // 带附件的给不出全文：人确认不了、凭据也绑不住它的说明文字（复查看不出变化），整批不撤（codex 029 #3）
        let text = send::resending::confirmable_text(&m).ok_or(CoreError::InvalidSend {
            why: InvalidSendWhy::NotText,
        })?;
        items.push(DeleteItem {
            id,
            text: Some(text),
        });
    }
    Ok(items)
}

/// `pending_cancel`（L2）：验凭据 → 交出前逐条再查（已发出 / 失败 / 正文变了就拒，什么都不删）→ 交出。
/// 返回确认方式；撤没撤下看之后的事件（见模块文档）
pub async fn cancel<B: Backend>(
    b: &B,
    plan: &DeletePlan,
    token: ConfirmToken,
) -> Result<ConfirmMode, CoreError> {
    let mode = b.gate().verify(token, plan)?;
    // 从准备到交出最长 60 秒、还等了人确认：这期间可能已经发出。发出了再删就是「给所有人删除」，不是撤销——拒
    let ids: Vec<i64> = plan.items.iter().map(|i| i.id).collect();
    let now = load_pending(b, plan.target.id, &ids).await?;
    // 正文变了也按「不再是确认过的那条待发消息」拒（说明里不能用发送场景的「没有发、请重新发起」，自审第 1 轮）
    if now != plan.items {
        return Err(CoreError::InvalidSend {
            why: InvalidSendWhy::NotPending,
        });
    }
    b.gate().before_submit(mode, plan)?;
    match crate::send_limits::submit(b, Sealed::delete(&plan.request)).await {
        Ok(Answer::Deleted) => {
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Succeeded, None);
            Ok(mode)
        }
        Ok(_) => {
            b.gate()
                .after_outcome(mode, plan, 0, None, Outcome::Failed, None);
            Err(CoreError::Internal {
                what: "delete_answer_shape",
            })
        }
        // 同发送：断线、网络、应答转不回来 = 可能已交出，按结果未知记账、报 `send_outcome_unknown`（codex 029 第 3 轮 #3）
        Err(e) => Err(send::submit_failed(b, mode, plan, e)),
    }
}

#[cfg(test)]
mod tests;
