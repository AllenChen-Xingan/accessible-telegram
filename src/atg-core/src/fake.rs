//! `fake`：假后端 [`FakeBackend`]（`specs/contracts/testing.md` 第一节第 2 小节）。
//!
//! 和真实后端用**同一个**收线程运行器（[`crate::link`]）：喂进来的是 TDLib JSON 形状的更新，走同一段
//! `classify` → 序号 → 三线合成 → 事件流。所以假后端测到的监督、序号、故障广播就是正式的那份。
//! 不碰 tdlib-rs 的全局收线程，测试可以并行跑多个。
//!
//! 能编排：`start` 的结果（成功 / 指定错误 / 一直不应答）、`shutdown` 卡住、收线程崩溃、来源断掉；
//! 每次调用记账（[`FakeBackend::calls`]）。场景目录（`fixtures/scenarios/`）与假时钟留给用到它的块（CLI `--fake`）。

use crate::backend::{Backend, StatusReport, already_subscribed};
use crate::config::{Profile, ProfileLock};
use crate::error::{CoreError, Holder};
use crate::link::{self, EventStream, Shared, Step};
use crate::status::CoreState;
use serde_json::{Value, json};
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
    Start { profile: String, use_test_dc: bool },
    Subscribe,
    Shutdown,
}

enum Feed {
    Update(Value),
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
        })
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
