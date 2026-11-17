//! `Hub` 与会话租约：同一会话几个订阅者，先走的不关，最后走的才关；别的使用者仍占着时也不关。
//! 后端那层（真调了几次 `chat_open` / `chat_close`、开失败退没退掉）用假后端断言
use super::*;
use crate::out::Sink;

/// 读方走没走由测试拨
struct Out(Arc<AtomicBool>);

impl LineOut for Out {
    fn line(&mut self, _: &str) -> bool {
        !self.0.load(Ordering::SeqCst)
    }
    fn gone(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// 日志丢掉
struct Quiet;

impl Sink for Quiet {
    fn out(&mut self, _: &str) {}
    fn err(&mut self, _: &str) {}
}

/// 起好的假后端，`PEER`、`GROUP` 两个会话都在
fn started(tag: &str) -> FakeBackend {
    let b = backend();
    b.set_chat(
        PEER,
        vec![user(PEER, "林小雨"), chat(PEER, private(), "林小雨")],
    );
    let group = json!({"@type": "chatTypeSupergroup", "supergroup_id": 1, "is_channel": false});
    b.set_chat(GROUP, vec![chat(GROUP, group, "读书会")]);
    block_on(b.start(&profile(tag))).unwrap();
    // 起好到登录态就绪是异步的：等到就绪再开会话，否则满载并行时会先拿到 401
    chats::ensure_ready(&b).unwrap_or_else(|_| panic!("假后端没就绪"));
    b
}

fn count(b: &FakeBackend, c: &Call) -> usize {
    b.calls().iter().filter(|x| *x == c).count()
}

/// 照 `serve` 与直跑的次序挂一个带 `--chat` 的订阅者：先挂上 → 经 `lease::open` 要租约、必要时开 → 记会话。
/// 回拨「读方走了」的开关
fn subscribe(b: &FakeBackend, hub: &mut Hub, chat: ChatId) -> Arc<AtomicBool> {
    let gone = Arc::new(AtomicBool::new(false));
    hub.add(Sub {
        out: Box::new(Out(gone.clone())),
        watcher: Watcher::new(cx(false), Format::Json, None, clock),
        chat: None,
        dead: false,
    });
    lease::open(b, hub.leases(), chat).unwrap();
    hub.open_chat(chat);
    gone
}

#[test]
fn two_subscribers_same_chat_open_once_close_after_last_leaves() {
    let b = started("hub-two");
    let mut hub = Hub::default();
    let x = subscribe(&b, &mut hub, PEER);
    let y = subscribe(&b, &mut hub, PEER);
    assert_eq!(count(&b, &Call::ChatOpen(PEER)), 1, "第二份不再开");
    x.store(true, Ordering::SeqCst);
    let close = hub.reap();
    assert!(close.is_empty(), "还有一个开着，不关");
    lease::close(&b, close, "t", &mut Quiet);
    assert_eq!(count(&b, &Call::ChatClose(PEER)), 0);
    y.store(true, Ordering::SeqCst);
    lease::close(&b, hub.reap(), "t", &mut Quiet);
    assert_eq!(count(&b, &Call::ChatClose(PEER)), 1);
    assert!(hub.is_empty());
}

#[test]
fn end_all_hands_back_each_chat_once() {
    let b = started("hub-end");
    let mut hub = Hub::default();
    subscribe(&b, &mut hub, PEER);
    subscribe(&b, &mut hub, PEER);
    subscribe(&b, &mut hub, GROUP);
    assert_eq!(hub.finish_all(), vec![GROUP, PEER]);
}

/// 别的使用者（将来的转写）也占着一份：订阅者走了、执行方收尾（正常或故障）都不关，等它放
#[test]
fn other_holder_keeps_chat_open() {
    let b = started("hub-other");
    let mut hub = Hub::default();
    let leases = hub.leases().clone();
    lease::open(&b, &leases, PEER).unwrap();
    let x = subscribe(&b, &mut hub, PEER);
    subscribe(&b, &mut hub, PEER);
    assert_eq!(count(&b, &Call::ChatOpen(PEER)), 1);
    x.store(true, Ordering::SeqCst);
    assert!(hub.reap().is_empty());
    // 还挂着一个订阅者时故障收尾：它那份放回，仍不关
    assert!(
        hub.fail_all(&Fail::Core(CoreError::Disconnected))
            .is_empty()
    );
    assert_eq!(leases.held(PEER), 1);
    assert!(leases.give(PEER), "最后一份由它放，它去关");
}

/// 读方断开与后端故障同时到：先摘掉、再判故障的循环里也不关（codex 026 第 1 轮 #1）
#[test]
fn no_close_after_backend_fault() {
    let b = started("hub-fault");
    let mut hub = Hub::default();
    let x = subscribe(&b, &mut hub, PEER);
    b.crash();
    b.join_receiver();
    x.store(true, Ordering::SeqCst);
    let close = hub.reap();
    assert_eq!(close, vec![PEER], "租约照放");
    lease::close(&b, close, "t", &mut Quiet);
    assert_eq!(count(&b, &Call::ChatClose(PEER)), 0);
}

/// 关到一半后端坏了：剩下的不再发（codex 026 第 2 轮 #1）。假后端在第一次 `chat_close` 里不会自己坏，
/// 这里用「进来就已故障」的两会话批量断言一个都不发，与「每关一个之前都看」同一条判据
#[test]
fn no_close_for_any_chat_in_batch_after_fault() {
    let b = started("hub-fault-batch");
    let mut hub = Hub::default();
    subscribe(&b, &mut hub, PEER);
    subscribe(&b, &mut hub, GROUP);
    b.crash();
    b.join_receiver();
    lease::close(&b, hub.finish_all(), "t", &mut Quiet);
    assert_eq!(
        count(&b, &Call::ChatClose(PEER)) + count(&b, &Call::ChatClose(GROUP)),
        0
    );
}

/// 开失败退掉那份：下一个要的仍是第一份、真去开（不会以为「已有人开着」而永远不开）
#[test]
fn failed_open_is_undone() {
    let b = started("hub-fail");
    let hub = Hub::default();
    b.fail_next("chat_open", CoreError::Network { td: None });
    assert!(lease::open(&b, hub.leases(), PEER).is_err());
    assert_eq!(hub.leases().held(PEER), 0);
    lease::open(&b, hub.leases(), PEER).unwrap();
    assert_eq!(count(&b, &Call::ChatOpen(PEER)), 2);
    assert_eq!(hub.leases().held(PEER), 1);
}
