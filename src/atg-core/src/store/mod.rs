//! `store`：会话列表与当前消息窗口的状态。只是一个 reducer（旧状态 + 一件事 → 新状态），
//! 不开线程、不持锁、不碰时钟；谁用谁在自己的线程上跑它。
//!
//! 合并规则照 `docs/plan/001_arch-stack.md` 第一节第 3 小节第 6 条。更新按序号依次到达（序号不比已见过的大就丢掉）；
//! 更新与历史应答交错到达时，结果与谁先到无关：
//! - **身份**：消息一律用 [`MsgId`]
//! - **版本边界**：每条消息记两样——整条快照的版本（随更新到达的整条消息记 `(seq, 0)`，
//!   历史应答记 `(since_seq, request)`），和每个字段最后一次被更新的序号。历史应答回来时：
//!   快照比 store 的旧 → 整条不收；否则以应答为底，**请求之后被更新过的字段**取 store 的
//!   （发出之后才到的更新，要么和应答一样新，要么更新）
//! - **请求代次**：切会话、重新拉历史，代次加一。应答的代次不是当前的 → 只进缓存，不动当前窗口
//! - **按字段合并**：内容只认 `MessageContentChanged`（临时内容只认 `MessageEphemeralChanged`），
//!   编辑时间只认 `MessageEdited`，各管各的字段，不拿编辑时间比内容新旧
//! - **撤回留墓碑**：只认 `is_permanent && !from_cache`（`specs/contracts/actions.md` 事件 `messages_deleted`）；
//!   墓碑记下撤回前的发送人、时间（读法 R7）；撤回过的消息不会被晚到的应答或重复的更新加回来
//! - **发送成功换 id**：旧 id → 新 id；[`Store::stable_key`] 给出最初那个 id，供 GUI 沿用元素 id
//!
//! 状态到内容树的投影在 [`crate::project`]。

use crate::model::*;
use std::collections::{BTreeSet, HashMap, HashSet};

mod table;
use table::Table;

/// 记不全的「孤儿补丁」最多留多少条（见 [`Patch`]）
const MAX_ORPHANS: usize = 4096;

/// store 的可调参数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreConfig {
    /// 非当前会话最多留多少条活消息（拍板 006 甲1）。超出的从最旧的删起；
    /// 会话的最后一条、墓碑、换 id 的链不算在内、不删。当前打开的会话不限。
    pub keep_per_chat: usize,
}

impl Default for StoreConfig {
    /// 默认 50：一屏约十几条，第一次拉历史约一屏多，50 条够「切回去马上有一屏可读、往上翻一点」；
    /// 再多就是给几百个不常看的会话囤消息——按每条约 1–2 KB 估，500 个会话 × 50 条约 25–50 MB，
    /// 留 1000 条就上 GB。真实账号量过内存后再调（`StoreConfig { keep_per_chat }`）
    fn default() -> Self {
        Self { keep_per_chat: 50 }
    }
}

/// 喂给 reducer 的一件事
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// 收线程推来的更新，`seq` 由收线程递增编号。`seq` 不比已见过的大 → 重复，丢掉。
    /// 所以序号在一个 store 的整个生命里只许递增（重建 TDLib 客户端、重新登录时连 store 一起新建）
    Update { seq: u64, event: CoreEvent },
    /// 打开（切到）一个会话：代次加一，窗口清空。之后用 [`Store::ticket`] 取请求凭据去拉历史
    OpenChat { chat_id: ChatId },
    /// 重新拉当前会话的历史（如积压太多时丢掉窗口）：代次加一，窗口清空
    Refetch,
    /// 历史请求的应答（`messages` 顺序无所谓）
    HistoryLoaded {
        ticket: HistoryTicket,
        messages: Vec<Message>,
    },
    /// `resendMessages` 的应答：失败的那条 `old` 重发成了 `new`（TDLib 按请求顺序逐条返回）。
    /// 新的那条沿用旧的稳定键。删除失败那条的更新照常到（不当撤回）
    Resent { old: MsgId, new: MsgId },
    /// `getForumTopic` 的应答（`received_ms`：收到应答的时刻，换静音到期用）。**只在 store 里还没有这个话题时收**：
    /// 应答可能比之后推来的 `updateForumTopic` 旧，不许盖掉它
    TopicLoaded {
        chat_id: ChatId,
        topic_id: i64,
        state: ForumTopicState,
        received_ms: i64,
    },
}

/// 发历史请求时记下的东西：哪个会话、当时的代次、当时最后一条更新的序号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryTicket {
    pub chat_id: ChatId,
    pub generation: u64,
    pub since_seq: u64,
    /// 请求编号，每张凭据加一：同一序号下，后发的请求的应答更新
    pub request: u64,
}

/// 整条快照的版本：(序号, 请求编号)，按字典序比新旧。
/// 随更新到达的整条消息记 `(seq, 0)`；历史应答记 `(since_seq, request)`——同一序号下，后发出的请求更新
type Version = (u64, u64);

/// 每个字段最后一次被字段更新改动的序号（0 = 没被更新过）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FieldSeqs {
    content: u64,
    ephemeral: u64,
    edited: u64,
    reactions: u64,
    opened: u64,
}

fn update_version(seq: u64) -> Version {
    (seq, 0)
}

/// 消息表的一格：活的消息，或撤回后的墓碑
// 绝大多数格子是活消息，装箱只会给每条多一次分配
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Live(Message),
    Tombstone(Tomb),
}

/// 墓碑：撤回前记下的发送人、时间、是不是自己发的（没见过原消息就撤回的，都空）
#[derive(Debug, Clone, PartialEq)]
pub struct Tomb {
    pub sender: Option<SenderId>,
    pub date: Option<Time>,
    /// 自己发的（读法 R7 读「我撤回了一条消息」）
    pub outgoing: bool,
}

/// 当前打开的消息窗口
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Window {
    pub chat_id: ChatId,
    pub generation: u64,
    /// 窗口里的消息 id（会话内按 id 从旧到新）；撤回的仍在原位（投影成占位）
    pub ids: BTreeSet<i64>,
    /// 有应答因不可信被拒收：窗口可能缺消息，要重新拉（[`Store::needs_refetch`]）
    pub stale: bool,
    /// 从上一次打开这个会话带过来的撤回占位（拍板 006 甲3：重拉 / 重开时原位保留）。
    /// 只显示落在 `loaded_from` 之内的
    pub carried: BTreeSet<i64>,
    /// **历史应答确认过的加载边界**：消息 id 不小于它的都已由应答确认（空应答 = 全部，`i64::MIN`）；
    /// `None` = 还没有应答回来。只由当前代次的历史应答推进，新消息不动它，所以与先后无关
    pub loaded_from: Option<i64>,
}

/// 更新先于消息本身到达时记下的字段，等那条消息随历史应答进来再补上。
/// 只补比应答新的（`seq > since_seq`）。
///
/// 内容进补丁前已按已知的限制清过（[`table`]）；所属消息还没登记受限时内容原样存，登记那一刻再清。
/// 最多 [`MAX_ORPHANS`] 条，超了丢最旧的。不落盘、不进日志
#[derive(Debug, Clone, PartialEq, Default)]
struct Patch {
    /// 各字段里最新的那个序号
    seq: u64,
    // 每个字段各记自己的序号：只补比应答新的那几个
    content: Option<(u64, Body)>,
    ephemeral: Option<(u64, Option<Body>)>,
    edited: Option<(u64, Option<Time>)>,
    reactions: Option<(u64, Vec<Reaction>)>,
    opened: Option<u64>,
}

impl Patch {
    /// 把比 `since` 新的字段补到 `m` 上
    fn apply_to(self, m: &mut Message, since: u64) {
        let newer = |s: u64| s > since;
        let content_seq = self.content.as_ref().map(|(s, _)| *s);
        if let Some((s, c)) = self.content
            && newer(s)
        {
            m.content = c;
        }
        if let Some((s, e)) = self.ephemeral
            && newer(s)
        {
            m.ephemeral = e;
        }
        if let Some((s, t)) = self.edited
            && newer(s)
        {
            m.edited = t;
        }
        if let Some((s, r)) = self.reactions
            && newer(s)
        {
            m.reactions = r;
        }
        // 「已听」只作用在原内容上（与 TDLib 一致，临时内容不动），且只作用在它之前就有的那份上：
        // 之后又换的内容自带已听标记
        if let Some(o) = self.opened.filter(|&o| newer(o))
            && !content_seq.is_some_and(|s| newer(s) && s > o)
        {
            m.content.mark_opened();
        }
        m.enforce();
    }

    /// 各字段的序号
    fn seqs(&self) -> FieldSeqs {
        FieldSeqs {
            content: self.content.as_ref().map_or(0, |(s, _)| *s),
            ephemeral: self.ephemeral.as_ref().map_or(0, |(s, _)| *s),
            edited: self.edited.as_ref().map_or(0, |(s, _)| *s),
            reactions: self.reactions.as_ref().map_or(0, |(s, _)| *s),
            opened: self.opened.unwrap_or(0),
        }
    }

    /// 从 store 里的活消息取出「`since` 之后被更新过的字段」，形状同孤儿补丁
    fn from_live(m: &Message, f: FieldSeqs, since: u64) -> Self {
        let pick = |s: u64| s > since;
        Patch {
            seq: [f.content, f.ephemeral, f.edited, f.reactions, f.opened]
                .into_iter()
                .max()
                .unwrap_or(0),
            content: pick(f.content).then(|| (f.content, m.content.clone())),
            ephemeral: pick(f.ephemeral).then(|| (f.ephemeral, m.ephemeral.clone())),
            edited: pick(f.edited).then_some((f.edited, m.edited)),
            reactions: pick(f.reactions).then(|| (f.reactions, m.reactions.clone())),
            opened: pick(f.opened).then_some(f.opened),
        }
    }
}

/// 对一条消息的一处字段改动（内部用：活消息直接改，没见过的记成补丁）
enum FieldChange {
    Content(Body),
    Ephemeral(Option<Body>),
    /// `None` = 编辑时间被清掉（TDLib 用 0 表示）
    Edited(Option<Time>),
    Reactions(Vec<Reaction>),
    Opened,
}

#[derive(Debug, Clone, Default)]
pub struct Store {
    last_seq: u64,
    users: HashMap<UserId, User>,
    chats: HashMap<ChatId, Chat>,
    /// 消息表、孤儿补丁、受限登记、换 id 链：写内容只能经它（见 [`table`]）
    table: Table,
    /// 整条快照的版本；也记非撤回删除、换 id 的序号，挡住请求之前的旧应答
    versions: HashMap<MsgId, Version>,
    /// 活消息各字段最后一次被更新的序号
    fields: HashMap<MsgId, FieldSeqs>,
    scope_notify: HashMap<NotifyScope, ScopeNotify>,
    /// 论坛话题的通知状态（话题静音已按收到时刻换成绝对到期时刻；中途接入的订阅者也从这里取）
    topics: HashMap<(ChatId, i64), TopicNotify>,
    status: Option<UserStatus>,
    generation: u64,
    window: Option<Window>,
    /// 换 id、重发留下的「新 id → 上一个 id」。稳定键沿这条链走到头，所以先后无关
    parent: HashMap<MsgId, MsgId>,
    /// 每个会话「丢过东西」的最大序号（孤儿补丁被挤掉、超过 `keep_per_chat` 的消息被删）：
    /// 序号比它早的请求的应答里，store 里没有的消息不可信（可能正是丢掉的那条，旧内容会盖掉
    /// 已收到的更新），**不论有没有新补丁**都不收，并让当前窗口重新拉。按会话记，上限 = 会话数
    evicted_upto: HashMap<ChatId, Version>,
    /// 每个会话的活消息 id（删旧消息用）
    by_chat: HashMap<ChatId, BTreeSet<i64>>,
    /// 这次输入里新进了活消息的会话：输入处理完对它们按 `keep_per_chat` 删旧
    dirty: HashSet<ChatId>,
    /// 关窗口时留下的撤回占位，下次打开同一会话带回去
    window_tombs: HashMap<ChatId, BTreeSet<i64>>,
    config: StoreConfig,
    /// 发过的历史请求个数（凭据的 `request`）
    requests: u64,
    /// 自己的用户 id（事件 `my_id_changed`）；还不知道是 `None`
    me: Option<UserId>,
}

/// 话题设置 → 存下的状态：剩余静音秒数按收到时刻换成绝对到期时刻；没有收到时刻（夹具）当一直静音
fn topic_notify(s: ForumTopicState, received_ms: Option<i64>) -> TopicNotify {
    let muted_until_ms = match (s.notify.mute_for > 0, received_ms) {
        (false, _) => 0,
        (true, Some(r)) => r.saturating_add(s.notify.mute_for.saturating_mul(1000)),
        (true, None) => i64::MAX,
    };
    TopicNotify {
        notify: s.notify,
        muted_until_ms,
        last_read_inbox: s.last_read_inbox,
    }
}

/// 纯函数形式：旧状态 + 一件事 → 新状态
pub fn reduce(mut store: Store, input: Input) -> Store {
    store.apply(input);
    store
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: StoreConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn apply(&mut self, input: Input) {
        self.apply_inner(input);
        // 非当前会话按上限删旧
        let current = self.window.as_ref().map(|w| w.chat_id);
        for chat_id in std::mem::take(&mut self.dirty) {
            if Some(chat_id) != current {
                self.prune(chat_id);
            }
        }
    }

    fn apply_inner(&mut self, input: Input) {
        match input {
            Input::Update { seq, event } => {
                if seq <= self.last_seq {
                    return;
                }
                self.last_seq = seq;
                self.update(seq, event);
            }
            Input::OpenChat { chat_id } => self.open_window(chat_id),
            Input::Refetch => {
                if let Some(chat_id) = self.window.as_ref().map(|w| w.chat_id) {
                    self.open_window(chat_id);
                }
            }
            Input::HistoryLoaded { ticket, messages } => self.history(ticket, messages),
            Input::TopicLoaded {
                chat_id,
                topic_id,
                state,
                received_ms,
            } => {
                self.topics
                    .entry((chat_id, topic_id))
                    .or_insert_with(|| topic_notify(state, Some(received_ms)));
            }
            Input::Resent { old, new } => {
                // 只记一条链；新的那条后来又换过 id 的，稳定键沿链自然传过去
                if old != new {
                    self.parent.insert(new, old);
                }
            }
        }
    }

    // ───── 查询（只读） ─────

    /// 发历史请求前取一张凭据（请求编号加一，所以要 `&mut`）。会话不是当前窗口的，应答回来只进缓存
    pub fn ticket(&mut self, chat_id: ChatId) -> HistoryTicket {
        self.requests += 1;
        HistoryTicket {
            chat_id,
            generation: self.generation,
            since_seq: self.last_seq,
            request: self.requests,
        }
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }
    pub fn user(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }
    /// 自己的用户 id（收到过 `my_id_changed` 才有）
    pub fn me(&self) -> Option<UserId> {
        self.me
    }
    pub fn chat(&self, id: ChatId) -> Option<&Chat> {
        self.chats.get(&id)
    }
    pub fn entry(&self, id: MsgId) -> Option<&Entry> {
        self.table.get(id)
    }
    pub fn message(&self, id: MsgId) -> Option<&Message> {
        match self.table.get(id) {
            Some(Entry::Live(m)) => Some(m),
            Some(Entry::Tombstone(_)) | None => None,
        }
    }
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }
    pub fn status(&self) -> Option<UserStatus> {
        self.status
    }
    /// 范围的静音秒数；没收到过该范围的设置是 `None`
    pub fn scope_mute(&self, scope: NotifyScope) -> Option<i64> {
        self.scope_notify.get(&scope).map(|n| n.mute_for)
    }
    /// 论坛话题的通知状态；没收到过是 `None`
    pub fn topic_notify(&self, chat_id: ChatId, topic_id: i64) -> Option<TopicNotify> {
        self.topics.get(&(chat_id, topic_id)).copied()
    }
    /// 范围的默认通知设置（静音、提及、置顶）；没收到过是 `None`
    pub fn scope_notify(&self, scope: NotifyScope) -> Option<ScopeNotify> {
        self.scope_notify.get(&scope).copied()
    }

    /// 本次运行内稳定的消息键：换过 id 的给最初那个，没换过的就是它自己
    pub fn stable_key(&self, id: MsgId) -> MsgId {
        // 一直走到根；用走过的集合判环（坏数据），不设固定步数——合法的重发链可以很长
        let mut cur = id;
        let mut seen = HashSet::new();
        while seen.insert(cur) {
            match self.parent.get(&cur) {
                Some(&p) => cur = p,
                None => return cur,
            }
        }
        cur
    }

    /// 当前窗口是否需要重新拉历史（有应答因不可信被拒收过）。调用方应发 [`Input::Refetch`]
    pub fn needs_refetch(&self) -> bool {
        self.window.as_ref().is_some_and(|w| w.stale)
    }

    /// 消息现在的 id：换过 id 的旧 id 给最终的新 id（回复、置顶里引用的可能还是旧 id）
    pub fn current_id(&self, id: MsgId) -> MsgId {
        self.table.current(id)
    }

    /// 消息是否已知受限（不论它还在不在缓存里、有没有被撤回）
    pub fn is_restricted(&self, id: MsgId) -> bool {
        self.table.is_restricted(id)
    }

    /// 某个列表当前的会话顺序：按 (order, 会话 id) 从大到小；order 为 0 的不在列表里
    pub fn chat_list(&self, list: &ChatList) -> Vec<ChatId> {
        let mut v: Vec<(i64, ChatId)> = self
            .chats
            .values()
            .filter_map(|c| {
                c.positions
                    .iter()
                    .find(|p| &p.list == list && p.order != 0)
                    .map(|p| (p.order, c.id))
            })
            .collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// 主列表里 `after` 之后第一个有未读的会话；到底不回绕（A3）。`after` 不在列表里时从头找
    pub fn next_unread_chat(&self, after: Option<ChatId>) -> Option<ChatId> {
        let list = self.chat_list(&ChatList::Main);
        let start = after
            .and_then(|a| list.iter().position(|&c| c == a))
            .map_or(0, |i| i + 1);
        list.into_iter()
            .skip(start)
            .find(|id| self.chats.get(id).is_some_and(|c| c.unread_count > 0))
    }

    // ───── 更新 ─────

    fn update(&mut self, seq: u64, event: CoreEvent) {
        match event {
            CoreEvent::NewMessage(m) => {
                let id = m.id;
                self.upsert_snapshot(seq, m);
                self.show_in_window(id);
            }
            CoreEvent::MessageContentChanged { id, content } => {
                self.change(seq, id, FieldChange::Content(content));
            }
            CoreEvent::MessageEphemeralChanged { id, ephemeral } => {
                self.change(seq, id, FieldChange::Ephemeral(ephemeral));
            }
            CoreEvent::MessageEdited { id, edit_date } => {
                // TDLib 用 0 表示没有编辑时间；与整条消息的映射一致，存成 None
                let t = (edit_date.0 > 0).then_some(edit_date);
                self.change(seq, id, FieldChange::Edited(t));
            }
            CoreEvent::MessageContentOpened { id } => self.change(seq, id, FieldChange::Opened),
            CoreEvent::MessageInteractionChanged { id, reactions } => {
                self.change(seq, id, FieldChange::Reactions(reactions));
            }
            CoreEvent::MessagesDeleted {
                chat_id,
                message_ids,
                is_permanent,
                from_cache,
            } => self.delete(seq, chat_id, &message_ids, is_permanent && !from_cache),
            CoreEvent::SendSucceeded { old_id, message }
            | CoreEvent::SendFailed {
                old_id, message, ..
            } => self.rename(seq, old_id, message),
            CoreEvent::ChatAdded {
                mut chat,
                last_message,
            } => {
                // 同一会话重复到达：保留 store 里已维护的最后一条
                chat.last_message = self.chats.get(&chat.id).and_then(|c| c.last_message);
                let id = chat.id;
                self.chats.insert(id, chat);
                if let Some(m) = last_message {
                    self.set_last(seq, id, Some(m));
                }
            }
            CoreEvent::ChatPositionChanged { chat_id, position } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.positions.retain(|p| p.list != position.list);
                    if position.order != 0 {
                        c.positions.push(position);
                    }
                }
            }
            CoreEvent::ChatLastMessageChanged {
                chat_id,
                last_message,
                positions,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id)
                    && let Some(p) = positions
                {
                    c.positions = p.into_iter().filter(|p| p.order != 0).collect();
                }
                self.set_last(seq, chat_id, last_message);
            }
            CoreEvent::ChatDraftChanged { chat_id, positions } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.positions = positions.into_iter().filter(|p| p.order != 0).collect();
                }
            }
            CoreEvent::ChatReadInbox {
                chat_id,
                last_read_inbox_message_id,
                unread_count,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.last_read_inbox = last_read_inbox_message_id;
                    c.unread_count = unread_count;
                }
            }
            CoreEvent::ChatReadOutbox {
                chat_id,
                last_read_outbox_message_id,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.last_read_outbox = last_read_outbox_message_id;
                }
            }
            CoreEvent::ChatMentionCountChanged { chat_id, count } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.unread_mention_count = count;
                }
            }
            CoreEvent::ChatTitleChanged { chat_id, title } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.title = title;
                }
            }
            CoreEvent::ChatNotificationChanged { chat_id, notify } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.notify = notify;
                }
            }
            CoreEvent::ScopeNotificationChanged { scope, notify } => {
                self.scope_notify.insert(scope, notify);
            }
            // 话题静音到期 TDLib 不再推更新：按收线程记下的收到时刻换成绝对到期时刻（store 自己不碰时钟）
            CoreEvent::ForumTopicChanged {
                chat_id,
                topic_id,
                last_read_inbox,
                notify,
                received_ms,
            } => {
                let state = ForumTopicState {
                    last_read_inbox,
                    notify,
                };
                self.topics
                    .insert((chat_id, topic_id), topic_notify(state, received_ms));
            }
            // 正在输入归 `notify`；补发结束只给订阅方看
            CoreEvent::ChatAction { .. } | CoreEvent::SyncDone => {}
            // 0 = 没有（登出后 TDLib 会把选项清掉）
            CoreEvent::MyIdChanged { user_id } => self.me = (user_id != 0).then_some(user_id),
            CoreEvent::UserChanged(u) => {
                self.users.insert(u.id, u);
            }
            CoreEvent::UserStatusChanged(s) => self.status = Some(s),
            // 三条线的原值由后端的 `status()` 给；store 只存合成后的那个
            CoreEvent::AuthStepChanged(_)
            | CoreEvent::NetStateChanged(_)
            | CoreEvent::CoreStateChanged(_) => {}
            CoreEvent::Unknown { .. } => {}
        }
    }

    /// 一整条消息（新消息、会话的最后一条）随更新到达：它就是这个序号时的完整状态，整条替换
    /// （更新按序到达，此前的字段更新都已含在里面）。墓碑不复活，换掉的旧 id 不复活
    fn upsert_snapshot(&mut self, seq: u64, m: Message) {
        self.table.note(&m);
        let id = m.id;
        if self.table.is_tomb(id) || self.table.is_retired(id) {
            return;
        }
        // 补丁、字段序号都不可能比同一条流里后到的整条消息新（更新按序），丢掉
        self.table.drop_orphan(id);
        self.fields.remove(&id);
        self.put_live(m);
        self.versions.insert(id, update_version(seq));
    }

    /// 存活消息（经 [`Table::put_live`] 清过）并维护每会话的活消息索引
    fn put_live(&mut self, m: Message) {
        let id = m.id;
        self.by_chat
            .entry(id.chat_id)
            .or_default()
            .insert(id.message_id);
        self.dirty.insert(id.chat_id);
        self.table.put_live(m);
    }

    fn put_tomb(&mut self, id: MsgId, t: Tomb) {
        if let Some(ids) = self.by_chat.get_mut(&id.chat_id) {
            ids.remove(&id.message_id);
        }
        self.table.put_tomb(id, t);
    }

    fn take(&mut self, id: MsgId) {
        if let Some(ids) = self.by_chat.get_mut(&id.chat_id) {
            ids.remove(&id.message_id);
        }
        self.table.remove(id);
    }

    /// 记一道屏障：凭据 `(since_seq, request)` 不比它新的应答，store 里没有的消息不收
    fn mark_evicted(&mut self, chat_id: ChatId, seq: Version) {
        let w = self.evicted_upto.entry(chat_id).or_default();
        *w = (*w).max(seq);
    }

    /// 非当前会话只留最近 `keep_per_chat` 条活消息；最后一条不删
    fn prune(&mut self, chat_id: ChatId) {
        let keep = self.config.keep_per_chat;
        let last = self.chats.get(&chat_id).and_then(|c| c.last_message);
        let Some(ids) = self.by_chat.get(&chat_id) else {
            return;
        };
        if ids.len() <= keep {
            return;
        }
        let excess = ids.len() - keep;
        let victims: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|&m| Some(m) != last)
            .take(excess)
            .collect();
        if victims.is_empty() {
            return;
        }
        for message_id in victims {
            let id = MsgId {
                chat_id,
                message_id,
            };
            self.take(id);
            self.fields.remove(&id);
            self.versions.remove(&id);
        }
        // 删掉的消息之后还可能收到更新（记成补丁）；早于此刻的请求的应答不许把旧内容带回
        // 此刻之前发出的请求（编号不超过现有的）都算旧
        self.mark_evicted(chat_id, (self.last_seq, self.requests));
    }

    fn show_in_window(&mut self, id: MsgId) {
        if let Some(w) = self.window.as_mut()
            && w.chat_id == id.chat_id
            && self.table.contains(id)
        {
            w.ids.insert(id.message_id);
        }
    }

    fn change(&mut self, seq: u64, id: MsgId, ch: FieldChange) {
        // 换掉的旧 id 不再有消息，也不记补丁（不然晚到的应答会借补丁把它带回来）
        if self.table.is_retired(id) || self.table.is_tomb(id) {
            return;
        }
        if self.table.is_live(id) {
            let f = self.fields.entry(id).or_default();
            match &ch {
                FieldChange::Content(_) => f.content = seq,
                FieldChange::Ephemeral(_) => f.ephemeral = seq,
                FieldChange::Edited(_) => f.edited = seq,
                FieldChange::Reactions(_) => f.reactions = seq,
                FieldChange::Opened => f.opened = seq,
            }
            self.table.modify_live(id, |m| match ch {
                FieldChange::Content(c) => m.content = c,
                FieldChange::Ephemeral(e) => m.ephemeral = e,
                FieldChange::Edited(t) => m.edited = t,
                FieldChange::Reactions(r) => m.reactions = r,
                FieldChange::Opened => m.mark_opened(),
            });
            return;
        }
        // 消息还没到：记成补丁（各字段带序号），消息随应答进来时只补比请求新的字段
        if let Some((evicted, eseq)) = self.table.add_orphan(id, seq, ch) {
            // 补丁之前发出的请求（序号早于补丁）不可信；补丁之后发出的已含这次更新
            self.mark_evicted(evicted.chat_id, (eseq.saturating_sub(1), u64::MAX));
        }
    }

    fn delete(&mut self, seq: u64, chat_id: ChatId, ids: &[i64], revoked: bool) {
        for &message_id in ids {
            let id = MsgId {
                chat_id,
                message_id,
            };
            self.table.drop_orphan(id);
            self.fields.remove(&id);
            self.versions.insert(id, update_version(seq));
            // 还没发出去的（待发送 / 发送失败）被删：是自己取消或重发（TDLib 重发时先发一条
            // 永久删除，`MessagesManager.cpp:25221-25223`；新旧的关联由 `Input::Resent` 给），不是撤回
            let unsent = match self.table.get(id) {
                Some(Entry::Live(m)) => (m.sending != SendingState::Sent).then_some(m.sending),
                Some(Entry::Tombstone(_)) | None => None,
            };
            if revoked && unsent.is_none() {
                let tomb = match self.table.get(id) {
                    Some(Entry::Tombstone(_)) => continue,
                    Some(Entry::Live(m)) => Tomb {
                        sender: Some(m.sender.clone()),
                        date: Some(m.date),
                        outgoing: m.outgoing,
                    },
                    None => Tomb {
                        sender: None,
                        date: None,
                        outgoing: false,
                    },
                };
                // 窗口里的 id 原样留着，投影成原位占位
                self.put_tomb(id, tomb);
            } else {
                // 只是清缓存 / 暂时不可访问 / 没发出去的：不是撤回，不留占位，从窗口拿掉；墓碑不受影响
                if self.table.is_live(id) {
                    self.take(id);
                    if let Some(w) = self.window.as_mut()
                        && w.chat_id == chat_id
                    {
                        w.ids.remove(&message_id);
                    }
                    if let Some(c) = self.chats.get_mut(&chat_id)
                        && c.last_message == Some(message_id)
                    {
                        c.last_message = None;
                    }
                }
            }
        }
    }

    /// 发送成功 / 失败：临时 id 换成新 id，内容取新的那份，位置（窗口、会话最后一条）原样沿用
    fn rename(&mut self, seq: u64, old: MsgId, m: Message) {
        self.table.note(&m);
        let new = m.id;
        // 旧的或新的已撤回：不复活。同一次换 id 重复到达（旧的已不在、早换到这个新 id）：不重复做
        if self.table.is_tomb(old)
            || self.table.is_tomb(new)
            || (!self.table.contains(old) && self.table.retired_to(old) == Some(new))
        {
            return;
        }
        self.take(old);
        if old != new {
            self.versions.insert(old, update_version(seq));
            self.table.retire(old, new);
        }
        self.table.drop_orphan(old);
        self.fields.remove(&old);
        self.fields.remove(&new);
        // 新 id 名下不该有补丁（新 id 是这一刻才有的）；有也是坏数据，丢掉
        self.table.drop_orphan(new);
        if old != new {
            self.parent.insert(new, old);
        }
        self.put_live(m);
        self.versions.insert(new, update_version(seq));
        if let Some(w) = self.window.as_mut()
            && w.chat_id == old.chat_id
        {
            w.ids.remove(&old.message_id);
        }
        // 会话正开着就放进窗口：旧 id 原在窗口里的原位换上（按 id 排序）；旧 id 没见过的也补进来
        self.show_in_window(new);
        if let Some(c) = self.chats.get_mut(&old.chat_id)
            && c.last_message == Some(old.message_id)
        {
            c.last_message = Some(new.message_id);
        }
    }

    fn set_last(&mut self, seq: u64, chat_id: ChatId, m: Option<Message>) {
        // 已换掉的旧 id 指向它现在的 id（旧快照本身下面会被拒收）
        let last = m.as_ref().map(|m| self.table.current(m.id).message_id);
        let Some(c) = self.chats.get_mut(&chat_id) else {
            return;
        };
        c.last_message = last;
        if let Some(m) = m {
            let id = self.current_id(m.id);
            self.upsert_snapshot(seq, m);
            // 最后一条未知期间，新消息可以不带 updateNewMessage 就进会话（`td_api.tl:11312`）：
            // 它作为最后一条到达时，会话正开着就放进窗口
            self.show_in_window(id);
        }
    }

    // ───── 窗口与历史 ─────

    fn open_window(&mut self, chat_id: ChatId) {
        // 关掉的窗口里的撤回占位留下来，下次打开同一会话原位带回（拍板 006 甲3）
        if let Some(old) = self.window.take() {
            let tombs: Vec<i64> = old
                .ids
                .iter()
                .copied()
                .filter(|&m| {
                    self.table.is_tomb(MsgId {
                        chat_id: old.chat_id,
                        message_id: m,
                    })
                })
                .collect();
            self.window_tombs
                .entry(old.chat_id)
                .or_default()
                .extend(tombs);
            // 切走的会话变成「非当前」，按上限删旧
            self.dirty.insert(old.chat_id);
        }
        let carried: BTreeSet<i64> = self
            .window_tombs
            .get(&chat_id)
            .map(|set| {
                set.iter()
                    .copied()
                    .filter(|&m| {
                        self.table.is_tomb(MsgId {
                            chat_id,
                            message_id: m,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.generation += 1;
        self.window = Some(Window {
            chat_id,
            generation: self.generation,
            ids: carried.clone(),
            stale: false,
            carried,
            loaded_from: None,
        });
    }

    fn history(&mut self, t: HistoryTicket, messages: Vec<Message>) {
        let current = self
            .window
            .as_ref()
            .is_some_and(|w| w.chat_id == t.chat_id && w.generation == t.generation);
        // 这份应答确认到的最旧 id（空应答 = 全部）
        let mut oldest_in_response: Option<i64> = None;

        for mut m in messages {
            let id = m.id;
            // 应答只管请求的那个会话；混进来的别的会话的消息不收
            if id.chat_id != t.chat_id {
                continue;
            }
            oldest_in_response =
                Some(oldest_in_response.map_or(id.message_id, |o| o.min(id.message_id)));
            self.table.note(&m);
            // 已换过 id 的旧 id：应答里那份是换 id 之前的，认作新 id（新 id 在就放进窗口）
            if let Some(new) = self.table.retired_to(id) {
                if current && self.table.contains(new) {
                    self.show_in_window(new);
                }
                continue;
            }
            let this = (t.since_seq, t.request);
            let newer_in_store = self.versions.get(&id).is_some_and(|&v| v > this);
            enum Have {
                UnseenTomb,
                Tomb,
                Live,
                Nothing,
            }
            let have = match self.table.get(id) {
                Some(Entry::Tombstone(tb)) if tb.sender.is_none() => Have::UnseenTomb,
                Some(Entry::Tombstone(_)) => Have::Tomb,
                Some(Entry::Live(_)) => Have::Live,
                None => Have::Nothing,
            };
            match have {
                // 没见过原消息就被永久删除的墓碑还待判定：应答说它是没发出去的（待发送 / 失败），
                // 那次删除是取消或重发，不是撤回——拿掉占位（删除的版本屏障留着，旧应答不会把它带回）；
                // 否则补记应答里的发送人和时间（只记这两项，原文不留）
                Have::UnseenTomb => {
                    if m.sending != SendingState::Sent {
                        self.take(id);
                        continue;
                    }
                    self.table.fill_tomb(id, m.sender.clone(), m.date);
                }
                // 撤回过的不加回来；当前窗口里仍在原位显示占位
                Have::Tomb => {}
                // store 的整条快照比应答新：整条不收
                Have::Live if newer_in_store => {}
                // 应答为底，请求之后被更新过的字段取 store 的
                Have::Live => {
                    let f = self.fields.get(&id).copied().unwrap_or_default();
                    let p = match self.table.get(id) {
                        Some(Entry::Live(old)) => Patch::from_live(old, f, t.since_seq),
                        Some(Entry::Tombstone(_)) | None => Patch::default(),
                    };
                    p.apply_to(&mut m, t.since_seq);
                    self.put_live(m);
                    self.versions.insert(id, this);
                }
                Have::Nothing if newer_in_store && !self.table.has_orphan(id) => {
                    // 请求发出后被非撤回的删除拿掉了（清缓存 / 不可访问）：应答里那份是旧的，不收
                    continue;
                }
                Have::Nothing
                    if self
                        .evicted_upto
                        .get(&id.chat_id)
                        .is_some_and(|&b| this <= b) =>
                {
                    // 请求之后这个会话丢过东西：这条可能正是丢掉的那条，旧内容不可信。
                    // 有没有新补丁都一样（新补丁只记了丢之后的字段）。不收，窗口重拉；补丁留给重拉
                    if current && let Some(w) = self.window.as_mut() {
                        w.stale = true;
                    }
                    continue;
                }
                Have::Nothing => {
                    if let Some(p) = self.table.take_orphan(id) {
                        // 补丁的字段序号转成这条消息的字段序号：之后更新的应答到了还能照样合并
                        self.fields.insert(id, p.seqs());
                        p.apply_to(&mut m, t.since_seq);
                    }
                    self.put_live(m);
                    self.versions.insert(id, this);
                }
            }
            if current && let Some(w) = self.window.as_mut() {
                w.ids.insert(id.message_id);
            }
        }
        // 推进加载边界：只由应答推进，新消息不动它
        if current && let Some(w) = self.window.as_mut() {
            let reached = oldest_in_response.unwrap_or(i64::MIN);
            w.loaded_from = Some(w.loaded_from.map_or(reached, |b| b.min(reached)));
        }
    }
}

// 测试放文件末尾（`scripts/check-no-debug.sh` 的约定：从 `#[cfg(test)] mod` 起到文件尾不扫）
#[cfg(test)]
mod tests {
    // 穷举写入路径的受限原文测试（codex 005 第十轮要求）：受限消息的原文、临时内容、被引用的那段，
    // 从**每一条**能写内容的路径喂进来，断言整个 Store 的完整 `{:?}`（单元测试里是派生的完整 Debug，
    // 不是脱敏版）里一次都没有。对照组证明 dump 真能看到内容。

    use super::*;
    use crate::tree::Span;

    const SECRET: &str = "ZQX受限原文";
    const C: ChatId = 101;
    const D: ChatId = -1001;

    fn id(m: i64) -> MsgId {
        MsgId {
            chat_id: C,
            message_id: m,
        }
    }

    fn secret_body() -> Body {
        Body::Content(Content {
            body: vec![Span::Text {
                text: SECRET.into(),
            }],
            attachment: None,
            sensitive: false,
        })
    }

    fn msg(m: i64) -> Message {
        Message {
            id: id(m),
            sender: SenderId::User(7),
            date: Time(1),
            outgoing: false,
            reply_to: None,
            forward: None,
            album_id: None,
            topic: None,
            content: secret_body(),
            ephemeral: Some(secret_body()),
            restriction: None,
            sensitive: false,
            edited: None,
            reactions: vec![],
            sending: SendingState::Sent,
            mentions_me: false,
        }
    }

    /// 不含暗号的普通消息（垫底用，免得把暗号误算成泄漏）
    fn plain(m: i64) -> Message {
        let mut x = msg(m);
        x.content = Body::Content(Content {
            body: vec![Span::Text {
                text: "普通".into(),
            }],
            attachment: None,
            sensitive: false,
        });
        x.ephemeral = None;
        x
    }

    /// 受限的那条（id 5）
    fn r() -> Message {
        let mut m = msg(5);
        m.restriction = Some("版权".into());
        m
    }

    /// 引用 5 的原文的回复（id 6；它自己的正文是普通的，所以正文不用暗号）
    fn reply() -> Message {
        let mut m = msg(6);
        m.content = Body::Content(Content {
            body: vec![Span::Text { text: "回".into() }],
            attachment: None,
            sensitive: false,
        });
        m.ephemeral = None;
        m.reply_to = Some(ReplyRef::Message {
            target: id(5),
            quote: Some(vec![Span::Text {
                text: SECRET.into(),
            }]),
            origin: None,
        });
        m
    }

    struct S(Store);

    impl S {
        fn new() -> Self {
            Self(Store::with_config(StoreConfig { keep_per_chat: 1 }))
        }
        fn up(&mut self, e: CoreEvent) -> &mut Self {
            let seq = self.0.last_seq() + 1;
            self.0.apply(Input::Update { seq, event: e });
            self
        }
        fn hist(&mut self, chat: ChatId, ms: Vec<Message>) -> &mut Self {
            let t = self.0.ticket(chat);
            self.0.apply(Input::HistoryLoaded {
                ticket: t,
                messages: ms,
            });
            self
        }
        fn open(&mut self, chat: ChatId) -> &mut Self {
            self.0.apply(Input::OpenChat { chat_id: chat });
            self
        }
        fn content(&mut self, m: i64) -> &mut Self {
            self.up(CoreEvent::MessageContentChanged {
                id: id(m),
                content: secret_body(),
            })
        }
        fn ephemeral(&mut self, m: i64) -> &mut Self {
            self.up(CoreEvent::MessageEphemeralChanged {
                id: id(m),
                ephemeral: Some(secret_body()),
            })
        }
        fn revoke(&mut self, m: i64) -> &mut Self {
            self.up(CoreEvent::MessagesDeleted {
                chat_id: C,
                message_ids: vec![m],
                is_permanent: true,
                from_cache: false,
            })
        }
        fn clean(&self, what: &str) {
            let dump = format!("{:?}", self.0);
            assert!(!dump.contains(SECRET), "{what}：store 里留了受限原文");
        }
    }

    #[test]
    fn control_dump_really_shows_content() {
        let mut s = S::new();
        s.open(C).up(CoreEvent::NewMessage(msg(5)));
        assert!(
            format!("{:?}", s.0).contains(SECRET),
            "对照组：不受限的应能在 dump 里看到"
        );
    }

    #[test]
    fn restricted_text_never_stored_on_any_write_path() {
        type Scenario = (&'static str, fn(&mut S));
        let scenarios: &[Scenario] = &[
            ("新消息", |s| {
                s.open(C).up(CoreEvent::NewMessage(r()));
            }),
            ("历史应答", |s| {
                s.open(C).hist(C, vec![r()]);
            }),
            ("会话的最后一条", |s| {
                s.up(CoreEvent::ChatAdded {
                    chat: chat(),
                    last_message: Some(r()),
                })
                .up(CoreEvent::ChatLastMessageChanged {
                    chat_id: C,
                    last_message: Some(r()),
                    positions: None,
                });
            }),
            ("发送成功换 id", |s| {
                let mut p = msg(1_000_001);
                p.sending = SendingState::Pending;
                p.content = Body::Content(Content {
                    body: vec![],
                    attachment: None,
                    sensitive: false,
                });
                p.ephemeral = None;
                s.open(C)
                    .up(CoreEvent::NewMessage(p))
                    .up(CoreEvent::SendSucceeded {
                        old_id: id(1_000_001),
                        message: r(),
                    });
            }),
            ("活消息的内容 / 临时内容更新", |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(r()))
                    .content(5)
                    .ephemeral(5);
            }),
            ("同一条的另一份不带限制信息", |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(r()))
                    .up(CoreEvent::NewMessage(msg(5)))
                    .hist(C, vec![msg(5)]);
            }),
            ("补丁先到、受限消息后到", |s| {
                s.open(C).content(5).ephemeral(5).hist(C, vec![r()]);
            }),
            (
                "补丁先到、受限消息一直没被存（被撤回后到的历史）",
                |s| {
                    s.open(C).content(5).revoke(5).hist(C, vec![r()]);
                },
            ),
            ("受限消息被删旧后再来补丁", |s| {
                s.hist(C, vec![r(), plain(9)]).content(5).ephemeral(5);
            }),
            ("受限消息被撤回后再来补丁", |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(r()))
                    .revoke(5)
                    .content(5)
                    .ephemeral(5);
            }),
            ("回复先到、受限消息后到", |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(reply()))
                    .up(CoreEvent::NewMessage(r()));
            }),
            ("受限消息先到、回复后到", |s| {
                s.open(C)
                    .up(CoreEvent::NewMessage(r()))
                    .up(CoreEvent::NewMessage(reply()));
            }),
            (
                "回复先到、受限消息没加载就被撤回、受限历史后到",
                |s| {
                    s.open(C)
                        .up(CoreEvent::NewMessage(reply()))
                        .revoke(5)
                        .hist(C, vec![r()]);
                },
            ),
            ("受限消息被删旧后回复才到", |s| {
                s.hist(C, vec![r(), plain(9)])
                    .open(D)
                    .hist(C, vec![reply()])
                    .up(CoreEvent::NewMessage(reply()));
            }),
            ("回复走历史、受限消息走换 id", |s| {
                let mut p = msg(1_000_001);
                p.sending = SendingState::Pending;
                p.ephemeral = None;
                p.content = Body::Content(Content {
                    body: vec![],
                    attachment: None,
                    sensitive: false,
                });
                let mut rep = reply();
                rep.reply_to = Some(ReplyRef::Message {
                    target: id(1_000_001),
                    quote: Some(vec![Span::Text {
                        text: SECRET.into(),
                    }]),
                    origin: None,
                });
                s.open(C)
                    .up(CoreEvent::NewMessage(p))
                    .hist(C, vec![rep])
                    .up(CoreEvent::SendSucceeded {
                        old_id: id(1_000_001),
                        message: r(),
                    });
            }),
        ];
        for (name, run) in scenarios {
            let mut s = S::new();
            run(&mut s);
            s.clean(name);
            // 窗口切来切去、重拉，也不会把原文带出来
            s.open(D).open(C);
            s.0.apply(Input::Refetch);
            s.clean(name);
        }
    }

    fn chat() -> Chat {
        Chat {
            id: C,
            title: "c".into(),
            kind: ChatKind::Group,
            positions: vec![],
            unread_count: 0,
            unread_mention_count: 0,
            last_read_inbox: 0,
            last_read_outbox: 0,
            notify: ChatNotify::default(),
            peer_user: None,
            last_message: None,
        }
    }
}
