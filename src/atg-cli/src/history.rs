//! `history`、`links`：读某会话的消息、列出一条消息里的链接（T03 T06 的数据部分，第一刀第 8 块，#21）。
//! 契约：`specs/contracts/cli.md` 第一节第 2 小节（参数与 `result.data` 经大总管授权于 2026-10-26 改过）。
//!
//! **只读，不标已读**：只用 `getChat`（本地没有私聊时 `createPrivateChat(force=false)`）、`getChatHistory`、
//! `getMessage`；绝不调 `openChat` / `viewMessages`（`actions.md` A1：CLI 的 history / show 不标已读）。
//!
//! 落点（F2 人定：进会话落在第一条未读）：
//! - 不给 `--before` / `--after`：有未读就从第一条未读起往新取 N 条（`landing:"first_unread"`）——第一条未读 =
//!   `last_read_inbox` 之后第一条**别人发的**（自己发的不算未读）；没有未读取最新 N 条（`landing:"latest"`）。
//!   会话从没读过（`last_read_inbox` 为 0）时也取最新 N 条：要从整段历史的最旧一条起，得把整个会话拉一遍
//! - `--before X`（别名 `--from`）：比 X 旧的 N 条；`--after X`：比 X 新的 N 条；都不含 X
//!
//! 一页一律**从旧到新**输出（阅读顺序，落点在第一行）。TDLib 一次给的常比要的少（首次常只回 1 条），
//! 这里循环补满。只有空应答（或往新翻到了会话的最后一条）才算到头；TDLib 不空却没给新的（停住）时稍等再要，
//! 连着 [`MAX_IDLE`] 次还是这样就停，`partial` 为真，这一页可能不满 N 条，稍后重试同一条命令：拿到了消息，
//! 就按「可能还有」交出**停住的那一侧**（往旧停住是 `more`、往新停住是 `more_newer` 为真、照给游标），另一侧照常判断；
//! 一条都没拿到就没有游标可给，两侧都为假，只看 `partial`。
//!
//! 每条消息按 Store 投影成内容树（名字现查），读法 `speech.brief`；`--detail` 另带 `speech.detail`（日期、回复谁、
//! 转发自、已编辑、表情回应、话题名、送达状态；句子待人实听），`--full` 另带 `speech.full`（全文）。隐藏内容（剧透、18+）
//! 三层都只读「隐藏内容」，`--reveal`（= 人按了展开键）才读原文（R1）。

use crate::args::{Format, HistoryArgs, LinksArgs};
use crate::chats::{self, Feed};
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::{Backend, HistoryReq, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::json::Layers;
use atg_core::link::{EventStream, block_on};
use atg_core::model::{Body, ChatId, MsgId};
use atg_core::project;
use atg_core::speech::{self, SpeechCtx};
use atg_core::status::{CoreState, NetState};
use atg_core::store::Input;
use atg_core::tree::Doc;
use atg_core::tree::text::links_marked;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// 连着几次没拿到新消息就停（防 TDLib 一直回同一批时死循环）
pub const MAX_IDLE: u32 = 3;
/// 等「补发完了」最多多久（同 `chats`）；未读位置要等补发完才准
pub const SYNC_TIMEOUT: Duration = chats::SYNC_TIMEOUT;
/// `saved` 要自己的用户 id：登录后 TDLib 自己推 `my_id`，最多等这么久
const ME_TIMEOUT: Duration = Duration::from_secs(5);

/// 会话：数字 id，或 `saved`（自己的收藏夹）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRef {
    Id(ChatId),
    Saved,
}

impl ChatRef {
    /// `saved` 或整数；别的 → `None`（用法错误）
    pub fn parse(s: &str) -> Option<Self> {
        if s == "saved" {
            return Some(Self::Saved);
        }
        s.parse().ok().map(Self::Id)
    }
}

/// 从哪取
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// F2 落点：第一条未读，没有就最新
    Landing,
    Before(i64),
    After(i64),
}

#[derive(Debug, Clone, Copy)]
pub struct Query {
    pub chat: ChatRef,
    pub anchor: Anchor,
    /// 一页几条（≥ 1）
    pub limit: usize,
    pub local_only: bool,
}

/// 取到的一页
pub struct Page {
    pub chat_id: ChatId,
    /// 从旧到新
    pub docs: Vec<Doc>,
    /// 往旧还有
    pub more: bool,
    /// 往新还有
    pub more_newer: bool,
    /// 本页里的第一条未读（没有未读、或不在本页 → `None`）
    pub first_unread: Option<i64>,
    pub landing: &'static str,
    /// 等到补发完了没有
    pub synced: bool,
    /// 停住了（TDLib 连着几次不空却没给新的：往新是只回起点那一条，往旧是只有起点或更新的）：这页可能不满，
    /// 稍后重试同一条命令。一条都没拿到时 `more` / `more_newer` 为假，只看它
    pub partial: bool,
}

impl Page {
    fn ids(&self) -> impl Iterator<Item = i64> + '_ {
        self.docs.iter().filter_map(doc_id)
    }
}

fn doc_id(d: &Doc) -> Option<i64> {
    match d {
        Doc::Message(m) => Some(m.id.message_id),
        Doc::Service(s) => Some(s.id.message_id),
        Doc::Tombstone(t) => Some(t.id.message_id),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Unknown(_) => None,
    }
}

/// 参数 → 查询；形状不对是用法错误。直跑与执行方都走这里（执行方收到的参数绕过了 clap，检查只在这一处）
pub fn query(a: &HistoryArgs) -> Result<Query, Fail> {
    let chat = ChatRef::parse(&a.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?;
    if !(1..=1000).contains(&a.limit) {
        return Err(Fail::Usage("-n 要在 1 到 1000 之间"));
    }
    let anchor = match (a.before, a.after) {
        (None, None) => Anchor::Landing,
        (Some(x), None) => Anchor::Before(x),
        (None, Some(x)) => Anchor::After(x),
        (Some(_), Some(_)) => return Err(Fail::Usage("--before 和 --after 只能给一个")),
    };
    Ok(Query {
        chat,
        anchor,
        limit: usize::try_from(a.limit).unwrap_or(usize::MAX),
        local_only: a.local,
    })
}

// ───────────────────────── 起后端 → 取 → 关 ─────────────────────────

/// 直跑：起后端、取一页、关后端（任何后端都一样；单测用假后端走这里）
pub fn run<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &Query,
    started: Instant,
) -> Result<Page, Fail> {
    with_backend(b, profile, started, |b, stream, feed| {
        collect(b, stream, feed, q, SYNC_TIMEOUT)
    })
}

/// 直跑 `links`
pub fn run_links<B: Backend>(
    b: &B,
    profile: &Profile,
    q: &LinksQuery,
    started: Instant,
) -> Result<LinkList, Fail> {
    with_backend(b, profile, started, |b, stream, feed| {
        collect_links(b, stream, feed, q)
    })
}

fn with_backend<B: Backend, T>(
    b: &B,
    profile: &Profile,
    started: Instant,
    f: impl FnOnce(&B, &mut EventStream, &mut Feed) -> Result<T, Fail>,
) -> Result<T, Fail> {
    let mut stream = b.subscribe().map_err(Fail::Core)?;
    if let Err(e) = block_on(b.start(profile)) {
        let _ = b.shutdown(SHUTDOWN_TIMEOUT);
        return Err(Fail::Core(e));
    }
    let mut feed = Feed::new(started);
    let r = f(b, &mut stream, &mut feed);
    if let Err(e) = b.shutdown(SHUTDOWN_TIMEOUT) {
        eprintln!("atg-cli: 关闭时出错（{}）", e.code());
    }
    r
}

/// 会话 id：`saved` 换成自己的用户 id（与自己的私聊，会话 id 即用户 id）
fn resolve<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    chat: ChatRef,
) -> Result<ChatId, Fail> {
    match chat {
        ChatRef::Id(id) => Ok(id),
        ChatRef::Saved => {
            let deadline = Instant::now() + ME_TIMEOUT;
            loop {
                feed.drain(stream);
                if let Some(me) = feed.store().me() {
                    return Ok(me);
                }
                if Instant::now() >= deadline || b.status().lines.core == CoreState::Faulted {
                    return Err(Fail::Core(CoreError::Internal { what: "me_unknown" }));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// 让会话进 Store（`getChat`），找不到 → `not_found`
fn load_chat<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    chat_id: ChatId,
) -> Result<(), Fail> {
    block_on(b.chat_get(chat_id)).map_err(Fail::Core)?;
    feed.drain(stream);
    if feed.store().chat(chat_id).is_none() {
        return Err(Fail::Core(CoreError::NotFound {
            what: "chat",
            td: None,
        }));
    }
    Ok(())
}

// ───────────────────────── history ─────────────────────────

/// 在已起好的后端上取一页。`feed` 直跑时每次新建，`serve` 常驻时一直用同一个
pub fn collect<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &Query,
    sync: Duration,
) -> Result<Page, Fail> {
    chats::ensure_ready(b)?;
    let chat_id = resolve(b, stream, feed, q.chat)?;
    load_chat(b, stream, feed, chat_id)?;
    // 未读位置要等补发完才准；断网时等不到，照样从本地出
    let synced = b.wait_net(sync, &|n| *n == NetState::Ready).is_some();
    feed.drain(stream);
    // 开窗口（只在本进程的 Store 里，不是 TDLib 的 openChat）：历史应答按凭据进这个窗口
    feed.pump_mut().apply(Input::OpenChat { chat_id });

    let (last_read, unread, last_message) = {
        let c = feed
            .store()
            .chat(chat_id)
            .ok_or(Fail::Core(CoreError::NotFound {
                what: "chat",
                td: None,
            }))?;
        (c.last_read_inbox, c.unread_count, c.last_message)
    };
    let mut fetch = Fetcher {
        b,
        stream,
        feed,
        chat_id,
        local_only: q.local_only,
        last_message,
    };
    let n = q.limit;
    let newest_known = |page: &[i64]| match (page.last(), last_message) {
        (Some(&p), Some(l)) => p < l,
        (Some(_), None) | (None, _) => false,
    };
    let (ids, more, more_newer, landing, partial) = match q.anchor {
        Anchor::Before(x) => {
            let (ids, more, stalled) = older(&mut fetch, Some(x), n)?;
            // X 本身（或更新的）在往新那一侧
            let more_newer = !ids.is_empty();
            (ids, more, more_newer, "before", stalled)
        }
        Anchor::After(x) => {
            let (ids, more_newer, stalled) = newer(&mut fetch, x, n)?;
            // X 本身（或更旧的）在往旧那一侧
            (ids, true, more_newer, "after", stalled)
        }
        Anchor::Landing if unread > 0 && last_read > 0 => {
            let (mut ids, mut more_newer, mut stalled) = newer(&mut fetch, last_read, n)?;
            // 已读位置之后先是自己发的（不算未读）：落点挪到第一条别人发的，再往后补满
            let skip = ids
                .iter()
                .position(|&id| !fetch.outgoing(id))
                .unwrap_or(ids.len());
            if skip > 0 && skip < ids.len() {
                ids.drain(..skip);
                if more_newer && let Some(&tail) = ids.last() {
                    let (extra, still, s2) = newer(&mut fetch, tail, n - ids.len())?;
                    ids.extend(extra);
                    // 补的这一次决定往新那一侧的结论：它顺利翻到了最后一条，这页就是全的，不再报停住
                    more_newer = still;
                    stalled = s2;
                }
            }
            (ids, true, more_newer, "first_unread", stalled)
        }
        Anchor::Landing => {
            let (ids, more, stalled) = older(&mut fetch, None, n)?;
            let more_newer = newest_known(&ids);
            (ids, more, more_newer, "latest", stalled)
        }
    };
    let store = feed.store();
    let first_unread = (unread > 0)
        .then(|| {
            ids.iter().copied().find(|&id| {
                id > last_read
                    && store
                        .message(MsgId {
                            chat_id,
                            message_id: id,
                        })
                        .is_some_and(|m| !m.outgoing)
            })
        })
        .flatten();
    let docs: Vec<Doc> = ids
        .iter()
        .filter_map(|&message_id| {
            project::message(
                store,
                MsgId {
                    chat_id,
                    message_id,
                },
            )
        })
        .collect();
    let empty = docs.is_empty();
    Ok(Page {
        chat_id,
        docs,
        more: more && !empty,
        more_newer: more_newer && !empty,
        first_unread,
        landing,
        synced,
        partial,
    })
}

/// 发一次 `getChatHistory`，应答经凭据喂进 Store，返回这次拿到的 id
struct Fetcher<'a, B: Backend> {
    b: &'a B,
    stream: &'a mut EventStream,
    feed: &'a mut Feed,
    chat_id: ChatId,
    local_only: bool,
    /// 会话的最后一条（往新翻到它就停）
    last_message: Option<i64>,
}

impl<B: Backend> Fetcher<'_, B> {
    fn get(&mut self, from: i64, offset: i32, limit: usize) -> Result<Vec<i64>, Fail> {
        let req = HistoryReq {
            chat: self.chat_id,
            from,
            offset,
            limit: u32::try_from(limit.clamp(1, 100)).unwrap_or(100),
            local_only: self.local_only,
        };
        let ticket = self.feed.pump_mut().store_mut().ticket(self.chat_id);
        let messages = block_on(self.b.history(req)).map_err(Fail::Core)?;
        // 应答里提到的用户、会话已经先经更新到了：先喂更新，名字才查得到
        self.feed.drain(self.stream);
        let ids = messages
            .iter()
            .filter(|m| m.id.chat_id == self.chat_id)
            .map(|m| m.id.message_id)
            .collect();
        self.feed
            .pump_mut()
            .apply(Input::HistoryLoaded { ticket, messages });
        Ok(ids)
    }

    fn outgoing(&self, id: i64) -> bool {
        self.feed
            .store()
            .message(MsgId {
                chat_id: self.chat_id,
                message_id: id,
            })
            .is_some_and(|m| m.outgoing)
    }
}

/// 没拿到新的时，下一次重试前等多久（第 n 次等 n 倍）：TDLib 只回起点那一条，多半是正从服务器拉
const RETRY_BACKOFF: Duration = Duration::from_millis(150);

/// 比 `before` 旧的 `want` 条（`None` = 从最新一条起，含它），从旧到新；第二项 = 往旧还有没有，第三项 = 停住了（没拉全）。
/// 只有**空应答**才算到头：TDLib 内存里没有更早的时会自己去本地库 / 服务器拉，拉完还是空才回空
/// （`MessagesManager.cpp` `get_dialog_history`）。往旧取用 `offset` 0、不含起点，正常不会「不空却没有新的」；
/// 万一出现（应答里只有起点或更新的），按停住处理：稍等再要；连着 [`MAX_IDLE`] 次还是这样就停，
/// 按「可能还有」交出（第二项为真；拿到了消息时页上照给游标，一条没拿到时页上压成假、只看 `partial`）——不把「还没拉到」说成「没有了」。
fn older<B: Backend>(
    f: &mut Fetcher<'_, B>,
    before: Option<i64>,
    want: usize,
) -> Result<(Vec<i64>, bool, bool), Fail> {
    let ok = |id: i64| before.is_none_or(|b| id < b);
    let mut got = BTreeSet::new();
    let mut from = before.unwrap_or(0);
    let mut idle = 0;
    let mut stalled = false;
    // 多取一条看还有没有
    while got.len() <= want {
        let need = want + 1 - got.len();
        // offset 0 不含起点那条（TDLib 的切法）；多要一条不碍事，起点若混进来会被过滤掉
        let ids = f.get(from, 0, need + 1)?;
        if ids.is_empty() {
            break;
        }
        let before_len = got.len();
        got.extend(ids.iter().copied().filter(|&id| ok(id)));
        if got.len() == before_len {
            idle += 1;
            if idle >= MAX_IDLE {
                stalled = true;
                break;
            }
            std::thread::sleep(RETRY_BACKOFF * idle);
            continue;
        }
        idle = 0;
        from = got.first().copied().unwrap_or(from);
    }
    let more = got.len() > want || stalled;
    let ids: Vec<i64> = got.into_iter().rev().take(want).collect();
    Ok((ids.into_iter().rev().collect(), more, stalled))
}

/// 比 `after` 新的 `want` 条，从旧到新；第二项 = 往新还有没有，第三项 = 停住了（没拉全）。到了会话的最后一条就停（确实没有了）；
/// 停顿的处理同 [`older`]：只有空应答、或到了最后一条才算到头。往新取用负 `offset`，起点那条本来就在应答里，
/// 所以「停住」在这一侧就是只拿回起点那一条（比它新的还没进内存）；连着 [`MAX_IDLE`] 次这样按「可能还有」交出
fn newer<B: Backend>(
    f: &mut Fetcher<'_, B>,
    after: i64,
    want: usize,
) -> Result<(Vec<i64>, bool, bool), Fail> {
    let mut got = BTreeSet::new();
    let mut from = after;
    let mut idle = 0;
    let mut stalled = false;
    while got.len() <= want {
        if f.last_message.is_some_and(|l| from >= l) {
            break;
        }
        let k = (want + 1 - got.len()).min(98);
        // 负 offset 为 -(k+1)：TDLib 把起点那条算进去，另给比它新的 k 条（`OrderedMessage.cpp`）；
        // limit 至少等于 -offset，再多一条
        let ids = f.get(from, -i32::try_from(k + 1).unwrap_or(99), k + 2)?;
        if ids.is_empty() {
            break;
        }
        let before_len = got.len();
        got.extend(ids.iter().copied().filter(|&id| id > after));
        if got.len() == before_len {
            idle += 1;
            if idle >= MAX_IDLE {
                stalled = true;
                break;
            }
            std::thread::sleep(RETRY_BACKOFF * idle);
            continue;
        }
        idle = 0;
        from = got.last().copied().unwrap_or(from);
    }
    let more = got.len() > want || stalled;
    Ok((got.into_iter().take(want).collect(), more, stalled))
}

/// 读法上下文：CLI 没有按键；`--tz` 不给就跟系统时区
pub fn speech_ctx(tz: Option<&str>, reveal: bool) -> Result<SpeechCtx, Fail> {
    let (utc_offset, offset_at) = match tz {
        None => (
            crate::speech::local_offset_at(crate::speech::now()),
            Some(crate::speech::local_offset_at as fn(i64) -> i32),
        ),
        Some(s) => match crate::speech::parse_tz(s) {
            Some(o) => (o, None),
            None => {
                return Err(Fail::Usage(
                    "--tz 不对：写成 +08:00、-05:30、+0800、+8、Z 或 UTC",
                ));
            }
        },
    };
    Ok(SpeechCtx {
        utc_offset,
        offset_at,
        listen_key: None,
        expand_key: None,
        reveal,
    })
}

/// 把一页写到 `sink`，返回退出码（直跑与执行方共用这一段，输出才一致）。
/// JSON：每条一行 `kind:"message"` + 一行 `result`；文字：每条一句（`--full` 读全文，`--detail` 接「详情：」），
/// 第一条未读前另起一行「以下为未读」（F2），统计写标准错误
pub fn render(
    sink: &mut dyn Sink,
    format: Format,
    a: &HistoryArgs,
    cx: &SpeechCtx,
    page: &Page,
    started: Instant,
) -> u8 {
    let layers = Layers {
        detail: a.detail,
        full: a.full,
    };
    for d in &page.docs {
        match format {
            Format::Json => {
                let mut line = atg_core::json::value_with(d, cx, layers);
                if let Some(o) = line.as_object_mut() {
                    o.insert("at".into(), json!(out::now_rfc3339_ms()));
                }
                emit_to(sink, &line);
            }
            Format::Text => {
                if page.first_unread.is_some() && doc_id(d) == page.first_unread {
                    sink.out(UNREAD_BELOW);
                }
                // 文字模式一行一条：名称里的换行换成空格；全文本来就要保留换行，照原样
                let main = if a.full {
                    speech::full(d, cx)
                } else {
                    speech::brief(d, cx).replace('\n', " ")
                };
                let detail = if a.detail {
                    speech::detail(d, cx)
                } else {
                    String::new()
                };
                if detail.is_empty() {
                    sink.out(&main);
                } else {
                    sink.out(&format!("{main}。详情：{detail}"));
                }
            }
        }
    }
    let oldest = page.ids().next();
    let newest = page.ids().last();
    match format {
        Format::Json => {
            let mut data = json!({
                "command": "history",
                "chat_id": page.chat_id,
                "count": page.docs.len(),
                "more": page.more,
                "more_newer": page.more_newer,
                "first_unread": page.first_unread,
                "landing": page.landing,
                "synced": page.synced,
                "partial": page.partial,
                "elapsed_ms": crate::elapsed_ms(started),
            });
            if let Some(d) = data.as_object_mut() {
                if let (true, Some(o)) = (page.more, oldest) {
                    d.insert("next_cursor".into(), json!(o));
                }
                if let (true, Some(n)) = (page.more_newer, newest) {
                    d.insert("newer_cursor".into(), json!(n));
                }
            }
            emit_to(sink, &envelope("result", data));
        }
        Format::Text => {
            let where_ = match page.landing {
                "first_unread" if page.first_unread.is_some() => "从第一条未读起",
                "first_unread" | "latest" => "最新的",
                "before" => "更早的",
                _ => "更新的",
            };
            let mut s = format!("{where_} {} 条", page.docs.len());
            if page.more {
                s.push_str("；往前还有");
            }
            if page.more_newer {
                s.push_str("；往后还有");
            }
            if !page.synced {
                s.push_str("；没等到同步完，未读位置可能是旧的");
            }
            if page.partial {
                s.push_str("；Telegram 还没给全，可以稍后再试");
            }
            sink.err(&s);
        }
    }
    0
}

/// 文字模式里第一条未读上面那一行（F2「以下为未读」）
pub const UNREAD_BELOW: &str = "以下为未读";

// ───────────────────────── links ─────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct LinksQuery {
    pub chat: ChatRef,
    pub message_id: i64,
}

/// 一条消息里的链接：（文字，地址，是否在剧透里），按出现顺序
pub struct LinkList {
    pub id: MsgId,
    pub links: Vec<(String, String, bool)>,
}

pub fn links_query(a: &LinksArgs) -> Result<LinksQuery, Fail> {
    Ok(LinksQuery {
        chat: ChatRef::parse(&a.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?,
        message_id: a.message,
    })
}

/// 在已起好的后端上取这条消息、列出链接（`getMessage`，不标已读）
pub fn collect_links<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    q: &LinksQuery,
) -> Result<LinkList, Fail> {
    chats::ensure_ready(b)?;
    let chat_id = resolve(b, stream, feed, q.chat)?;
    load_chat(b, stream, feed, chat_id)?;
    let id = MsgId {
        chat_id,
        message_id: q.message_id,
    };
    let m = block_on(b.message_get(id)).map_err(Fail::Core)?;
    feed.drain(stream);
    // 有效内容（有临时内容就用它）；受限的存下来时已换成受限节点，里面没有正文，也就没有链接
    let links = match m.effective() {
        Body::Content(c) => links_marked(&c.body)
            .into_iter()
            .map(|(l, hidden)| (l.text, l.url, hidden))
            .collect(),
        Body::Service(_) => vec![],
    };
    Ok(LinkList { id, links })
}

/// 隐藏的链接文字换成这个（地址给 `null`）
const HIDDEN_LINK: &str = "隐藏内容";

pub fn render_links(
    sink: &mut dyn Sink,
    format: Format,
    reveal: bool,
    l: &LinkList,
    started: Instant,
) -> u8 {
    for (i, (text, url, hidden)) in l.links.iter().enumerate() {
        let masked = *hidden && !reveal;
        let (text, url): (&str, Value) = if masked {
            (HIDDEN_LINK, Value::Null)
        } else {
            (text, json!(url))
        };
        let index = i + 1;
        match format {
            Format::Json => {
                let line = envelope(
                    "link",
                    json!({ "index": index, "text": text, "url": url, "hidden": hidden }),
                );
                emit_to(sink, &line);
            }
            Format::Text => match url.as_str() {
                Some(u) if u == text => sink.out(&format!("链接 {index}，{u}")),
                Some(u) => sink.out(&format!("链接 {index}，{text}，{u}")),
                None => sink.out(&format!("链接 {index}，{text}")),
            },
        }
    }
    match format {
        Format::Json => emit_to(
            sink,
            &envelope(
                "result",
                json!({
                    "command": "links",
                    "chat_id": l.id.chat_id,
                    "message_id": l.id.message_id,
                    "count": l.links.len(),
                    "elapsed_ms": crate::elapsed_ms(started),
                }),
            ),
        ),
        Format::Text => sink.err(&if l.links.is_empty() {
            "这条消息里没有链接".to_owned()
        } else {
            format!("共 {} 个链接", l.links.len())
        }),
    }
    0
}

#[cfg(test)]
mod tests {
    //! 假后端单测：名字只取 `src/atg-core/fixtures/names.txt`，正文是编的（「第N条」），不碰真实账号。
    use super::*;
    use crate::out::Sink;
    use atg_core::config::ServerKind;
    use atg_core::fake::{Call, FakeAuth, FakeBackend};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    const ME: i64 = 100;
    const LIN: i64 = 101;
    const ZHOU: i64 = 102;
    const GROUP: i64 = -1001;
    /// 2026-10-26T11:30:00Z（东八区 19:30）
    const T0: i64 = 1_793_014_200;

    fn temp_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "atg-cli-history-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn profile(tag: &str) -> Profile {
        Profile::create(&temp_root(tag), "main", ServerKind::Main).unwrap()
    }

    fn user(id: i64, name: &str) -> Value {
        json!({"@type": "updateUser", "user": {"@type": "user", "id": id, "first_name": name, "last_name": "",
            "usernames": null, "phone_number": "", "is_contact": true, "type": {"@type": "userTypeRegular"}}})
    }

    fn text(t: &str) -> Value {
        json!({"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": []}})
    }

    /// 一条消息：`sender` 是自己就算自己发的；时间每条加一分钟
    fn msg(chat: i64, id: i64, sender: i64, content: Value) -> Value {
        json!({"@type": "message", "id": id, "chat_id": chat,
            "sender_id": {"@type": "messageSenderUser", "user_id": sender},
            "is_outgoing": sender == ME, "is_pinned": false, "date": T0 + id * 60, "edit_date": 0,
            "media_album_id": "0", "content": content})
    }

    fn plain(chat: i64, id: i64, sender: i64) -> Value {
        msg(chat, id, sender, text(&format!("第{id}条")))
    }

    /// 群：读到 `last_read`，`unread` 条未读，最后一条 `last`
    fn group(last_read: i64, unread: u32, last: i64) -> Vec<Value> {
        vec![
            json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}),
            user(ME, "我自己"),
            user(LIN, "林小雨"),
            user(ZHOU, "周大山"),
            json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": GROUP, "title": "周末爬山群",
                "type": {"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false},
                "positions": [], "unread_count": unread, "unread_mention_count": 0,
                "last_read_inbox_message_id": last_read, "last_read_outbox_message_id": 0,
                "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
                "last_message": plain(GROUP, last, LIN)}}),
        ]
    }

    /// 第 1–10 条：第 6 条是自己发的，其余林小雨、周大山轮流
    fn ten() -> Vec<Value> {
        (1..=10)
            .map(|i| {
                let s = match i {
                    6 => ME,
                    _ if i % 2 == 0 => ZHOU,
                    _ => LIN,
                };
                plain(GROUP, i, s)
            })
            .collect()
    }

    fn backend(last_read: i64, unread: u32) -> FakeBackend {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth {
            logged_in: true,
            ..FakeAuth::default()
        });
        b.push_net("connectionStateReady");
        b.set_chat(GROUP, group(last_read, unread, 10));
        b.set_history(GROUP, ten());
        b
    }

    fn q(anchor: Anchor, limit: usize) -> Query {
        Query {
            chat: ChatRef::Id(GROUP),
            anchor,
            limit,
            local_only: false,
        }
    }

    fn page(b: &FakeBackend, tag: &str, q: &Query) -> Page {
        let p = profile(tag);
        with_backend(b, &p, Instant::now(), |b, s, f| {
            collect(b, s, f, q, Duration::from_millis(300))
        })
        .unwrap_or_else(|_| panic!("取不到"))
    }

    fn ids(p: &Page) -> Vec<i64> {
        p.ids().collect()
    }

    fn fail_code(r: Result<Page, Fail>) -> String {
        match r {
            Err(Fail::Core(e)) => e.code().to_owned(),
            Err(_) => "other".into(),
            Ok(_) => "ok".into(),
        }
    }

    /// 只调了只读的那几个：没有 openChat / viewMessages 这类（后端上本来就没有，记账里也不许出现别的）
    fn only_reads(b: &FakeBackend) {
        for c in b.calls() {
            assert!(
                matches!(
                    c,
                    Call::Start { .. }
                        | Call::Subscribe
                        | Call::Shutdown
                        | Call::ChatGet(_)
                        | Call::History(_)
                        | Call::MessageGet(_)
                ),
                "多了别的调用"
            );
        }
    }

    #[test]
    fn lands_on_first_unread_skipping_own_messages() {
        // 读到第 5 条；第 6 条是自己发的，不算未读：落点是第 7 条
        let b = backend(5, 4);
        let p = page(&b, "land", &q(Anchor::Landing, 3));
        assert_eq!(ids(&p), vec![7, 8, 9]);
        assert_eq!(p.first_unread, Some(7));
        assert_eq!(p.landing, "first_unread");
        assert!(p.more && p.more_newer);
        only_reads(&b);
    }

    #[test]
    fn lands_on_latest_without_unread() {
        let b = backend(10, 0);
        let p = page(&b, "latest", &q(Anchor::Landing, 3));
        assert_eq!(ids(&p), vec![8, 9, 10]);
        assert_eq!(p.first_unread, None);
        assert_eq!(p.landing, "latest");
        assert!(p.more);
        assert!(!p.more_newer);
    }

    #[test]
    fn paging_back_is_contiguous_without_repeats() {
        let mut seen = vec![];
        let mut cursor = None;
        loop {
            let anchor = cursor.map_or(Anchor::Landing, Anchor::Before);
            let tag = format!("back{}", seen.len());
            // 一个假后端只起一次：每页一个（同直跑每次新起进程）
            let p = page(&backend(10, 0), &tag, &q(anchor, 3));
            let mut got = ids(&p);
            got.extend(seen);
            seen = got;
            if !p.more {
                break;
            }
            cursor = p.ids().next();
        }
        assert_eq!(seen, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn paging_forward_stops_at_last_message() {
        let p = page(&backend(10, 0), "fwd", &q(Anchor::After(4), 3));
        assert_eq!(ids(&p), vec![5, 6, 7]);
        assert!(p.more_newer);
        let p = page(&backend(10, 0), "fwd2", &q(Anchor::After(7), 5));
        assert_eq!(ids(&p), vec![8, 9, 10]);
        assert!(!p.more_newer);
        assert!(p.more);
    }

    #[test]
    fn short_first_answers_are_filled_up() {
        // TDLib 首次只回起点那一条：循环补满，落点和翻页都不缺不重
        let b = backend(5, 4);
        b.short_first_history();
        let p = page(&b, "short", &q(Anchor::Landing, 4));
        assert_eq!(ids(&p), vec![7, 8, 9, 10]);
        let b = backend(10, 0);
        b.short_first_history();
        let p = page(&b, "short2", &q(Anchor::Landing, 4));
        assert_eq!(ids(&p), vec![7, 8, 9, 10]);
    }

    /// TDLib 连着好几次不给新的（停住）：不当成到头。拿到了消息 → 按「可能还有」交出并给游标，接着翻不缺；
    /// 一条都没拿到 → 没有游标可给，more 为假，只看 partial
    #[test]
    fn stalled_answers_keep_more_or_report_partial() {
        // 往旧：第一次拿到最新一条（第 10 条）后，假后端故意连着只回起点（正常 TDLib 往旧不会，对应契约里「万一」那种）
        let b = backend(10, 0);
        b.short_first_history();
        b.stall_history(4);
        let p = page(&b, "stall", &q(Anchor::Landing, 4));
        assert!(ids(&p).len() < 4, "停住了，这页不满");
        assert!(p.more, "停住不等于到头");
        let oldest = p.ids().next().unwrap_or_else(|| panic!("至少有一条"));
        let p2 = page(&backend(10, 0), "stall2", &q(Anchor::Before(oldest), 4));
        assert_eq!(ids(&p2).last().copied(), Some(oldest - 1), "接着翻不缺");

        // 往新：一开始就停住、一条都没拿到
        let b = backend(10, 0);
        b.stall_history(4);
        let p = page(&b, "stall3", &q(Anchor::After(3), 4));
        assert!(p.partial, "停住了要说没拉全");
        // 一条都没拿到：没有游标可给，more_newer 为假，只看 partial（契约 cli.md）
        assert!(
            p.ids().next().is_none(),
            "备了 4 次停住，比上限 3 多，第 3 次就放弃了，一条都没拿到"
        );
        assert!(!p.more_newer);

        // 真到头（空应答）才说没有了：从最旧一条再往前
        let p = page(&backend(10, 0), "edge", &q(Anchor::Before(1), 4));
        assert!(ids(&p).is_empty());
        assert!(!p.more);
    }

    /// 落点：第一次往新取拿到两条后停住，跳过自己发的之后补的那一次顺利翻到最后一条——这页是全的，不报 partial
    #[test]
    fn landing_refill_that_reaches_the_end_is_not_partial() {
        // 读到第 5 条，第 6 条是自己发的；每次最多回 3 条；第一次照常（拿到 6、7），接着停住 3 次，之后照常
        let b = backend(5, 4);
        b.cap_history(3);
        b.stall_history_after(1, 3);
        let p = page(&b, "refill", &q(Anchor::Landing, 10));
        assert_eq!(ids(&p), vec![7, 8, 9, 10]);
        assert!(!p.partial, "补的那次翻到了最后一条，不该说没拉全");
        assert!(!p.more_newer);
        assert_eq!(p.first_unread, Some(7));
    }

    #[test]
    fn saved_uses_own_chat() {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth {
            logged_in: true,
            ..FakeAuth::default()
        });
        b.push_net("connectionStateReady");
        b.push(json!({"@type": "updateOption", "name": "my_id", "value": {"@type": "optionValueInteger", "value": ME.to_string()}}));
        b.set_chat(
            ME,
            vec![user(ME, "我自己"), json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": ME,
                "title": "我自己", "type": {"@type": "chatTypePrivate", "user_id": ME}, "positions": [],
                "unread_count": 0, "unread_mention_count": 0, "last_read_inbox_message_id": 2,
                "last_read_outbox_message_id": 2,
                "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0},
                "last_message": plain(ME, 2, ME)}})],
        );
        b.set_history(ME, vec![plain(ME, 1, ME), plain(ME, 2, ME)]);
        let mut query = q(Anchor::Landing, 5);
        query.chat = ChatRef::Saved;
        let p = page(&b, "saved", &query);
        assert_eq!(p.chat_id, ME);
        assert_eq!(ids(&p), vec![1, 2]);
        assert!(!p.more && !p.more_newer);
        assert!(b.calls().contains(&Call::ChatGet(ME)));
    }

    #[test]
    fn errors_not_logged_in_and_unknown_chat() {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth::default());
        let p = profile("nologin");
        let r = with_backend(&b, &p, Instant::now(), |b, s, f| {
            collect(b, s, f, &q(Anchor::Landing, 3), Duration::ZERO)
        });
        assert_eq!(fail_code(r), "auth_required");

        let b = backend(5, 4);
        let p = profile("unknown");
        let mut query = q(Anchor::Landing, 3);
        query.chat = ChatRef::Id(-999);
        let r = with_backend(&b, &p, Instant::now(), |b, s, f| {
            collect(b, s, f, &query, Duration::ZERO)
        });
        assert_eq!(fail_code(r), "not_found");
    }

    #[test]
    fn chat_ref_parsing() {
        assert_eq!(ChatRef::parse("saved"), Some(ChatRef::Saved));
        assert_eq!(ChatRef::parse("-1001"), Some(ChatRef::Id(-1001)));
        assert_eq!(ChatRef::parse("x"), None);
    }

    // ───── 输出（快照）─────

    #[derive(Default)]
    struct Cap {
        out: Vec<String>,
        err: Vec<String>,
    }
    impl Sink for Cap {
        fn out(&mut self, line: &str) {
            self.out.push(line.to_owned());
        }
        fn err(&mut self, line: &str) {
            self.err.push(line.to_owned());
        }
    }

    /// 去掉每次都变的时间字段
    fn norm(lines: &[String]) -> String {
        lines
            .iter()
            .map(|l| {
                let mut v: Value = serde_json::from_str(l).unwrap();
                if let Some(o) = v.as_object_mut() {
                    o.insert("at".into(), json!("…"));
                }
                if let Some(d) = v["data"].as_object_mut()
                    && d.contains_key("elapsed_ms")
                {
                    d.insert("elapsed_ms".into(), json!(0));
                }
                v.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn args(detail: bool, full: bool, reveal: bool) -> HistoryArgs {
        HistoryArgs {
            chat: GROUP.to_string(),
            limit: 20,
            before: None,
            after: None,
            detail,
            full,
            reveal,
            local: false,
            tz: None,
        }
    }

    fn cx(reveal: bool) -> SpeechCtx {
        speech_ctx(Some("+08:00"), reveal).unwrap_or_else(|_| panic!("时区"))
    }

    /// 详情、全文、隐藏内容要用的几条：回复（带引用）、转发自频道、已编辑、表情回应、论坛话题、剧透、遮挡的图片
    fn rich() -> FakeBackend {
        let b = FakeBackend::new().unwrap();
        b.set_auth(FakeAuth {
            logged_in: true,
            ..FakeAuth::default()
        });
        b.push_net("connectionStateReady");
        let mut chat = group(2, 4, 6);
        chat.push(json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": -1002, "title": "示例频道",
            "type": {"@type": "chatTypeSupergroup", "supergroup_id": 2, "is_channel": true}, "positions": [],
            "unread_count": 0, "unread_mention_count": 0, "last_read_inbox_message_id": 0, "last_read_outbox_message_id": 0,
            "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}}));
        b.set_chat(GROUP, chat);
        let mut reply = msg(GROUP, 3, LIN, text("收到"));
        reply["reply_to"] = json!({"@type": "messageReplyToMessage", "chat_id": GROUP, "message_id": 2,
            "quote": {"@type": "textQuote", "text": {"@type": "formattedText", "text": "周六出发", "entities": []}, "position": 0, "is_manual": true}});
        reply["edit_date"] = json!(T0 + 3 * 60 + 600);
        reply["interaction_info"] = json!({"@type": "messageInteractionInfo", "view_count": 0, "forward_count": 0,
            "reactions": {"@type": "messageReactions", "reactions": [
                {"@type": "messageReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "👍"}, "total_count": 3, "is_chosen": true},
                {"@type": "messageReaction", "type": {"@type": "reactionTypeEmoji", "emoji": "❤️"}, "total_count": 1, "is_chosen": false}]}});
        reply["topic_id"] = json!({"@type": "messageTopicForum", "forum_topic_id": 3});
        let mut fwd = msg(GROUP, 4, ZHOU, text("周六全天晴。\n注意防晒补水！"));
        fwd["forward_info"] = json!({"@type": "messageForwardInfo",
            "origin": {"@type": "messageOriginChannel", "chat_id": -1002, "message_id": 9, "author_signature": ""},
            "date": T0 - 86_400});
        // 「答案是42，别说出去」：「42」是剧透
        let spoiler = msg(
            GROUP,
            5,
            LIN,
            json!({"@type": "messageText", "text": {"@type": "formattedText",
            "text": "答案是42，别说出去", "entities": [{"@type": "textEntity", "offset": 3, "length": 2,
            "type": {"@type": "textEntityTypeSpoiler"}}]}}),
        );
        let photo = msg(
            GROUP,
            6,
            ME,
            json!({"@type": "messagePhoto", "photo": {"@type": "photo", "has_stickers": false, "minithumbnail": null, "sizes": []},
            "caption": {"@type": "formattedText", "text": "山顶的风景", "entities": []}, "has_spoiler": true, "show_caption_above_media": false, "is_secret": false}),
        );
        b.set_history(
            GROUP,
            vec![
                plain(GROUP, 1, ZHOU),
                plain(GROUP, 2, ZHOU),
                reply,
                fwd,
                spoiler,
                photo,
            ],
        );
        b
    }

    fn render_rich(tag: &str, format: Format, a: &HistoryArgs) -> Cap {
        let b = rich();
        let p = page(&b, tag, &q(Anchor::Landing, 20));
        let mut cap = Cap::default();
        render(&mut cap, format, a, &cx(a.reveal), &p, Instant::now());
        cap
    }

    #[test]
    fn json_brief_detail_full_snapshot() {
        let cap = render_rich("snapj", Format::Json, &args(true, true, false));
        insta::assert_snapshot!("history_json_detail_full", norm(&cap.out));
        // 不加 --reveal：data 树里也没有剧透的字（大总管授权 2026-10-26）
        // 只比文字节点：原始行里的 `at` 时刻也可能含「42」
        let secret = r#""text":"42""#;
        assert!(!cap.out.join("").contains(secret), "剧透漏进 data");
        assert!(
            cap.out
                .join("")
                .contains(r#""type":"spoiler","children":[]"#)
        );
        let shown = render_rich("snapjr", Format::Json, &args(false, false, true));
        assert!(
            shown.out.join("").contains(secret),
            "--reveal 后 data 该带原文"
        );
    }

    #[test]
    fn text_snapshots_with_unread_marker() {
        let plain_text = render_rich("snapt", Format::Text, &args(false, false, false));
        let detail = render_rich("snapd", Format::Text, &args(true, false, false));
        let full = render_rich("snapf", Format::Text, &args(false, true, false));
        let revealed = render_rich("snapr", Format::Text, &args(false, true, true));
        insta::assert_snapshot!(
            "history_text",
            [
                "# brief",
                &plain_text.out.join("\n"),
                "# detail",
                &detail.out.join("\n"),
                "# full",
                &full.out.join("\n"),
                "# full --reveal",
                &revealed.out.join("\n"),
                "# stderr",
                &plain_text.err.join("\n"),
            ]
            .join("\n")
        );
        // 第一条未读（第 3 条）上面一行是「以下为未读」
        assert_eq!(plain_text.out[0], UNREAD_BELOW);
        // 不加 --reveal：剧透的字一个都不出
        for l in plain_text.out.iter().chain(&detail.out).chain(&full.out) {
            assert!(!l.contains("42"), "剧透漏出来了");
        }
        assert!(revealed.out.iter().any(|l| l.contains("42")));
    }

    // ───── links ─────

    fn links_backend() -> FakeBackend {
        let b = backend(10, 0);
        // 「看下路线图 https://example.invalid/a 秘密」：「路线图」是文字链接，网址照写，「秘密」是剧透里的文字链接
        let t = "看下路线图 https://example.invalid/a 秘密";
        let utf16 = |s: &str| s.encode_utf16().count();
        let url_at = utf16("看下路线图 ");
        let secret_at = utf16("看下路线图 https://example.invalid/a ");
        let m = msg(
            GROUP,
            11,
            LIN,
            json!({"@type": "messageText", "text": {"@type": "formattedText", "text": t, "entities": [
            {"@type": "textEntity", "offset": 2, "length": 3, "type": {"@type": "textEntityTypeTextUrl", "url": "https://example.invalid/route"}},
            {"@type": "textEntity", "offset": url_at, "length": utf16("https://example.invalid/a"), "type": {"@type": "textEntityTypeUrl"}},
            {"@type": "textEntity", "offset": secret_at, "length": 2, "type": {"@type": "textEntityTypeSpoiler"}},
            {"@type": "textEntity", "offset": secret_at, "length": 2, "type": {"@type": "textEntityTypeTextUrl", "url": "https://example.invalid/secret"}}]}}),
        );
        let mut all = ten();
        all.push(m);
        b.set_history(GROUP, all);
        b
    }

    fn links_of(tag: &str, message_id: i64) -> Result<LinkList, Fail> {
        let b = links_backend();
        let p = profile(tag);
        let q = LinksQuery {
            chat: ChatRef::Id(GROUP),
            message_id,
        };
        let r = with_backend(&b, &p, Instant::now(), |b, s, f| collect_links(b, s, f, &q));
        only_reads(&b);
        r
    }

    #[test]
    fn links_masked_and_revealed() {
        let Ok(l) = links_of("links", 11) else {
            panic!("取不到")
        };
        assert_eq!(l.links.len(), 3);
        let mut masked = Cap::default();
        render_links(&mut masked, Format::Json, false, &l, Instant::now());
        let mut shown = Cap::default();
        render_links(&mut shown, Format::Text, true, &l, Instant::now());
        let mut hidden_text = Cap::default();
        render_links(&mut hidden_text, Format::Text, false, &l, Instant::now());
        insta::assert_snapshot!(
            "links",
            [
                norm(&masked.out),
                shown.out.join("\n"),
                hidden_text.out.join("\n"),
            ]
            .join("\n")
        );
        assert!(
            !masked.out.join("").contains("secret"),
            "隐藏的地址漏出来了"
        );
        assert!(!hidden_text.out.join("").contains("秘密"));
    }

    #[test]
    fn links_none_and_missing_message() {
        let Ok(l) = links_of("nolinks", 3) else {
            panic!("取不到")
        };
        assert!(l.links.is_empty());
        let code = match links_of("missing", 999) {
            Err(Fail::Core(e)) => e.code(),
            _ => "?",
        };
        assert_eq!(code, "not_found");
    }
}
