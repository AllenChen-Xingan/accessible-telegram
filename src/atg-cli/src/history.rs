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
//!
//! 文件分工：本文件放类型、参数、起后端和取数；读法与渲染在 `render.rs`，`links` 命令在 `links.rs`。子模块只写
//! `use super::*`，所以它们要的导入也写在本文件头上（测试同样经 `use super::*` 用到其中几个）。

use crate::args::{Format, HistoryArgs, LinksArgs};
use crate::chats::{self, Feed};
use crate::deadlines::{QUERY_TIMEOUT, START_TIMEOUT, SYNC_TIMEOUT, bounded};
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::{Backend, HistoryReq, SHUTDOWN_TIMEOUT};
use atg_core::config::Profile;
use atg_core::error::CoreError;
use atg_core::json::Layers;
use atg_core::link::EventStream;
use atg_core::model::{Body, ChatId, MsgId};
use atg_core::project;
use atg_core::speech::{self, SpeechCtx};
use atg_core::status::NetState;
use atg_core::store::Input;
use atg_core::tree::Doc;
use atg_core::tree::text::links_marked;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

mod links;
mod render;
pub use links::{LinkList, LinksQuery, collect_links, links_query, render_links};
pub use render::{render, speech_ctx};
// 只有测试用（`tests.rs` 经 `use super::*` 拿）；正式路径在 render.rs 里
#[cfg(test)]
use render::UNREAD_BELOW;

/// 连着几次没拿到新消息就停（防 TDLib 一直回同一批时死循环）
pub const MAX_IDLE: u32 = 3;

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

/// 会话参数写错时的那句用法提示（#60 R4，`docs/plan/010` 第二节 S7）。用的有 history、links、voice，
/// 以及（R4 第二步 2a 起）edit、pending、forwarding、send-file、vote；`send`、`react` 里还各有一份同样的字面量，
/// 等 13b-2 合了再收（2b）。`watch --chat`、`delete` 的提示本来就不同
pub(crate) const CHAT_USAGE: &str = "会话写数字 id，或 saved（收藏夹）";

/// 命令行的会话参数 → [`ChatRef`]，写错了报用法错误并带 [`CHAT_USAGE`]（#60 R4）
pub(crate) fn chat_arg(s: &str) -> Result<ChatRef, Fail> {
    ChatRef::parse(s).ok_or(Fail::Usage(CHAT_USAGE))
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
    /// 停住了（TDLib 连着几次不空却没给新的：往新是只回起点那一条，往旧是只有起点或更新的）：这页可能不满，稍后重试同一条命令。一条都没拿到时 `more` / `more_newer` 为假，只看它
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
        Doc::Transcription(t) => Some(t.id.message_id),
        Doc::ChatItem(_) | Doc::Alert(_) | Doc::Status(_) | Doc::Playback(_) => None,
        Doc::Reactor(_) | Doc::Unknown(_) => None,
    }
}

/// 参数 → 查询；形状不对是用法错误。直跑与执行方都走这里（执行方收到的参数绕过了 clap，检查只在这一处）
pub fn query(a: &HistoryArgs) -> Result<Query, Fail> {
    let chat = chat_arg(&a.chat)?;
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

pub(crate) fn with_backend<B: Backend, T>(
    b: &B,
    profile: &Profile,
    started: Instant,
    f: impl FnOnce(&B, &mut EventStream, &mut Feed) -> Result<T, Fail>,
) -> Result<T, Fail> {
    let mut stream = b.subscribe().map_err(Fail::Core)?;
    if let Err(e) = bounded(b, b.start(profile), START_TIMEOUT) {
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

/// 会话 id：`saved` 换成自己的收藏夹 id。问后端 `me()`（发送闸判收藏夹用的也是它，`actions.md` 第一节第 4 小节），按查询类期限
/// （[`QUERY_TIMEOUT`]），到点报 `network`、内核故障报 `disconnected`，别的错照 `me()` 报（如没登录 `auth_required`）（#60 R4 起；原来等事件流里的 `my_id`
/// 最多 5 秒，等不到报 `internal`）。`send.rs` 自己还用 `send::resolve`（同一做法），等 13b-2 合了删（R4 2b）
pub(crate) fn resolve_chat<B: Backend>(b: &B, chat: ChatRef) -> Result<ChatId, Fail> {
    match chat {
        ChatRef::Id(id) => Ok(id),
        ChatRef::Saved => bounded(b, b.me(), QUERY_TIMEOUT)
            .map(|m| m.saved_chat_id)
            .map_err(Fail::Core),
    }
}

/// 同 [`resolve_chat`]，留着旧签名给还没改过来的调用方（`react`；R4 2b 删）。`stream`、`feed` 不再用
pub(crate) fn resolve<B: Backend>(
    b: &B,
    _stream: &mut EventStream,
    _feed: &mut Feed,
    chat: ChatRef,
) -> Result<ChatId, Fail> {
    resolve_chat(b, chat)
}

/// 让会话进 Store（`getChat`），找不到 → `not_found`
pub(crate) fn load_chat<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    chat_id: ChatId,
) -> Result<(), Fail> {
    bounded(b, b.chat_get(chat_id), QUERY_TIMEOUT).map_err(Fail::Core)?;
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
    let chat_id = resolve_chat(b, q.chat)?;
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
        let messages = bounded(self.b, self.b.history(req), QUERY_TIMEOUT).map_err(Fail::Core)?;
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

#[cfg(test)]
mod tests;
