//! 假后端给测试用的设置方法：排数据（会话、历史、话题）、编排结果（发送、`start`、注入的错误）、
//! 喂更新、让收线程崩溃或断掉、记账。从 `fake` 拆出（#60 R7），函数体照原样。
//! 只属于某一项能力、跟它的实现放在一起的不在这里：`set_auth`（`login.rs`）、`set_file`（`files.rs`）、
//! `set_uploads`（`upload.rs`）、`deny` / `set_no_reply`（`caps.rs`）。

use super::{Call, FakeBackend, FakeSend, FakeStart, Feed, lock};
use crate::error::CoreError;
use crate::model::{ChatId, ChatList};
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::Duration;

impl FakeBackend {
    /// 自己的用户 id（收藏夹的会话 id 同它）
    pub fn set_me(&self, user_id: i64) {
        *lock(&self.me_id) = Some(user_id);
        // 同 TDLib：登录后推 `updateOption my_id`
        self.push(json!({"@type": "updateOption", "name": "my_id",
            "value": {"@type": "optionValueInteger", "value": user_id.to_string()}}));
    }

    /// 这个会话是话题群
    pub fn set_forum(&self, chat: ChatId) {
        lock(&self.forums).push(chat);
    }

    /// 接下来几次预览是否把回复去掉（TDLib 遇到被回复那条不在 / 不能回复时就这样，发6）
    /// 下一次预览（交给 TDLib 的 `only_preview`）回来之前先跑 `hook`
    pub fn on_next_preview(&self, hook: impl FnOnce() + Send + 'static) {
        *lock(&self.preview_hook) = Some(Box::new(hook));
    }

    pub fn script_previews(&self, drops: &[bool]) {
        lock(&self.preview_drops).extend(drops.iter().copied());
    }

    /// 接下来几次真发送的结果
    pub fn script_sends(&self, s: Vec<FakeSend>) {
        lock(&self.send_script).extend(s);
    }

    /// 接下来按回调按钮时机器人回什么（`None` = 不回话）
    pub fn script_callback(&self, reply: Option<crate::send::CallbackReply>) {
        *lock(&self.callback_reply) = reply;
    }

    /// 真交出去的请求（预览不算）
    pub fn submitted(&self) -> Vec<Value> {
        lock(&self.submitted).clone()
    }

    /// 给某个列表排好会话（按 TDLib 的顺序交出；每个会话是几条 TDLib JSON 更新，如 `updateNewChat` +
    /// `updateChatPosition`）。再调一次是追加
    pub fn set_chats(&self, list: ChatList, chats: Vec<Vec<Value>>) {
        lock(&self.chats).entry(list).or_default().extend(chats);
    }

    /// `chat_get(id)` 时推这几条更新（`updateNewChat`、发送人的 `updateUser`……）；没设的会话 → `not_found`
    pub fn set_chat(&self, id: ChatId, updates: Vec<Value>) {
        lock(&self.chat_info).insert(id, updates);
    }

    /// `forum_topic_get(chat, topic)` 回这个话题（TDLib `forumTopic` 对象）；没设的 → `not_found`
    pub fn set_forum_topic(&self, chat: ChatId, topic_id: i64, topic: Value) {
        lock(&self.forum_topics).insert((chat, topic_id), topic);
    }

    /// 某会话的全部消息（TDLib `message` 对象，顺序无所谓）
    pub fn set_history(&self, chat: ChatId, messages: Vec<Value>) {
        lock(&self.histories).insert(chat, messages);
    }

    /// 每个会话第一次 `history` 只回 1 条（之后照常）
    pub fn short_first_history(&self) {
        *lock(&self.short_first) = Some(std::collections::HashSet::new());
    }

    /// 接下来 `n` 次 `history` 只回起点那一条，之后照常
    pub fn stall_history(&self, n: u32) {
        *lock(&self.stall) = (0, n);
    }

    /// 先照常应答 `ok` 次，再连着 `n` 次只回起点那一条，之后照常
    pub fn stall_history_after(&self, ok: u32, n: u32) {
        *lock(&self.stall) = (ok, n);
    }

    /// 每次 `history` 最多回 `c` 条（离起点最近的）
    pub fn cap_history(&self, c: usize) {
        *lock(&self.cap) = Some(c);
    }

    /// 前 `n` 次 `chats_load` 照常应答，之后一直不应答（测断网时加载卡住）
    pub fn hang_chats_load_after(&self, n: u32) {
        *lock(&self.chats_load_answers) = Some(n);
    }

    /// 推一条连接状态（`connectionStateReady` 这类类型名）
    pub fn push_net(&self, td_type: &str) {
        self.push(json!({
            "@type": "updateConnectionState",
            "state": { "@type": td_type },
        }));
    }

    /// 等收线程处理完已喂进去的更新（最多 5 秒；收线程没了就不等）。测试拿它做屏障：返回后推过的都已进事件流
    pub fn settle(&self) {
        let (tx, rx) = mpsc::channel();
        if lock(&self.feed).send(Feed::Barrier(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    }

    /// 注入「一直不应答」（测期限）：`fail_next(动作, CoreError::Internal { what: HANG })`
    pub const HANG: &str = "fake_hang";
    /// 下一次 `action`（记账里的名字）回这个错误，一次性
    pub fn fail_next(&self, action: &'static str, e: CoreError) {
        lock(&self.fail_next).push((action, e));
    }

    pub fn set_start(&self, s: FakeStart) {
        *lock(&self.start) = s;
    }

    /// `shutdown` 时收线程不给 `authorizationStateClosed`（测超时）
    pub fn set_close_hangs(&self, on: bool) {
        *lock(&self.close_hangs) = on;
    }

    /// 喂一条 TDLib JSON 形状的更新
    pub fn push(&self, update: Value) {
        let _ = lock(&self.feed).send(Feed::Update(update));
    }

    /// 喂一条授权状态更新（`authorizationStateWaitPhoneNumber` 这类类型名）
    pub fn push_auth(&self, td_type: &str) {
        self.push(json!({
            "@type": "updateAuthorizationState",
            "authorization_state": { "@type": td_type },
        }));
    }

    /// 让收线程崩溃（panic）
    pub fn crash(&self) {
        let _ = lock(&self.feed).send(Feed::Crash);
    }

    /// 让来源断掉（收线程没等到 Closed 就退出）
    pub fn end(&self) {
        let _ = lock(&self.feed).send(Feed::End);
    }

    /// 等收线程退出（测试里要确定故障已广播完再断言时用）
    pub fn join_receiver(&self) {
        if let Some(h) = lock(&self.receiver).take() {
            let _ = h.join();
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }
}
