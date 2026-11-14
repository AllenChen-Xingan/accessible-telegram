//! `Hub` 与会话租约：同一会话几个订阅者，先走的不关，最后走的才关；别的使用者仍占着时也不关
use super::*;

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

/// 挂一个替 `chat` 开着会话的订阅者（照 `serve` 的次序：要租约 → 挂上 → 记会话）；回拨「读方走了」的开关
fn subscribe(hub: &mut Hub, chat: ChatId) -> Arc<AtomicBool> {
    let gone = Arc::new(AtomicBool::new(false));
    hub.leases().take(chat);
    hub.add(Sub {
        out: Box::new(Out(gone.clone())),
        watcher: Watcher::new(cx(false), Format::Json, None, clock),
        chat: Some(chat),
        dead: false,
    });
    hub.open_chat(chat);
    gone
}

#[test]
fn two_subscribers_same_chat_close_after_last_leaves() {
    let mut hub = Hub::default();
    let a = subscribe(&mut hub, PEER);
    let b = subscribe(&mut hub, PEER);
    assert_eq!(hub.leases().held(PEER), 2);
    a.store(true, Ordering::SeqCst);
    assert_eq!(hub.reap(), Vec::<ChatId>::new(), "还有一个开着，不关");
    assert_eq!(hub.len(), 1);
    b.store(true, Ordering::SeqCst);
    assert_eq!(hub.reap(), vec![PEER]);
    assert!(hub.is_empty());
    assert_eq!(hub.leases().held(PEER), 0);
}

#[test]
fn end_all_hands_back_each_chat_once() {
    let mut hub = Hub::default();
    subscribe(&mut hub, PEER);
    subscribe(&mut hub, PEER);
    subscribe(&mut hub, GROUP);
    let mut close = hub.finish_all();
    close.sort_unstable();
    assert_eq!(close, vec![GROUP, PEER]);
}

/// 别的使用者（将来的转写）也占着一份：订阅者全走了也不关，等它放
#[test]
fn other_holder_keeps_chat_open() {
    let mut hub = Hub::default();
    let leases = hub.leases().clone();
    assert!(leases.take(PEER));
    let a = subscribe(&mut hub, PEER);
    a.store(true, Ordering::SeqCst);
    assert_eq!(hub.reap(), Vec::<ChatId>::new());
    assert_eq!(
        hub.fail_all(&Fail::Core(CoreError::Disconnected)),
        Vec::<ChatId>::new()
    );
    assert!(leases.give(PEER), "最后一份由它放，它去关");
}
