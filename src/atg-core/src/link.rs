//! `link`：收线程与它的监督——真实后端（`td`）与假后端（[`crate::fake`]）共用这一份。
//!
//! 设计（`docs/plan/001_arch-stack.md` 第一节第 3、4 小节）：
//! - **收线程**循环取「来源」（真实后端是 tdlib-rs 的 `receive()`，假后端是测试喂的队列）。每条更新先过
//!   [`tdjson::classify`]：授权、连接两条线由这里维护并合成 [`UserStatus`]，其余变成 [`CoreEvent`]
//! - 事件带递增 `seq` 进**无界**通道（有界的话收线程一堵，请求的应答也跟着卡住）。序号与发送都在同一把锁里，
//!   所以通道里的序号严格递增，与谁（收线程 / 起停流程 / 故障）产出的无关
//! - **监督**：收线程外面套 `catch_unwind`。线程崩溃、或没等到 `authorizationStateClosed` 就退出，都广播「故障」：
//!   内核线变 `faulted`、等待中的请求以 `disconnected` 返回（[`guard`]）、之后的新请求不再发出直接拒绝
//! - 消费方（CLI 循环 / GUI 前台任务）从 [`EventStream`] 成批取，交给 [`crate::pump::Pump`] 跑 reducer
//!
//! 不引异步运行时：请求的应答由 tdlib-rs 的一次性信道唤醒，任何执行器都行；这里自带一个极小的 [`block_on`]。

use crate::auth::AuthPrompt;
use crate::error::CoreError;
use crate::logline::{self, Field};
use crate::model::CoreEvent;
use crate::status::{AuthStep, CoreState, Lines, NetState};
use crate::store::Input;
use crate::tdjson::{self, Classified};
use crate::tree::UserStatus;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 通道里的一件事：序号 + 事件
pub struct Envelope {
    pub seq: u64,
    pub event: CoreEvent,
}

impl Envelope {
    pub fn into_input(self) -> Input {
        Input::Update {
            seq: self.seq,
            event: self.event,
        }
    }
}

/// 来源每次给出的东西
pub enum Step {
    /// 这一轮没有东西（超时），接着取
    Idle,
    /// 一条更新，TDLib JSON 形状
    Update(Value),
    /// 来源没了（不该发生：正常退出只认 `authorizationStateClosed`）
    End,
}

struct State {
    lines: Lines,
    status: UserStatus,
    /// 授权线当前这一步的提示（给人看的；`auth_prompt()` 交出去）
    prompt: AuthPrompt,
    /// 收到过几条授权状态更新。同一步重新下发（换号、重发验证码后又是 `wait_code`）步骤名不变，靠它看出「有新的了」
    auth_gen: u64,
    seq: u64,
    /// 故障或关闭后丢掉，消费方取完剩下的就看到流结束
    tx: Option<UnboundedSender<Envelope>>,
}

/// 后端与收线程共享的状态：三条线、序号、通道发送端、故障信号
pub struct Shared {
    state: Mutex<State>,
    cond: Condvar,
    faulted: AtomicBool,
    /// 收到 `authorizationStateClosed`、收线程已退出：之后不会再有应答
    closed: AtomicBool,
    wakers: Mutex<HashMap<u64, Waker>>,
    next_waiter: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    /// 新建共享状态与它唯一的事件流（`subscribe()` 交出去，一个后端只许一个订阅者）
    pub fn new() -> (Arc<Self>, EventStream) {
        let (tx, rx) = unbounded();
        let lines = Lines::starting();
        let status = lines.user_status();
        let shared = Arc::new(Self {
            state: Mutex::new(State {
                lines,
                status,
                prompt: AuthPrompt::bare(AuthStep::WaitParameters),
                auth_gen: 0,
                seq: 0,
                tx: Some(tx),
            }),
            cond: Condvar::new(),
            faulted: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            wakers: Mutex::new(HashMap::new()),
            next_waiter: AtomicU64::new(0),
        });
        (shared, EventStream { rx })
    }

    fn emit(st: &mut State, event: CoreEvent) {
        st.seq += 1;
        if let Some(tx) = &st.tx {
            // 订阅者丢掉了流：事件没人要，照常编号
            let _ = tx.unbounded_send(Envelope { seq: st.seq, event });
        }
    }

    /// 推一个事件（序号在锁里编）
    pub fn event(&self, event: CoreEvent) {
        Self::emit(&mut lock(&self.state), event);
    }

    /// 改三条线；变了的线各推一个事件，合成状态变了再推 `UserStatusChanged`。
    /// 内核已 `faulted` / `closed` 后不再回头
    pub fn set_lines(&self, f: impl FnOnce(&mut Lines)) {
        let mut st = lock(&self.state);
        let old = st.lines.clone();
        f(&mut st.lines);
        if matches!(old.core, CoreState::Faulted | CoreState::Closed) {
            st.lines.core = old.core;
        }
        let new = st.lines.clone();
        if new.core != old.core {
            Self::emit(&mut st, CoreEvent::CoreStateChanged(new.core));
        }
        if new.auth != old.auth {
            Self::emit(&mut st, CoreEvent::AuthStepChanged(new.auth));
        }
        if new.net != old.net {
            Self::emit(&mut st, CoreEvent::NetStateChanged(new.net.clone()));
        }
        let status = new.user_status();
        if status != st.status {
            st.status = status;
            Self::emit(&mut st, CoreEvent::UserStatusChanged(status));
        }
        drop(st);
        self.cond.notify_all();
    }

    pub fn set_core(&self, core: CoreState) {
        self.set_lines(|l| l.core = core);
    }

    pub fn lines(&self) -> Lines {
        lock(&self.state).lines.clone()
    }

    pub fn status(&self) -> UserStatus {
        lock(&self.state).status
    }

    /// 收到一条授权状态更新：先换提示、再改授权线（等步骤变化的一方醒来时提示已是新的）
    pub fn set_auth(&self, prompt: AuthPrompt) {
        let step = prompt.step;
        {
            let mut st = lock(&self.state);
            st.prompt = prompt;
            st.auth_gen += 1;
        }
        // set_lines 末尾总会唤醒等待方，步骤名没变也一样
        self.set_lines(|l| l.auth = step);
    }

    /// 到现在为止收到过几条授权状态更新（提交之前记下，之后用 [`Shared::wait_auth_after`] 等新的）
    pub fn auth_gen(&self) -> u64 {
        lock(&self.state).auth_gen
    }

    /// 等到第 `seen` 条之后又来了一条授权状态更新（或故障 / 超时 → `None`），返回那时的步骤
    pub fn wait_auth_after(&self, timeout: Duration, seen: u64) -> Option<AuthStep> {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.state);
        loop {
            if st.auth_gen > seen {
                return Some(st.lines.auth);
            }
            let now = Instant::now();
            if st.lines.core == CoreState::Faulted || now >= deadline {
                return None;
            }
            st = self
                .cond
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// 当前步骤 + 提示。提示与授权线对不上（如内核故障后）就只给步骤
    pub fn auth_prompt(&self) -> AuthPrompt {
        let st = lock(&self.state);
        if st.prompt.step == st.lines.auth {
            st.prompt.clone()
        } else {
            AuthPrompt::bare(st.lines.auth)
        }
    }

    pub fn is_faulted(&self) -> bool {
        self.faulted.load(Ordering::SeqCst)
    }

    /// 广播故障：内核线 `faulted`、唤醒所有等待中的请求、关掉事件流。已正常关闭的不算故障
    pub fn fault(&self, why: &'static str) {
        {
            let st = lock(&self.state);
            if st.lines.core == CoreState::Closed {
                return;
            }
        }
        logline::warn("core_fault", &[Field::Name("why", why)]);
        self.faulted.store(true, Ordering::SeqCst);
        self.set_core(CoreState::Faulted);
        lock(&self.state).tx = None;
        self.wake_all();
    }

    /// 内核已关（收到 `authorizationStateClosed`）：再发的请求不会有应答
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn wake_all(&self) {
        let wakers: Vec<Waker> = lock(&self.wakers).drain().map(|(_, w)| w).collect();
        for w in wakers {
            w.wake();
        }
        self.cond.notify_all();
    }

    /// 收到 `authorizationStateClosed`：内核线 `closed`，关掉事件流
    fn finish_closed(&self) {
        self.set_core(CoreState::Closed);
        lock(&self.state).tx = None;
        self.closed.store(true, Ordering::SeqCst);
        self.wake_all();
    }

    /// 等到授权线变 `closed`（或内核故障 / 关闭），最多等 `timeout`。返回是否等到了 `closed`
    pub fn wait_closed(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.state);
        loop {
            if st.lines.auth == AuthStep::Closed || st.lines.core == CoreState::Closed {
                return true;
            }
            if st.lines.core == CoreState::Faulted {
                return false;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            st = self
                .cond
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// 等到连接线满足 `pred`（或故障），最多等 `timeout`。列会话时等「补发完了」用
    pub fn wait_net(
        &self,
        timeout: Duration,
        pred: impl Fn(&NetState) -> bool,
    ) -> Option<NetState> {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.state);
        loop {
            if pred(&st.lines.net) {
                return Some(st.lines.net.clone());
            }
            let now = Instant::now();
            if st.lines.core == CoreState::Faulted || now >= deadline {
                return None;
            }
            st = self
                .cond
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// 等到授权线满足 `pred`（或故障），最多等 `timeout`。冒烟测试与登录流程用
    pub fn wait_auth(
        &self,
        timeout: Duration,
        pred: impl Fn(AuthStep) -> bool,
    ) -> Option<AuthStep> {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.state);
        loop {
            if pred(st.lines.auth) {
                return Some(st.lines.auth);
            }
            let now = Instant::now();
            if st.lines.core == CoreState::Faulted || now >= deadline {
                return None;
            }
            st = self
                .cond
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

// ───────────────────────── 请求：应答与故障，谁先到算谁 ─────────────────────────

/// 把一个请求包起来：故障先到就以 `disconnected` 返回。**故障之后的新请求根本不会被轮询**，
/// 所以不会发出去（tdlib-rs 的请求在第一次轮询时才交给 TDLib）。
/// 内核已关（收线程正常退出）后同样以 `disconnected` 返回——应答只能经收线程分发，再等就是永远；
/// 但先轮询一次，关之前已经到了的应答照收
pub fn guard<F: Future>(shared: Arc<Shared>, fut: F) -> Guarded<F> {
    let id = shared.next_waiter.fetch_add(1, Ordering::Relaxed);
    Guarded {
        shared,
        id,
        fut: Box::pin(fut),
    }
}

pub struct Guarded<F> {
    shared: Arc<Shared>,
    id: u64,
    fut: Pin<Box<F>>,
}

impl<F: Future> Future for Guarded<F> {
    type Output = Result<F::Output, CoreError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.shared.is_faulted() {
            return Poll::Ready(Err(CoreError::Disconnected));
        }
        lock(&this.shared.wakers).insert(this.id, cx.waker().clone());
        // 登记之后再查一次：故障若恰好发生在两步之间，上面那次没看到、广播时还没登记
        if this.shared.is_faulted() {
            return Poll::Ready(Err(CoreError::Disconnected));
        }
        match this.fut.as_mut().poll(cx) {
            Poll::Ready(v) => Poll::Ready(Ok(v)),
            Poll::Pending if this.shared.is_closed() => Poll::Ready(Err(CoreError::Disconnected)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<F> Drop for Guarded<F> {
    fn drop(&mut self) {
        lock(&self.shared.wakers).remove(&self.id);
    }
}

// ───────────────────────── 收线程 ─────────────────────────

/// 起收线程。`source` 每次调用给一步；收到 `authorizationStateClosed` 正常退出，其余退出一律算故障
pub fn spawn_receiver<S>(shared: Arc<Shared>, mut source: S) -> Result<JoinHandle<()>, CoreError>
where
    S: FnMut() -> Step + Send + 'static,
{
    thread::Builder::new()
        .name("atg-td-recv".into())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| run(&shared, &mut source)));
            match outcome {
                Ok(true) => shared.finish_closed(),
                Ok(false) => shared.fault("receiver_ended"),
                Err(_) => shared.fault("receiver_panicked"),
            }
        })
        .map_err(|_| CoreError::Internal {
            what: "spawn_receiver",
        })
}

/// 收线程本体。返回 true = 正常关闭
fn run(shared: &Shared, source: &mut dyn FnMut() -> Step) -> bool {
    loop {
        match source() {
            Step::Idle => {}
            Step::End => return false,
            Step::Update(v) => match tdjson::classify(&v) {
                Classified::Auth(step) => {
                    let prompt = AuthPrompt::from_td(&v["authorization_state"]);
                    shared.set_auth(prompt);
                    if step == AuthStep::Closed {
                        return true;
                    }
                }
                Classified::Net(net) => shared.set_lines(|l| l.net = net),
                Classified::Event(e) => shared.event(e),
            },
        }
    }
}

// ───────────────────────── 事件流 ─────────────────────────

/// `subscribe()` 交出的事件流。故障或关闭后，取完剩下的就结束（`None`）
pub struct EventStream {
    rx: UnboundedReceiver<Envelope>,
}

impl EventStream {
    /// 等到至少一件，再把已积压的最多取到 `max` 件（GUI 每批有上限，处理完让出主线程）
    pub async fn next_batch(&mut self, max: usize) -> Option<Vec<Envelope>> {
        let first = self.rx.recv().await.ok()?;
        let mut batch = vec![first];
        batch.extend(self.try_batch(max.max(1) - 1));
        Some(batch)
    }

    /// 不等：把已积压的最多取 `max` 件
    pub fn try_batch(&mut self, max: usize) -> Vec<Envelope> {
        let mut batch = Vec::new();
        while batch.len() < max {
            match self.rx.try_recv() {
                Ok(e) => batch.push(e),
                Err(_) => break,
            }
        }
        batch
    }

    /// 阻塞版 [`EventStream::next_batch`]（CLI 循环用）
    pub fn blocking_batch(&mut self, max: usize) -> Option<Vec<Envelope>> {
        block_on(self.next_batch(max))
    }
}

// ───────────────────────── 极小的执行器 ─────────────────────────

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// 在当前线程上跑完一个 future（CLI 与测试用；GUI 放在自己的前台执行器上）
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        thread::park();
    }
}

/// 同 [`block_on`]，但最多等 `timeout`；超时返回 `None`（future 被丢掉）。退出路径用，免得卡死
pub fn block_on_timeout<F: Future>(fut: F, timeout: Duration) -> Option<F::Output> {
    let deadline = Instant::now() + timeout;
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return Some(v);
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        thread::park_timeout(deadline - now);
    }
}
