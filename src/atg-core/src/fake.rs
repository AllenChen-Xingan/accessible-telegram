//! `fake`：假后端 [`FakeBackend`]（`specs/contracts/testing.md` 第一节第 2 小节）。
//!
//! 和真实后端用**同一个**收线程运行器（[`crate::link`]）：喂进来的是 TDLib JSON 形状的更新，走同一段
//! `classify` → 序号 → 三线合成 → 事件流。所以假后端测到的监督、序号、故障广播就是正式的那份。
//! 不碰 tdlib-rs 的全局收线程，测试可以并行跑多个。
//!
//! 能编排：`start` 的结果（成功 / 指定错误 / 一直不应答）、`shutdown` 卡住、收线程崩溃、来源断掉；每次调用记账（[`FakeBackend::calls`]）。场景目录（`fixtures/scenarios/`）与假时钟留给用到它的块（CLI `--fake`）。
//!
//! 登录（第 4 块）：[`FakeBackend::set_auth`] 给一个假账号 [`FakeAuth`]，`start` 后自己推到 `wait_phone`
//! （已登录的推到 `ready`）；每步提交按账号核对，对了就推下一步的授权状态（TDLib JSON 形状，带提示字段，
//! 走与真实后端同一段 [`AuthPrompt::from_td`]），错了回 TDLib 会回的那个错误（经 `map_td_error`）。
//! [`FakeBackend::fail_next`] 给某一步注入一次错误（或一直不应答）。记账只记动作名，**不记输入的值**。
//!
//! 会话（第 5 块）：[`FakeBackend::set_chats`] 给某个列表排好一串会话（每个会话是几条 TDLib JSON 更新）。
//! `chats_load(list, limit)` 每次推出前 `limit` 个会话的更新，推完了回「已到底」（TDLib 的 404）；没登录回 401。应答之前先等收线程把这批更新处理完（`Feed::Barrier`），与真实 TDLib「更新先于应答」一致。
//! [`FakeBackend::push_net`] 推一条连接状态。
//!
//! 消息（第 8 块）：[`FakeBackend::set_chat`] 给 `chat_get` 时要推的更新（`updateNewChat`、`updateUser`……），
//! [`FakeBackend::set_history`] 给某会话的消息（TDLib `message` 对象）。`history` 照 TDLib 的切法（`offset` 为 0
//! 不含 `from` 那条、往旧；负 `offset` 含它、另给比它新的），返回从新到旧；[`FakeBackend::stall_history`] 让接下来
//! 几次只回起点那一条；[`FakeBackend::short_first_history`] 让每个会话第一次只回 1 条（TDLib 首次常这样）。`message_get` 从同一份消息里找。
//!
//! 文件分工（#60 R7）：根上留共用状态（[`FakeBackend`] 的字段、收线程）、几个各处都用的内部助手（`started`、`injected`、
//! `ready_or_401`、`find_message`……）和 `Backend` 的实现；`Backend` 方法的主体放在子模块，根上一行转给它的 `do_*`。
//! `script.rs` 测试用的设置方法 · `login.rs` 登录 · `read.rs` 会话与历史 · `submit.rs` 交出口 · `edit.rs` 编辑 ·
//! `files.rs` 文件与语音 · `upload.rs` 上传进度 · `caps.rs` 消息能力 · `call.rs` 记账 · `scenario.rs` 场景夹具（读 `fixtures/scenarios/<名>/`、装进假后端，`atg-cli --fake` 与 `atg-gui --fixture` 共用）

use crate::auth::{self, AuthPrompt};
use crate::backend::{
    Backend, ChatsLoad, Downloaded, HistoryReq, StatusReport, Upload, already_subscribed,
};
use crate::config::{Profile, ProfileLock};
use crate::error::{CoreError, Holder, map_td_error};
use crate::gate::Gate;
use crate::link::{self, EventStream, Shared, Step};
use crate::lock;
use crate::model::{AddedReactions, FailedInfo, MessageCaps};
use crate::model::{ChatId, ChatList, ForumTopicState, Message, MsgId};
use crate::model::{ChatTarget, Me};
use crate::secret::Secret;
use crate::send::{Answer, Sealed};
use crate::status::{AuthStep, CoreState, NetState};
use crate::tree::{FileRef, ReactionKind};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

mod call;
mod caps;
mod edit;
mod files;
mod login;
mod read;
mod scenario;
mod script;
mod submit;
mod upload;

pub use call::Call;
pub use caps::Cap;
pub use login::FakeAuth;
pub use scenario::Scenario;

/// 一次真发送之后「TDLib」怎么推结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeSend {
    /// 推 `updateMessageSendSucceeded`（正式 id 由假后端编）
    Succeed,
    /// 推 `updateMessageSendFailed`
    Fail {
        code: i32,
        message: String,
        can_retry: bool,
    },
    /// 待发那条被删（`updateDeleteMessages`，不来失败）
    Delete,
    /// 什么都不推（等到超时）
    Silent,
    /// `sendMessage` 当场回错（还没造出待发消息）
    Reject(CoreError),
}

/// `start` 怎么回
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeStart {
    Ok,
    Fail(CoreError),
    /// 一直不应答（测故障时等待中的请求会不会返回）
    Hang,
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
    /// `file_download` 回什么：`None` = TDLib 回来了但没下完；没登记 = 找不到
    files: Mutex<HashMap<i32, Option<Downloaded>>>,
    uploads: Mutex<VecDeque<Upload>>, // `upload_progress` 按次回的进度（`fake/upload.rs`，#12）
    /// 每个会话第一次 `history` 只回 1 条
    short_first: Mutex<Option<std::collections::HashSet<ChatId>>>,
    /// 停住：（先照常应答几次，再连着几次只回起点那一条）——TDLib 正从服务器拉、一直没拉到的样子
    stall: Mutex<(u32, u32)>,
    /// 每次 `history` 最多回几条（离起点最近的那几条，与 TDLib 只给内存里连着起点那一段同形）
    cap: Mutex<Option<usize>>,
    /// 发送闸
    gate: Gate,
    /// 自己的用户 id（`me()`）；没设 → 401
    me_id: Mutex<Option<i64>>,
    forums: Mutex<Vec<ChatId>>,
    denied: Mutex<Vec<(MsgId, caps::Cap)>>, // 被关掉的能力，见 `fake/caps.rs`
    /// 接下来的几次预览是否「把回复去掉」（按次取，取完照常保留）
    preview_drops: Mutex<VecDeque<bool>>,
    /// 下一次预览时先跑它（测「凭据已验过、复核预览还没回」那一刻发生的事）
    preview_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// 接下来的几次真发送怎么推结果（取完默认成功）
    send_script: Mutex<VecDeque<FakeSend>>,
    next_id: Mutex<i64>,
    /// 真交出去的请求（测试核对交给「TDLib」的就是确认过的那份）
    submitted: Mutex<Vec<Value>>,
}

impl FakeBackend {
    /// 新建并立刻起收线程（测试可以在 `start` 之前就喂更新）
    pub fn new() -> Result<Self, CoreError> {
        Self::with_gate(Gate::new(Holder::Cli))
    }

    /// 带指定的发送闸（直发模式、假认证器）
    pub fn with_gate(gate: Gate) -> Result<Self, CoreError> {
        let (shared, stream) = Shared::new();
        shared.watch_send_limits(gate.limits());
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
            files: Mutex::new(HashMap::new()),
            uploads: Mutex::new(VecDeque::new()),
            short_first: Mutex::new(None),
            stall: Mutex::new((0, 0)),
            cap: Mutex::new(None),
            gate,
            me_id: Mutex::new(None),
            forums: Mutex::new(Vec::new()),
            denied: Mutex::new(Vec::new()),
            preview_drops: Mutex::new(VecDeque::new()),
            preview_hook: Mutex::new(None),
            send_script: Mutex::new(VecDeque::new()),
            next_id: Mutex::new(1_000_000),
            submitted: Mutex::new(Vec::new()),
        })
    }

    fn new_id(&self) -> i64 {
        let mut n = lock(&self.next_id);
        *n += 1;
        *n
    }

    /// 自己发的一条（TDLib `message` 对象形状），正文与回复、话题取自请求
    fn own_message(&self, chat: ChatId, id: i64, request: &Value, state: Value) -> Value {
        let me = lock(&self.me_id).unwrap_or(0);
        let reply = &request["reply_to"];
        let mut m = json!({
            "@type": "message", "id": id, "chat_id": chat,
            "sender_id": {"@type": "messageSenderUser", "user_id": me},
            "is_outgoing": true, "date": 1_792_755_420,
            "content": {"@type": "messageText", "text": request["input_message_content"]["text"].clone()},
            "topic_id": request["topic_id"].clone(),
            "sending_state": state,
        });
        if reply.is_object() {
            m["reply_to"] = json!({"@type": "messageReplyToMessage", "chat_id": chat,
                "message_id": reply["message_id"].clone()});
        }
        m
    }

    fn find_message(&self, id: MsgId) -> Option<Value> {
        lock(&self.histories).get(&id.chat_id).and_then(|ms| {
            ms.iter()
                .find(|m| m["id"].as_i64() == Some(id.message_id))
                .cloned()
        })
    }

    fn take_failure(&self, action: &'static str) -> Option<CoreError> {
        let mut f = lock(&self.fail_next);
        let i = f.iter().position(|(a, _)| *a == action)?;
        Some(f.remove(i).1)
    }

    /// 注入的错误（[`Self::HANG`] 则一直不应答）；内核故障时照样以 `disconnected` 返回
    async fn injected(&self, action: &'static str) -> Result<(), CoreError> {
        let injected = self.take_failure(action);
        link::guard(self.shared.clone(), async move {
            match injected {
                Some(CoreError::Internal { what: Self::HANG }) => std::future::pending().await,
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
        .await?
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
        // 假后端的「库钥匙」是写死的：只用来派生防重复指纹的密钥
        self.gate.set_server(
            profile.kind().use_test_dc(),
            profile.dir(),
            crate::gate::derive_guard_key(b"fake-db-key", profile.name()),
        );
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

    async fn chat_get(&self, chat: ChatId) -> Result<(), CoreError> {
        self.do_chat_get(chat).await
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

    async fn history(&self, req: HistoryReq) -> Result<Vec<Message>, CoreError> {
        self.do_history(req).await
    }

    async fn message_get(&self, id: MsgId) -> Result<Message, CoreError> {
        self.do_message_get(id).await
    }

    async fn file_download(&self, file: FileRef) -> Result<Downloaded, CoreError> {
        self.do_file_download(file).await
    }

    async fn mark_listened(&self, id: MsgId) -> Result<(), CoreError> {
        self.do_mark_listened(id).await
    }

    async fn upload_progress(&self, id: MsgId) -> Result<Option<Upload>, CoreError> {
        self.do_upload_progress(id).await
    }

    fn wait_net(&self, timeout: Duration, pred: &dyn Fn(&NetState) -> bool) -> Option<NetState> {
        self.shared.wait_net(timeout, pred)
    }

    fn gate(&self) -> &Gate {
        &self.gate
    }

    async fn me(&self) -> Result<Me, CoreError> {
        self.started()?;
        self.ready_or_401("me").await?;
        let id = lock(&self.me_id).ok_or(map_td_error(401, "Unauthorized"))?;
        let test_number = lock(&self.auth)
            .as_ref()
            .is_some_and(|a| auth::is_test_number(&a.phone));
        let me = Me {
            user_id: id,
            saved_chat_id: id,
            test_number,
        };
        self.gate.set_me(me);
        Ok(me)
    }

    async fn chat_target(&self, chat: ChatId) -> Result<ChatTarget, CoreError> {
        self.started()?;
        self.ready_or_401("chat_target").await?;
        let updates = lock(&self.chat_info).get(&chat).cloned();
        let c = updates
            .iter()
            .flatten()
            .find(|u| u["@type"] == "updateNewChat" && u["chat"]["id"].as_i64() == Some(chat))
            .map(|u| crate::tdjson::chat_object(&u["chat"]))
            .ok_or(CoreError::NotFound {
                what: "chat",
                td: None,
            })?;
        Ok(ChatTarget {
            id: c.id,
            kind: c.kind,
            title: c.title,
            is_forum: lock(&self.forums).contains(&chat),
            saved: false,
        })
    }

    async fn message_caps(&self, id: MsgId) -> Result<MessageCaps, CoreError> {
        self.started()?;
        self.ready_or_401("message_caps").await?;
        self.caps(id)
    }

    async fn failed_info(&self, id: MsgId) -> Result<Option<FailedInfo>, CoreError> {
        self.started()?;
        self.ready_or_401("failed_info").await?;
        let m = self.find_message(id).ok_or(CoreError::NotFound {
            what: "message",
            td: None,
        })?;
        Ok(crate::tdjson::failed_info(&m))
    }

    async fn submit(&self, req: Sealed<'_>) -> Result<Answer, CoreError> {
        self.do_submit(req).await
    }

    async fn added_reactions(
        &self,
        id: MsgId,
        kind: Option<&ReactionKind>,
        offset: &str,
        limit: u32,
    ) -> Result<AddedReactions, CoreError> {
        self.do_added_reactions(id, kind, offset, limit).await
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
            // 正常关闭：先收尾交出未完成标记（#88，放锁之前），再拿掉标记、放掉档案锁（同一进程里可以再开这个档案）
            if let Some((p, l)) = lock(&self.run).as_mut() {
                self.gate().limits().closed(timeout);
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
        self.injected(action).await?;
        if self.step() != AuthStep::Ready {
            return Err(map_td_error(401, "Unauthorized"));
        }
        Ok(())
    }
}
