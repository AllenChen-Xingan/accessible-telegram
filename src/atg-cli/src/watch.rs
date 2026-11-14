//! `watch [--chat CHAT_ID] [--reveal]`：订阅更新，一行一个事件，Ctrl+C 结束（M2.3，issue #53，T05 T10）。
//! 契约：`specs/contracts/cli.md` 第一节第 2、5 小节；事件名与字段 `actions.md` 第一节第 7 小节。
//!
//! - 先一行 `status`，之后每个事件一行 `kind:"event"`（带 `seq`），`data.type` 是事件名。带消息的事件（新消息、
//!   内容变了、最后一条变了、发送成败……）在 `data.message` 放消息节点、`speech.brief` 放这条的读法；隐藏内容
//!   默认遮住，`--reveal` 才出原文（同 `history`，`tree::mask`）
//! - 新消息与正在输入另带 `data.notify`：提不提示、理由、出不出声、连发计数（`atg_core::notify`，策略 v0）。
//!   这里只给判定，真正出声在 GUI 的播报器（M4.3）
//! - `--chat`：成对调 `chat_open` / `chat_close`（超级群、频道只有打开的会话才推新消息），也是「当前打开的会话」：
//!   正在输入只在它是私聊时播。**`chat_open` 只为订阅：不调 `viewMessages`、不标已读**（大总管 2026-10-26）
//! - Ctrl+C：结尾一行 `result`（`reason:"interrupted"`），退出码 0；再按一次立刻以 130 退出、不出结尾行
//! - 标准输出的读方走了（管道断了）：不再写，照常收尾、以 0 退出（结尾行写不出去）
//! - 档案被 `serve` / 界面占着：经本机接口流式订阅（`config-privacy.md` 协议 v0「流式订阅」）；执行方那边由
//!   [`Hub`] 给每个连进来的 `watch` 各自一份策略状态
//!
//! **CLI 永不标已读、不设在线、不发正在输入**（`actions.md` 第一节第 2 小节）。

use crate::args::{Format, WatchArgs};
use crate::chats::{self, Feed};
use crate::deadlines::{ME_TIMEOUT, QUERY_TIMEOUT, START_TIMEOUT, TOPIC_TIMEOUT, bounded, within};
use crate::history::ChatRef;
use crate::out::{Fail, envelope};
use atg_core::backend::{Backend, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::EventStream;
use atg_core::model::{
    ChatActionKind, ChatId, ChatList, ChatNotify, CoreEvent, Message, MsgId, NotifyScope, SenderId,
};
use atg_core::notify::{Config, Judged, Policy};
use atg_core::project;
use atg_core::speech::SpeechCtx;
use atg_core::status::CoreState;
use atg_core::store::Store;
use atg_core::tree::Doc;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 直跑时多久看一次退出标志、喂一次事件流
pub const POLL: Duration = Duration::from_millis(50);
/// 一个执行方最多同时挂几个 `watch`（大总管定 2026-10-26）
pub const MAX_SUBSCRIBERS: usize = 8;
/// 一次喂进来超过这么多条就在标准错误记一句「待处理 N 条」（003 收10；阈值待人定，冒烟实测最大 188）
pub const PENDING_WARN: usize = 2000;
/// 文字模式：补发结束那一句
pub const SYNC_DONE_ZH: &str = "同步完成，之后的是新消息";
/// 重连汇总的读法（人定 2026-10-26 给的例句；**待人实听**）
pub fn missed_zh(messages: u32, chats: u32) -> String {
    format!("{MISSED_PREFIX_ZH} {messages} 条，涉及 {chats} 个会话")
}

/// 重连汇总读法的开头（转发那头数「要提示」的行时认它）
pub const MISSED_PREFIX_ZH: &str = "断网期间收到";

/// 文字模式：要提示的新消息前面加的
pub const NEW_PREFIX_ZH: &str = "新消息，";
/// 「正在输入」的读法（F6：播一次「对方正在输入」；不带名字）
pub const TYPING_ZH: &str = "对方正在输入";

/// 现在（Unix 毫秒）
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 解析好的参数
#[derive(Debug, Clone, Copy)]
pub struct Query {
    pub chat: Option<ChatRef>,
}

/// 参数 → 查询；形状不对是用法错误（直跑与执行方都走这里）
pub fn query(a: &WatchArgs) -> Result<Query, Fail> {
    let chat = match a.chat.as_deref() {
        None => None,
        Some(s) => {
            Some(ChatRef::parse(s).ok_or(Fail::Usage("--chat 写数字 id，或 saved（收藏夹）"))?)
        }
    };
    Ok(Query { chat })
}

// ───────────────────────── 出口与订阅者 ─────────────────────────

/// 一个订阅者的出口：直跑是标准输出，执行方那边是本机接口的连接
pub trait LineOut {
    /// 写一行；读方已经走了回 `false`
    fn line(&mut self, line: &str) -> bool;
    /// 读方是不是已经走了（不写也能看出来；看不出来回 `false`）
    fn gone(&self) -> bool {
        false
    }
    /// 收尾：执行方那边发 `exit` 帧；标准输出不用
    fn finish(&mut self, _code: u8) {}
}

/// 本进程的标准输出：写失败（管道断了）就记下，之后不再写
#[derive(Default)]
pub struct StdoutLines {
    broken: bool,
}

impl LineOut for StdoutLines {
    fn line(&mut self, line: &str) -> bool {
        use std::io::Write;
        if self.broken {
            return false;
        }
        let mut out = std::io::stdout().lock();
        if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
            self.broken = true;
        }
        !self.broken
    }

    fn gone(&self) -> bool {
        self.broken
    }
}

/// 一个订阅者的状态：策略、读法上下文、计数
pub struct Watcher {
    policy: Policy,
    cx: SpeechCtx,
    format: Format,
    clock: fn() -> i64,
    /// 这个订阅从什么时候起（结尾行的 `elapsed_ms`）
    started: Instant,
    events: u64,
    alerts: u64,
}

impl Watcher {
    pub fn new(
        cx: SpeechCtx,
        format: Format,
        open_chat: Option<ChatId>,
        clock: fn() -> i64,
    ) -> Self {
        let mut policy = Policy::new(Config::default());
        policy.set_open_chat(open_chat);
        Self {
            policy,
            cx,
            format,
            clock,
            started: Instant::now(),
            events: 0,
            alerts: 0,
        }
    }

    /// 结尾时间从这一刻算（直跑：进程启动）
    pub fn since(mut self, started: Instant) -> Self {
        self.started = started;
        self
    }

    /// 中途接入：执行方早已补发完，接着的都算新消息
    pub fn mark_synced(&mut self) {
        self.policy.mark_synced((self.clock)());
    }

    /// 中途接入时执行方正在重连补发：补发结束时汇总这个订阅者接入后看到的
    pub fn mark_reconnecting(&mut self) {
        self.policy.mark_reconnecting();
    }

    /// 一条事件 → 要写出的行（JSON 一行；文字模式只写要给人看的那几种，可能没有）
    pub fn render(&mut self, store: &Store, seq: u64, event: &CoreEvent) -> Option<String> {
        let line = self.render_inner(store, seq, event)?;
        // 只数真写出去的事件行（文字模式没出的不算），转发那头按同一口径数（`forward::count_line`）
        self.events += 1;
        if line.1 {
            self.alerts += 1;
        }
        Some(line.0)
    }

    /// （这一行，要不要提示）
    fn render_inner(
        &mut self,
        store: &Store,
        seq: u64,
        event: &CoreEvent,
    ) -> Option<(String, bool)> {
        let judged = self.policy.observe(store, event, (self.clock)());
        let verdict = judged.as_ref().map(|j| match j {
            Judged::Message { verdict, .. }
            | Judged::Typing { verdict, .. }
            | Judged::Summary { verdict, .. } => *verdict,
        });
        // 重连补发结束的汇总（人定 2026-10-26）：断网期间本该提示的条数、会话数
        let missed = match &judged {
            Some(Judged::Summary {
                messages, chats, ..
            }) => Some((*messages, *chats)),
            _ => None,
        };
        let alert = verdict.is_some_and(|v| v.alert);
        let (mut data, doc) = event_data(store, event);
        let brief = match (&doc, event) {
            (Some(d), _) => Some(atg_core::speech::brief(d, &self.cx)),
            (None, CoreEvent::ChatAction { .. }) if verdict.is_some_and(|v| v.alert) => {
                Some(TYPING_ZH.to_owned())
            }
            (None, CoreEvent::SyncDone) => missed.map(|(n, m)| missed_zh(n, m)),
            _ => None,
        };
        match self.format {
            Format::Json => {
                if let (Some(d), Some(o)) = (&doc, data.as_object_mut()) {
                    // 与 history 同一段：没展开时 data 里的剧透原文遮掉（`json::value` 里做）
                    let v = atg_core::json::value(d, &self.cx);
                    o.insert("message".into(), v["data"].clone());
                }
                if let (Some(v), Some(o)) = (verdict, data.as_object_mut()) {
                    o.insert("notify".into(), json!(v));
                }
                if let (Some((n, m)), Some(o)) = (missed, data.as_object_mut()) {
                    o.insert("missed".into(), json!({ "messages": n, "chats": m }));
                }
                let mut line = envelope("event", data);
                if let Some(o) = line.as_object_mut() {
                    o.insert("seq".into(), json!(seq));
                    if let Some(b) = &brief {
                        o.insert("speech".into(), json!({ "brief": b }));
                    }
                }
                Some((line.to_string(), alert))
            }
            // 文字模式：每条新消息一句读法（要提示的前面加「新消息，」）；正在输入播的那次一句；补发结束一句
            Format::Text => match event {
                CoreEvent::SyncDone => Some(match brief {
                    // 重连汇总：出声的那一行，后面接补发结束那句
                    Some(b) => (format!("{b}。{SYNC_DONE_ZH}"), true),
                    None => (SYNC_DONE_ZH.to_owned(), false),
                }),
                CoreEvent::ChatAction { .. } => brief.map(|b| (b, alert)),
                _ => match (verdict, brief) {
                    (Some(v), Some(b)) if v.alert => Some((format!("{NEW_PREFIX_ZH}{b}"), true)),
                    (Some(_), Some(b)) => Some((b, false)),
                    _ => None,
                },
            },
        }
    }

    pub fn result_value(&self) -> Value {
        envelope(
            "result",
            json!({
                "command": "watch",
                "reason": "interrupted",
                "events": self.events,
                "alerts": self.alerts,
                "elapsed_ms": crate::elapsed_ms(self.started),
            }),
        )
    }

    /// 出错收尾的那一行（按这个订阅者自己的格式），连同退出码
    pub fn error_line(&self, f: &Fail) -> (String, u8) {
        let mut s = Collect::default();
        let code = crate::out::fail_with_to(&mut s, self.format, f);
        (s.0.unwrap_or_default(), code)
    }

    /// 结尾行（文字模式一句统计）
    pub fn result_line(&self) -> String {
        match self.format {
            Format::Json => self.result_value().to_string(),
            Format::Text => format!(
                "已停止；写出了 {} 条事件，其中 {} 条要提示",
                self.events, self.alerts
            ),
        }
    }
}

/// 一个挂着的 `watch`
pub struct Sub {
    pub out: Box<dyn LineOut>,
    pub watcher: Watcher,
    /// 替它打开的会话（关时配对 `chat_close`）
    pub chat: Option<ChatId>,
    /// 读方走了
    pub dead: bool,
}

/// 所有挂着的 `watch`。空的时候 [`Feed`] 按原来的成批方式喂，不多花一次拷贝
#[derive(Default)]
pub struct Hub {
    subs: Vec<Sub>,
}

impl Hub {
    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.subs.len()
    }

    pub fn add(&mut self, sub: Sub) {
        self.subs.push(sub);
    }

    /// 各订阅者判到时还没设置的论坛话题（去重）：调用方去取，应答喂 store
    pub fn take_missing_topics(&mut self) -> Vec<(ChatId, i64)> {
        let mut v: Vec<(ChatId, i64)> = self
            .subs
            .iter_mut()
            .flat_map(|s| s.watcher.policy.take_missing_topics())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// 某会话现在有几个订阅者替它开着。已标走、还没摘掉的也算：它开的那次要等摘掉时才配对关
    pub fn opened(&self, chat: ChatId) -> usize {
        self.subs.iter().filter(|s| s.chat == Some(chat)).count()
    }

    pub fn on_event(&mut self, store: &Store, seq: u64, event: &CoreEvent) {
        for s in &mut self.subs {
            if s.dead {
                continue;
            }
            if let Some(line) = s.watcher.render(store, seq, event)
                && !s.out.line(&line)
            {
                s.dead = true;
            }
        }
    }

    /// 摘掉读方已走的；回它们替开的会话里**已没人再开着**的那些（调用方去 `chat_close`）
    pub fn reap(&mut self) -> Vec<ChatId> {
        for s in &mut self.subs {
            if s.out.gone() {
                s.dead = true;
            }
        }
        let gone: Vec<Option<ChatId>> = self
            .subs
            .iter()
            .filter(|s| s.dead)
            .map(|s| s.chat)
            .collect();
        self.subs.retain(|s| !s.dead);
        let mut close: Vec<ChatId> = gone
            .into_iter()
            .flatten()
            .filter(|c| self.opened(*c) == 0)
            .collect();
        close.sort_unstable();
        close.dedup();
        close
    }

    /// 全部正常收尾：各写结尾行 `result` + 退出码 0，交出它们替开过的会话
    pub fn finish_all(&mut self) -> Vec<ChatId> {
        self.end_all(|w| (w.result_line(), 0))
    }

    /// 全部出错收尾：各按自己的格式写同一个错误，交出替开过的会话
    pub fn fail_all(&mut self, f: &Fail) -> Vec<ChatId> {
        self.end_all(|w| w.error_line(f))
    }

    fn end_all(&mut self, end: impl Fn(&Watcher) -> (String, u8)) -> Vec<ChatId> {
        let mut close = Vec::new();
        for mut s in std::mem::take(&mut self.subs) {
            if !s.dead {
                let (line, code) = end(&s.watcher);
                let _ = s.out.line(&line);
                s.out.finish(code);
            }
            close.extend(s.chat);
        }
        close.sort_unstable();
        close.dedup();
        close
    }

    /// 只给最后挂上的那个写错误、摘掉它（`watch` 接入途中出错，如 `--chat` 找不到）；回退出码
    pub fn fail_last(&mut self, f: &Fail) -> u8 {
        let Some(mut s) = self.subs.pop() else {
            return 1;
        };
        let (line, code) = s.watcher.error_line(f);
        let _ = s.out.line(&line);
        s.out.finish(code);
        code
    }

    /// 最后挂上的那个订阅者打开了这个会话（它也是正在输入要看的「当前会话」）
    pub fn open_chat(&mut self, chat: ChatId) {
        if let Some(s) = self.subs.last_mut() {
            s.chat = Some(chat);
            s.watcher.policy.set_open_chat(Some(chat));
        }
    }
}

// ───────────────────────── 事件 → JSON ─────────────────────────

fn sender_value(s: &SenderId) -> Value {
    match s {
        SenderId::User(id) => json!({ "type": "user", "user_id": id }),
        SenderId::Chat(id) => json!({ "type": "chat", "chat_id": id }),
        SenderId::Unknown { td_type } => json!({ "type": "unknown", "td_type": td_type }),
    }
}

fn notify_value(n: &ChatNotify) -> Value {
    json!({
        "use_default_mute_for": n.use_default_mute_for,
        "mute_for": n.mute_for,
        "use_default_disable_mention": n.use_default_disable_mention,
        "disable_mention": n.disable_mention,
        "use_default_disable_pinned": n.use_default_disable_pinned,
        "disable_pinned": n.disable_pinned,
    })
}

fn scope_name(s: NotifyScope) -> &'static str {
    match s {
        NotifyScope::Private => "private",
        NotifyScope::Group => "group",
        NotifyScope::Channel => "channel",
    }
}

fn list_value(l: &ChatList) -> Value {
    match l {
        ChatList::Main => json!("main"),
        ChatList::Archive => json!("archive"),
        ChatList::Folder(f) => json!({ "folder": f }),
        ChatList::Unknown { td_type } => json!({ "unknown": td_type }),
    }
}

/// 事件名（`actions.md` 第一节第 7 小节；变体与事件名一一对应）
pub fn event_name(e: &CoreEvent) -> &'static str {
    match e {
        CoreEvent::NewMessage(_) => "new_message",
        CoreEvent::MessageContentChanged { .. } => "message_content_changed",
        CoreEvent::MessageEphemeralChanged { .. } => "message_ephemeral_changed",
        CoreEvent::MessageEdited { .. } => "message_edited",
        CoreEvent::MessageContentOpened { .. } => "message_content_opened",
        CoreEvent::MessageInteractionChanged { .. } => "message_interaction_changed",
        CoreEvent::MessagesDeleted { .. } => "messages_deleted",
        CoreEvent::SendSucceeded { .. } => "send_succeeded",
        CoreEvent::SendFailed { .. } => "send_failed",
        CoreEvent::ChatAdded { .. } => "chat_added",
        CoreEvent::ChatPositionChanged { .. } => "chat_position_changed",
        CoreEvent::ChatLastMessageChanged { .. } => "chat_last_message_changed",
        CoreEvent::ChatDraftChanged { .. } => "chat_draft_changed",
        CoreEvent::ChatReadInbox { .. } => "chat_read_inbox",
        CoreEvent::ChatReadOutbox { .. } => "chat_read_outbox",
        CoreEvent::ChatMentionCountChanged { .. } => "chat_mention_count_changed",
        CoreEvent::ChatTitleChanged { .. } => "chat_title_changed",
        CoreEvent::ChatNotificationChanged { .. } => "chat_notification_changed",
        CoreEvent::ScopeNotificationChanged { .. } => "scope_notification_changed",
        CoreEvent::ForumTopicChanged { .. } => "forum_topic_changed",
        CoreEvent::ChatAction { .. } => "chat_action",
        CoreEvent::UserChanged(_) => "user_changed",
        CoreEvent::MyIdChanged { .. } => "my_id_changed",
        CoreEvent::UserStatusChanged(_) => "user_status_changed",
        CoreEvent::AuthStepChanged(_) => "auth_step_changed",
        CoreEvent::NetStateChanged(_) => "net_state_changed",
        CoreEvent::CoreStateChanged(_) => "core_state_changed",
        CoreEvent::SyncDone => "sync_done",
        CoreEvent::Unknown { .. } => "unknown_update",
    }
}

/// 一条消息现在在 store 里的样子（投影；换过 id 的按新 id 找）
fn message_doc(store: &Store, id: MsgId) -> Option<Doc> {
    project::message(store, store.current_id(id))
}

fn msg_doc(store: &Store, m: &Message) -> Option<Doc> {
    message_doc(store, m.id)
}

/// 事件的 `data`（不含消息节点与判定）+ 带的那条消息（要放进 `data.message`、算读法）
fn event_data(store: &Store, e: &CoreEvent) -> (Value, Option<Doc>) {
    let name = event_name(e);
    let id_fields =
        |id: &MsgId| json!({ "type": name, "chat_id": id.chat_id, "message_id": id.message_id });
    match e {
        CoreEvent::NewMessage(m) => (id_fields(&m.id), msg_doc(store, m)),
        CoreEvent::MessageContentChanged { id, .. }
        | CoreEvent::MessageEphemeralChanged { id, .. }
        | CoreEvent::MessageEdited { id, .. }
        | CoreEvent::MessageInteractionChanged { id, .. } => {
            (id_fields(id), message_doc(store, *id))
        }
        CoreEvent::MessageContentOpened { id } => (id_fields(id), None),
        CoreEvent::MessagesDeleted {
            chat_id,
            message_ids,
            is_permanent,
            from_cache,
        } => (
            json!({ "type": name, "chat_id": chat_id, "message_ids": message_ids,
                    "is_permanent": is_permanent, "from_cache": from_cache }),
            None,
        ),
        CoreEvent::SendSucceeded { old_id, message } => (
            json!({ "type": name, "chat_id": old_id.chat_id, "old_message_id": old_id.message_id,
                    "message_id": message.id.message_id }),
            msg_doc(store, message),
        ),
        CoreEvent::SendFailed {
            old_id,
            message,
            error,
            can_retry,
        } => (
            json!({ "type": name, "chat_id": old_id.chat_id, "old_message_id": old_id.message_id,
                    "message_id": message.id.message_id, "error": error.code(), "can_retry": can_retry }),
            msg_doc(store, message),
        ),
        CoreEvent::ChatAdded { chat, .. } => (
            json!({ "type": name, "chat_id": chat.id }),
            // 会话节点不放 data.message：会话项另有读法，`watch` 只报来了哪个会话（要全量用 `chats`）
            None,
        ),
        CoreEvent::ChatPositionChanged { chat_id, position } => (
            json!({ "type": name, "chat_id": chat_id, "list": list_value(&position.list),
                    "order": position.order.to_string(), "is_pinned": position.is_pinned }),
            None,
        ),
        CoreEvent::ChatLastMessageChanged {
            chat_id,
            last_message,
            ..
        } => (
            json!({ "type": name, "chat_id": chat_id,
                    "message_id": last_message.as_ref().map(|m| m.id.message_id) }),
            last_message.as_ref().and_then(|m| msg_doc(store, m)),
        ),
        CoreEvent::ChatDraftChanged { chat_id, .. } => {
            (json!({ "type": name, "chat_id": chat_id }), None)
        }
        CoreEvent::ChatReadInbox {
            chat_id,
            last_read_inbox_message_id,
            unread_count,
        } => (
            json!({ "type": name, "chat_id": chat_id,
                    "last_read_inbox_message_id": last_read_inbox_message_id, "unread_count": unread_count }),
            None,
        ),
        CoreEvent::ChatReadOutbox {
            chat_id,
            last_read_outbox_message_id,
        } => (
            json!({ "type": name, "chat_id": chat_id,
                    "last_read_outbox_message_id": last_read_outbox_message_id }),
            None,
        ),
        CoreEvent::ChatMentionCountChanged { chat_id, count } => (
            json!({ "type": name, "chat_id": chat_id, "unread_mention_count": count }),
            None,
        ),
        CoreEvent::ChatTitleChanged { chat_id, title } => (
            json!({ "type": name, "chat_id": chat_id, "title": title }),
            None,
        ),
        CoreEvent::ChatNotificationChanged { chat_id, notify } => (
            json!({ "type": name, "chat_id": chat_id, "settings": notify_value(notify) }),
            None,
        ),
        CoreEvent::ScopeNotificationChanged { scope, notify } => (
            json!({ "type": name, "scope": scope_name(*scope), "settings": {
                "mute_for": notify.mute_for, "disable_mention": notify.disable_mention,
                "disable_pinned": notify.disable_pinned } }),
            None,
        ),
        CoreEvent::ForumTopicChanged {
            chat_id,
            topic_id,
            last_read_inbox,
            notify,
            ..
        } => (
            json!({ "type": name, "chat_id": chat_id, "topic_id": topic_id,
                    "last_read_inbox_message_id": last_read_inbox, "settings": notify_value(notify) }),
            None,
        ),
        CoreEvent::ChatAction {
            chat_id,
            topic,
            sender,
            action,
        } => (
            json!({ "type": name, "chat_id": chat_id, "topic": topic, "sender": sender_value(sender),
                    "action": match action {
                        ChatActionKind::Typing => json!("typing"),
                        ChatActionKind::Cancel => json!("cancel"),
                        ChatActionKind::Other { td_type } => json!({ "other": td_type }),
                    } }),
            None,
        ),
        CoreEvent::UserChanged(u) => (
            json!({ "type": name, "user_id": u.id, "deleted": u.deleted }),
            None,
        ),
        CoreEvent::MyIdChanged { user_id } => (json!({ "type": name, "user_id": user_id }), None),
        CoreEvent::UserStatusChanged(s) => (json!({ "type": name, "user_status": s }), None),
        CoreEvent::AuthStepChanged(s) => (json!({ "type": name, "step": s }), None),
        CoreEvent::NetStateChanged(n) => (json!({ "type": name, "net": n }), None),
        CoreEvent::CoreStateChanged(c) => (json!({ "type": name, "core": c }), None),
        CoreEvent::SyncDone => (json!({ "type": name }), None),
        CoreEvent::Unknown { td_type } => (json!({ "type": name, "td_type": td_type }), None),
    }
}

/// 起好后的那一行 `status`
pub fn status_line<B: Backend>(b: &B, format: Format) -> String {
    let r = b.status();
    match format {
        Format::Json => envelope(
            "status",
            json!({
                "core": r.lines.core,
                "auth": r.lines.auth,
                "net": r.lines.net,
                "user_status": r.status,
                "pending_sends": [],
                "direct_send": false,
                "direct_sent": 0,
            }),
        )
        .to_string(),
        Format::Text => "开始订阅；Ctrl+C 结束".to_owned(),
    }
}

/// 会话 id：`saved` 换成自己的用户 id；`--chat` 的会话要在 store 里（`getChat`），找不到 → `not_found`
pub fn resolve<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    chat: ChatRef,
) -> Result<ChatId, Fail> {
    let id = match chat {
        ChatRef::Id(id) => id,
        ChatRef::Saved => {
            let deadline = Instant::now() + ME_TIMEOUT;
            loop {
                feed.drain(stream);
                if let Some(me) = feed.store().me() {
                    break me;
                }
                if Instant::now() >= deadline || b.status().lines.core == CoreState::Faulted {
                    return Err(Fail::Core(CoreError::Internal { what: "me_unknown" }));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    bounded(b, b.chat_get(id), QUERY_TIMEOUT).map_err(Fail::Core)?;
    feed.drain(stream);
    if feed.store().chat(id).is_none() {
        return Err(Fail::Core(CoreError::NotFound {
            what: "chat",
            td: None,
        }));
    }
    Ok(id)
}

/// 直跑怎么结束的
pub enum Ended {
    /// Ctrl+C 或标准输出的读方走了
    Stopped,
    /// 后端故障（事件流断了）
    Faulted,
}

/// 直跑：起后端 → 订阅直到 `stop` → 关后端。返回退出码；输出全写 `out`（单测用假出口）
#[allow(clippy::too_many_arguments)]
pub fn run<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &Query,
    cx: SpeechCtx,
    format: Format,
    stop: &AtomicBool,
    out: Box<dyn LineOut>,
    clock: fn() -> i64,
    started: Instant,
) -> u8 {
    let mut stream = match b.subscribe() {
        Ok(s) => s,
        Err(e) => return fail_line(out, format, &Fail::Core(e)),
    };
    // 起后端不看 Ctrl+C：watch 的约定是 Ctrl+C 照常出 `result`、退 0，起后端这一步也守它——按了就等它起完
    // （最多 START_TIMEOUT），进循环第一圈就停。登录组不同，按了就 `interrupted`（`session::drive`）
    if let Err(e) = bounded(b, b.start(profile), START_TIMEOUT) {
        let _ = b.shutdown(SHUTDOWN_TIMEOUT);
        return fail_line(out, format, &Fail::Core(e));
    }
    let mut feed = Feed::new(started);
    let r = watch_loop(
        b,
        &mut stream,
        &mut feed,
        q,
        cx,
        format,
        stop,
        out,
        clock,
        started,
    );
    if let Err(e) = b.shutdown(SHUTDOWN_TIMEOUT) {
        eprintln!("atg-cli: 关闭时出错（{}）", e.code());
    }
    r
}

/// 出错时写一行 `error`，回退出码
fn fail_line(mut out: Box<dyn LineOut>, format: Format, f: &Fail) -> u8 {
    let mut sink = LineSink(&mut *out);
    crate::out::fail_with_to(&mut sink, format, f)
}

/// 把 [`LineOut`] 当 [`crate::out::Sink`] 用（错误行走同一段渲染）
struct LineSink<'a>(&'a mut dyn LineOut);

impl crate::out::Sink for LineSink<'_> {
    fn out(&mut self, line: &str) {
        let _ = self.0.line(line);
    }
    fn err(&mut self, line: &str) {
        eprintln!("{line}");
    }
}

#[allow(clippy::too_many_arguments)]
fn watch_loop<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &Query,
    cx: SpeechCtx,
    format: Format,
    stop: &AtomicBool,
    mut out: Box<dyn LineOut>,
    clock: fn() -> i64,
    started: Instant,
) -> u8 {
    if let Err(f) = chats::ensure_ready(b) {
        return fail_line(out, format, &f);
    }
    if !out.line(&status_line(b, format)) {
        return 0;
    }
    // 订阅者先挂上、再喂流：之后喂的每一条（含找 `--chat` 会话那几步里到的积压）它都看得到，
    // 补发结束的 `sync_done` 也是它自己看到的那条
    feed.hub.add(Sub {
        out,
        watcher: Watcher::new(cx, format, None, clock).since(started),
        chat: None,
        dead: false,
    });
    let chat = match q.chat.map(|c| resolve(b, stream, feed, c)).transpose() {
        Ok(c) => c,
        Err(f) => return feed.hub.fail_last(&f),
    };
    if let Some(c) = chat {
        if let Err(e) = bounded(b, b.chat_open(c), QUERY_TIMEOUT) {
            return feed.hub.fail_last(&Fail::Core(e));
        }
        feed.hub.open_chat(c);
    }
    let mut max_pending = 0;
    let ended = loop {
        // 处理落后（调研 003 收10 叫「积压监控」；这里叫「待处理」，免得与补发那批「积压」混）：一次喂进来的条数
        // 就是收线程已收进来、这边还没处理的事件数；超过阈值记一句（阈值待人定）
        let fed = feed.drain(stream);
        if fed > PENDING_WARN {
            eprintln!("atg-cli watch: 待处理 {fed} 条事件（处理跟不上收线程）");
        }
        max_pending = max_pending.max(fed);
        fetch_topics(b, feed, stop);
        feed.hub.reap();
        if feed.hub.is_empty() {
            break Ended::Stopped;
        }
        if b.status().lines.core == CoreState::Faulted {
            break Ended::Faulted;
        }
        if stop.load(Ordering::SeqCst) {
            break Ended::Stopped;
        }
        std::thread::sleep(POLL);
    };
    // 后端故障时不再关会话（请求发不出去；同 serve）。正常结束才配对 chat_close
    if matches!(ended, Ended::Stopped) {
        close_chat(b, chat);
    }
    eprintln!("atg-cli watch: 单次最多待处理 {max_pending} 条");
    match ended {
        Ended::Stopped => {
            feed.hub.finish_all();
            0
        }
        Ended::Faulted => {
            let f = Fail::Core(CoreError::Disconnected);
            feed.hub.fail_all(&f);
            crate::out::core_exit(&CoreError::Disconnected)
        }
    }
}

/// 收下一行（把错误行渲染成字符串）
#[derive(Default)]
struct Collect(Option<String>);

impl crate::out::Sink for Collect {
    fn out(&mut self, line: &str) {
        self.0 = Some(line.to_owned());
    }
    fn err(&mut self, line: &str) {
        eprintln!("{line}");
    }
}

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

/// 收尾时关会话：停止标志已立起，只看期限
fn close_chat<B: Backend>(b: &B, chat: Option<ChatId>) {
    if let Some(c) = chat
        && let Err(e) = bounded(b, b.chat_close(c), QUERY_TIMEOUT)
    {
        eprintln!("atg-cli: 关闭会话时出错（{}）", e.code());
    }
}

#[cfg(test)]
mod tests;
