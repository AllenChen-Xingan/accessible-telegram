//! `td`：真实后端 [`TdBackend`]——起 TDLib、收线程、发请求、关闭流程。
//!
//! 只有这个模块许直接用 tdlib-rs；它的类型不许渗出去（`docs/plan/001_arch-stack.md` 第一节第 1、3、4 小节）。
//! TDLib 对象绝不用 `{:?}` 整个打印（拍板 005 补丁 ③），由 crate 根的 lint 行与 `scripts/check-no-debug.sh` 拦。
//! 代码都挂在 `tdlib-api` 特性下：不开特性时这个模块是空的，`cargo build` / CI 默认不碰 TDLib。
//!
//! 薄壳：收线程、序号、三线合成、监督都在 [`crate::link`]（与假后端共用）；映射在 [`crate::tdjson::classify`]
//! ——这里把 tdlib-rs 的 `Update` 原样序列化回 TDLib JSON 交过去，所以映射只有一份、夹具测过。
//!
//! 起步顺序（`config-privacy.md`）：校验档案路径 → 拿档案锁 → 读 `.env.local` → 从钥匙串取钥匙 → **建客户端之前**
//! 用同步 `execute` 把 TDLib 日志降到 0、写到 `~/Library/Logs/atg/tdlib-<档案>.log`（级别 1 也会漏正文，
//! `MessageContent.cpp:7696`）→ 放「运行中」标记 → 建客户端、起收线程 → `setTdlibParameters`（加密钥匙、
//! `use_test_dc` 只从档案标记来）→ 显式设 `online = false`。
//! 客户端建好之后 `start` 再失败（如钥匙不对、网络错）：客户端还在，调用方照常调 `shutdown` 收尾。
//!
//! tdlib-rs 的 `receive()` 是全进程共用的，内部 `OBSERVER` 是全局单例，所以一个进程只许一个 [`TdBackend`]。

#[cfg(feature = "tdlib-api")]
pub use real::{TdBackend, TdConfig, tdlib_version};

#[cfg(feature = "tdlib-api")]
mod real {
    use crate::auth::{self, AuthPrompt};
    use crate::backend::{Backend, StatusReport, already_subscribed};
    use crate::config::{self, Profile, ProfileLock, ServerKind};
    use crate::error::{CoreError, Holder, map_td_error};
    use crate::keystore::KeyStore;
    use crate::link::{self, EventStream, Shared, Step};
    use crate::logline::{self, Field};
    use crate::secret::{self, Secret};
    use crate::status::{AuthStep, CoreState};
    use serde_json::{Value, json};
    use std::future::Future;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use tdlib_rs::{enums, functions, types};

    /// 一个进程只许一个真实后端（tdlib-rs 的收取与应答分发都是全局的）
    static INSTANCE: AtomicBool = AtomicBool::new(false);

    /// TDLib 原生日志单文件上限（`config-privacy.md` 第一节第 5 小节）
    const TD_LOG_MAX_BYTES: i64 = 10 * 1024 * 1024;

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 真实后端的配置
    pub struct TdConfig {
        /// `.env.local`（开发期；默认 [`config::default_env_file`]）
        pub env_file: PathBuf,
        /// 钥匙放哪（正式用 [`crate::keystore::Keychain`]）
        pub keystore: Box<dyn KeyStore>,
        /// 日志目录（默认 [`config::default_log_dir`]）
        pub log_dir: PathBuf,
        /// 本进程是 CLI 还是 GUI（写进档案锁，别人打不开时能说清是谁占着）
        pub holder: Holder,
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
        run: Mutex<Option<Running>>,
        previous_unclean: AtomicBool,
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
            Ok(Self {
                cfg,
                shared,
                stream: Mutex::new(Some(stream)),
                run: Mutex::new(None),
                previous_unclean: AtomicBool::new(false),
            })
        }

        /// 本进程**实际**传给 `setTdlibParameters` 的 `use_test_dc`（发送闸只认它；还没起是 `None`）
        pub fn use_test_dc(&self) -> Option<bool> {
            lock(&self.run).as_ref().map(|r| r.use_test_dc)
        }

        fn client_id(&self) -> Option<i32> {
            lock(&self.run).as_ref().map(|r| r.client_id)
        }

        fn kind(&self) -> Option<ServerKind> {
            lock(&self.run).as_ref().map(|r| r.profile.kind())
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
            if e.code == 500
                && matches!(
                    self.shared.lines().core,
                    CoreState::Closing | CoreState::Closed
                )
            {
                return CoreError::Disconnected;
            }
            map_td_error(e.code, &e.message)
        }
    }

    impl Drop for TdBackend {
        fn drop(&mut self) {
            // 收线程还活着就不放名额：tdlib-rs 的收取是全局的，再建一个会抢同一个 receive()
            let done = lock(&self.run)
                .as_ref()
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

    /// 建客户端之前：TDLib 日志级别降到 0、输出改到档案自己的日志文件
    fn quiet_tdlib_log(log_dir: &Path, profile: &str) -> Result<(), CoreError> {
        let ok = |r: Option<Value>| r.is_some_and(|v| v["@type"] == "ok");
        let level = tdlib_rs::execute(json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 0,
        }));
        if !ok(level) {
            return Err(CoreError::Internal {
                what: "td_log_level",
            });
        }
        config::ensure_private_dir(log_dir)?;
        let path = log_dir.join(format!("tdlib-{profile}.log"));
        let path = path.to_str().ok_or(CoreError::Internal {
            what: "td_log_path",
        })?;
        let stream = tdlib_rs::execute(json!({
            "@type": "setLogStream",
            "log_stream": {
                "@type": "logStreamFile",
                "path": path,
                "max_file_size": TD_LOG_MAX_BYTES,
                "redirect_stderr": false,
            },
        }));
        if !ok(stream) {
            return Err(CoreError::Internal {
                what: "td_log_stream",
            });
        }
        Ok(())
    }

    /// 系统语言（`setTdlibParameters` 必填）：取 `LANG` 的语言部分，取不到用 `en`
    fn system_language() -> String {
        std::env::var("LANG")
            .ok()
            .and_then(|l| {
                let code: String = l.chars().take_while(char::is_ascii_lowercase).collect();
                (code.len() == 2).then_some(code)
            })
            .unwrap_or_else(|| "en".to_owned())
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
            if self.shared.is_faulted() {
                return Err(CoreError::Disconnected);
            }
            if lock(&self.run).is_some() {
                return Err(CoreError::Internal {
                    what: "already_started",
                });
            }
            // 先做纯校验，再占资源（锁、标记、客户端、收线程）
            let db = profile.db_dir();
            let files = profile.files_dir();
            let (Some(db), Some(files)) = (db.to_str(), files.to_str()) else {
                return Err(CoreError::Internal {
                    what: "profile_path_not_utf8",
                });
            };
            let profile_lock = profile.lock(self.cfg.holder)?;
            self.previous_unclean
                .store(profile.previous_unclean(), Ordering::SeqCst);
            let creds = config::load_api_credentials(&self.cfg.env_file)?;
            let key = self.cfg.keystore.get_or_create(profile.name())?;
            let key_b64 = secret::base64(key.expose());
            drop(key);
            quiet_tdlib_log(&self.cfg.log_dir, profile.name())?;
            profile.mark_running()?;

            let use_test_dc = profile.kind().use_test_dc();
            let client_id = tdlib_rs::create_client();
            let receiver = link::spawn_receiver(self.shared.clone(), source(client_id))?;
            *lock(&self.run) = Some(Running {
                client_id,
                use_test_dc,
                profile: profile.clone(),
                lock: Some(profile_lock),
                receiver: Some(receiver),
            });
            logline::info(
                "td_start",
                &[
                    Field::Num("client_id", i64::from(client_id)),
                    Field::Name("server", if use_test_dc { "test" } else { "main" }),
                ],
            );

            let params = functions::set_tdlib_parameters(
                use_test_dc,
                db.to_owned(),
                files.to_owned(),
                key_b64.expose_str().to_owned(),
                true,
                true,
                true,
                false,
                creds.api_id,
                creds.api_hash.expose_str().to_owned(),
                system_language(),
                "Mac".to_owned(),
                String::new(),
                env!("CARGO_PKG_VERSION").to_owned(),
                client_id,
            );
            drop(key_b64);
            drop(creds);
            link::guard(self.shared.clone(), params)
                .await?
                .map_err(|e| self.map_error(&e))?;
            self.shared.set_core(CoreState::Running);

            // 别人会看到「在线」：启动时显式设 false，不靠默认值（`actions.md` `set_online`）
            let offline = functions::set_option(
                "online".to_owned(),
                Some(enums::OptionValue::Boolean(types::OptionValueBoolean {
                    value: false,
                })),
                client_id,
            );
            if let Err(e) = link::guard(self.shared.clone(), offline)
                .await?
                .map_err(|e| self.map_error(&e))
            {
                logline::warn("td_set_offline_failed", &[Field::Name("code", e.code())]);
            }
            Ok(())
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

        fn wait_auth(
            &self,
            timeout: Duration,
            pred: &dyn Fn(AuthStep) -> bool,
        ) -> Option<AuthStep> {
            self.shared.wait_auth(timeout, pred)
        }

        fn auth_generation(&self) -> u64 {
            self.shared.auth_gen()
        }

        fn wait_auth_update(&self, timeout: Duration, after: u64) -> Option<AuthStep> {
            self.shared.wait_auth_after(timeout, after)
        }

        // 交给 TDLib 的值要转成 String（tdlib-rs 的接口如此）：那份副本清不掉，`secret.rs` 头部已说明
        async fn auth_submit_phone(&self, phone: &Secret) -> Result<(), CoreError> {
            let kind = self.kind().ok_or_else(Self::not_started)?;
            let id = self.client_id().ok_or_else(Self::not_started)?;
            let phone = auth::prepare_phone(self.step(), kind, phone)?;
            let req =
                functions::set_authentication_phone_number(phone.expose_str().to_owned(), None, id);
            self.auth_request("phone", req).await
        }

        async fn auth_submit_code(&self, code: &Secret) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::AuthAction::Code.check(self.step())?;
            let req = functions::check_authentication_code(code.expose_str().to_owned(), id);
            self.auth_request("code", req).await
        }

        async fn auth_resend_code(&self) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::AuthAction::Resend.check(self.step())?;
            let req = functions::resend_authentication_code(
                Some(enums::ResendCodeReason::UserRequest),
                id,
            );
            self.auth_request("resend", req).await
        }

        async fn auth_submit_email(&self, address: &Secret) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::AuthAction::Email.check(self.step())?;
            let req =
                functions::set_authentication_email_address(address.expose_str().to_owned(), id);
            self.auth_request("email", req).await
        }

        async fn auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::AuthAction::EmailCode.check(self.step())?;
            let req = functions::check_authentication_email_code(
                enums::EmailAddressAuthentication::Code(types::EmailAddressAuthenticationCode {
                    code: code.expose_str().to_owned(),
                }),
                id,
            );
            self.auth_request("email_code", req).await
        }

        async fn auth_submit_password(&self, password: &Secret) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::AuthAction::Password.check(self.step())?;
            let req =
                functions::check_authentication_password(password.expose_str().to_owned(), id);
            self.auth_request("password", req).await
        }

        async fn auth_register(&self) -> Result<(), CoreError> {
            let kind = self.kind().ok_or_else(Self::not_started)?;
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::prepare_register(self.step(), kind)?;
            let req = functions::register_user(
                auth::TEST_REGISTER_FIRST_NAME.to_owned(),
                auth::TEST_REGISTER_LAST_NAME.to_owned(),
                true,
                id,
            );
            self.auth_request("register", req).await
        }

        async fn logout(&self) -> Result<(), CoreError> {
            let id = self.client_id().ok_or_else(Self::not_started)?;
            auth::prepare_logout(self.step())?;
            self.auth_request("logout", functions::log_out(id)).await
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
                if let Some(r) = run.as_mut() {
                    if let Some(h) = r.receiver.take() {
                        let _ = h.join();
                    }
                    r.profile.mark_clean();
                    // 正常关闭：放掉档案锁（同一进程里可以再开这个档案）
                    r.lock = None;
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
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tdlib-api")]
    mod roundtrip {
        use crate::tdjson;
        use serde_json::{Value, json};
        use tdlib_rs::enums::Update;

        /// 把 `patch` 深合并到 `base` 上
        fn merge(base: &mut Value, patch: Value) {
            match (base, patch) {
                (Value::Object(b), Value::Object(p)) => {
                    for (k, v) in p {
                        merge(b.entry(k).or_insert(Value::Null), v);
                    }
                }
                (b, p) => *b = p,
            }
        }

        fn text(t: &str) -> Value {
            json!({"@type": "formattedText", "text": t, "entities": [
                {"@type": "textEntity", "offset": 0, "length": 2, "type": {"@type": "textEntityTypeBold"}}
            ]})
        }

        /// TDLib 真会给的整条消息：必带字段齐全（编的数据，id 在保留区间）
        fn message(id: i64, patch: Value) -> Value {
            let mut m = json!({
                "@type": "message", "id": id, "chat_id": 777,
                "sender_id": {"@type": "messageSenderUser", "user_id": 777},
                "is_outgoing": false, "is_pinned": false, "is_from_offline": false, "can_be_saved": true,
                "has_timestamped_media": false, "is_channel_post": false,
                "is_paid_star_suggested_post": false, "is_paid_gram_suggested_post": false,
                "contains_unread_mention": false, "contains_unread_poll_votes": false,
                "date": 1792755420, "edit_date": 0, "unread_reactions": [],
                "self_destruct_in": 0.0, "auto_delete_in": 0.0, "via_bot_user_id": 0,
                "sender_business_bot_user_id": 0, "sender_boost_count": 0, "sender_tag": "",
                "paid_message_star_count": 0, "author_signature": "", "media_album_id": "0",
                "effect_id": "0", "summary_language_code": "",
                "content": {"@type": "messageText", "text": text("晚上好")},
            });
            merge(&mut m, patch);
            m
        }

        fn position(order: &str) -> Value {
            json!({"@type": "chatPosition", "list": {"@type": "chatListMain"}, "order": order, "is_pinned": false})
        }

        fn samples() -> Vec<Value> {
            vec![
                json!({"@type": "updateNewMessage", "message": message(5, json!({}))}),
                json!({"@type": "updateNewMessage", "message": message(6, json!({
                    "edit_date": 1792755500,
                    "reply_to": {"@type": "messageReplyToMessage", "chat_id": 777, "message_id": 5,
                                 "checklist_task_id": 0, "poll_option_id": "", "origin_send_date": 0},
                    "forward_info": {"@type": "messageForwardInfo", "date": 1792755000,
                        "origin": {"@type": "messageOriginHiddenUser", "sender_name": "苏木"},
                        "public_service_announcement_type": ""},
                    "sending_state": {"@type": "messageSendingStatePending", "sending_id": 0},
                }))}),
                json!({"@type": "updateNewMessage", "message": message(7, json!({
                    "content": {"@type": "messageChatChangeTitle", "title": "周末读书会"},
                }))}),
                json!({"@type": "updateMessageContent", "chat_id": 777, "message_id": 5,
                   "new_content": {"@type": "messageText", "text": text("改过的")}}),
                json!({"@type": "updateMessageEdited", "chat_id": 777, "message_id": 5, "edit_date": 1792755600}),
                json!({"@type": "updateDeleteMessages", "chat_id": 777, "message_ids": [5, 6],
                   "is_permanent": true, "from_cache": false}),
                json!({"@type": "updateMessageSendFailed", "old_message_id": 6,
                   "message": message(8, json!({"sending_state": {"@type": "messageSendingStateFailed",
                        "error": {"@type": "error", "code": 400, "message": "MESSAGE_TOO_LONG"},
                        "can_retry": true, "need_another_sender": false, "need_another_reply_quote": false,
                        "need_drop_reply": false, "required_paid_message_star_count": 0, "retry_after": 0.0}})),
                   "error": {"@type": "error", "code": 400, "message": "MESSAGE_TOO_LONG"}}),
                json!({"@type": "updateChatPosition", "chat_id": 777, "position": position("200")}),
                json!({"@type": "updateChatLastMessage", "chat_id": 777, "last_message": message(5, json!({})),
                   "positions": [position("300")]}),
                json!({"@type": "updateChatDraftMessage", "chat_id": 777, "positions": [position("400")]}),
                json!({"@type": "updateChatReadInbox", "chat_id": 777, "last_read_inbox_message_id": 5, "unread_count": 2}),
                json!({"@type": "updateChatTitle", "chat_id": 777, "title": "林小雨"}),
                json!({"@type": "updateConnectionState", "state": {"@type": "connectionStateReady"}}),
                json!({"@type": "updateAuthorizationState",
                   "authorization_state": {"@type": "authorizationStateWaitPhoneNumber"}}),
            ]
        }

        /// 映射只有一份的前提：tdlib-rs 的 `Update` 序列化回去，与 TDLib 原样 JSON 走 `tdjson::classify` / `event`
        /// 得到同一个结果（`td` 的来源就是这么喂收线程的）
        #[test]
        fn reserialized_updates_map_to_the_same_event() {
            // 防空转：样本确实映射出了有内容的消息，回复、转发、待发送都认出来了
            use crate::model::{Body, CoreEvent, SendingState};
            let s = samples();
            let CoreEvent::NewMessage(m) = tdjson::event(&s[1]) else {
                panic!("样本 1 不是新消息");
            };
            assert!(m.reply_to.is_some() && m.forward.is_some() && m.edited.is_some());
            assert_eq!(m.sending, SendingState::Pending);
            assert!(matches!(&m.content, Body::Content(c) if !c.body.is_empty()));
            assert!(matches!(
                tdjson::event(&s[6]),
                CoreEvent::SendFailed {
                    can_retry: true,
                    ..
                }
            ));
            for v in s {
                let ty = v["@type"].as_str().unwrap_or("?").to_owned();
                let update: Update = match serde_json::from_value(v.clone()) {
                    Ok(u) => u,
                    Err(e) => panic!("{ty} 解析不了：{e}"),
                };
                let round = serde_json::to_value(&update).unwrap();
                assert_eq!(tdjson::event(&round), tdjson::event(&v), "{ty}");
                match (tdjson::classify(&round), tdjson::classify(&v)) {
                    (tdjson::Classified::Auth(a), tdjson::Classified::Auth(b)) => assert_eq!(a, b),
                    (tdjson::Classified::Net(a), tdjson::Classified::Net(b)) => assert_eq!(a, b),
                    (tdjson::Classified::Event(a), tdjson::Classified::Event(b)) => {
                        assert_eq!(a, b)
                    }
                    _ => panic!("{ty} 去向不同"),
                }
            }
        }
    }
}
