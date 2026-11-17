//! 等结果：交出之后等 TDLib 的回音（[`fn@wait`]）并记账（[`finish`]）。流程与规则见模块根 `send.rs` 的头注释。

use super::*;

// ───────────────────────── 等结果 ─────────────────────────

/// 一条待发消息的下落
#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub enum Waited {
    /// 服务器收下了：发出后的整条（含正式 id）
    Succeeded(Message),
    /// 失败：失败后的那条（TDLib 也给它新 id）、原因、能否重发
    Failed {
        message: Message,
        error: CoreError,
        can_retry: bool,
    },
    /// 结果未知：超时、待发那条被删、连接断了
    Unknown(UnknownWhy),
}

/// 这条事件说了这几条待发消息里哪些的下落（一次删几条只来一条 `messages_deleted`，所以可能不止一条）
fn verdict(ev: &CoreEvent, temps: &[MsgId]) -> Vec<(MsgId, Waited)> {
    match ev {
        CoreEvent::SendSucceeded { old_id, message } if temps.contains(old_id) => {
            vec![(*old_id, Waited::Succeeded(message.clone()))]
        }
        CoreEvent::SendFailed {
            old_id,
            message,
            error,
            can_retry,
        } if temps.contains(old_id) => vec![(
            *old_id,
            Waited::Failed {
                message: message.clone(),
                error: error.clone(),
                can_retry: *can_retry,
            },
        )],
        // 发3：待发那条被删，TDLib 不来失败。谁会删：人在别的设备上删了它、会话变得不可访问、TDLib 重发前先删旧的。
        // 为什么「被删」反而可能已送达：请求可能早已到了服务器，删除只停本机的等待，服务器的成功应答照样回来
        // （TDLib 1.8.69 `MessagesManager.cpp:27848-27851`）。所以按结果未知报、不许重发——当成失败会让人重发出第二条
        CoreEvent::MessagesDeleted {
            chat_id,
            message_ids,
            ..
        } => temps
            .iter()
            .filter(|t| t.chat_id == *chat_id && message_ids.contains(&t.message_id))
            .map(|t| (*t, Waited::Unknown(UnknownWhy::PendingDeleted)))
            .collect(),
        _ => vec![],
    }
}

/// 等这几条待发消息的下落，最多 `timeout`。每个事件都先交给 `on_event`（调用方喂 store），再看是不是要等的。
/// 返回与 `temps` 同序
pub fn wait(
    stream: &mut EventStream,
    temps: &[MsgId],
    timeout: Duration,
    on_event: &mut dyn FnMut(Envelope),
) -> Vec<Waited> {
    let deadline = Instant::now() + timeout;
    let mut got: Vec<Option<Waited>> = vec![None; temps.len()];
    while got.iter().any(Option::is_none) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match block_on_timeout(stream.next_batch(256), left) {
            None => break,
            // 流结束：收线程故障或已关闭
            Some(None) => {
                for g in got.iter_mut().filter(|g| g.is_none()) {
                    *g = Some(Waited::Unknown(UnknownWhy::Disconnected));
                }
            }
            Some(Some(batch)) => {
                for env in batch {
                    for (id, w) in verdict(&env.event, temps) {
                        if let Some(i) = temps.iter().position(|t| *t == id)
                            && got[i].is_none()
                        {
                            got[i] = Some(w);
                        }
                    }
                    on_event(env);
                }
            }
        }
    }
    got.into_iter()
        .map(|g| g.unwrap_or(Waited::Unknown(UnknownWhy::Timeout)))
        .collect()
}

/// 等到结果后交给闸记账（直发的防重复记录与留痕）
pub fn finish<B: Backend>(b: &B, plan: &dyn Gated, sub: Submitted, w: &Waited) {
    let Submitted { mode, temp } = sub;
    let (outcome, final_id, err) = match w {
        Waited::Succeeded(m) => (Outcome::Succeeded, Some(m.id.message_id), None),
        Waited::Failed { message, error, .. } => {
            (Outcome::Failed, Some(message.id.message_id), Some(error))
        }
        Waited::Unknown(_) => (Outcome::Unknown, None, None),
    };
    b.gate()
        .after_outcome(mode, plan, temp.message_id, final_id, outcome, err);
}
