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

use crate::deadlines::{AUTH_TIMEOUT, LOAD_TIMEOUT, START_TIMEOUT, SYNC_TIMEOUT, bounded};
use crate::out::Fail;
use atg_core::backend::{Backend, ChatsLoad, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::link::{EventStream, block_on_timeout};
use atg_core::model::{ChatId, ChatList};
use atg_core::project;
use atg_core::pump::Pump;
use atg_core::status::{AuthStep, NetState};
use atg_core::store::Store;
use atg_core::tree::ChatItem;
use std::time::{Duration, Instant};

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
    if let Err(e) = bounded(b, b.start(profile), START_TIMEOUT) {
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
        // 到点：内核故障 → `disconnected`，否则 `network`（`cli.md`「最多等多久」，codex 022 第 2 轮 #3）
        None => Err(Fail::Core(crate::deadlines::stalled(b))),
    }
}

/// 把事件流喂进 Store，顺带记下各列表第一次有会话的时刻（从 `started` 起算：直跑是本进程启动，`serve` 是执行方启动）
pub struct Feed {
    pump: Pump,
    started: Instant,
    /// 问过的列表 → 第一次有会话的毫秒数
    first: Vec<(ChatList, Option<u64>)>,
    /// `watch` 的订阅者（直跑时是本进程的标准输出，`serve` 时是各个连进来的 `watch`）：每喂完一条事件就交给它们，
    /// 所以不论是空闲时喂、还是别的命令（`chats`、`history`）中途喂，订阅者都不漏
    pub hub: crate::watch::Hub,
    /// 喂过的最后一个序号（中途接入的 `watch` 补 `sync_done` 时用）
    last_seq: u64,
    /// 喂到的事件里，最近一次补发已结束（见过 `sync_done`，之后连接线没再离开 ready）。中途接入的 `watch` 按它判，
    /// 不按后端此刻的连接线：收线程先改连接线、事件还排在流里没喂，两者会差一截
    synced: bool,
    /// 喂到的事件里见过至少一次 `sync_done`（之后不在补发中的话，就是重连补发）
    ever_synced: bool,
}

impl Feed {
    /// 主列表与归档从一开始就记（`serve` 常驻时空闲也在喂，记的是会话真到的时刻，不是第一次被问的时刻）
    pub fn new(started: Instant) -> Self {
        Self {
            pump: Pump::default(),
            started,
            first: vec![(ChatList::Main, None), (ChatList::Archive, None)],
            hub: crate::watch::Hub::default(),
            last_seq: 0,
            synced: false,
            ever_synced: false,
        }
    }

    pub fn ever_synced(&self) -> bool {
        self.ever_synced
    }

    pub fn synced(&self) -> bool {
        self.synced
    }

    fn note_sync(&mut self, event: &atg_core::model::CoreEvent) {
        use atg_core::model::CoreEvent;
        match event {
            CoreEvent::SyncDone => {
                self.synced = true;
                self.ever_synced = true;
            }
            CoreEvent::NetStateChanged(n) if *n != NetState::Ready => self.synced = false,
            _ => {}
        }
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    pub fn store(&self) -> &Store {
        self.pump.store()
    }

    /// 喂一条事件（发送等结果时逐条喂，不丢）
    pub fn apply(&mut self, env: atg_core::link::Envelope) {
        let _ = self.pump.apply_batch(vec![env]);
        self.note_first();
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

    /// 不等：把已到的事件全喂完，回喂了几条（`watch` 拿它看处理落后多少）
    pub fn drain(&mut self, stream: &mut EventStream) -> usize {
        let mut fed = 0;
        loop {
            let batch = stream.try_batch(512);
            if batch.is_empty() {
                return fed;
            }
            fed += batch.len();
            if let Some(e) = batch.last() {
                self.last_seq = e.seq;
            }
            for e in &batch {
                self.note_sync(&e.event);
            }
            if self.hub.is_empty() {
                // 这里不开会话窗口，不会要求重拉
                let _ = self.pump.apply_batch(batch);
            } else {
                // 有订阅者：一条一条喂，每条喂完交给订阅者（判提示要看喂过之后的 store）
                for e in batch {
                    let (seq, event) = (e.seq, e.event.clone());
                    let _ = self.pump.apply(e.into_input());
                    self.hub.on_event(self.pump.store(), seq, &event);
                }
            }
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
mod tests;
