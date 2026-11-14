//! `notify`：新消息、正在输入提不提示的策略 v0（M2.3；`docs/plan/002_roadmap.md`；调研 003 收1–收7）。
//! 纯逻辑：时钟由调用方每次传进来（`now_ms`，Unix 毫秒），测试用假时钟随手拨。
//!
//! 「003 收N」= 调研 `docs/research/003_send-receive-prior-art/README.md` 第一节「#53」那组条目的编号（收1–收16）；
//! 它末尾「建议改契约与路线图」另有自己的第 1–8 条，契约修订记录里说的「建议第 5、6 条」指那个。
//!
//! 用法：消费方每喂完一条事件给 store，再把同一条事件交给 [`Policy::observe`]（store 已是喂过之后的状态），
//! 拿回这条事件的判定 [`Judged`]。CLI 的 `watch` 把判定写进事件行；GUI 的播报器（M4.3）按它出声。
//!
//! **新消息**（按顺序判，先中先停；理由写进 [`Reason`]）：
//! 1. 补发结束（`sync_done`）前不提示——那是补发的积压（契约已有；003 收6）。**重连也算**：连接线一离开 `ready`
//!    就回到「积压」，等下一条 `sync_done`（同 Unigram 连接「更新中」时整批丢）
//! 2. 自己发的不提示（`is_outgoing`，以频道身份发言也算；还没发出、发送失败的也算；003 收3）；收藏夹不提示；
//!    store 里还没有这个会话的不提示（判不了静音，宁可不吵：`unknown_chat`）
//! 3. 别处已读的不提示：消息 id 不大于「已读到」位置——会话的与（论坛里）话题的取较大的那个（003 收4）
//! 4. 超过 1 小时的旧消息不提示（[`Config::max_age_s`]；003 收6，留余量防时钟偏差）
//! 5. 论坛话题的设置还没取到的不提示（`topic_unknown`）：TDLib 只在话题加载过时才推 `updateForumTopic`，
//!    所以先请调用方 `getForumTopic` 取一次（[`Policy::take_missing_topics`]），取到之后这个话题按话题层判
//! 6. 置顶通知关了的，「某人置顶了一条消息」那条系统提示不提示
//! 7. **有效静音**不提示：按「话题 → 会话 → 私聊 / 群 / 频道三档」逐层取（003 收1）。话题静音按收到设置那一刻
//!    换成的绝对到期时刻判（TDLib 话题到期不推更新，003 收2；收线程记收到时刻，store 换算、存下，所以中途接入的
//!    订阅者也拿得到）；会话与范围的到期 TDLib 自己推，按剩余秒数 > 0 判
//! 8. **提及**（`contains_unread_mention`：@ 我、回复我，只算没读过的）且提及通知没关（同样三层取）：同 TDLib，
//!    改按**发送人那个会话**的静音判、不看群本身——发送人没静音就提示（`mention`，静音群里也穿透），
//!    静音了就不提示（`muted_sender`）
//!
//! 提示的再分一次：**同一会话** [`Config::burst_ms`] 内的后续提示 `sound:false`、`burst` 计数加一，不同会话各响各的
//! （003 收7：连发只播一次 + 计数；按会话分开是人定 2026-10-26）。
//!
//! **重连补发**（人定 2026-10-26）：补发完过一次之后连接线又离开 ready，再来的补发消息照旧逐条不提示（`backlog`），
//! 但先记下（最多 [`MISSED_CAP`] 条，超过的不计）；补发结束那条 `sync_done` 上按**那一刻**的 store 逐条再判（后到的
//! 已读、静音都算进去），仍该提示的汇成一次（[`Judged::Summary`]：出一声，「断网期间收到 N 条，涉及 M 个会话」）。
//! 几段补发（途中又断）合成一次；汇总不占各会话的 7 秒窗；启动时的第一次补发不汇总（那是上次关掉以来的积压）；
//! 中途接入正在重连补发的执行方时也当重连（[`Policy::mark_reconnecting`]），只汇总接入后看到的。
//! 论坛话题设置到 `sync_done` 时还没取到的不计（取话题要调用方在喂完这一批之后做，通常赶不上同一批里的 `sync_done`）。
//!
//! **正在输入**（F6、N5；003 收5）：只在**当前打开的私聊**里、对方开始输入、**人自己停下**（不在朗读、不在按键）时
//! 播一次「对方正在输入」。键 = (会话, 话题, 发送人)；取消靠 TDLib 推的 `chatActionCancel`（它 5.5 秒没续就自己推），
//! 本地不计时。人正忙时先记下，[`Policy::set_user_idle`] 报「停下了」时再补播（还在输入的才播）。CLI 没有朗读和按键，
//! 一律当「停下了」。其余动作（录音、选贴纸……）、表情回应都不播。
//!
//! 调研里 #53 的其余条目：收8（拿 TDLib 内置通知接口作对照组）本期**没做**，留待实测（调研第七节⑤，已报大总管）；
//! 收9 归 GUI 播报器（M4.3）；收10（调研叫「积压监控」，这里叫「处理落后」以免与补发的积压混）在 `atg-cli` 的 `watch`；收11–收16 是「不采用」。
//!
//! 同一条消息只判一次：新消息与「最后一条变了」都可能带它来（超级群、频道不开会话时，新消息可能只随后者到达），
//! 后者只认比这个会话见过的都新、且不早于补发结束前 [`LAST_MESSAGE_SLACK_S`] 秒的那条（删掉最新一条后「最后一条」
//! 退回旧消息；补发结束后才加载进来的会话带着它早先的最后一条——都不是新来的）。

use crate::model::{
    Body, ChatActionKind, ChatId, CoreEvent, Message, MsgId, SenderId, SendingState,
};
use crate::status::NetState;
use crate::store::Store;
use crate::tree::{Action, ChatKind, Topic};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

// 有效通知设置拆在子模块里，对外仍是 `notify::Effective` 等原路径
mod settings;

pub use settings::{Effective, Layer, effective};

/// 行为默认值（人定 2026-10-26 按默认收下，界面做出来后实听再调；题面与结果在 `state/handoff/watch.md`「要人定的事」）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// 比现在早这么多秒以上的消息不提示（003 收6，Unigram 同值）
    pub max_age_s: i64,
    /// 同一会话上次出声后这么多毫秒内的提示不再出声（003 收7，Unigram 7 秒；按会话分开是人定 2026-10-26）
    pub burst_ms: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_age_s: 3600,
            burst_ms: 7000,
        }
    }
}

/// 为什么提示 / 不提示（JSON 里是 snake_case 字符串）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    // ── 提示 ──
    /// 新消息
    New,
    /// 含未读提及（@ 我、回复我）、提及通知没关、发送人没被静音：不论群静没静音都提示
    Mention,
    /// 当前打开的私聊里对方正在输入
    Typing,
    /// 重连补发结束：断网期间按规则本该提示的那些，汇成一声（只在 `sync_done` 上；人定 2026-10-26）
    ReconnectSummary,
    // ── 不提示：新消息 ──
    /// 补发结束（`sync_done`）前：启动时、以及每次重连后的补发（积压）
    Backlog,
    /// 自己发的
    Outgoing,
    /// 收藏夹
    Saved,
    /// 别处已读
    ReadElsewhere,
    /// 超过 [`Config::max_age_s`]
    TooOld,
    /// 置顶通知关了
    PinnedOff,
    /// 含提及，但发送人那个会话静音了（同 TDLib：提及按发送人的设置判）
    MutedSender,
    /// 话题静音
    MutedTopic,
    /// 会话静音
    MutedChat,
    /// 私聊 / 群 / 频道那一档静音
    MutedScope,
    /// store 里没有这个会话（判不了静音，宁可不吵）
    UnknownChat,
    /// 论坛话题的设置还没取到（判不了话题静音，宁可不吵；同时请调用方去取，见 [`Policy::take_missing_topics`]）
    TopicUnknown,
    // ── 不提示：正在输入 ──
    /// 不是当前打开的会话
    NotOpenChat,
    /// 不是私聊
    NotPrivate,
    /// 这一轮输入已经播过
    AlreadyAnnounced,
    /// 人正在朗读或按键，等停下再播
    UserBusy,
}

/// 一条判定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Verdict {
    /// 提不提示
    pub alert: bool,
    pub reason: Reason,
    /// 出不出提示音（只有提示才可能出；短窗内的后续提示不出）
    pub sound: bool,
    /// 这是本轮连发里的第几条提示（不提示的为 0）
    pub burst: u32,
}

impl Verdict {
    fn quiet(reason: Reason) -> Self {
        Self {
            alert: false,
            reason,
            sound: false,
            burst: 0,
        }
    }
}

/// 一条事件的判定结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    Message {
        id: MsgId,
        verdict: Verdict,
    },
    Typing {
        key: TypingKey,
        verdict: Verdict,
    },
    /// 重连补发结束的汇总：断网期间本该提示的有几条、涉及几个会话
    Summary {
        messages: u32,
        chats: u32,
        verdict: Verdict,
    },
}

/// 正在输入的键：会话 + 话题 + 发送人（003 收5）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypingKey {
    pub chat_id: ChatId,
    pub topic: Option<Topic>,
    pub sender: SenderId,
}

/// 发送人那个会话（私聊的会话 id 即用户 id；以频道 / 群身份发言的就是那个会话）静音了没有。store 里没有它 =
/// 没有设置，按不静音（同 TDLib：拿不到设置就不拦）
fn sender_muted(store: &Store, sender: &SenderId, now_ms: i64) -> bool {
    let id = match sender {
        SenderId::User(u) => *u,
        SenderId::Chat(c) => *c,
        SenderId::Unknown { .. } => return false,
    };
    store
        .chat(id)
        .is_some_and(|c| effective(store, c, None, now_ms).muted.is_some())
}

/// 只随「最后一条变了」到达的消息，比补发结束早这么多秒以上的不算新来的（留余量防时钟偏差）
pub const LAST_MESSAGE_SLACK_S: i64 = 60;

/// 重连补发最多记多少条待汇总（超了不再记，汇总只算记下的）
const MISSED_CAP: usize = 10_000;

/// 记多少条「判过了」（超了忘最早的；只用来去重，忘了顶多重判一次）
const DECIDED_CAP: usize = 4096;

/// 策略状态。一个订阅者一份（CLI 每个 `watch` 一份；GUI 一份）
#[derive(Debug, Clone)]
pub struct Policy {
    cfg: Config,
    synced: bool,
    /// 最近一次补发结束的时刻（Unix 毫秒）
    synced_at_ms: i64,
    /// 每个会话见过的最大消息 id（判「最后一条变了」是不是新来的）
    seen_max: HashMap<ChatId, i64>,
    decided: HashSet<MsgId>,
    decided_order: VecDeque<MsgId>,
    /// 正在输入：键 → 这一轮播过没有
    typing: HashMap<TypingKey, bool>,
    open_chat: Option<ChatId>,
    user_idle: bool,
    /// 每个会话上次出声的时刻与连发计数（人定 2026-10-26：按会话分开，不同会话各响各的）
    sound: HashMap<ChatId, (i64, u32)>,
    /// 曾经补发完过（之后再来的补发就是重连补发）
    synced_once: bool,
    /// 重连补发里来的消息（人定 2026-10-26：连上后响一声并汇总）。补发结束时再按**那时**的状态逐条判一遍才计数：
    /// 已读位置、静音这些更新常比消息晚到（断网期间在手机上读过的，补发里先到消息、后到已读）
    missed: Vec<Message>,
    /// 判到时还没有设置的论坛话题：等调用方取（每个只要一次）
    missing_topics: Vec<(ChatId, i64)>,
    asked_topics: HashSet<(ChatId, i64)>,
}

impl Policy {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            synced: false,
            synced_at_ms: 0,
            seen_max: HashMap::new(),
            decided: HashSet::new(),
            decided_order: VecDeque::new(),
            typing: HashMap::new(),
            open_chat: None,
            user_idle: true,
            sound: HashMap::new(),
            synced_once: false,
            missed: Vec::new(),
            missing_topics: Vec::new(),
            asked_topics: HashSet::new(),
        }
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    pub fn synced(&self) -> bool {
        self.synced
    }

    /// 中途接入时执行方正在重连补发：这个订阅者也当「重连」，补发结束时汇总它接入之后看到的那些
    pub fn mark_reconnecting(&mut self) {
        self.synced = false;
        self.synced_once = true;
    }

    /// 已同步完（收到 `sync_done`；或中途接入时执行方早已补发完）
    pub fn mark_synced(&mut self, now_ms: i64) {
        self.synced = true;
        self.synced_once = true;
        self.synced_at_ms = now_ms;
    }

    /// 当前打开的会话（CLI：`watch --chat`；GUI：消息列表那个会话）
    pub fn set_open_chat(&mut self, chat: Option<ChatId>) {
        self.open_chat = chat;
    }

    /// 人停下了（不在朗读、不在按键）/ 又忙起来（N5）。停下时补播还在输入、还没播过的
    pub fn set_user_idle(&mut self, store: &Store, idle: bool) -> Vec<Judged> {
        self.user_idle = idle;
        if !idle {
            return vec![];
        }
        let due: Vec<TypingKey> = self
            .typing
            .iter()
            .filter(|(k, announced)| !**announced && self.typing_reason(store, k).is_none())
            .map(|(k, _)| k.clone())
            .collect();
        due.into_iter()
            .map(|key| {
                self.typing.insert(key.clone(), true);
                Judged::Typing {
                    key,
                    verdict: Verdict {
                        alert: true,
                        reason: Reason::Typing,
                        sound: false,
                        burst: 0,
                    },
                }
            })
            .collect()
    }

    /// 交出还没取到设置的论坛话题（会话, 话题 id），每个只交一次。调用方去 `getForumTopic`，应答喂 store
    /// （`Input::TopicLoaded`）；之后这个话题里的消息就按话题层判
    pub fn take_missing_topics(&mut self) -> Vec<(ChatId, i64)> {
        std::mem::take(&mut self.missing_topics)
    }

    /// 喂完 store 之后，判这条事件。与提示无关的事件回 `None`
    pub fn observe(&mut self, store: &Store, event: &CoreEvent, now_ms: i64) -> Option<Judged> {
        match event {
            CoreEvent::SyncDone => {
                let reconnect = self.synced_once && !self.synced;
                let missed = std::mem::take(&mut self.missed);
                self.mark_synced(now_ms);
                if !reconnect {
                    return None;
                }
                // 按补发结束时的状态逐条再判：只算按规则此刻仍该提示的
                let due: Vec<&Message> = missed
                    .iter()
                    .filter(|m| self.rules(store, m, now_ms).is_ok())
                    .collect();
                let chats: HashSet<ChatId> = due.iter().map(|m| m.id.chat_id).collect();
                (!due.is_empty()).then(|| Judged::Summary {
                    messages: u32::try_from(due.len()).unwrap_or(u32::MAX),
                    chats: u32::try_from(chats.len()).unwrap_or(u32::MAX),
                    verdict: Verdict {
                        alert: true,
                        reason: Reason::ReconnectSummary,
                        sound: true,
                        burst: 0,
                    },
                })
            }
            // 重连：连接线离开 ready，之后补发来的都是积压，等下一条 sync_done
            CoreEvent::NetStateChanged(n) => {
                if *n != NetState::Ready {
                    self.synced = false;
                }
                None
            }
            CoreEvent::NewMessage(m) => self.candidate(store, m, now_ms),
            CoreEvent::ChatLastMessageChanged {
                last_message: Some(m),
                ..
            } => {
                let seen = self.seen_max.get(&m.id.chat_id).copied().unwrap_or(0);
                let fresh = !self.synced
                    || m.date.0 >= self.synced_at_ms.div_euclid(1000) - LAST_MESSAGE_SLACK_S;
                if m.id.message_id > seen && fresh {
                    self.candidate(store, m, now_ms)
                } else {
                    None
                }
            }
            CoreEvent::ChatAdded {
                last_message: Some(m),
                ..
            }
            | CoreEvent::SendSucceeded { message: m, .. }
            | CoreEvent::SendFailed { message: m, .. } => {
                // 不判（积压 / 自己发的），只记见过，免得之后随「最后一条变了」再来一次被当成新的
                self.note_seen(m.id);
                self.remember(m.id);
                None
            }
            CoreEvent::ChatAction {
                chat_id,
                topic,
                sender,
                action,
            } => {
                let key = TypingKey {
                    chat_id: *chat_id,
                    topic: topic.clone(),
                    sender: sender.clone(),
                };
                self.typing_event(store, key, action)
            }
            _ => None,
        }
    }

    fn note_seen(&mut self, id: MsgId) {
        let e = self.seen_max.entry(id.chat_id).or_insert(0);
        *e = (*e).max(id.message_id);
    }

    /// 记下判过了；已判过回 false
    fn remember(&mut self, id: MsgId) -> bool {
        if !self.decided.insert(id) {
            return false;
        }
        self.decided_order.push_back(id);
        if self.decided_order.len() > DECIDED_CAP
            && let Some(old) = self.decided_order.pop_front()
        {
            self.decided.remove(&old);
        }
        true
    }

    fn candidate(&mut self, store: &Store, m: &Message, now_ms: i64) -> Option<Judged> {
        self.note_seen(m.id);
        if !self.remember(m.id) {
            return None;
        }
        let verdict = match self.quiet_reason(store, m, now_ms) {
            Ok(reason) => self.alert(m.id.chat_id, reason, now_ms),
            Err(Reason::Backlog) => {
                // 重连补发：先记下，补发结束时按那时的状态判（见 `missed`）。话题设置没有的顺手请调用方去取
                if self.synced_once && self.missed.len() < MISSED_CAP {
                    if let Err(Reason::TopicUnknown) = self.rules(store, m, now_ms) {
                        self.ask_topic(m);
                    }
                    self.missed.push(m.clone());
                }
                Verdict::quiet(Reason::Backlog)
            }
            Err(Reason::TopicUnknown) => {
                self.ask_topic(m);
                Verdict::quiet(Reason::TopicUnknown)
            }
            Err(reason) => Verdict::quiet(reason),
        };
        Some(Judged::Message { id: m.id, verdict })
    }

    /// `Ok(提示的理由)` 或 `Err(不提示的理由)`
    fn quiet_reason(&self, store: &Store, m: &Message, now_ms: i64) -> Result<Reason, Reason> {
        if !self.synced {
            return Err(Reason::Backlog);
        }
        self.rules(store, m, now_ms)
    }

    /// 论坛话题设置还没有：记下请调用方取（每个只一次）
    fn ask_topic(&mut self, m: &Message) {
        if let Some(Topic::Forum { topic_id, .. }) = &m.topic {
            let key = (m.id.chat_id, *topic_id);
            if self.asked_topics.insert(key) {
                self.missing_topics.push(key);
            }
        }
    }

    /// 除「补发中」以外的规则（第 2 条起）
    fn rules(&self, store: &Store, m: &Message, now_ms: i64) -> Result<Reason, Reason> {
        if m.outgoing || m.sending != SendingState::Sent {
            return Err(Reason::Outgoing);
        }
        let chat_id = m.id.chat_id;
        if store.me() == Some(chat_id) {
            return Err(Reason::Saved);
        }
        let chat = store.chat(chat_id).ok_or(Reason::UnknownChat)?;
        let (topic, forum) = match &m.topic {
            Some(Topic::Forum { topic_id, .. }) => (store.topic_notify(chat_id, *topic_id), true),
            _ => (None, false),
        };
        let topic = topic.as_ref();
        let read_upto = chat
            .last_read_inbox
            .max(topic.map_or(0, |t| t.last_read_inbox));
        if m.id.message_id <= read_upto {
            return Err(Reason::ReadElsewhere);
        }
        if now_ms.div_euclid(1000).saturating_sub(m.date.0) > self.cfg.max_age_s {
            return Err(Reason::TooOld);
        }
        // 论坛话题里的消息、话题设置还没取到：判不了话题静音（003 收1 点名的场景），宁可不吵
        if forum && topic.is_none() {
            return Err(Reason::TopicUnknown);
        }
        let eff = effective(store, chat, topic, now_ms);
        let pinned = matches!(&m.content, Body::Service(Action::Pinned { .. }));
        if pinned && eff.disable_pinned {
            return Err(Reason::PinnedOff);
        }
        // 提及（提及通知没关时）：同 TDLib，改按**发送人那个会话**的静音判，群本身静没静音不看
        // （`MessagesManager.cpp:26706-26718`）。所以静音了某人的私聊，他在群里 @ 你也不响
        if m.mentions_me && !eff.disable_mention {
            return if sender_muted(store, &m.sender, now_ms) {
                Err(Reason::MutedSender)
            } else {
                Ok(Reason::Mention)
            };
        }
        match eff.muted {
            None => Ok(Reason::New),
            Some(Layer::Topic) => Err(Reason::MutedTopic),
            Some(Layer::Chat) => Err(Reason::MutedChat),
            Some(Layer::Scope) => Err(Reason::MutedScope),
        }
    }

    fn alert(&mut self, chat: ChatId, reason: Reason, now_ms: i64) -> Verdict {
        let window = self.cfg.burst_ms;
        let e = self.sound.entry(chat).or_insert((i64::MIN, 0));
        let sound = e.0 == i64::MIN || now_ms.saturating_sub(e.0) >= window;
        if sound {
            *e = (now_ms, 1);
        } else {
            e.1 = e.1.saturating_add(1);
        }
        Verdict {
            alert: true,
            reason,
            sound,
            burst: e.1,
        }
    }

    /// 正在输入不该播的理由；该播回 `None`
    fn typing_reason(&self, store: &Store, key: &TypingKey) -> Option<Reason> {
        if self.open_chat != Some(key.chat_id) {
            return Some(Reason::NotOpenChat);
        }
        let private = store
            .chat(key.chat_id)
            .is_some_and(|c| matches!(c.kind, ChatKind::Private | ChatKind::Secret));
        if !private {
            return Some(Reason::NotPrivate);
        }
        if let (SenderId::User(u), Some(me)) = (&key.sender, store.me())
            && *u == me
        {
            return Some(Reason::Outgoing);
        }
        if !self.user_idle {
            return Some(Reason::UserBusy);
        }
        None
    }

    fn typing_event(
        &mut self,
        store: &Store,
        key: TypingKey,
        action: &ChatActionKind,
    ) -> Option<Judged> {
        match action {
            ChatActionKind::Cancel => {
                self.typing.remove(&key);
                None
            }
            ChatActionKind::Other { .. } => None,
            ChatActionKind::Typing => {
                if self.typing.get(&key) == Some(&true) {
                    return Some(Judged::Typing {
                        key,
                        verdict: Verdict::quiet(Reason::AlreadyAnnounced),
                    });
                }
                let verdict = match self.typing_reason(store, &key) {
                    Some(r) => {
                        self.typing.insert(key.clone(), false);
                        Verdict::quiet(r)
                    }
                    None => {
                        self.typing.insert(key.clone(), true);
                        Verdict {
                            alert: true,
                            reason: Reason::Typing,
                            sound: false,
                            burst: 0,
                        }
                    }
                };
                Some(Judged::Typing { key, verdict })
            }
        }
    }
}

#[cfg(test)]
mod tests;
