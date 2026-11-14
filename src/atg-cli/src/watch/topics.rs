//! 论坛话题设置按需取（#73）：`watch` 与 `serve` 共用
use super::now_ms;
use crate::chats::Feed;
use crate::deadlines::{TOPIC_TIMEOUT, within};
use crate::out::Fail;
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// 取判到时还没设置的论坛话题（`getForumTopic`，只读），应答喂 store（只在还没有时收）；取不到只记错误码。
/// 每个最多等 [`TOPIC_TIMEOUT`]；`stop` 立起就把剩下的留着不取、直接回去让主循环收尾（#73）
pub fn fetch_topics<B: Backend>(b: &B, feed: &mut Feed, stop: &AtomicBool) {
    for (chat, topic) in feed.hub.take_missing_topics() {
        let deadline = Instant::now() + TOPIC_TIMEOUT;
        let r = match within(b, b.forum_topic_get(chat, topic), deadline, stop) {
            Ok(r) => r,
            Err(Fail::Core(e)) => Err(e),
            Err(Fail::Interrupted) => return,
            Err(Fail::Usage(_) | Fail::NotImplemented { .. } | Fail::Unsupported(_)) => continue,
        };
        match r {
            Ok(state) => {
                let _ = feed.pump_mut().apply(atg_core::store::Input::TopicLoaded {
                    chat_id: chat,
                    topic_id: topic,
                    state,
                    received_ms: now_ms(),
                });
            }
            // 取不到：这个话题本次订阅里不再取，一直按 topic_unknown 不提示（宁可不吵）
            Err(e) => eprintln!("atg-cli watch: 取论坛话题设置出错（{}）", error_label(&e)),
        }
    }
}

/// 标准错误里说错误用的：码，`internal` 另带写死的 `what`（与 `errors.md` 登记的对得上）
fn error_label(e: &CoreError) -> String {
    match e {
        CoreError::Internal { what } => format!("internal:{what}"),
        other => other.code().to_owned(),
    }
}
