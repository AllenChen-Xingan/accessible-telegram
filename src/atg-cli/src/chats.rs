//! `chats`：列会话（T02 的数据部分，第一刀第 5 块）。契约：`specs/contracts/cli.md` 第一节第 2 小节。
//!
//! 做法（`docs/plan/001_arch-stack.md` 第六节第 5 条、R14）：
//! 1. 起后端，等授权线离开「等参数」；不是「已登录」就报 `auth_required`
//! 2. 用 `loadChats` 一批批加载，会话与位置经更新进 Store；**顺序只看 Store**（按 (order, 会话 id) 从大到小），
//!    不用 `getChats` 的结果。凑够这一页（多取一个看还有没有下一页）或 TDLib 说到底了就停。**整段加载最多
//!    10 秒**：断网时本地库不够一页，`loadChats` 会转去问服务器、一直没有应答——到点就停，已有的照出，`more:true`
//! 3. 等「补发完了」：离线期间漏掉的更新（新消息、未读数变化）由服务器补送过来，送完 TDLib 把连接状态报成
//!    `ready`。最多等 10 秒；等不到就 `synced:false`，未读数、顺序可能是旧的——断网时就是这样，会话照样从本地库出。
//!    ⚠️ `cli.md`「什么时候算数据齐了」写的是等 `sync_done` 事件；那个事件还没做（归 `watch`），这里暂以
//!    「连接线到 `ready`」代替，两者以后要统一成同一个判据（已报大总管）
//! 4. 投影成 `ChatItem`，一行一个；`--format text` 每行一句 `chat_brief`
//!
//! 冷启动到第一条会话的耗时：从进程启动到 Store 里这个列表第一次有会话，进 `result.data.first_chat_ms`
//! （文字模式写标准错误）。
//!
//! 游标：上一页最后一个会话的 `<order>:<会话 id>`（取自结尾那行 `result` 的 `next_cursor`），下一页从它之后接着列。
//! order 是 TDLib 给会话在某个列表里排序用的数（大的在前，0 = 不在这个列表里），会随新消息变，所以翻页途中
//! 有会话挪位时可能漏或重——与 TDLib 自己的分页同一性质，要全量就一次 `-n` 给够。

use crate::out::Fail;
use atg_core::backend::{Backend, ChatsLoad, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::{EventStream, block_on, block_on_timeout};
use atg_core::model::{ChatId, ChatList};
use atg_core::project;
use atg_core::pump::Pump;
use atg_core::status::{AuthStep, CoreState, NetState};
use atg_core::store::Store;
use atg_core::tree::ChatItem;
use std::time::{Duration, Instant};

/// 等授权线离开「等参数」最多多久
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
/// 等补发完最多多久（`cli.md`：超时 10 秒）
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(10);
/// 整段加载最多多久（断网时 `loadChats` 可能一直不应答）
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(10);

/// 两段等待的上限（单测用短的）
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub load: Duration,
    pub sync: Duration,
}

impl Limits {
    pub const DEFAULT: Self = Self {
        load: LOAD_TIMEOUT,
        sync: SYNC_TIMEOUT,
    };
}
/// 要筛选（`--unread`、`--cursor`）时每批向 TDLib 要几个：凑不准还差几个，按批拉
const LOAD_CHUNK: u32 = 100;
/// 不筛选时每批最少要几个
const LOAD_MIN: u32 = 20;
/// TDLib 一直回「加载了」却不见新会话：连着这么多批就停（防死循环），按「还有更多」交出
const MAX_IDLE_LOADS: u32 = 5;

/// 一页从哪接着列：上一页最后一个会话在列表里的位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub order: i64,
    pub chat_id: ChatId,
}

impl Cursor {
    /// `<order>:<会话 id>`；形状不对 → `None`
    pub fn parse(s: &str) -> Option<Self> {
        let (order, chat_id) = s.split_once(':')?;
        Some(Self {
            order: order.parse().ok()?,
            chat_id: chat_id.parse().ok()?,
        })
    }

    pub fn encode(self) -> String {
        format!("{}:{}", self.order, self.chat_id)
    }

    /// 排序键：TDLib 按 (order, 会话 id) 从大到小
    fn key(self) -> (i64, ChatId) {
        (self.order, self.chat_id)
    }
}

/// 要列什么
#[derive(Debug, Clone)]
pub struct Query {
    pub list: ChatList,
    /// 这一页最多几个（≥ 1）
    pub limit: usize,
    /// 只列有未读的
    pub unread: bool,
    /// 从这之后接着列
    pub after: Option<Cursor>,
}

/// 列出来的结果
pub struct Listing {
    pub items: Vec<ChatItem>,
    pub more: bool,
    /// `more` 为真时才有
    pub next_cursor: Option<String>,
    /// 等到补发完了没有（连接线到 `ready`）
    pub synced: bool,
    /// 加载到点没应答，提前停了（断网时）；这时 `more` 一定为真
    pub load_timed_out: bool,
    /// 进程启动到这个列表第一次有会话；整个列表是空的就没有
    pub first_chat_ms: Option<u64>,
}

/// 起后端 → 列 → 关后端（任何后端都一样；单测用假后端走这里）。`started` 是进程启动的时刻
pub fn run<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &Query,
    started: Instant,
) -> Result<Listing, Fail> {
    run_with(b, profile, q, started, Limits::DEFAULT)
}

/// 同 [`run`]，等待上限可调（单测用短的）
pub fn run_with<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &Query,
    started: Instant,
    limits: Limits,
) -> Result<Listing, Fail> {
    let mut stream = b.subscribe().map_err(Fail::Core)?;
    if let Err(e) = block_on(b.start(profile)) {
        let _ = b.shutdown(SHUTDOWN_TIMEOUT);
        return Err(Fail::Core(e));
    }
    let mut feed = Feed::new(started);
    let r = collect(b, &mut stream, &mut feed, q, limits);
    if let Err(e) = b.shutdown(SHUTDOWN_TIMEOUT) {
        eprintln!("atg-cli: 关闭时出错（{}）", e.code());
    }
    r
}

/// 在已起好的后端上列一页。`feed` 是这个后端的事件流喂成的 Store：直跑时每次新建，`serve` 常驻时一直用同一个
pub fn collect<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &Query,
    limits: Limits,
) -> Result<Listing, Fail> {
    ensure_ready(b)?;

    feed.watch(&q.list);
    feed.drain(stream);
    let mut end = false;
    let mut idle = 0;
    let mut load_timed_out = false;
    let load_deadline = Instant::now() + limits.load;
    loop {
        let have = select(feed.store(), q).len();
        if have > q.limit || end || idle >= MAX_IDLE_LOADS {
            break;
        }
        let filtered = q.unread || q.after.is_some();
        let want = if filtered {
            LOAD_CHUNK
        } else {
            u32::try_from(q.limit + 1 - have)
                .unwrap_or(u32::MAX)
                .max(LOAD_MIN)
        };
        let before = feed.store().chat_list(&q.list).len();
        let left = load_deadline.saturating_duration_since(Instant::now());
        match block_on_timeout(b.chats_load(&q.list, want), left) {
            Some(r) => match r.map_err(Fail::Core)? {
                ChatsLoad::Loaded => {}
                ChatsLoad::End => end = true,
            },
            // 到点没应答（请求被丢下，TDLib 之后的应答没人收）：已有的照出
            None => {
                load_timed_out = true;
                feed.drain(stream);
                break;
            }
        }
        feed.drain(stream);
        if feed.store().chat_list(&q.list).len() > before {
            idle = 0;
        } else {
            idle += 1;
        }
    }

    // 补发完了（连接线到 ready）再定未读数；断网时等不到。加载已经超时的不再多等一轮
    let sync_wait = if load_timed_out {
        Duration::ZERO
    } else {
        limits.sync
    };
    let synced = b.wait_net(sync_wait, &|n| *n == NetState::Ready).is_some();
    feed.drain(stream);

    let mut page = select(feed.store(), q);
    // 多取的那一个只用来判断有没有下一页；没到底就算还有（含防死循环停下的情况）
    let more = page.len() > q.limit || !end || load_timed_out;
    page.truncate(q.limit);
    let next_cursor = more
        .then(|| page.last().map(|&(c, _)| c).or(q.after))
        .flatten()
        .map(Cursor::encode);
    let items = page
        .iter()
        .filter_map(|&(_, id)| project::chat_item(feed.store(), id, &q.list))
        .collect();
    Ok(Listing {
        items,
        more,
        next_cursor,
        synced,
        load_timed_out,
        first_chat_ms: feed.first_chat_ms(&q.list),
    })
}

/// 等授权线离开「等参数」；不是「已登录」→ `auth_required`（`history`、`links` 共用）
pub fn ensure_ready<B: Backend>(b: &B) -> Result<(), Fail> {
    match b.wait_auth(AUTH_TIMEOUT, &|s| s != AuthStep::WaitParameters) {
        Some(AuthStep::Ready) => Ok(()),
        Some(_) => Err(Fail::Core(CoreError::AuthRequired { td: None })),
        None if b.status().lines.core == CoreState::Faulted => {
            Err(Fail::Core(CoreError::Disconnected))
        }
        None => Err(Fail::Core(CoreError::Internal {
            what: "auth_state_timeout",
        })),
    }
}

/// 把事件流喂进 Store，顺带记下各列表第一次有会话的时刻（从 `started` 起算：直跑是本进程启动，`serve` 是执行方启动）
pub struct Feed {
    pump: Pump,
    started: Instant,
    /// 问过的列表 → 第一次有会话的毫秒数
    first: Vec<(ChatList, Option<u64>)>,
}

impl Feed {
    /// 主列表与归档从一开始就记（`serve` 常驻时空闲也在喂，记的是会话真到的时刻，不是第一次被问的时刻）
    pub fn new(started: Instant) -> Self {
        Self {
            pump: Pump::default(),
            started,
            first: vec![(ChatList::Main, None), (ChatList::Archive, None)],
        }
    }

    pub fn store(&self) -> &Store {
        self.pump.store()
    }

    /// 历史应答、开窗口这类不经事件流的输入（`history`）
    pub fn pump_mut(&mut self) -> &mut Pump {
        &mut self.pump
    }

    /// 开始记这个列表第一次有会话的时刻（已经有了就记现在）
    fn watch(&mut self, list: &ChatList) {
        if !self.first.iter().any(|(l, _)| l == list) {
            self.first.push((list.clone(), None));
            self.note_first();
        }
    }

    fn first_chat_ms(&self, list: &ChatList) -> Option<u64> {
        self.first.iter().find(|(l, _)| l == list).and_then(|x| x.1)
    }

    fn note_first(&mut self) {
        let ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let store = self.pump.store();
        for (list, at) in &mut self.first {
            if at.is_none() && !store.chat_list(list).is_empty() {
                *at = Some(ms);
            }
        }
    }

    /// 不等：把已到的事件全喂完
    pub fn drain(&mut self, stream: &mut EventStream) {
        loop {
            let batch = stream.try_batch(512);
            if batch.is_empty() {
                return;
            }
            // 这里不开会话窗口，不会要求重拉
            let _ = self.pump.apply_batch(batch);
            self.note_first();
        }
    }
}

/// 这一页的候选：列表顺序里在游标之后的、（`--unread` 时）有未读的，最多 `limit + 1` 个
fn select(store: &Store, q: &Query) -> Vec<(Cursor, ChatId)> {
    store
        .chat_list(&q.list)
        .into_iter()
        .filter_map(|id| {
            let c = store.chat(id)?;
            let order = c
                .positions
                .iter()
                .find(|p| p.list == q.list && p.order != 0)?
                .order;
            let cur = Cursor { order, chat_id: id };
            let after_ok = q.after.is_none_or(|a| cur.key() < a.key());
            let unread_ok = !q.unread || c.unread_count > 0;
            (after_ok && unread_ok).then_some((cur, id))
        })
        .take(q.limit.saturating_add(1))
        .collect()
}

#[cfg(test)]
mod tests {
    //! 假后端单测：数据全是编的（名字「会话N」、正文「第N条」），不碰真实账号。
    use super::*;
    use atg_core::config::ServerKind;
    use atg_core::error::map_td_error;
    use atg_core::fake::{Call, FakeAuth, FakeBackend};
    use atg_core::speech::chat_brief;
    use atg_core::tree::{ChatKind, ChatLast};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn temp_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "atg-cli-chats-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn profile(tag: &str) -> Profile {
        Profile::create(&temp_root(tag), "main", ServerKind::Main).unwrap()
    }

    /// 已登录的假后端
    fn backend() -> FakeBackend {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth {
            logged_in: true,
            ..FakeAuth::default()
        });
        b
    }

    fn list_json(list: &ChatList) -> Value {
        match list {
            ChatList::Archive => json!({"@type": "chatListArchive"}),
            _ => json!({"@type": "chatListMain"}),
        }
    }

    fn position(id: i64, list: &ChatList, order: i64, pinned: bool) -> Value {
        json!({"@type": "updateChatPosition", "chat_id": id, "position": {
            "@type": "chatPosition", "list": list_json(list), "order": order.to_string(), "is_pinned": pinned,
        }})
    }

    /// 一个私聊会话：`updateNewChat`（TDLib 加载时位置是空的）+ 位置
    fn chat_in(list: &ChatList, id: i64, order: i64, patch: Value) -> Vec<Value> {
        let mut c = json!({
            "@type": "chat", "id": id, "title": format!("会话{id}"),
            "type": {"@type": "chatTypePrivate", "user_id": id},
            "positions": [], "unread_count": 0, "unread_mention_count": 0,
            "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
        });
        if let (Some(c), Value::Object(p)) = (c.as_object_mut(), patch) {
            c.extend(p);
        }
        vec![
            json!({"@type": "updateNewChat", "chat": c}),
            position(id, list, order, false),
        ]
    }

    fn chat(id: i64, order: i64) -> Vec<Value> {
        chat_in(&ChatList::Main, id, order, json!({}))
    }

    fn last_message(chat_id: i64, msg_id: i64, text: &str, outgoing: bool) -> Value {
        json!({"@type": "updateChatLastMessage", "chat_id": chat_id, "last_message": {
            "@type": "message", "id": msg_id, "chat_id": chat_id,
            "sender_id": {"@type": "messageSenderUser", "user_id": if outgoing { 1 } else { chat_id }},
            "is_outgoing": outgoing, "date": 1_792_755_420, "edit_date": 0,
            "media_album_id": "0",
            "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}},
        }})
    }

    fn query(limit: usize) -> Query {
        Query {
            list: ChatList::Main,
            limit,
            unread: false,
            after: None,
        }
    }

    fn ids(l: &Listing) -> Vec<i64> {
        l.items.iter().map(|c| c.chat_id).collect()
    }

    /// 起后端之前先把连接线推到 ready（补发完了）
    fn ready(b: &FakeBackend) {
        b.push_net("connectionStateReady");
    }

    fn fast() -> Limits {
        Limits {
            load: Duration::from_secs(5),
            sync: Duration::from_millis(300),
        }
    }

    fn run_fast(b: &FakeBackend, p: &Profile, q: &Query) -> Result<Listing, Fail> {
        run_with(b, p, q, Instant::now(), fast())
    }

    fn ok(r: Result<Listing, Fail>) -> Listing {
        match r {
            Ok(l) => l,
            Err(Fail::Core(e)) => panic!("出错：{}", e.code()),
            Err(_) => panic!("出错"),
        }
    }

    fn core_err(r: Result<Listing, Fail>) -> CoreError {
        match r {
            Err(Fail::Core(e)) => e,
            Err(_) => panic!("不是 CoreError"),
            Ok(_) => panic!("没出错"),
        }
    }

    #[test]
    fn order_follows_store_not_load_order() {
        let b = backend();
        // TDLib 交出的先后与 order 不一致；同 order 按会话 id 从大到小
        b.set_chats(
            ChatList::Main,
            vec![chat(10, 300), chat(11, 500), chat(12, 300), chat(13, 100)],
        );
        ready(&b);
        let l = ok(run_fast(&b, &profile("order"), &query(10)));
        assert_eq!(ids(&l), vec![11, 12, 10, 13]);
        assert!(!l.more);
        assert_eq!(l.next_cursor, None);
        assert!(l.synced);
        assert!(l.first_chat_ms.is_some());
    }

    #[test]
    fn updates_while_waiting_for_sync_reorder_the_list() {
        let b = backend();
        b.set_chats(
            ChatList::Main,
            vec![chat(1, 300), chat(2, 200), chat(3, 100)],
        );
        let p = profile("reorder");
        let l = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                // 补发途中：会话 3 来了新消息挪到最前，会话 1 被挪出主列表（如归档）
                b.push(position(3, &ChatList::Main, 900, false));
                b.push(position(1, &ChatList::Main, 0, false));
                b.push(last_message(3, 1 << 20, "第一条", false));
                ready(&b);
            });
            ok(run_with(
                &b,
                &p,
                &query(10),
                Instant::now(),
                Limits {
                    load: Duration::from_secs(5),
                    sync: Duration::from_secs(5),
                },
            ))
        });
        assert!(l.synced);
        assert_eq!(ids(&l), vec![3, 2]);
        let last = l.items[0].last.as_deref();
        assert!(matches!(last, Some(ChatLast::Message(_))));
    }

    #[test]
    fn fields_unread_mentions_mute_pin_last() {
        let b = backend();
        b.set_chats(
            ChatList::Main,
            vec![
                // 自己设了静音、有未读与提及、置顶
                [
                    chat_in(
                        &ChatList::Main,
                        1,
                        900,
                        json!({
                            "unread_count": 7, "unread_mention_count": 2,
                            "notification_settings": {"@type": "chatNotificationSettings",
                                "use_default_mute_for": false, "mute_for": 3600},
                        }),
                    ),
                    vec![
                        position(1, &ChatList::Main, 900, true),
                        last_message(1, 2 << 20, "明天见。", false),
                    ],
                ]
                .concat(),
                // 跟随默认：私聊范围静音 → 有效静音
                [
                    chat_in(&ChatList::Main, 2, 800, json!({})),
                    vec![last_message(2, 3 << 20, "收到", true)],
                ]
                .concat(),
                // 一个群：跟随默认，群范围不静音
                chat_in(
                    &ChatList::Main,
                    3,
                    700,
                    json!({"type": {"@type": "chatTypeBasicGroup", "basic_group_id": 3}}),
                ),
            ],
        );
        b.push(json!({"@type": "updateScopeNotificationSettings",
            "scope": {"@type": "notificationSettingsScopePrivateChats"},
            "notification_settings": {"@type": "scopeNotificationSettings", "mute_for": 86400}}));
        b.push(json!({"@type": "updateScopeNotificationSettings",
            "scope": {"@type": "notificationSettingsScopeGroupChats"},
            "notification_settings": {"@type": "scopeNotificationSettings", "mute_for": 0}}));
        ready(&b);
        let l = ok(run_fast(&b, &profile("fields"), &query(10)));
        assert_eq!(ids(&l), vec![1, 2, 3]);
        let [a, c2, g] = l.items.as_slice() else {
            panic!("要三个")
        };
        assert_eq!((a.unread_count, a.unread_mention_count), (7, 2));
        assert!(a.muted && !a.mute_inherited && a.pinned);
        assert_eq!(chat_brief(a), "会话1，7 条未读，2 条提到你，明天见");
        assert!(c2.muted && c2.mute_inherited && !c2.pinned);
        assert_eq!(chat_brief(c2), "会话2，我：收到");
        assert_eq!(g.kind, ChatKind::Group);
        assert!(!g.muted && g.mute_inherited);
        assert!(g.last.is_none());
        assert_eq!(chat_brief(g), "会话3");
    }

    #[test]
    fn last_message_is_not_truncated() {
        // R2：最后一条不截断
        let long = "很长".repeat(100);
        let b = backend();
        b.set_chats(
            ChatList::Main,
            vec![[chat(1, 9), vec![last_message(1, 1 << 20, &long, false)]].concat()],
        );
        ready(&b);
        let l = ok(run_fast(&b, &profile("long"), &query(1)));
        assert_eq!(chat_brief(&l.items[0]), format!("会话1，{long}"));
    }

    #[test]
    fn empty_list() {
        let b = backend();
        ready(&b);
        let l = ok(run_fast(&b, &profile("empty"), &query(100)));
        assert!(l.items.is_empty());
        assert!(!l.more);
        assert_eq!(l.next_cursor, None);
        assert_eq!(l.first_chat_ms, None);
        assert!(l.synced);
        assert!(b.calls().contains(&Call::ChatsLoad {
            list: ChatList::Main,
            limit: 101,
        }));
    }

    /// 超多会话的 order：TDLib 的 `loadChats` 从列表顶上往下交，所以按交出的先后递减
    fn order_of(id: i64) -> i64 {
        1_000_000 - id
    }

    #[test]
    fn many_chats_page_with_cursor_covers_everything_once() {
        const N: i64 = 2500;
        let b = backend();
        b.set_chats(
            ChatList::Main,
            (1..=N).map(|id| chat(id, order_of(id))).collect(),
        );
        ready(&b);
        let p = profile("many");
        let first = ok(run_fast(&b, &p, &query(100)));
        assert_eq!(first.items.len(), 100);
        assert!(first.more);
        let cursor = first.next_cursor.clone().unwrap();
        // 只加载了够一页的，没把 2500 个全拉下来
        let loads: u32 = b
            .calls()
            .iter()
            .filter_map(|c| match c {
                Call::ChatsLoad { limit, .. } => Some(*limit),
                _ => None,
            })
            .sum();
        assert!(loads < 300, "{loads}");

        // 一次全列：与 store 顺序一致、严格递减、不重
        let b = backend();
        b.set_chats(
            ChatList::Main,
            (1..=N).map(|id| chat(id, order_of(id))).collect(),
        );
        ready(&b);
        let all = ok(run_fast(&b, &profile("many-all"), &query(100_000)));
        assert_eq!(all.items.len(), usize::try_from(N).unwrap());
        assert!(!all.more);
        let keys: Vec<(i64, i64)> = all
            .items
            .iter()
            .map(|c| (order_of(c.chat_id), c.chat_id))
            .collect();
        assert!(keys.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(ids(&first), ids(&all)[..100].to_vec());

        // 用游标接着翻：每页接上、整体等于全列
        let mut got = ids(&first);
        let mut cur = Some(cursor);
        while let Some(c) = cur {
            let b = backend();
            b.set_chats(
                ChatList::Main,
                (1..=N).map(|id| chat(id, order_of(id))).collect(),
            );
            ready(&b);
            let q = Query {
                after: Cursor::parse(&c),
                ..query(1000)
            };
            let page = ok(run_fast(&b, &profile("many-page"), &q));
            got.extend(ids(&page));
            cur = page.next_cursor;
        }
        assert_eq!(got, ids(&all));
    }

    #[test]
    fn unread_filter_keeps_loading_until_page_is_full() {
        let b = backend();
        // 300 个会话，每 50 个里第一个有未读
        b.set_chats(
            ChatList::Main,
            (1..=300)
                .map(|id| {
                    let unread = if id % 50 == 1 { 3 } else { 0 };
                    chat_in(
                        &ChatList::Main,
                        id,
                        1000 - id,
                        json!({ "unread_count": unread }),
                    )
                })
                .collect(),
        );
        ready(&b);
        let q = Query {
            unread: true,
            ..query(5)
        };
        let l = ok(run_fast(&b, &profile("unread"), &q));
        assert_eq!(ids(&l), vec![1, 51, 101, 151, 201]);
        assert!(l.more);
        assert!(l.items.iter().all(|c| c.unread_count > 0));
    }

    #[test]
    fn archive_is_a_separate_list() {
        let b = backend();
        b.set_chats(ChatList::Main, vec![chat(1, 10)]);
        b.set_chats(
            ChatList::Archive,
            vec![
                chat_in(&ChatList::Archive, 2, 20, json!({})),
                chat_in(&ChatList::Archive, 3, 30, json!({})),
            ],
        );
        ready(&b);
        let q = Query {
            list: ChatList::Archive,
            ..query(10)
        };
        let l = ok(run_fast(&b, &profile("archive"), &q));
        assert_eq!(ids(&l), vec![3, 2]);
        assert!(b.calls().iter().all(|c| !matches!(
            c,
            Call::ChatsLoad {
                list: ChatList::Main,
                ..
            }
        )));
    }

    #[test]
    fn not_synced_when_connection_never_ready() {
        // 断网：会话照样从（假）本地库出，只是 synced=false
        let b = backend();
        b.set_chats(ChatList::Main, vec![chat(1, 10)]);
        b.push_net("connectionStateWaitingForNetwork");
        let l = ok(run_fast(&b, &profile("offline"), &query(10)));
        assert_eq!(ids(&l), vec![1]);
        assert!(!l.synced);
    }

    #[test]
    fn load_that_never_answers_stops_at_the_deadline() {
        // 断网、本地库不够一页：第一批出了本地的，再要就一直没应答。到点照出已有的，不卡死
        let b = backend();
        b.set_chats(ChatList::Main, vec![chat(1, 20), chat(2, 10)]);
        b.hang_chats_load_after(1);
        b.push_net("connectionStateWaitingForNetwork");
        let t = Instant::now();
        let l = ok(run_with(
            &b,
            &profile("hang"),
            &query(100),
            Instant::now(),
            Limits {
                load: Duration::from_millis(300),
                sync: Duration::from_secs(30),
            },
        ));
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "加载超时后不该再等补发"
        );
        assert_eq!(ids(&l), vec![1, 2]);
        assert!(l.load_timed_out && l.more && !l.synced);
        assert_eq!(l.next_cursor.as_deref(), Some("10:2"));
    }

    #[test]
    fn not_logged_in_is_auth_required() {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth::default());
        let e = core_err(run_fast(&b, &profile("noauth"), &query(10)));
        assert_eq!(e.code(), "auth_required");
        // 后端照样关掉
        assert_eq!(b.calls().last(), Some(&Call::Shutdown));
    }

    #[test]
    fn load_error_is_reported_and_backend_closed() {
        let b = backend();
        b.set_chats(ChatList::Main, vec![chat(1, 10)]);
        b.fail_next(
            "chats_load",
            map_td_error(429, "Too Many Requests: retry after 7"),
        );
        ready(&b);
        let e = core_err(run_fast(&b, &profile("loaderr"), &query(10)));
        assert_eq!(e.code(), "rate_limited");
        assert_eq!(b.calls().last(), Some(&Call::Shutdown));
    }

    #[test]
    fn cursor_roundtrip_and_rejects_garbage() {
        let c = Cursor {
            order: 9_221_294_780_217_032_704,
            chat_id: -1_001_234_567_890,
        };
        assert_eq!(Cursor::parse(&c.encode()), Some(c));
        for bad in ["", "1", "a:b", "1:2:3", ":5", "5:"] {
            assert_eq!(Cursor::parse(bad), None, "{bad}");
        }
    }
}
