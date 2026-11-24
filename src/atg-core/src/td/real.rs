//! 真实后端 [`TdBackend`] 的本体（只在 `tdlib-api` 特性下编）：类型、共用的内部助手、`Backend` 的实现。
//! 从 `td.rs` 的内联 `mod real` 原样挪出（#60 R7）；`start`、登录、读这几组 `Backend` 方法的主体放在子模块，
//! 根上一行转给它的 `do_*`：`start.rs` · `login.rs` · `read.rs`；另有 `submit.rs` 交出口 · `reactions.rs` 谁点了回应 ·
//! `upload.rs` 上传进度。

use super::lifecycle::{self, Run};
use super::{boot, requests};
use crate::auth::{self, AuthPrompt};
use crate::backend::{
    Backend, ChatsLoad, Downloaded, HistoryReq, StatusReport, Upload, already_subscribed,
};
use crate::config::{Profile, ProfileLock, ServerKind};
use crate::error::{CoreError, Holder, map_td_error};
use crate::gate::Gate;
use crate::keystore::KeyStore;
use crate::link::{self, EventStream, Shared, Step};
use crate::lock;
use crate::logline::{self, Field};
use crate::model::{AddedReactions, FailedInfo, MessageCaps};
use crate::model::{ChatId, ChatList, ForumTopicState, Message, MsgId};
use crate::model::{ChatTarget, Me};
use crate::secret::Secret;
use crate::send::{Answer, Sealed};
use crate::status::{AuthStep, CoreState, NetState};
use crate::tdjson;
use crate::tree::{FileRef, ReactionKind};
use serde_json::json;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tdlib_rs::{enums, functions, types};

// `start`、登录、读：见这三个子模块（#60 R7）
mod login;
mod read;
mod start;

// 交出口（发送、重发、表情回应）与「谁点了回应」：见这两个子模块（从本文件拆出，#60 行数线）
mod reactions;
mod submit;
mod upload;

/// 一个进程只许一个真实后端（tdlib-rs 的收取与应答分发都是全局的）
static INSTANCE: AtomicBool = AtomicBool::new(false);

/// 真实后端的配置
pub struct TdConfig {
    /// `.env.local`（开发期；默认 [`crate::config::default_env_file`]）
    pub env_file: PathBuf,
    /// 钥匙放哪（正式用 [`crate::keystore::Keychain`]）
    pub keystore: Box<dyn KeyStore>,
    /// 日志目录（默认 [`crate::config::default_log_dir`]）
    pub log_dir: PathBuf,
    /// 本进程是 CLI 还是 GUI（写进档案锁，别人打不开时能说清是谁占着）
    pub holder: Holder,
    /// 发送闸（直发与否只由启动参数定，建后端时交进来）
    pub gate: Gate,
}

struct Running {
    client_id: i32,
    use_test_dc: bool,
    profile: Profile,
    /// 正常关闭后放掉
    lock: Option<ProfileLock>,
    receiver: Option<JoinHandle<()>>,
}

pub struct TdBackend {
    cfg: TdConfig,
    shared: Arc<Shared>,
    stream: Mutex<Option<EventStream>>,
    run: Mutex<Run<Running>>,
    previous_unclean: AtomicBool,
}

/// TDLib 的错误 → 本项目的码；正在关闭时的 500 算断开
fn map_error_on(shared: &Shared, e: &types::Error) -> CoreError {
    if e.code == 500 && matches!(shared.lines().core, CoreState::Closing | CoreState::Closed) {
        return CoreError::Disconnected;
    }
    map_td_error(e.code, &e.message)
}

impl TdBackend {
    /// 一个进程只能建一个；第二个 → `internal`
    pub fn new(cfg: TdConfig) -> Result<Self, CoreError> {
        if INSTANCE.swap(true, Ordering::SeqCst) {
            return Err(CoreError::Internal {
                what: "td_backend_exists",
            });
        }
        let (shared, stream) = Shared::new();
        shared.watch_send_limits(cfg.gate.limits());
        Ok(Self {
            cfg,
            shared,
            stream: Mutex::new(Some(stream)),
            run: Mutex::new(Run::Idle),
            previous_unclean: AtomicBool::new(false),
        })
    }

    /// 本进程**实际**传给 `setTdlibParameters` 的 `use_test_dc`（发送闸只认它；还没起是 `None`）
    pub fn use_test_dc(&self) -> Option<bool> {
        lock(&self.run).running().map(|r| r.use_test_dc)
    }

    fn client_id(&self) -> Option<i32> {
        lock(&self.run).running().map(|r| r.client_id)
    }

    fn kind(&self) -> Option<ServerKind> {
        lock(&self.run).running().map(|r| r.profile.kind())
    }

    fn step(&self) -> AuthStep {
        self.shared.lines().auth
    }

    /// 还没 `start` 就调登录动作
    fn not_started() -> CoreError {
        CoreError::Internal {
            what: "not_started",
        }
    }

    /// 发一个登录请求：故障先到 → `disconnected`；TDLib 回错 → 映射。日志只记动作名与错误机读码
    async fn auth_request<F>(&self, action: &'static str, req: F) -> Result<(), CoreError>
    where
        F: Future<Output = Result<(), types::Error>>,
    {
        logline::info("auth_submit", &[Field::Name("action", action)]);
        let r = link::guard(self.shared.clone(), req)
            .await?
            .map_err(|e| self.map_error(&e));
        if let Err(e) = &r {
            logline::info(
                "auth_submit_failed",
                &[Field::Name("action", action), Field::Name("code", e.code())],
            );
        }
        r
    }

    /// TDLib 错误 → `CoreError`；关闭途中的 500（如 `Request aborted`）归 `disconnected`（`errors.md` 规则 9 的例外）
    fn map_error(&self, e: &types::Error) -> CoreError {
        map_error_on(&self.shared, e)
    }

    /// 发送类的交出：交给 TDLib、等应答、出错就登记进发送限制，整段放在自己的线程里。调用方等不及（发送的
    /// 30 秒上限）丢掉等待时，这条线程照样等到应答、照样登记晚到的受限 / 限流（codex 016 第 6 轮 #1）
    /// `accepted`：应答里交出成功、成败还要等的消息（预览、删除给空），线程里当场登记（#88：调用方丢掉等待后
    /// 没人替它登记）
    fn detached<T: Send + 'static>(
        &self,
        fut: impl Future<Output = Result<T, types::Error>> + Send + 'static,
        accepted: impl FnOnce(&T) -> Vec<MsgId> + Send + 'static,
    ) -> impl Future<Output = Result<T, CoreError>> {
        let (tx, rx) = futures_channel::oneshot::channel();
        let shared = self.shared.clone();
        let limits = self.cfg.gate.limits();
        // 这条线程自己的守卫：调用方先放（丢掉等待），交出未完成标记也盖到应答登记完（#88）
        let hold = limits.hold();
        let spawned = std::thread::Builder::new()
            .name("atg-send-submit".into())
            .spawn(move || {
                let r = match link::block_on_guarded(shared.clone(), fut) {
                    Ok(Ok(v)) => Ok(v),
                    Ok(Err(e)) => Err(map_error_on(&shared, &e)),
                    Err(e) => Err(e),
                };
                match &r {
                    Err(e) => {
                        limits.note(e);
                        logline::info("send_failed", &[Field::Name("code", e.code())]);
                    }
                    Ok(v) => limits.await_outcome(&accepted(v)),
                }
                drop(hold);
                let _ = tx.send(r);
            });
        async move {
            if spawned.is_err() {
                return Err(CoreError::Internal {
                    what: "send_thread_spawn",
                });
            }
            rx.await.unwrap_or(Err(CoreError::Disconnected))
        }
    }

    /// 读消息这组的错误：TDLib 对查不到的会话 / 消息回 400 / 404 带「not found」，归 `not_found`；
    /// 日志只记动作名与机读码
    fn read_error(&self, action: &'static str, e: &types::Error) -> CoreError {
        let mapped = if e.message.to_ascii_lowercase().contains("not found") {
            CoreError::NotFound {
                what: action,
                td: None,
            }
        } else {
            self.map_error(e)
        };
        logline::info(
            "read_failed",
            &[
                Field::Name("action", action),
                Field::Name("code", mapped.code()),
            ],
        );
        mapped
    }

    /// tdlib-rs 的消息序列化回 TDLib JSON，走唯一一份映射（`tdjson::message`）；失败回 `None` 并记一笔，调用方把它报成 `internal{td_message_reserialize_failed}`（整次报错，不丢那一条）
    fn map_message(m: types::Message) -> Option<Message> {
        match serde_json::to_value(enums::Message::Message(m)) {
            Ok(v) => Some(tdjson::message(&v)),
            Err(_) => {
                logline::warn("td_message_reserialize_failed", &[]);
                None
            }
        }
    }
}

impl Drop for TdBackend {
    fn drop(&mut self) {
        // 收线程还活着就不放名额：tdlib-rs 的收取是全局的，再建一个会抢同一个 receive()
        let done = lock(&self.run)
            .running()
            .is_none_or(|r| r.receiver.as_ref().is_none_or(JoinHandle::is_finished));
        if done {
            INSTANCE.store(false, Ordering::SeqCst);
        }
    }
}

/// 链进来的 TDLib 版本号（同步 `getOption version`，不建客户端、不联网）。`atg-cli version` 用
pub fn tdlib_version() -> Option<String> {
    let v = tdlib_rs::execute(json!({ "@type": "getOption", "name": "version" }))?;
    v["value"].as_str().map(str::to_owned)
}

/// 收线程的来源：`receive()` 最多等 2 秒；更新原样序列化回 TDLib JSON
fn source(client_id: i32) -> impl FnMut() -> Step + Send + 'static {
    move || match tdlib_rs::receive() {
        None => Step::Idle,
        Some((_, id)) if id != client_id => Step::Idle,
        Some((update, _)) => match serde_json::to_value(&update) {
            Ok(v) => Step::Update(v),
            Err(_) => {
                logline::warn("td_update_reserialize_failed", &[]);
                Step::Idle
            }
        },
    }
}

impl Backend for TdBackend {
    async fn start(&self, profile: &Profile) -> Result<(), CoreError> {
        self.do_start(profile).await
    }

    fn subscribe(&self) -> Result<EventStream, CoreError> {
        lock(&self.stream).take().ok_or_else(already_subscribed)
    }

    fn status(&self) -> StatusReport {
        StatusReport {
            status: self.shared.status(),
            lines: self.shared.lines(),
            previous_unclean: self.previous_unclean.load(Ordering::SeqCst),
        }
    }

    fn auth_prompt(&self) -> AuthPrompt {
        self.shared.auth_prompt()
    }

    fn wait_auth(&self, timeout: Duration, pred: &dyn Fn(AuthStep) -> bool) -> Option<AuthStep> {
        self.shared.wait_auth(timeout, pred)
    }

    fn auth_generation(&self) -> u64 {
        self.shared.auth_gen()
    }

    fn wait_auth_update(&self, timeout: Duration, after: u64) -> Option<AuthStep> {
        self.shared.wait_auth_after(timeout, after)
    }

    async fn auth_submit_phone(&self, phone: &Secret) -> Result<(), CoreError> {
        self.do_auth_submit_phone(phone).await
    }

    async fn auth_submit_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.do_auth_submit_code(code).await
    }

    async fn auth_resend_code(&self) -> Result<(), CoreError> {
        self.do_auth_resend_code().await
    }

    async fn auth_submit_email(&self, address: &Secret) -> Result<(), CoreError> {
        self.do_auth_submit_email(address).await
    }

    async fn auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.do_auth_submit_email_code(code).await
    }

    async fn auth_submit_password(&self, password: &Secret) -> Result<(), CoreError> {
        self.do_auth_submit_password(password).await
    }

    async fn auth_register(&self) -> Result<(), CoreError> {
        self.do_auth_register().await
    }

    async fn logout(&self) -> Result<(), CoreError> {
        self.do_logout().await
    }

    async fn chats_load(&self, list: &ChatList, limit: u32) -> Result<ChatsLoad, CoreError> {
        self.do_chats_load(list, limit).await
    }

    async fn chat_open(&self, chat: ChatId) -> Result<(), CoreError> {
        self.do_chat_open(chat).await
    }

    async fn chat_close(&self, chat: ChatId) -> Result<(), CoreError> {
        self.do_chat_close(chat).await
    }

    async fn forum_topic_get(
        &self,
        chat: ChatId,
        topic_id: i64,
    ) -> Result<ForumTopicState, CoreError> {
        self.do_forum_topic_get(chat, topic_id).await
    }

    async fn chat_get(&self, chat: ChatId) -> Result<(), CoreError> {
        self.do_chat_get(chat).await
    }

    async fn history(&self, req: HistoryReq) -> Result<Vec<Message>, CoreError> {
        self.do_history(req).await
    }

    async fn message_get(&self, mid: MsgId) -> Result<Message, CoreError> {
        self.do_message_get(mid).await
    }

    async fn upload_progress(&self, mid: MsgId) -> Result<Option<Upload>, CoreError> {
        self.upload_progress_impl(mid).await
    }

    async fn file_download(&self, file: FileRef) -> Result<Downloaded, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        if file.id == 0 {
            return Err(CoreError::NotFound {
                what: "file",
                td: None,
            });
        }
        // 优先级 1–32，取最高；offset 0 + limit 0 = 整个文件；synchronous = 等下完再回
        let r = link::guard(
            self.shared.clone(),
            functions::download_file(file.id, 32, 0, 0, true, id),
        )
        .await?;
        match r {
            Ok(enums::File::File(f)) if f.local.is_downloading_completed => Ok(Downloaded {
                path: PathBuf::from(f.local.path),
                // `file.size` 在大小未知时为 0（`tl/api.tl:322` `file.size`：「0 if unknown」），下完时已下字节数就是实际大小
                size: u64::try_from(f.size.max(f.local.downloaded_size)).unwrap_or(0),
            }),
            Ok(_) => Err(CoreError::Internal {
                what: "download_incomplete",
            }),
            Err(e) => Err(self.read_error("file", &e)),
        }
    }

    async fn mark_listened(&self, mid: MsgId) -> Result<(), CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(
            self.shared.clone(),
            functions::open_message_content(mid.chat_id, mid.message_id, id),
        )
        .await?;
        r.map_err(|e| self.read_error("message", &e))
    }

    fn wait_net(&self, timeout: Duration, pred: &dyn Fn(&NetState) -> bool) -> Option<NetState> {
        self.shared.wait_net(timeout, pred)
    }

    fn gate(&self) -> &Gate {
        &self.cfg.gate
    }

    async fn me(&self) -> Result<Me, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(self.shared.clone(), functions::get_me(id)).await?;
        let enums::User::User(u) = r.map_err(|e| self.read_error("me", &e))?;
        // 收藏夹 = 与自己的私聊，会话 id 即用户 id；让它进本地（不新建别的会话）
        self.chat_get(u.id).await?;
        let me = Me {
            user_id: u.id,
            saved_chat_id: u.id,
            test_number: auth::is_test_number(&u.phone_number),
        };
        self.cfg.gate.set_me(me);
        Ok(me)
    }

    async fn chat_target(&self, chat: ChatId) -> Result<ChatTarget, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        self.chat_get(chat).await?;
        let r = link::guard(self.shared.clone(), functions::get_chat(chat, id)).await?;
        let c = r.map_err(|e| self.read_error("chat", &e))?;
        let v = serde_json::to_value(&c).map_err(|_| CoreError::Internal {
            what: "td_chat_reserialize_failed",
        })?;
        let mapped = tdjson::chat_object(&v);
        let is_forum = if v["type"]["@type"] == "chatTypeSupergroup" {
            let sg = v["type"]["supergroup_id"].as_i64().unwrap_or(0);
            let r = link::guard(self.shared.clone(), functions::get_supergroup(sg, id)).await?;
            let enums::Supergroup::Supergroup(g) =
                r.map_err(|e| self.read_error("supergroup", &e))?;
            g.is_forum
        } else {
            false
        };
        Ok(ChatTarget {
            id: mapped.id,
            kind: mapped.kind,
            title: mapped.title,
            is_forum,
            saved: false,
        })
    }

    async fn message_caps(&self, mid: MsgId) -> Result<MessageCaps, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(
            self.shared.clone(),
            functions::get_message_properties(mid.chat_id, mid.message_id, id),
        )
        .await?;
        let enums::MessageProperties::MessageProperties(p) =
            r.map_err(|e| self.read_error("message", &e))?;
        Ok(requests::caps(&p))
    }

    async fn failed_info(&self, mid: MsgId) -> Result<Option<FailedInfo>, CoreError> {
        let id = self.client_id().ok_or_else(Self::not_started)?;
        let r = link::guard(
            self.shared.clone(),
            functions::get_message(mid.chat_id, mid.message_id, id),
        )
        .await?;
        let m = r.map_err(|e| self.read_error("message", &e))?;
        let v = serde_json::to_value(&m).map_err(|_| CoreError::Internal {
            what: "td_message_reserialize_failed",
        })?;
        Ok(tdjson::failed_info(&v))
    }

    async fn submit(&self, req: Sealed<'_>) -> Result<Answer, CoreError> {
        self.submit_sealed(req).await
    }

    async fn added_reactions(
        &self,
        id: MsgId,
        kind: Option<&ReactionKind>,
        offset: &str,
        limit: u32,
    ) -> Result<AddedReactions, CoreError> {
        self.added_reactions_impl(id, kind, offset, limit).await
    }

    fn shutdown(&self, timeout: Duration) -> Result<(), CoreError> {
        let Some(client_id) = self.client_id() else {
            return Ok(());
        };
        if self.shared.is_faulted() {
            return Err(CoreError::Disconnected);
        }
        let deadline = Instant::now() + timeout;
        // TDLib 自己已经关了（别处退出登录、会话被终止……）：不再发 close——没人收得到它的应答
        if !self.shared.is_closed() {
            self.shared.set_core(CoreState::Closing);
            // close 的应答也只等到期限为止：TDLib 卡住时退出路径不能跟着卡死
            let closed = link::block_on_timeout(
                link::guard(self.shared.clone(), functions::close(client_id)),
                timeout,
            );
            if let Some(Ok(Err(e))) = &closed {
                logline::warn(
                    "td_close_request_failed",
                    &[Field::Num("td_code", i64::from(e.code))],
                );
            }
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if self.shared.wait_closed(left) {
            let mut run = lock(&self.run);
            if let Some(r) = run.running_mut() {
                if let Some(h) = r.receiver.take() {
                    let _ = h.join();
                }
                // 收线程已停：放锁之前收尾交出未完成标记（#88）；再放掉档案锁（同一进程里可以再开）；重复关不再动标记
                let left = deadline.saturating_duration_since(Instant::now());
                self.cfg.gate.limits().closed(left);
                lifecycle::release(&mut r.lock, || r.profile.mark_clean());
            }
            logline::info("td_closed", &[]);
            Ok(())
        } else if self.shared.is_faulted() {
            Err(CoreError::Disconnected)
        } else {
            logline::warn("td_close_timeout", &[]);
            Err(CoreError::Internal {
                what: "close_timeout",
            })
        }
    }
}
