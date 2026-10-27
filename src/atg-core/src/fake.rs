//! `fake`：假后端 [`FakeBackend`]（`specs/contracts/testing.md` 第一节第 2 小节）。
//!
//! 和真实后端用**同一个**收线程运行器（[`crate::link`]）：喂进来的是 TDLib JSON 形状的更新，走同一段
//! `classify` → 序号 → 三线合成 → 事件流。所以假后端测到的监督、序号、故障广播就是正式的那份。
//! 不碰 tdlib-rs 的全局收线程，测试可以并行跑多个。
//!
//! 能编排：`start` 的结果（成功 / 指定错误 / 一直不应答）、`shutdown` 卡住、收线程崩溃、来源断掉；
//! 每次调用记账（[`FakeBackend::calls`]）。场景目录（`fixtures/scenarios/`）与假时钟留给用到它的块（CLI `--fake`）。
//!
//! 登录（第 4 块）：[`FakeBackend::set_auth`] 给一个假账号 [`FakeAuth`]，`start` 后自己推到 `wait_phone`
//! （已登录的推到 `ready`）；每步提交按账号核对，对了就推下一步的授权状态（TDLib JSON 形状，带提示字段，
//! 走与真实后端同一段 [`AuthPrompt::from_td`]），错了回 TDLib 会回的那个错误（经 `map_td_error`）。
//! [`FakeBackend::fail_next`] 给某一步注入一次错误。记账只记动作名，**不记输入的值**。
//!
//! 会话（第 5 块）：[`FakeBackend::set_chats`] 给某个列表排好一串会话（每个会话是几条 TDLib JSON 更新）。
//! `chats_load(list, limit)` 每次推出前 `limit` 个会话的更新，推完了回「已到底」（TDLib 的 404）；没登录回 401。
//! 应答之前先等收线程把这批更新处理完（[`Feed::Barrier`]），与真实 TDLib「更新先于应答」一致。
//! [`FakeBackend::push_net`] 推一条连接状态。
//!
//! 消息（第 8 块）：[`FakeBackend::set_chat`] 给 `chat_get` 时要推的更新（`updateNewChat`、`updateUser`……），
//! [`FakeBackend::set_history`] 给某会话的消息（TDLib `message` 对象）。`history` 照 TDLib 的切法（`offset` 为 0
//! 不含 `from` 那条、往旧；负 `offset` 含它、另给比它新的），返回从新到旧；[`FakeBackend::stall_history`] 让接下来
//! 几次只回起点那一条；[`FakeBackend::short_first_history`]
//! 让每个会话第一次只回 1 条（TDLib 首次常这样）。`message_get` 从同一份消息里找。

use crate::auth::{self, AuthPrompt};
use crate::backend::{Backend, ChatsLoad, HistoryReq, StatusReport, already_subscribed};
use crate::config::{Profile, ProfileLock, ServerKind};
use crate::error::{CoreError, Holder, map_td_error};
use crate::link::{self, EventStream, Shared, Step};
use crate::model::{ChatId, ChatList, ForumTopicState, Message, MsgId};
use crate::secret::Secret;
use crate::status::{AuthStep, CoreState, NetState};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// `start` 怎么回
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeStart {
    Ok,
    Fail(CoreError),
    /// 一直不应答（测故障时等待中的请求会不会返回）
    Hang,
}

/// 一次调用的记账
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Start {
        profile: String,
        use_test_dc: bool,
    },
    Subscribe,
    /// `chat_get`
    ChatGet(ChatId),
    /// `history`：原样记下请求
    History(HistoryReq),
    /// `message_get`
    MessageGet(MsgId),
    /// `chat_open` / `chat_close`
    ChatOpen(ChatId),
    ChatClose(ChatId),
    /// `forum_topic_get`
    ForumTopicGet(ChatId, i64),
    Shutdown,
    /// 登录动作：只记名字（`phone` `code` `resend` `email` `email_code` `password` `register` `logout`），
    /// 不记值。只记真交到「TDLib」那一侧的——本地检查拦下的不记
    Auth(&'static str),
    /// `chats_load`：哪个列表、要几个
    ChatsLoad {
        list: ChatList,
        limit: u32,
    },
}

/// 假账号：登录各步该输什么。测试数据，值都是编的
#[derive(Clone, Default)]
pub struct FakeAuth {
    pub phone: String,
    pub code: String,
    /// 号码之后先要设登录邮箱：（邮箱，邮箱里的验证码）。之后不再要短信验证码
    pub email: Option<(String, String)>,
    /// 二步验证：（密码，提示语）
    pub password: Option<(String, String)>,
    /// 号码还没注册（测试服务器被清空后会这样）
    pub needs_registration: bool,
    /// 起来就已登录
    pub logged_in: bool,
}

enum Feed {
    Update(Value),
    /// 收线程处理完它之前的所有更新后回一声（更新先于应答）
    Barrier(mpsc::Sender<()>),
    Crash,
    End,
}

pub struct FakeBackend {
    shared: Arc<Shared>,
    stream: Mutex<Option<EventStream>>,
    feed: Mutex<mpsc::Sender<Feed>>,
    receiver: Mutex<Option<JoinHandle<()>>>,
    calls: Mutex<Vec<Call>>,
    start: Mutex<FakeStart>,
    close_hangs: Mutex<bool>,
    previous_unclean: Mutex<bool>,
    /// 起过之后：档案与锁（与真实后端同样的起停前置）
    run: Mutex<Option<(Profile, Option<ProfileLock>)>>,
    auth: Mutex<Option<FakeAuth>>,
    fail_next: Mutex<Vec<(&'static str, CoreError)>>,
    /// 每个列表还没交出去的会话；一个会话 = 几条更新
    chats: Mutex<HashMap<ChatList, VecDeque<Vec<Value>>>>,
    /// 还剩几次 `chats_load` 照常应答；之后一直不应答（断网时转去问服务器的样子）。`None` = 一直照常
    chats_load_answers: Mutex<Option<u32>>,
    /// `chat_get` 时推的更新
    chat_info: Mutex<HashMap<ChatId, Vec<Value>>>,
    /// `forum_topic_get` 回的话题（TDLib `forumTopic` JSON）
    forum_topics: Mutex<HashMap<(ChatId, i64), Value>>,
    /// 每个会话的消息（TDLib `message` 对象）
    histories: Mutex<HashMap<ChatId, Vec<Value>>>,
    /// 每个会话第一次 `history` 只回 1 条
    short_first: Mutex<Option<std::collections::HashSet<ChatId>>>,
    /// 停住：（先照常应答几次，再连着几次只回起点那一条）——TDLib 正从服务器拉、一直没拉到的样子
    stall: Mutex<(u32, u32)>,
    /// 每次 `history` 最多回几条（离起点最近的那几条，与 TDLib 只给内存里连着起点那一段同形）
    cap: Mutex<Option<usize>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeBackend {
    /// 新建并立刻起收线程（测试可以在 `start` 之前就喂更新）
    pub fn new() -> Result<Self, CoreError> {
        let (shared, stream) = Shared::new();
        let (tx, rx) = mpsc::channel::<Feed>();
        let handle = link::spawn_receiver(shared.clone(), move || {
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(Feed::Update(v)) => Step::Update(v),
                // 读到它时，前面的更新都已处理完（收线程处理完一条才回来取下一条）
                Ok(Feed::Barrier(done)) => {
                    let _ = done.send(());
                    Step::Idle
                }
                Ok(Feed::Crash) => panic!("fake receiver crash"),
                Ok(Feed::End) | Err(RecvTimeoutError::Disconnected) => Step::End,
                Err(RecvTimeoutError::Timeout) => Step::Idle,
            }
        })?;
        Ok(Self {
            shared,
            stream: Mutex::new(Some(stream)),
            feed: Mutex::new(tx),
            receiver: Mutex::new(Some(handle)),
            calls: Mutex::new(Vec::new()),
            start: Mutex::new(FakeStart::Ok),
            close_hangs: Mutex::new(false),
            previous_unclean: Mutex::new(false),
            run: Mutex::new(None),
            auth: Mutex::new(None),
            fail_next: Mutex::new(Vec::new()),
            chats: Mutex::new(HashMap::new()),
            chats_load_answers: Mutex::new(None),
            chat_info: Mutex::new(HashMap::new()),
            forum_topics: Mutex::new(HashMap::new()),
            histories: Mutex::new(HashMap::new()),
            short_first: Mutex::new(None),
            stall: Mutex::new((0, 0)),
            cap: Mutex::new(None),
        })
    }

    /// 给一个假账号（`start` 之前设）
    pub fn set_auth(&self, a: FakeAuth) {
        *lock(&self.auth) = Some(a);
    }

    /// 给某个列表排好会话（按 TDLib 的顺序交出；每个会话是几条 TDLib JSON 更新，如 `updateNewChat` +
    /// `updateChatPosition`）。再调一次是追加
    pub fn set_chats(&self, list: ChatList, chats: Vec<Vec<Value>>) {
        lock(&self.chats).entry(list).or_default().extend(chats);
    }

    /// `chat_get(id)` 时推这几条更新（`updateNewChat`、发送人的 `updateUser`……）；没设的会话 → `not_found`
    pub fn set_chat(&self, id: ChatId, updates: Vec<Value>) {
        lock(&self.chat_info).insert(id, updates);
    }

    /// `forum_topic_get(chat, topic)` 回这个话题（TDLib `forumTopic` 对象）；没设的 → `not_found`
    pub fn set_forum_topic(&self, chat: ChatId, topic_id: i64, topic: Value) {
        lock(&self.forum_topics).insert((chat, topic_id), topic);
    }

    /// 某会话的全部消息（TDLib `message` 对象，顺序无所谓）
    pub fn set_history(&self, chat: ChatId, messages: Vec<Value>) {
        lock(&self.histories).insert(chat, messages);
    }

    /// 每个会话第一次 `history` 只回 1 条（之后照常）
    pub fn short_first_history(&self) {
        *lock(&self.short_first) = Some(std::collections::HashSet::new());
    }

    /// 接下来 `n` 次 `history` 只回起点那一条，之后照常
    pub fn stall_history(&self, n: u32) {
        *lock(&self.stall) = (0, n);
    }

    /// 先照常应答 `ok` 次，再连着 `n` 次只回起点那一条，之后照常
    pub fn stall_history_after(&self, ok: u32, n: u32) {
        *lock(&self.stall) = (ok, n);
    }

    /// 每次 `history` 最多回 `c` 条（离起点最近的）
    pub fn cap_history(&self, c: usize) {
        *lock(&self.cap) = Some(c);
    }

    /// 前 `n` 次 `chats_load` 照常应答，之后一直不应答（测断网时加载卡住）
    pub fn hang_chats_load_after(&self, n: u32) {
        *lock(&self.chats_load_answers) = Some(n);
    }

    /// 推一条连接状态（`connectionStateReady` 这类类型名）
    pub fn push_net(&self, td_type: &str) {
        self.push(json!({
            "@type": "updateConnectionState",
            "state": { "@type": td_type },
        }));
    }

    /// 等收线程处理完已喂进去的更新（最多 5 秒；收线程没了就不等）
    fn settle(&self) {
        let (tx, rx) = mpsc::channel();
        if lock(&self.feed).send(Feed::Barrier(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    }

    /// 下一次 `action`（记账里的名字）回这个错误，一次性
    pub fn fail_next(&self, action: &'static str, e: CoreError) {
        lock(&self.fail_next).push((action, e));
    }

    fn take_failure(&self, action: &'static str) -> Option<CoreError> {
        let mut f = lock(&self.fail_next);
        let i = f.iter().position(|(a, _)| *a == action)?;
        Some(f.remove(i).1)
    }

    fn kind(&self) -> ServerKind {
        lock(&self.run)
            .as_ref()
            .map_or(ServerKind::Main, |(p, _)| p.kind())
    }

    fn account(&self) -> Result<FakeAuth, CoreError> {
        lock(&self.auth).clone().ok_or(CoreError::Internal {
            what: "fake_no_account",
        })
    }

    /// 一个登录动作的公共部分：记账 → 注入的错误 → 内核故障。过了才轮到账号核对
    async fn auth_call(&self, action: &'static str) -> Result<FakeAuth, CoreError> {
        lock(&self.calls).push(Call::Auth(action));
        let injected = self.take_failure(action);
        link::guard(self.shared.clone(), async move {
            match injected {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
        .await??;
        self.account()
    }

    fn push_state(&self, state: Value) {
        self.push(json!({ "@type": "updateAuthorizationState", "authorization_state": state }));
    }

    fn push_wait_code(&self, a: &FakeAuth, via: &str, can_resend: bool) {
        let next = if can_resend {
            json!({ "@type": "authenticationCodeTypeSms", "length": a.code.len() })
        } else {
            Value::Null
        };
        self.push_state(json!({
            "@type": "authorizationStateWaitCode",
            "code_info": {
                "@type": "authenticationCodeInfo",
                "phone_number": a.phone,
                "type": { "@type": via, "length": a.code.len() },
                "next_type": next,
                "timeout": 0,
            },
        }));
    }

    /// 验证码（或邮箱验证码）对了之后的下一步
    fn push_after_code(&self, a: &FakeAuth) {
        if a.needs_registration {
            self.push_state(json!({ "@type": "authorizationStateWaitRegistration" }));
        } else {
            self.push_after_registration(a);
        }
    }

    fn push_after_registration(&self, a: &FakeAuth) {
        match &a.password {
            Some((_, hint)) => self.push_state(json!({
                "@type": "authorizationStateWaitPassword",
                "password_hint": hint,
                "has_recovery_email_address": false,
                "has_passport_data": false,
                "recovery_email_address_pattern": "",
            })),
            None => self.push_auth("authorizationStateReady"),
        }
    }

    fn step(&self) -> AuthStep {
        self.shared.lines().auth
    }

    /// 与真实后端同一个先后：还没 `start` 就调登录动作 → `internal{not_started}`
    fn started(&self) -> Result<(), CoreError> {
        if lock(&self.run).is_some() {
            Ok(())
        } else {
            Err(CoreError::Internal {
                what: "not_started",
            })
        }
    }

    pub fn set_start(&self, s: FakeStart) {
        *lock(&self.start) = s;
    }

    /// `shutdown` 时收线程不给 `authorizationStateClosed`（测超时）
    pub fn set_close_hangs(&self, on: bool) {
        *lock(&self.close_hangs) = on;
    }

    /// 喂一条 TDLib JSON 形状的更新
    pub fn push(&self, update: Value) {
        let _ = lock(&self.feed).send(Feed::Update(update));
    }

    /// 喂一条授权状态更新（`authorizationStateWaitPhoneNumber` 这类类型名）
    pub fn push_auth(&self, td_type: &str) {
        self.push(json!({
            "@type": "updateAuthorizationState",
            "authorization_state": { "@type": td_type },
        }));
    }

    /// 让收线程崩溃（panic）
    pub fn crash(&self) {
        let _ = lock(&self.feed).send(Feed::Crash);
    }

    /// 让来源断掉（收线程没等到 Closed 就退出）
    pub fn end(&self) {
        let _ = lock(&self.feed).send(Feed::End);
    }

    /// 等收线程退出（测试里要确定故障已广播完再断言时用）
    pub fn join_receiver(&self) {
        if let Some(h) = lock(&self.receiver).take() {
            let _ = h.join();
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }
}

impl Backend for FakeBackend {
    async fn start(&self, profile: &Profile) -> Result<(), CoreError> {
        lock(&self.calls).push(Call::Start {
            profile: profile.name().to_owned(),
            use_test_dc: profile.kind().use_test_dc(),
        });
        // 起停前置与真实后端一致：故障后拒、不许起两次、先拿档案锁、放「运行中」标记
        if self.shared.is_faulted() {
            return Err(CoreError::Disconnected);
        }
        if lock(&self.run).is_some() {
            return Err(CoreError::Internal {
                what: "already_started",
            });
        }
        let profile_lock = profile.lock(Holder::Cli)?;
        *lock(&self.previous_unclean) = profile.previous_unclean();
        profile.mark_running()?;
        *lock(&self.run) = Some((profile.clone(), Some(profile_lock)));
        let answer = lock(&self.start).clone();
        link::guard(self.shared.clone(), async move {
            match answer {
                FakeStart::Ok => Ok(()),
                FakeStart::Fail(e) => Err(e),
                FakeStart::Hang => std::future::pending().await,
            }
        })
        .await??;
        self.shared.set_core(CoreState::Running);
        if let Some(a) = lock(&self.auth).as_ref() {
            self.push_auth(if a.logged_in {
                "authorizationStateReady"
            } else {
                "authorizationStateWaitPhoneNumber"
            });
        }
        Ok(())
    }

    fn subscribe(&self) -> Result<EventStream, CoreError> {
        lock(&self.calls).push(Call::Subscribe);
        lock(&self.stream).take().ok_or_else(already_subscribed)
    }

    fn status(&self) -> StatusReport {
        StatusReport {
            status: self.shared.status(),
            lines: self.shared.lines(),
            previous_unclean: *lock(&self.previous_unclean),
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
        self.started()?;
        let phone = auth::prepare_phone(self.step(), self.kind(), phone)?;
        let a = self.auth_call("phone").await?;
        let want = auth::normalize_phone(a.phone.as_bytes())?;
        if phone.expose() != want.expose() {
            return Err(map_td_error(400, "PHONE_NUMBER_INVALID"));
        }
        if a.email.is_some() {
            self.push_state(json!({
                "@type": "authorizationStateWaitEmailAddress",
                "allow_apple_id": false,
                "allow_google_id": false,
            }));
        } else {
            self.push_wait_code(&a, "authenticationCodeTypeTelegramMessage", true);
        }
        Ok(())
    }

    async fn auth_submit_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Code.check(self.step())?;
        let a = self.auth_call("code").await?;
        if !same(code, &a.code) {
            return Err(map_td_error(400, "PHONE_CODE_INVALID"));
        }
        self.push_after_code(&a);
        Ok(())
    }

    async fn auth_resend_code(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Resend.check(self.step())?;
        let a = self.auth_call("resend").await?;
        if !self.shared.auth_prompt().can_resend {
            // TDLib：没有下一种方式时重发被拒
            return Err(map_td_error(400, "SEND_CODE_UNAVAILABLE"));
        }
        self.push_wait_code(&a, "authenticationCodeTypeSms", false);
        Ok(())
    }

    async fn auth_submit_email(&self, address: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Email.check(self.step())?;
        let a = self.auth_call("email").await?;
        let Some((want, code)) = &a.email else {
            return Err(map_td_error(400, "EMAIL_NOT_ALLOWED"));
        };
        if !same(address, want) {
            return Err(map_td_error(400, "EMAIL_INVALID"));
        }
        self.push_state(json!({
            "@type": "authorizationStateWaitEmailCode",
            "allow_apple_id": false,
            "allow_google_id": false,
            "code_info": {
                "@type": "emailAddressAuthenticationCodeInfo",
                "email_address_pattern": "e***@example.com",
                "length": code.len(),
            },
            "email_address_reset_state": null,
        }));
        Ok(())
    }

    async fn auth_submit_email_code(&self, code: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::EmailCode.check(self.step())?;
        let a = self.auth_call("email_code").await?;
        let ok = a.email.as_ref().is_some_and(|(_, want)| same(code, want));
        if !ok {
            return Err(map_td_error(400, "EMAIL_CODE_INVALID"));
        }
        self.push_after_code(&a);
        Ok(())
    }

    async fn auth_submit_password(&self, password: &Secret) -> Result<(), CoreError> {
        self.started()?;
        auth::AuthAction::Password.check(self.step())?;
        let a = self.auth_call("password").await?;
        let ok = a
            .password
            .as_ref()
            .is_some_and(|(want, _)| same(password, want));
        if !ok {
            return Err(map_td_error(400, "PASSWORD_HASH_INVALID"));
        }
        self.push_auth("authorizationStateReady");
        Ok(())
    }

    async fn auth_register(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::prepare_register(self.step(), self.kind())?;
        let a = self.auth_call("register").await?;
        self.push_after_registration(&a);
        Ok(())
    }

    async fn logout(&self) -> Result<(), CoreError> {
        self.started()?;
        auth::prepare_logout(self.step())?;
        self.auth_call("logout").await?;
        self.push_auth("authorizationStateLoggingOut");
        self.push_auth("authorizationStateClosing");
        self.push_auth("authorizationStateClosed");
        Ok(())
    }

    async fn chats_load(&self, list: &ChatList, limit: u32) -> Result<ChatsLoad, CoreError> {
        self.started()?;
        // 本地检查拦下的不记账（同登录那组）
        if let ChatList::Unknown { .. } = list {
            return Err(CoreError::InvalidInput { td: None });
        }
        lock(&self.calls).push(Call::ChatsLoad {
            list: list.clone(),
            limit,
        });
        let hang = match lock(&self.chats_load_answers).as_mut() {
            Some(0) => true,
            Some(n) => {
                *n -= 1;
                false
            }
            None => false,
        };
        let injected = self.take_failure("chats_load");
        link::guard(self.shared.clone(), async move {
            if hang {
                std::future::pending::<()>().await;
            }
            match injected {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
        .await??;
        if self.step() != AuthStep::Ready {
            return Err(map_td_error(401, "Unauthorized"));
        }
        let batch: Vec<Vec<Value>> = {
            let mut all = lock(&self.chats);
            let q = all.entry(list.clone()).or_default();
            let n = q.len().min(usize::try_from(limit).unwrap_or(usize::MAX));
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            return Ok(ChatsLoad::End);
        }
        for update in batch.into_iter().flatten() {
            self.push(update);
        }
        self.settle();
        Ok(ChatsLoad::Loaded)
    }

    async fn chat_get(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatGet(chat));
        self.ready_or_401("chat_get").await?;
        let Some(updates) = lock(&self.chat_info).get(&chat).cloned() else {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        };
        for u in updates {
            self.push(u);
        }
        self.settle();
        Ok(())
    }

    async fn chat_open(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatOpen(chat));
        self.ready_or_401("chat_open").await?;
        if !lock(&self.chat_info).contains_key(&chat) {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        }
        Ok(())
    }

    async fn chat_close(&self, chat: ChatId) -> Result<(), CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ChatClose(chat));
        self.ready_or_401("chat_close").await?;
        Ok(())
    }

    async fn forum_topic_get(
        &self,
        chat: ChatId,
        topic_id: i64,
    ) -> Result<ForumTopicState, CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::ForumTopicGet(chat, topic_id));
        self.ready_or_401("forum_topic_get").await?;
        let t = lock(&self.forum_topics).get(&(chat, topic_id)).cloned();
        t.map(|t| crate::tdjson::forum_topic(&t))
            .ok_or(CoreError::NotFound {
                what: "forum_topic",
                td: None,
            })
    }

    async fn history(&self, req: HistoryReq) -> Result<Vec<Message>, CoreError> {
        self.started()?;
        req.check()?;
        lock(&self.calls).push(Call::History(req));
        self.ready_or_401("history").await?;
        if !lock(&self.chat_info).contains_key(&req.chat) {
            return Err(CoreError::NotFound {
                what: "chat",
                td: None,
            });
        }
        let mut all: Vec<Value> = lock(&self.histories)
            .get(&req.chat)
            .cloned()
            .unwrap_or_default();
        let id = |v: &Value| v["id"].as_i64().unwrap_or(0);
        all.sort_by_key(|v| std::cmp::Reverse(id(v)));
        // 照 TDLib 1.8.69 的切法（`td/telegram/OrderedMessage.cpp` `OrderedMessages::get_history`）：
        // - `from` 为 0：从最新一条起（含它）往旧
        // - `offset` 为 0：**不含** `from` 那条，从比它旧的起
        // - `offset` 为 -k：`from` 那条算进去，另给比它新的 k-1 条，再往旧接上
        let from_end = req.from == 0;
        let k = usize::try_from(-i64::from(req.offset)).unwrap_or(0);
        let newer_n = k.saturating_sub(1);
        let mut newer: Vec<Value> = all
            .iter()
            .filter(|v| !from_end && id(v) > req.from)
            .cloned()
            .collect();
        let skip = newer.len().saturating_sub(newer_n);
        let newer = newer.split_off(skip);
        let older: Vec<Value> = all
            .iter()
            .filter(|v| from_end || id(v) < req.from || (k > 0 && id(v) == req.from))
            .cloned()
            .collect();
        // 停住：只回起点那一条（有的话），一条新的都不给——内存里只有它、更早的还在后台拉
        let stalled = {
            let mut st = lock(&self.stall);
            if st.0 > 0 {
                st.0 -= 1;
                false
            } else {
                let s = st.1 > 0;
                st.1 = st.1.saturating_sub(1);
                s
            }
        };
        // 从最新一条起时「起点」就是最新那条
        let at_from = || {
            if from_end {
                all.first().cloned()
            } else {
                all.iter().find(|v| id(v) == req.from).cloned()
            }
        };
        if stalled {
            return Ok(at_from().iter().map(crate::tdjson::message).collect());
        }
        // TDLib 首次常只回 1 条（内存里只有挨着起点的那一条）：TDLib 给的总是连着起点的一段，所以是离起点最近的那条
        let short = lock(&self.short_first)
            .as_mut()
            .is_some_and(|seen| seen.insert(req.chat));
        if short {
            let one = older.first().cloned().or_else(|| newer.last().cloned());
            return Ok(one.iter().map(crate::tdjson::message).collect());
        }
        // 限了条数：留离起点最近的那几条（往新时是起点和紧挨着它的新的，往旧时是紧挨着它的旧的）
        let (newer, older) = match *lock(&self.cap) {
            Some(c) if k > 0 => {
                let keep_newer = c.saturating_sub(1).min(newer.len());
                let newer = newer[newer.len() - keep_newer..].to_vec();
                let older = older.into_iter().take(c - keep_newer).collect();
                (newer, older)
            }
            Some(c) => (newer, older.into_iter().take(c).collect()),
            None => (newer, older),
        };
        let mut out: Vec<Value> = newer.into_iter().chain(older).collect();
        out.truncate(usize::try_from(req.limit).unwrap_or(usize::MAX));
        Ok(out.iter().map(crate::tdjson::message).collect())
    }

    async fn message_get(&self, id: MsgId) -> Result<Message, CoreError> {
        self.started()?;
        lock(&self.calls).push(Call::MessageGet(id));
        self.ready_or_401("message_get").await?;
        lock(&self.histories)
            .get(&id.chat_id)
            .and_then(|ms| {
                ms.iter()
                    .find(|v| v["id"].as_i64() == Some(id.message_id))
                    .map(crate::tdjson::message)
            })
            .ok_or(CoreError::NotFound {
                what: "message",
                td: None,
            })
    }

    fn wait_net(&self, timeout: Duration, pred: &dyn Fn(&NetState) -> bool) -> Option<NetState> {
        self.shared.wait_net(timeout, pred)
    }

    fn shutdown(&self, timeout: Duration) -> Result<(), CoreError> {
        lock(&self.calls).push(Call::Shutdown);
        if lock(&self.run).is_none() {
            return Ok(());
        }
        if self.shared.is_faulted() {
            return Err(CoreError::Disconnected);
        }
        // 已经自己关了就不再发 close
        if !self.shared.is_closed() {
            self.shared.set_core(CoreState::Closing);
            if !*lock(&self.close_hangs) {
                self.push_auth("authorizationStateClosing");
                self.push_auth("authorizationStateClosed");
            }
        }
        if self.shared.wait_closed(timeout) {
            // 正常关闭：拿掉标记、放掉档案锁（同一进程里可以再开这个档案）
            if let Some((p, l)) = lock(&self.run).as_mut() {
                p.mark_clean();
                *l = None;
            }
            Ok(())
        } else if self.shared.is_faulted() {
            Err(CoreError::Disconnected)
        } else {
            Err(CoreError::Internal {
                what: "close_timeout",
            })
        }
    }
}

impl FakeBackend {
    /// 读消息这组的公共部分：注入的错误 → 内核故障 → 没登录回 401
    async fn ready_or_401(&self, action: &'static str) -> Result<(), CoreError> {
        let injected = self.take_failure(action);
        link::guard(self.shared.clone(), async move {
            match injected {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
        .await??;
        if self.step() != AuthStep::Ready {
            return Err(map_td_error(401, "Unauthorized"));
        }
        Ok(())
    }
}

/// 输入与假账号里的值一样（逐字节）
fn same(given: &Secret, want: &str) -> bool {
    given.expose() == want.as_bytes()
}
