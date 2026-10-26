//! `pump`：消费方那一侧——在自己的线程上把事件成批喂给 reducer（`store` 只是 reducer，不开线程、不持锁）。
//!
//! 依据：拍板 006 甲5（`docs/plan/006_decisions-b7.md`）与 `specs/contracts/actions.md` 第一节第 7 小节末段：
//! - 每处理完一批看一眼 [`Store::needs_refetch`]，为真就发 [`Input::Refetch`]，并交回一张新凭据让调用方重拉
//!   当前会话历史（拍板 006 甲5）
//! - `resendMessages` 的应答逐条喂 [`Input::Resent`]（TDLib 对不能重发的那条返回 null，跳过）——[`resent_inputs`]
//! - 序号在一个 store 的生命里只许递增：重建 TDLib 客户端 / 重新登录时连 [`Pump`] 一起新建

use crate::link::Envelope;
use crate::model::{ChatId, MsgId};
use crate::store::{HistoryTicket, Input, Store};

/// 一批处理完的结果
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchOutcome {
    /// 喂进去的事件数
    pub applied: usize,
    /// 窗口要重拉：已发过 `Refetch`，调用方拿这张凭据去拉当前会话历史，应答喂 [`Input::HistoryLoaded`]
    pub refetch: Option<HistoryTicket>,
}

/// 唯一一份状态的持有者（CLI 循环里一份；GUI 的 `Store` 实体里一份）
#[derive(Default)]
pub struct Pump {
    store: Store,
}

impl Pump {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// 发历史请求、切会话等直接改 store 的输入
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// 喂一批事件；喂完查一次要不要重拉
    pub fn apply_batch(&mut self, batch: Vec<Envelope>) -> BatchOutcome {
        let applied = batch.len();
        for e in batch {
            self.store.apply(e.into_input());
        }
        BatchOutcome {
            applied,
            refetch: self.refetch_if_needed(),
        }
    }

    /// 喂一条不经事件流的输入（历史应答、`Resent`……），喂完同样查重拉
    pub fn apply(&mut self, input: Input) -> Option<HistoryTicket> {
        self.store.apply(input);
        self.refetch_if_needed()
    }

    fn refetch_if_needed(&mut self) -> Option<HistoryTicket> {
        if !self.store.needs_refetch() {
            return None;
        }
        let chat = self.store.window().map(|w| w.chat_id)?;
        self.store.apply(Input::Refetch);
        Some(self.store.ticket(chat))
    }
}

/// `resendMessages(chat, olds)` 的应答 → 逐条的 [`Input::Resent`]。
/// `answers` 是应答里按请求顺序排的新消息 id，不能重发的那条是 `None`（TDLib 给 null），跳过。
/// 两边条数对不上时只配前面对得上的（多出来的不猜）
pub fn resent_inputs(chat_id: ChatId, olds: &[i64], answers: &[Option<i64>]) -> Vec<Input> {
    olds.iter()
        .zip(answers)
        .filter_map(|(&old, new)| {
            new.map(|new| Input::Resent {
                old: MsgId {
                    chat_id,
                    message_id: old,
                },
                new: MsgId {
                    chat_id,
                    message_id: new,
                },
            })
        })
        .collect()
}
