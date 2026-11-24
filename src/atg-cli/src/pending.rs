//! `pending`：列出待发出的消息（#57，T04）。契约：`specs/contracts/cli.md` `pending` 行，
//! 动作：`specs/contracts/actions.md`「待发出的消息」；列表本身是 Store 查询 [`atg_core::store::Store::pending_sends`]。
//!
//! 开着消息库时，上次没发完的消息在 TDLib 启动时**当场续发**（TDLib 1.8.69 `MessagesManager.cpp:35925-35930`），
//! 跑 `pending` 本身也会触发；续发时临时 id 与时间都重编（`:35851-35856`），与上次 `send_outcome_unknown` 报的临时 id
//! 对不上。所以这里列的是「等连接就绪之后**仍**没发出的」，另把本次运行里看着它从待发变成已发出 / 失败 / 被删的
//! 记进 `resolved`（只算看到的：续发与结果在第一次取事件之前就都到了的，两头都不出现）。
//!
//! 输出（直跑与经本机接口转发同一段代码）：每条待发一行 `kind:"message"`（内容树，读法同 `history`，带「发送中」）+
//! 结尾一行 `result`（`pending_sends`、`resolved`、`synced`）。
//!
//! `pending cancel CHAT_ID`（`pending_cancel`，L2，按删除过闸，动作在 [`atg_core::delete`]）：撤这个会话里**此刻**全部
//! 待发消息——不按 id 选：临时 id 每次启动都重编，上一次运行列出的 id 到这次多半对不上，撤销直发、没有确认框，
//! 旧 id 万一被另一条待发消息复用就会撤错（自审第 2 轮 P1）。所以在同一进程里取 Store 的待发清单再定计划。一行 `plan` →
//! 过闸（同 `send`，直发）→ 交出 `deleteMessages` → 等这几条的下落：临时 id 出现在 `messages_deleted` 里 = 撤下了（`cancelled`）；
//! 先来了 `send_succeeded` = 没赶上、已发出（`already_sent`；若随后它的正式 id 也被删了，是 TDLib 把删除换成了正式 id、
//! 只从自己这边删掉，报 `sent_deleted_here`，对方那边还在，见 `atg_core::delete`）；`send_failed` = 失败了；都没等到 = 未知。
//! 「撤下了」也只是尽力：请求已到服务器的照样送达，之后 TDLib 会给所有人撤回（模块 `atg_core::delete` 的文档）。

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::deadlines::{CANCEL_RACE_WAIT, CANCEL_WAIT, QUERY_TIMEOUT, SYNC_TIMEOUT, bounded};
use crate::history;
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::delete::{self, DeletePlan};
use atg_core::error::{CoreError, UnknownWhy};
use atg_core::gate::Outcome;
use atg_core::link::EventStream;
use atg_core::link::block_on_timeout;
use atg_core::model::{MsgId, SendingState};
use atg_core::project;
use atg_core::send::{SendOrigin, Waited};
use atg_core::speech;
use atg_core::status::NetState;
use atg_core::store::{Entry, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Instant;

/// 一次 `pending` 要的东西（经本机接口转发时原样序列化过去）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingParams {
    /// 读法时区（全局选项，发起方填进来）
    #[serde(default)]
    pub tz: Option<String>,
}

/// 按命令名执行（`pending` 列表 / `pending_cancel` 撤销；直跑与执行方共用）。参数解不开 → `ipc_bad_request`
#[allow(clippy::too_many_arguments)]
pub fn execute<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    command: &str,
    params: Value,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    let bad = Fail::Core(atg_core::error::CoreError::Internal {
        what: "ipc_bad_request",
    });
    match command {
        "pending_cancel" => match serde_json::from_value::<CancelParams>(params) {
            Ok(p) => execute_cancel(b, stream, feed, &p, sink, format, started),
            Err(_) => out::fail_with_to(sink, format, &bad),
        },
        "pending" => match serde_json::from_value::<PendingParams>(params) {
            Ok(p) => execute_list(b, stream, feed, &p, sink, format, started),
            Err(_) => out::fail_with_to(sink, format, &bad),
        },
        _ => out::fail_with_to(sink, format, &bad),
    }
}

/// 在已起好的后端上列待发消息，输出写到 `sink`，返回退出码（直跑与执行方共用）
pub fn execute_list<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &PendingParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    match list(b, stream, feed, p, sink, format, started) {
        Ok(code) => code,
        Err(f) => out::fail_with_to(sink, format, &f),
    }
}

fn list<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &PendingParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> Result<u8, Fail> {
    let cx = history::speech_ctx(p.tz.as_deref(), false)?;
    chats::ensure_ready(b)?;
    // 先取一次：续发的那几条此时多半还是「待发」（网络请求要等连接）
    feed.drain(stream);
    let before = feed.store().pending_sends();
    let synced = b
        .wait_net(SYNC_TIMEOUT, &|n| *n == NetState::Ready)
        .is_some();
    feed.drain(stream);
    let now = feed.store().pending_sends();

    let mut items = Vec::new();
    for &id in &now {
        if let Some(doc) = project::message(feed.store(), id) {
            match format {
                Format::Json => {
                    let mut line = atg_core::json::value(&doc, &cx);
                    if let Some(o) = line.as_object_mut() {
                        o.insert("at".into(), json!(out::now_rfc3339_ms()));
                    }
                    emit_to(sink, &line);
                }
                // 文字格式也带会话 id 与临时 id：认消息、对照 `send` / `pending cancel` 的输出用（撤销本身按会话，不收 id）
                Format::Text => sink.out(&format!(
                    "会话 {} · 临时 id {}：{}",
                    id.chat_id,
                    id.message_id,
                    speech::brief(&doc, &cx).replace('\n', " ")
                )),
            }
        }
        let date = feed.store().message(id).map(|m| m.date.rfc3339());
        items.push(json!({"chat_id": id.chat_id, "temp_message_id": id.message_id, "date": date}));
    }
    let resolved: Vec<Value> = before
        .iter()
        .filter(|id| !now.contains(id))
        .filter_map(|&id| fate(feed.store(), id))
        .collect();

    let data = json!({
        "command": "pending",
        "count": items.len(),
        "pending_sends": items,
        "resolved": resolved,
        "synced": synced,
        "elapsed_ms": crate::elapsed_ms(started),
    });
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(&summary_zh(now.len(), &resolved, synced)),
    }
    Ok(0)
}

/// 本次运行里不再待发的那条去了哪：换了 id 的看新 id 那条的状态；不在了 = 被删（自己撤销、别的设备上删了，
/// 或 TDLib 放弃）。换了 id 仍在待发的（新 id 已在待发清单里）不算有着落，回 `None`
fn fate(store: &Store, old: MsgId) -> Option<Value> {
    let cur = store.current_id(old);
    let (outcome, id) = match store.entry(cur) {
        Some(Entry::Live(m)) if cur != old => match m.sending {
            SendingState::Sent => ("succeeded", Some(cur.message_id)),
            SendingState::Failed => ("failed", Some(cur.message_id)),
            SendingState::Pending => return None,
        },
        // 换了 id 的正式消息不在了：可能只是被删旧，证明不了删过，不报（codex 029 #2）
        None if cur != old => return None,
        // 没换 id 就不在了：被删掉（自己撤销、别的设备删了、TDLib 放弃），也可能只是清了缓存——分不清，报 `gone`
        // （待发的不参与删旧，`store::table::prunable`；codex 029 第 2 轮 #1）
        _ => ("gone", None),
    };
    Some(
        json!({"chat_id": old.chat_id, "temp_message_id": old.message_id, "outcome": outcome, "message_id": id}),
    )
}

/// 文字格式的结尾一句（参数用法以外的说明；数字来自上面同一份数据）
fn summary_zh(n: usize, resolved: &[Value], synced: bool) -> String {
    let count = |o: &str| resolved.iter().filter(|r| r["outcome"] == o).count();
    let mut s = if n == 0 {
        "没有待发出的消息".to_owned()
    } else {
        format!("待发出 {n} 条（之后每次启动都会自动续发；待发超过 24 小时的不再续发、变成失败）")
    };
    let (ok, failed, gone) = (count("succeeded"), count("failed"), count("gone"));
    if ok + failed + gone > 0 {
        s.push_str(&format!(
            "；这次启动后已有着落：发出 {ok} 条，失败 {failed} 条，不在了 {gone} 条（被删掉或只是清了缓存，分不清；去会话里看）"
        ));
    }
    if !synced {
        s.push_str("；还没连上服务器，列表可能不全");
    }
    s
}

// ───────────────────────── cancel ─────────────────────────

/// 一次 `pending cancel` 要的东西
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelParams {
    pub chat: String,
    pub dry_run: bool,
}

/// 在已起好的后端上撤销待发消息（直跑与执行方共用）
pub fn execute_cancel<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &CancelParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
) -> u8 {
    execute_cancel_within(b, stream, feed, p, sink, format, started, QUERY_TIMEOUT)
}

/// 同 [`execute_cancel`]，交出的期限由调用方给（单测用短的）
#[allow(clippy::too_many_arguments)]
fn execute_cancel_within<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &CancelParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: std::time::Duration,
) -> u8 {
    match run_cancel(b, stream, feed, p, sink, format, started, submit_timeout) {
        Ok(code) => code,
        // 交出后断线等「可能已交出」：换成撤销的说明（通用说明讲的是发送）
        Err(Fail::Core(e @ CoreError::SendOutcomeUnknown { .. })) => {
            crate::send::l2_fail(sink, format, &e, json!({ "message_zh": CANCEL_UNKNOWN_ZH }))
        }
        // L2 的错误一律 `retryable:false`（同 `send`）
        Err(Fail::Core(e)) => crate::send::l2_fail(sink, format, &e, json!({})),
        Err(f) => out::fail_with_to(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_cancel<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &CancelParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    submit_timeout: std::time::Duration,
) -> Result<u8, Fail> {
    let chat =
        history::ChatRef::parse(&p.chat).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))?;
    chats::ensure_ready(b)?;
    let chat_id = crate::send::resolve(b, chat)?;
    // 续发的那几条在 TDLib 启动重放本地日志时就推来了（授权到 ready 之前），这里已在流里
    feed.drain(stream);
    let ids: Vec<i64> = feed
        .store()
        .pending_sends()
        .into_iter()
        .filter(|m| m.chat_id == chat_id)
        .map(|m| m.message_id)
        .collect();
    // 「撤此刻全部」：超了不截断（截断后报 `all_cancelled` 是假话，codex 029 #1）
    if ids.len() > delete::MAX_IDS {
        return Err(Fail::Core(atg_core::error::CoreError::InvalidSend {
            why: atg_core::error::InvalidSendWhy::TooMany,
        }));
    }
    if ids.is_empty() {
        return Err(Fail::Core(atg_core::error::CoreError::InvalidSend {
            why: atg_core::error::InvalidSendWhy::NotPending,
        }));
    }
    let plan = bounded(
        b,
        delete::prepare_cancel(b, chat_id, &ids, SendOrigin::Cli),
        QUERY_TIMEOUT,
    )
    .map_err(Fail::Core)?;
    feed.drain(stream);
    emit_plan(sink, format, &plan);
    if p.dry_run {
        let data = json!({"command": "pending_cancel", "dry_run": true, "chat_id": chat_id,
            "elapsed_ms": crate::elapsed_ms(started)});
        finish(sink, format, data, "只看了计划，没有撤销");
        return Ok(0);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    // 交出（验凭据 → 复查 → 交给 TDLib）到点没回：删除可能已经交出，撤没撤下未知——不是能重跑的普通 `network`
    // （#109，同 `send` 交出超时报 `send_outcome_unknown`），留痕记「未知」
    let mode = match block_on_timeout(delete::cancel(b, &plan, token), submit_timeout) {
        Some(r) => r.map_err(Fail::Core)?,
        None => return Ok(cancel_unknown(b, &plan, sink, format)),
    };
    let temps: Vec<MsgId> = plan
        .items()
        .iter()
        .map(|i| MsgId {
            chat_id,
            message_id: i.id,
        })
        .collect();
    // `send::wait` 不看删除的标志：只有永久删除（`is_permanent && !from_cache`）才算撤下，清缓存、暂时不可访问的
    // 另记下来，按未知报（codex 029 第 2 轮 #1）
    let mut soft: Vec<MsgId> = Vec::new();
    let waited = atg_core::send::wait(stream, &temps, CANCEL_WAIT, &mut |env| {
        note_soft(&env.event, &mut soft);
        feed.apply(env);
    });
    // 没赶上、已发出的：TDLib 可能把删除换成了它的正式 id（`atg_core::delete` 模块文档）。那条删除多半和发出成功
    // 落在同一批、已被上面喂进 store（自审第 4 轮 P1），所以下落一律看 store；还活着的再给晚到的更新留一小会儿
    let still_live = |feed: &Feed| {
        waited.iter().any(|w| match w {
            Waited::Succeeded(m) => matches!(feed.store().entry(m.id), Some(Entry::Live(_))),
            _ => false,
        })
    };
    if still_live(feed) {
        let live: Vec<MsgId> = waited
            .iter()
            .filter_map(|w| match w {
                Waited::Succeeded(m) => Some(m.id),
                _ => None,
            })
            .collect();
        atg_core::send::wait(stream, &live, CANCEL_RACE_WAIT, &mut |env| {
            note_soft(&env.event, &mut soft);
            feed.apply(env);
        });
    }
    let items: Vec<Value> = temps
        .iter()
        .zip(&waited)
        .map(|(t, w)| {
            let (outcome, id) = cancel_outcome(feed.store(), w, soft.contains(t));
            json!({"temp_message_id": t.message_id, "outcome": outcome, "message_id": id})
        })
        .collect();
    let count = |o: &str| items.iter().filter(|i| i["outcome"] == o).count();
    let zh = format!(
        "撤销了 {} 条（{}）；没赶上、已发出 {} 条（其中 {} 条只从自己这边删掉了，对方那边还在）；原消息发送失败 {} 条；{} 秒内没等到下落 {} 条（跑 pending 按正文再看）",
        count("cancelled"),
        delete::CANCEL_NOTE_ZH,
        count("already_sent") + count("sent_deleted_here"),
        count("sent_deleted_here"),
        count("failed"),
        CANCEL_WAIT.as_secs(),
        count("unknown"),
    );
    // 文字格式也给出没撤销的那几条的 id（`resend` 要用失败那条的新 id，自审第 4 轮）
    let mut zh = zh;
    for i in items.iter().filter(|i| i["outcome"] != "cancelled") {
        zh.push_str(&format!(
            "；临时 id {} → {}{}",
            i["temp_message_id"],
            i["outcome"].as_str().unwrap_or(""),
            i["message_id"]
                .as_i64()
                .map(|m| format!("，消息 id {m}"))
                .unwrap_or_default()
        ));
    }
    let all = count("cancelled") == items.len();
    let data = json!({"command": "pending_cancel", "chat_id": chat_id, "confirm": mode.as_str(),
        "all_cancelled": all, "items": items, "note_zh": delete::CANCEL_NOTE_ZH,
        "elapsed_ms": crate::elapsed_ms(started)});
    // 每条的下落都是确定的数据（含「没等到」），照总规矩以 `result` 结尾、退出码 0；没全撤下看 `all_cancelled`
    finish(sink, format, data, &zh);
    Ok(0)
}

/// 撤销之后一条待发消息的下落（`cli.md` `pending` 行的 `outcome`）。没赶上、已发出的，看 store 里它的正式 id 还在不在：
/// 不在了（被删成墓碑或拿掉）= TDLib 把删除换成了正式 id、只删了自己这边（`sent_deleted_here`）
/// `soft`：这条临时 id 的删除不是永久删除（清缓存 / 暂时不可访问），证明不了撤下
fn cancel_outcome(store: &Store, w: &Waited, soft: bool) -> (&'static str, Option<i64>) {
    match w {
        Waited::Unknown(atg_core::error::UnknownWhy::PendingDeleted) if soft => ("unknown", None),
        Waited::Unknown(atg_core::error::UnknownWhy::PendingDeleted) => ("cancelled", None),
        // 只认删除留下的墓碑；store 里没有了（被删旧、清缓存）证明不了删过，按未知报（codex 029 #2）
        Waited::Succeeded(m) => match store.entry(m.id) {
            Some(Entry::Live(_)) => ("already_sent", Some(m.id.message_id)),
            Some(Entry::Tombstone(_)) => ("sent_deleted_here", Some(m.id.message_id)),
            None => ("unknown", Some(m.id.message_id)),
        },
        Waited::Failed { message, .. } => ("failed", Some(message.id.message_id)),
        Waited::Unknown(_) => ("unknown", None),
    }
}

/// 记下不是永久删除的那些 id（`messages_deleted` 的 `is_permanent` 为假或 `from_cache` 为真）
fn note_soft(event: &atg_core::model::CoreEvent, soft: &mut Vec<MsgId>) {
    if let atg_core::model::CoreEvent::MessagesDeleted {
        chat_id,
        message_ids,
        is_permanent,
        from_cache,
    } = event
        && (!is_permanent || *from_cache)
    {
        soft.extend(message_ids.iter().map(|&message_id| MsgId {
            chat_id: *chat_id,
            message_id,
        }));
    }
}

/// 撤销的「结果未知」说明（`send_outcome_unknown` 的通用说明讲的是发送，不贴撤销）
pub(crate) const CANCEL_UNKNOWN_ZH: &str = "撤销已交出，但还不知道撤没撤下；不要直接重做，先跑 pending 看这个会话里还有没有待发的，或去会话里看最近的消息";

/// 撤销交出到点没回（#109）：内核已故障报 `disconnected` 这一种来由，否则 `timeout`；退出码 10、不可重试
fn cancel_unknown<B: Backend>(b: &B, plan: &DeletePlan, sink: &mut dyn Sink, format: Format) -> u8 {
    let why = match crate::deadlines::stalled(b) {
        CoreError::Disconnected => UnknownWhy::Disconnected,
        _ => UnknownWhy::Timeout,
    };
    b.gate()
        .after_outcome(plan.mode(), plan, 0, None, Outcome::Unknown, None);
    let e = CoreError::SendOutcomeUnknown {
        chat_id: Some(plan.target().id),
        temp_message_id: 0,
        why,
    };
    crate::send::l2_fail(sink, format, &e, json!({ "message_zh": CANCEL_UNKNOWN_ZH }))
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &DeletePlan) {
    match format {
        Format::Json => emit_to(sink, &envelope("plan", plan.json())),
        Format::Text => {
            let items: Vec<String> = plan
                .items()
                .iter()
                .map(|i| match &i.text {
                    Some(t) => format!("id {}：{}", i.id, atg_core::gate::visible(t)),
                    None => format!("id {}：不是纯文字", i.id),
                })
                .collect();
            sink.out(&format!(
                "将撤销 {} 里的待发消息 {}；{}",
                atg_core::send::target_zh(plan.target()),
                items.join("；"),
                delete::CANCEL_NOTE_ZH,
            ));
        }
    }
}

fn finish(sink: &mut dyn Sink, format: Format, data: Value, zh: &str) {
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(zh),
    }
}

// 真后端实测（`#[ignore]`，要显式开）：见 `pending/exp_real.rs` 文件头
#[cfg(all(test, feature = "tdlib"))]
mod exp_real;
#[cfg(test)]
mod tests;
