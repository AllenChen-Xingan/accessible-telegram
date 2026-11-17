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

mod query;
mod table;
mod ticket;
mod transcripts;
mod write;
use table::Table;
pub use ticket::{HistoryTicket, TranscriptTicket};
pub use transcripts::{Stale, Transcribed};

/// 记不全的「孤儿补丁」最多留多少条（见 [`Patch`]）
const MAX_ORPHANS: usize = 4096;

/// store 的可调参数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreConfig {
    /// 非当前会话最多留多少条活消息（拍板 006 甲1）。超出的从最旧的删起；
    /// 会话的最后一条、待发的、墓碑、换 id 的链不算在内、不删。当前打开的会话不限。
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
    /// 一条语音转写完了：凭据过得了裁决（[`Store::transcript_verdict`]）才写进转写表，过不了丢掉
    Transcribed(Transcribed),
    /// `getForumTopic` 的应答（`received_ms`：收到应答的时刻，换静音到期用）。**只在 store 里还没有这个话题时收**：
    /// 应答可能比之后推来的 `updateForumTopic` 旧，不许盖掉它
    TopicLoaded {
        chat_id: ChatId,
        topic_id: i64,
        state: ForumTopicState,
        received_ms: i64,
    },
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
    /// 转写表（[`transcripts`]）：消息现在的 id → 转写。只在内存
    transcripts: transcripts::Transcripts,
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
        self.tidy_transcripts();
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
            Input::Transcribed(t) => self.commit_transcript(t),
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
}

// 测试放文件末尾（`scripts/check-no-debug.sh` 的约定：从 `#[cfg(test)] mod` 起到文件尾不扫）
#[cfg(test)]
mod tests;
