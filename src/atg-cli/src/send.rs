//! `send`、`resend`：发文字 / 回复某条、重发失败的消息（M2.2，T04）。契约：`specs/contracts/cli.md` `send` / `resend` 行，
//! 动作与闸：`specs/contracts/actions.md` 第一节第 3–5 小节；流程在 `atg_core::send`。
//!
//! 输出（直跑与经本机接口转发同一段代码）：
//! 1. 直发模式直跑时先一行 `status`（`direct_send:true`），标准错误一句醒目提示
//! 2. 一行 `plan`：目标（种类、名字、id）、全文、回复的那条（id + 简短读法）、最终话题、发起方、确认方式、摘要
//! 3. `--dry-run` 到此为止（`result` 带 `dry_run:true`）
//! 4. 闸开凭据：测试服务器 + 收藏夹自动；正式服务器弹 Touch ID / 系统密码；直发模式不问（但查防重复，`--again` 放行）
//! 5. 等结果（30 秒）：成功 → 回显发出的那条（读法同 `history`）+ `result`（最终 `message_id`）；失败 → `error`
//!    （码来自失败原因，带失败那条的新 id、能否 `resend`）；没等到 / 待发那条被删 / 断了 → `send_outcome_unknown`（退出码 10，**不许重发**）
//!
//! L2 = 对外、不可逆的动作（发送、删除……；分级见 `specs/contracts/actions.md` 第一节第 2 小节）。它的错误一律
//! `retryable:false`（`errors.md`）：重试一次就可能让对方收到两条，AI 不得据此重发。正文只从 `--text` / `--text-file` 来，不从标准输入读。

use crate::args::Format;
use crate::chats::{self, Feed};
use crate::history::{self, ChatRef};
use crate::out::{self, Fail, Sink, emit_to, envelope};
use atg_core::backend::Backend;
use atg_core::error::CoreError;
use atg_core::gate::{ConfirmMode, Gated};
use atg_core::link::{EventStream, block_on};
use atg_core::model::Message;
use atg_core::project;
use atg_core::send::{self, Draft, SendOrigin, TextPlan, Waited};
use atg_core::speech::{self, SpeechCtx};
use atg_core::store::Input;
use atg_core::tree::Doc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

// 正文上限的定义在 core（GUI 也受它管）；这里在起后端之前先查一遍，好报用法错误
use atg_core::send::TEXT_MAX_BYTES;

/// 一次 `send` 要的东西（经本机接口转发时原样序列化过去；`--text-file` 在发起方读成 `text`）
// 带正文：`Debug` 只在单测里派生（拍板 005 补丁 ③，同 `atg_core` 的 `redact`）
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct SendParams {
    pub chat: String,
    pub text: String,
    pub reply_to: Option<i64>,
    pub dry_run: bool,
    pub again: bool,
    /// 读法时区（全局选项，发起方填进来）
    #[serde(default)]
    pub tz: Option<String>,
}

/// 一次 `resend` 要的东西
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResendParams {
    pub chat: String,
    pub ids: Vec<i64>,
    pub dry_run: bool,
    #[serde(default)]
    pub tz: Option<String>,
}

/// 从参数组出 `send` 的请求：正文只从 `--text` / `--text-file` 来
pub fn send_params(a: &crate::args::SendArgs, tz: Option<&str>) -> Result<SendParams, Fail> {
    chat_ref(&a.chat)?;
    ctx(tz)?;
    let text = match (&a.text, &a.text_file) {
        (Some(t), None) => t.clone(),
        (None, Some(p)) => read_text_file(p)?,
        _ => return Err(Fail::Usage("正文用 --text 或 --text-file 给，二选一")),
    };
    check_len(&text)?;
    Ok(SendParams {
        chat: a.chat.clone(),
        text,
        reply_to: a.reply_to,
        dry_run: a.dry_run,
        again: a.again,
        tz: tz.map(str::to_owned),
    })
}

/// 正文上限（`--text`、`--text-file`、经本机接口转来的都查；core 的准备步骤还会再查一次）
fn check_len(text: &str) -> Result<(), Fail> {
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > TEXT_MAX_BYTES {
        return Err(Fail::Usage("正文要在 64 KB 以内"));
    }
    Ok(())
}

fn read_text_file(p: &std::path::Path) -> Result<String, Fail> {
    let meta = std::fs::metadata(p).map_err(|_| Fail::Usage("--text-file 读不了"))?;
    if !meta.is_file() || meta.len() > TEXT_MAX_BYTES {
        return Err(Fail::Usage("--text-file 要是 64 KB 以内的普通文件"));
    }
    let b = std::fs::read(p).map_err(|_| Fail::Usage("--text-file 读不了"))?;
    String::from_utf8(b).map_err(|_| Fail::Usage("--text-file 不是 UTF-8 文字"))
}

pub fn resend_params(a: &crate::args::ResendArgs, tz: Option<&str>) -> Result<ResendParams, Fail> {
    chat_ref(&a.chat)?;
    ctx(tz)?;
    Ok(ResendParams {
        chat: a.chat.clone(),
        ids: a.ids.clone(),
        dry_run: a.dry_run,
        tz: tz.map(str::to_owned),
    })
}

/// `saved` → 自己的收藏夹：问 `me()`（闸判收藏夹用的也是它），不等事件流里 TDLib 登录后推来的 `my_id` 选项
fn resolve<B: Backend>(b: &B, chat: ChatRef) -> Result<atg_core::model::ChatId, Fail> {
    match chat {
        ChatRef::Id(id) => Ok(id),
        ChatRef::Saved => block_on(b.me())
            .map(|m| m.saved_chat_id)
            .map_err(Fail::Core),
    }
}

fn chat_ref(s: &str) -> Result<ChatRef, Fail> {
    ChatRef::parse(s).ok_or(Fail::Usage("会话写数字 id，或 saved（收藏夹）"))
}

/// L2 的错误：`retryable` 一律为假；`extra` 平铺进去
fn l2_fail(sink: &mut dyn Sink, format: Format, e: &CoreError, extra: Value) -> u8 {
    let mut data = out::core_error_data(e);
    data["retryable"] = json!(false);
    if let (Some(d), Value::Object(x)) = (data.as_object_mut(), extra) {
        d.extend(x);
    }
    match format {
        Format::Json => emit_to(sink, &envelope("error", data)),
        Format::Text => sink.out(data["message_zh"].as_str().unwrap_or(e.message_zh())),
    }
    out::core_exit(e)
}

fn fail_any(sink: &mut dyn Sink, format: Format, f: &Fail) -> u8 {
    match f {
        Fail::Core(e) => l2_fail(sink, format, e, json!({})),
        other => out::fail_with_to(sink, format, other),
    }
}

/// 要人按 Touch ID / 输系统密码前，在标准错误说一句人话（JSON 计划行里只有 `confirm:"human_auth"`，人看不懂；
/// 系统框可能被别的窗口挡住，读屏用户也不一定马上找到）
pub const HUMAN_AUTH_PROMPT_ZH: &str =
    "atg-cli：请在弹出的系统框里按 Touch ID 或输入系统密码，确认这次发送（60 秒内）";

/// 直发模式直跑时的开场：标准错误一句醒目提示 + 标准输出第一行 `status`（`actions.md` 第一节第 5 小节）
pub fn direct_banner<B: Backend>(b: &B, sink: &mut dyn Sink, format: Format) {
    sink.err("⚠️ atg-cli：直发模式已开启——本次运行发送不再确认（--unsafe-direct-send）");
    let r = b.status();
    if format == Format::Json {
        emit_to(
            sink,
            &envelope(
                "status",
                json!({
                    "core": r.lines.core,
                    "auth": r.lines.auth,
                    "net": r.lines.net,
                    "user_status": r.status,
                    "pending_sends": [],
                    "direct_send": true,
                    "direct_sent": b.gate().direct_sent(),
                }),
            ),
        );
    }
}

/// 一条消息的读法上下文（同 `history`；CLI 没有按键）
fn ctx(tz: Option<&str>) -> Result<SpeechCtx, Fail> {
    history::speech_ctx(tz, false)
}

/// 把一条消息放进 store 再投影（被回复那条：store 里不一定有它）。store = 本进程的消息与会话缓存；投影
/// （`atg_core::project`）= 从 store 生成要读、要打印的内容树，只读 store 里已有的；store 只收经 `pump` 喂的输入。
/// 单条消息没有专门的输入，借「打开会话 + 这一页历史到了」放进去：这里的 `ticket` 是**历史请求凭据**
/// （`HistoryTicket`，防旧应答盖新的），与发送闸的确认凭据无关；只动本进程的 store，不是 TDLib 的 `openChat`，不标已读
fn project_one(feed: &mut Feed, m: Message) -> Option<Doc> {
    let id = m.id;
    if feed.store().message(id).is_none() {
        feed.pump_mut().apply(Input::OpenChat {
            chat_id: id.chat_id,
        });
        let ticket = feed.pump_mut().store_mut().ticket(id.chat_id);
        feed.pump_mut().apply(Input::HistoryLoaded {
            ticket,
            messages: vec![m],
        });
    }
    project::message(feed.store(), id)
}

/// 回显一条消息（JSON：`kind:"message"` 一行；文字：一句读法）——同 `history`
fn echo(sink: &mut dyn Sink, format: Format, cx: &SpeechCtx, doc: &Doc) {
    match format {
        Format::Json => {
            let mut line = atg_core::json::value(doc, cx);
            if let Some(o) = line.as_object_mut() {
                o.insert("at".into(), json!(out::now_rfc3339_ms()));
            }
            emit_to(sink, &line);
        }
        Format::Text => sink.out(&speech::brief(doc, cx).replace('\n', " ")),
    }
}

fn confirm_zh(m: ConfirmMode) -> &'static str {
    match m {
        ConfirmMode::TestExempt => "测试服务器发收藏夹，不用确认",
        ConfirmMode::HumanAuth => "要你按 Touch ID 或输入系统密码确认",
        ConfirmMode::DirectMode => "直发模式，不确认",
    }
}

// ───────────────────────── send ─────────────────────────

/// 在已起好的后端上发一条，输出写到 `sink`，返回退出码（直跑与执行方共用）
#[allow(clippy::too_many_arguments)]
pub fn execute_send<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &SendParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> u8 {
    match run_send(b, stream, feed, p, sink, format, started, timeout) {
        Ok(code) => code,
        Err(f) => fail_any(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_send<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &SendParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> Result<u8, Fail> {
    let chat = chat_ref(&p.chat)?;
    let cx = ctx(p.tz.as_deref())?;
    // 经本机接口转来的参数绕过了发起方的检查，这里再查一次
    check_len(&p.text)?;
    chats::ensure_ready(b)?;
    let chat_id = resolve(b, chat)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let draft = Draft {
        text: p.text.clone(),
        reply_to: p.reply_to,
    };
    let plan =
        block_on(send::prepare_text(b, chat_id, &draft, SendOrigin::Cli)).map_err(Fail::Core)?;
    feed.drain(stream);

    // 回复的那条：简短读法（同 history）
    let reply_brief = match plan.reply_to() {
        None => None,
        Some(id) => block_on(b.message_get(id))
            .ok()
            .and_then(|m| project_one(feed, m))
            .map(|d| speech::brief(&d, &cx).replace('\n', " ")),
    };
    emit_plan(sink, format, &plan, reply_brief.as_deref());
    if p.dry_run {
        finish_line(
            sink,
            format,
            json!({"command": "send", "dry_run": true, "chat_id": chat_id, "elapsed_ms": crate::elapsed_ms(started)}),
            "只看了计划，没有发",
        );
        return Ok(0);
    }

    if plan.mode() == ConfirmMode::HumanAuth && plan.confirm_fits() {
        sink.err(HUMAN_AUTH_PROMPT_ZH);
    }
    let token = b.gate().issue(&plan, p.again).map_err(Fail::Core)?;
    let (mode, temp, waited) =
        send::send_and_wait(b, &plan, token, stream, timeout, &mut |env| feed.apply(env))
            .map_err(Fail::Core)?;

    match waited {
        Waited::Succeeded(m) => {
            let kept = send::check_kept(&plan, &m).is_ok();
            if let Some(d) = project::message(feed.store(), m.id) {
                echo(sink, format, &cx, &d);
            }
            let mut data = json!({
                "command": "send",
                "chat_id": chat_id,
                "message_id": m.id.message_id,
                "temp_message_id": temp.message_id,
                "confirm": mode.as_str(),
                "elapsed_ms": crate::elapsed_ms(started),
            });
            // 发6③：发出后的那条丢了回复或换了话题（事后兜底，只能标明）
            if plan.reply_to().is_some() || plan.topic().is_some() {
                data["reply_kept"] = json!(kept);
            }
            let zh = if kept {
                "已发出"
            } else {
                "已发出，但 Telegram 去掉了回复关系或换了话题"
            };
            finish_line(sink, format, data, zh);
            Ok(0)
        }
        Waited::Failed {
            message,
            error,
            can_retry,
        } => Ok(l2_fail(
            sink,
            format,
            &error,
            json!({
                "chat_id": chat_id,
                "message_id": message.id.message_id,
                "temp_message_id": temp.message_id,
                "can_resend": can_retry,
            }),
        )),
        Waited::Unknown(why) => Err(Fail::Core(CoreError::SendOutcomeUnknown {
            chat_id: Some(chat_id),
            temp_message_id: temp.message_id,
            why,
        })),
    }
}

fn emit_plan(sink: &mut dyn Sink, format: Format, plan: &TextPlan, reply_brief: Option<&str>) {
    match format {
        Format::Json => {
            let mut d = send::plan_json(plan);
            d["reply_brief"] = json!(reply_brief);
            emit_to(sink, &envelope("plan", d));
        }
        Format::Text => {
            let reply = reply_brief
                .map(|r| format!("；回复：{r}"))
                .unwrap_or_default();
            sink.out(&format!(
                "将发给 {}：{}{reply}。{}；{}",
                send::target_zh(plan.target()),
                atg_core::gate::visible(plan.text()),
                confirm_zh(plan.mode()),
                send::QUEUE_NOTE_ZH,
            ));
        }
    }
}

fn finish_line(sink: &mut dyn Sink, format: Format, data: Value, zh: &str) {
    match format {
        Format::Json => emit_to(sink, &envelope("result", data)),
        Format::Text => sink.out(zh),
    }
}

// ───────────────────────── resend ─────────────────────────

/// 重发：一行 `plan`（每条写明会不会去掉回复关系）→ 确认 → 逐条等结果，成功的回显；结尾 `result` 带每条的下落。
/// 全部成功退出码 0；否则按第一条没成功的那条定（失败按原因，未知 10）
#[allow(clippy::too_many_arguments)]
pub fn execute_resend<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ResendParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> u8 {
    match run_resend(b, stream, feed, p, sink, format, started, timeout) {
        Ok(code) => code,
        Err(f) => fail_any(sink, format, &f),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_resend<B: Backend>(
    b: &B,
    stream: &mut EventStream,
    feed: &mut Feed,
    p: &ResendParams,
    sink: &mut dyn Sink,
    format: Format,
    started: Instant,
    timeout: Duration,
) -> Result<u8, Fail> {
    let chat = chat_ref(&p.chat)?;
    if p.ids.is_empty() {
        return Err(Fail::Usage("要重发哪几条：至少给一个消息 id"));
    }
    let cx = ctx(p.tz.as_deref())?;
    chats::ensure_ready(b)?;
    let chat_id = resolve(b, chat)?;
    history::load_chat(b, stream, feed, chat_id)?;
    let plan =
        block_on(send::prepare_resend(b, chat_id, &p.ids, SendOrigin::Cli)).map_err(Fail::Core)?;
    match format {
        Format::Json => emit_to(sink, &envelope("plan", send::resend_plan_json(&plan))),
        Format::Text => {
            let drop = if plan.items().iter().any(|i| i.drops_reply) {
                "；重发时会去掉回复关系"
            } else {
                ""
            };
            let texts: Vec<String> = plan
                .items()
                .iter()
                .map(|i| match &i.text {
                    Some(t) => format!("第 {} 条：{}", i.id, atg_core::gate::visible(t)),
                    None => format!("第 {} 条：不是纯文字", i.id),
                })
                .collect();
            sink.out(&format!(
                "将重发 {} 里的 {} 条{drop}：{}。{}；{}",
                send::target_zh(plan.target()),
                plan.items().len(),
                texts.join("；"),
                confirm_zh(plan.mode()),
                send::QUEUE_NOTE_ZH,
            ));
        }
    }
    if p.dry_run {
        finish_line(
            sink,
            format,
            json!({"command": "resend", "dry_run": true, "chat_id": chat_id, "elapsed_ms": crate::elapsed_ms(started)}),
            "只看了计划，没有重发",
        );
        return Ok(0);
    }
    // 重发不进直发防重复（`Gated::dedup` 为假），`again` 对它不起作用，传什么都一样
    if plan.mode() == ConfirmMode::HumanAuth && plan.confirm_fits() {
        sink.err(HUMAN_AUTH_PROMPT_ZH);
    }
    let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
    let (mode, outcomes) =
        send::resend_and_wait(b, &plan, token, stream, timeout, &mut |fed| match fed {
            send::Fed::Event(env) => feed.apply(env),
            send::Fed::Input(input) => {
                let _ = feed.pump_mut().apply(input);
            }
        })
        .map_err(Fail::Core)?;

    let mut items = Vec::new();
    // 第一条没成功的那条的错误：结尾 `error` 行的码、字段、退出码都从它来（与 send 同一条路）
    let mut first: Option<CoreError> = None;
    let mut unknown: Option<CoreError> = None;
    for o in outcomes {
        let old = o.old;
        let Some((temp, w)) = o.result else {
            items.push(json!({"message_id": old, "outcome": "not_resent"}));
            first.get_or_insert(CoreError::InvalidSend {
                why: atg_core::error::InvalidSendWhy::CannotRetry,
            });
            continue;
        };
        match w {
            Waited::Succeeded(m) => {
                if let Some(d) = project::message(feed.store(), m.id) {
                    echo(sink, format, &cx, &d);
                }
                items.push(json!({"message_id": old, "outcome": "succeeded", "new_message_id": m.id.message_id}));
            }
            Waited::Failed {
                message,
                error,
                can_retry,
            } => {
                items.push(json!({"message_id": old, "outcome": "failed", "new_message_id": message.id.message_id,
                    "code": error.code(), "can_resend": can_retry}));
                first.get_or_insert(error);
            }
            Waited::Unknown(why) => {
                items.push(json!({"message_id": old, "outcome": "unknown", "temp_message_id": temp.message_id, "why": why}));
                unknown.get_or_insert(CoreError::SendOutcomeUnknown {
                    chat_id: Some(chat_id),
                    temp_message_id: temp.message_id,
                    why,
                });
            }
        }
    }
    let count = |o: &str| items.iter().filter(|i| i["outcome"] == o).count();
    let (ok, failed, unsure) = (count("succeeded"), count("failed"), count("unknown"));
    let mut zh = format!("重发了 {} 条，成功 {ok} 条", p.ids.len());
    if failed > 0 {
        zh.push_str(&format!("，失败 {failed} 条"));
    }
    if unsure > 0 {
        let ids: Vec<String> = items
            .iter()
            .filter(|i| i["outcome"] == "unknown")
            .map(|i| i["message_id"].to_string())
            .collect();
        zh.push_str(&format!(
            "，结果未知 {unsure} 条（id {}：可能已发出、会自动补发，不要再重发）",
            ids.join("、")
        ));
    }
    // 结尾行的码：有结果未知的就报它（退出码 10，最要紧的是别重发），否则报第一条失败（codex 016 第 2 轮 #3）
    let Some(e) = unknown.or(first) else {
        finish_line(
            sink,
            format,
            json!({"command": "resend", "chat_id": chat_id, "confirm": mode.as_str(), "items": items,
                "elapsed_ms": crate::elapsed_ms(started)}),
            &zh,
        );
        return Ok(0);
    };
    // 有没成功的：结尾行是 `error`（退出码与最后一行一致），码与字段取第一条没成功的那条；未知的那条不许重发
    let msg = format!("{zh}；{}", e.message_zh());
    Ok(l2_fail(
        sink,
        format,
        &e,
        json!({"message_zh": msg, "chat_id": chat_id, "confirm": mode.as_str(), "items": items}),
    ))
}

#[cfg(test)]
mod tests {
    //! 假后端：数据全是编的（名字取自 `src/atg-core/fixtures/names.txt`），不碰真实账号与本机档案目录。
    use super::*;
    use atg_core::config::{Profile, ServerKind};
    use atg_core::error::{Holder, SendRefusedReason};
    use atg_core::fake::{FakeAuth, FakeBackend, FakeSend};
    use atg_core::gate::{Authenticator, Gate};
    use std::sync::atomic::{AtomicU32, Ordering};

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

    struct Deny;
    impl Authenticator for Deny {
        fn authenticate(&self, _: &str) -> Result<(), SendRefusedReason> {
            Err(SendRefusedReason::AuthFailed)
        }
    }

    const ME: i64 = 4242;
    const FRIEND: i64 = 777;

    fn backend(direct: bool, test_phone: bool) -> FakeBackend {
        let b = FakeBackend::with_gate(Gate::new(direct, Holder::Cli, Box::new(Deny))).unwrap();
        b.set_auth(FakeAuth {
            phone: if test_phone {
                "+9996612345"
            } else {
                "+15550001234"
            }
            .into(),
            logged_in: true,
            ..FakeAuth::default()
        });
        b.set_me(ME);
        let chat = |id: i64, title: &str| {
            vec![
                json!({"@type": "updateNewChat", "chat": {"@type": "chat", "id": id, "title": title,
                "type": {"@type": "chatTypePrivate", "user_id": id}, "positions": [],
                "notification_settings": {"@type": "chatNotificationSettings", "use_default_mute_for": true, "mute_for": 0}}}),
            ]
        };
        b.set_chat(ME, chat(ME, "我自己"));
        b.set_chat(FRIEND, chat(FRIEND, "林小雨"));
        b.push_net("connectionStateReady");
        b
    }

    fn profile(kind: ServerKind) -> Profile {
        static N: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "atg-cli-send-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        Profile::create(&root, "p", kind).unwrap()
    }

    fn params(chat: &str, text: &str) -> SendParams {
        SendParams {
            chat: chat.into(),
            text: text.into(),
            reply_to: None,
            dry_run: false,
            again: false,
            tz: Some("+08:00".into()),
        }
    }

    /// 在同一个后端上连发几次，每次的输出分开收
    fn run_many(b: &FakeBackend, p: &Profile, ps: &[SendParams]) -> Vec<(u8, Cap)> {
        crate::history::with_backend(b, p, Instant::now(), |b, st, fe| {
            Ok(ps
                .iter()
                .map(|sp| {
                    let mut cap = Cap::default();
                    let code = execute_send(
                        b,
                        st,
                        fe,
                        sp,
                        &mut cap,
                        Format::Json,
                        Instant::now(),
                        Duration::from_millis(500),
                    );
                    (code, cap)
                })
                .collect())
        })
        .unwrap_or_else(|_| panic!("起不来"))
    }

    fn last(cap: &Cap) -> Value {
        serde_json::from_str(cap.out.last().unwrap()).unwrap()
    }

    #[test]
    fn direct_duplicate_is_refused_with_when_and_what() {
        let b = backend(true, false);
        let p = profile(ServerKind::Main);
        let mut again = params("777", "好的");
        again.again = true;
        let r = run_many(
            &b,
            &p,
            &[params("777", "好的"), params("777", "好的"), again],
        );
        assert_eq!(r[0].0, 0);
        let e = last(&r[1].1);
        assert_eq!(r[1].0, 8);
        assert_eq!(e["kind"], "error");
        assert_eq!(e["data"]["reason"], "duplicate_recent");
        assert_eq!(e["data"]["last_outcome"], "succeeded");
        assert_eq!(e["data"]["retryable"], false);
        let zh = e["data"]["message_zh"].as_str().unwrap();
        assert!(
            zh.contains("秒前") && zh.contains("已发出") && zh.contains("--again"),
            "{zh}"
        );
        // 被拒的那次：plan 照出，没有交出去
        assert_eq!(b.submitted().len(), 2);
        assert_eq!(r[2].0, 0);
    }

    #[test]
    fn failure_ends_with_error_carrying_failed_id_and_resend_flag() {
        let b = backend(false, true);
        b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        }]);
        let p = profile(ServerKind::Test);
        let r = run_many(&b, &p, &[params("saved", "测试")]);
        let (code, cap) = &r[0];
        assert_eq!(*code, 7);
        let e = last(cap);
        assert_eq!(e["data"]["code"], "invalid_input");
        assert_eq!(e["data"]["can_resend"], true);
        assert_eq!(e["data"]["retryable"], false);
        assert!(e["data"]["message_id"].as_i64().is_some());
    }

    #[test]
    fn unknown_outcome_exits_10_and_says_do_not_resend() {
        for (step, why) in [
            (FakeSend::Silent, "timeout"),
            (FakeSend::Delete, "pending_deleted"),
        ] {
            let b = backend(false, true);
            b.script_sends(vec![step]);
            let p = profile(ServerKind::Test);
            let r = run_many(&b, &p, &[params("saved", "测试")]);
            let (code, cap) = &r[0];
            assert_eq!(*code, 10, "{why}");
            let e = last(cap);
            assert_eq!(e["data"]["code"], "send_outcome_unknown");
            assert_eq!(e["data"]["why"], why);
            assert!(
                e["data"]["message_zh"]
                    .as_str()
                    .unwrap()
                    .contains("不要重发")
            );
        }
    }

    #[test]
    fn main_server_without_touch_id_sends_nothing() {
        let b = backend(false, false);
        let p = profile(ServerKind::Main);
        let r = run_many(&b, &p, &[params("saved", "测试")]);
        let (code, cap) = &r[0];
        assert_eq!(*code, 8);
        assert_eq!(last(cap)["data"]["reason"], "auth_failed");
        // plan 先出，确认方式写明要 Touch ID
        let plan: Value = serde_json::from_str(&cap.out[0]).unwrap();
        assert_eq!(plan["kind"], "plan");
        assert_eq!(plan["data"]["confirm"], "human_auth");
        assert_eq!(plan["data"]["target"]["saved"], true);
        // 弹框前在标准错误说一句人话（自审第 12 轮 P2）
        assert!(cap.err.iter().any(|l| l == HUMAN_AUTH_PROMPT_ZH));
        assert!(b.submitted().is_empty());
    }

    #[test]
    fn dry_run_only_plans() {
        let b = backend(true, false);
        let p = profile(ServerKind::Main);
        let mut sp = params("777", "好的");
        sp.dry_run = true;
        let r = run_many(&b, &p, &[sp]);
        assert_eq!(r[0].0, 0);
        assert_eq!(r[0].1.out.len(), 2);
        assert_eq!(last(&r[0].1)["data"]["dry_run"], true);
        assert!(b.submitted().is_empty());
    }

    #[test]
    fn test_dc_other_chat_refused() {
        let b = backend(true, true);
        let p = profile(ServerKind::Test);
        let r = run_many(&b, &p, &[params("777", "好的")]);
        assert_eq!(r[0].0, 8);
        assert_eq!(last(&r[0].1)["data"]["reason"], "test_dc_saved_only");
    }

    fn resend_params(ids: Vec<i64>) -> ResendParams {
        ResendParams {
            chat: "saved".into(),
            ids,
            dry_run: false,
            tz: Some("+08:00".into()),
        }
    }

    /// 先发一条失败的，再按 error 行里的 message_id 重发：plan → 回显 → result（items 逐条）
    #[test]
    fn resend_after_failure_reports_each_item() {
        let b = backend(false, true);
        b.script_sends(vec![FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        }]);
        let p = profile(ServerKind::Test);
        let out = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
            let mut first = Cap::default();
            let c1 = execute_send(
                b,
                st,
                fe,
                &params("saved", "测试"),
                &mut first,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            let failed_id = last(&first)["data"]["message_id"].as_i64().unwrap();
            let mut second = Cap::default();
            let c2 = execute_resend(
                b,
                st,
                fe,
                &resend_params(vec![failed_id]),
                &mut second,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            Ok((c1, c2, failed_id, second))
        })
        .unwrap_or_else(|_| panic!("起不来"));
        let (c1, c2, failed_id, cap) = out;
        assert_eq!((c1, c2), (7, 0));
        let kinds: Vec<String> = cap
            .out
            .iter()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["kind"].to_string())
            .collect();
        assert_eq!(kinds, ["\"plan\"", "\"message\"", "\"result\""]);
        let r = last(&cap);
        assert_eq!(r["data"]["items"][0]["message_id"], failed_id);
        assert_eq!(r["data"]["items"][0]["outcome"], "succeeded");
        assert!(r["data"]["items"][0]["new_message_id"].as_i64().is_some());
    }

    /// 几条一起重发，一条又失败、一条结果未知：结尾报结果未知（退出码 10），说明里逐项计数、点名未知那条不要再重发
    /// （codex 016 第 2 轮 #3：原来只报第一条失败，文字模式看不到未知项）
    #[test]
    fn mixed_resend_reports_unknown_first() {
        let b = backend(false, true);
        let fail = || FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        };
        b.script_sends(vec![fail(), fail(), fail(), FakeSend::Silent]);
        let p = profile(ServerKind::Test);
        let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
            let mut ids = Vec::new();
            for text in ["测试一", "测试二"] {
                let mut c = Cap::default();
                execute_send(
                    b,
                    st,
                    fe,
                    &params("saved", text),
                    &mut c,
                    Format::Json,
                    Instant::now(),
                    Duration::from_millis(500),
                );
                ids.push(last(&c)["data"]["message_id"].as_i64().unwrap());
            }
            let mut second = Cap::default();
            let c = execute_resend(
                b,
                st,
                fe,
                &resend_params(ids),
                &mut second,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            Ok((c, second))
        })
        .unwrap_or_else(|_| panic!("起不来"));
        assert_eq!(code, 10);
        let e = last(&cap);
        assert_eq!(e["data"]["code"], "send_outcome_unknown");
        let zh = e["data"]["message_zh"].as_str().unwrap();
        assert!(
            zh.contains("失败 1 条") && zh.contains("结果未知 1 条") && zh.contains("不要再重发"),
            "{zh}"
        );
    }

    /// 重发又失败：结尾是 error，码与字段取那一条，items 照带
    #[test]
    fn resend_that_fails_again_ends_with_error() {
        let b = backend(false, true);
        let fail = || FakeSend::Fail {
            code: 400,
            message: "MESSAGE_TOO_LONG".into(),
            can_retry: true,
        };
        b.script_sends(vec![fail(), fail()]);
        let p = profile(ServerKind::Test);
        let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
            let mut first = Cap::default();
            execute_send(
                b,
                st,
                fe,
                &params("saved", "测试"),
                &mut first,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            let id = last(&first)["data"]["message_id"].as_i64().unwrap();
            let mut second = Cap::default();
            let c = execute_resend(
                b,
                st,
                fe,
                &resend_params(vec![id]),
                &mut second,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            Ok((c, second))
        })
        .unwrap_or_else(|_| panic!("起不来"));
        assert_eq!(code, 7);
        let e = last(&cap);
        assert_eq!(e["kind"], "error");
        assert_eq!(e["data"]["code"], "invalid_input");
        assert_eq!(e["data"]["retryable"], false);
        assert_eq!(e["data"]["items"][0]["outcome"], "failed");
        assert_eq!(e["data"]["items"][0]["can_resend"], true);
    }

    #[test]
    fn resend_of_message_that_did_not_fail_is_invalid() {
        let b = backend(false, true);
        let p = profile(ServerKind::Test);
        let (code, cap) = crate::history::with_backend(&b, &p, Instant::now(), |b, st, fe| {
            let mut first = Cap::default();
            execute_send(
                b,
                st,
                fe,
                &params("saved", "测试"),
                &mut first,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            let id = last(&first)["data"]["message_id"].as_i64().unwrap();
            let mut second = Cap::default();
            let c = execute_resend(
                b,
                st,
                fe,
                &resend_params(vec![id]),
                &mut second,
                Format::Json,
                Instant::now(),
                Duration::from_millis(500),
            );
            Ok((c, second))
        })
        .unwrap_or_else(|_| panic!("起不来"));
        assert_eq!(code, 7);
        assert_eq!(last(&cap)["data"]["why"], "not_failed");
    }
}
