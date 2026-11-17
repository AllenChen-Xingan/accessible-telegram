//! `watch` 的出口与订阅者：一行一行写出去的出口（`LineOut`）、把事件渲染成一行的 `Watcher`（套信封、加读法与提示）、
//! 同一份事件流分给多个订阅者的 `Hub`。每种事件的数据（`event_data`）在模块根 `watch.rs`。
//! 词：直跑 = `atg-cli watch` 直接写标准输出；执行方 = `serve` 那边，经本机接口的连接写给读方；租约 = 替订阅者开着的会话。

use super::*;

/// 一个订阅者的出口：直跑是标准输出，执行方那边是本机接口的连接
pub trait LineOut {
    /// 写一行；读方已经走了回 `false`
    fn line(&mut self, line: &str) -> bool;
    /// 读方是不是已经走了（不写也能看出来；看不出来回 `false`）
    fn gone(&self) -> bool {
        false
    }
    /// 收尾：执行方那边发 `exit` 帧；标准输出不用
    fn finish(&mut self, _code: u8) {}
}

/// 本进程的标准输出：写失败（管道断了）就记下，之后不再写
#[derive(Default)]
pub struct StdoutLines {
    broken: bool,
}

impl LineOut for StdoutLines {
    fn line(&mut self, line: &str) -> bool {
        use std::io::Write;
        if self.broken {
            return false;
        }
        let mut out = std::io::stdout().lock();
        if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
            self.broken = true;
        }
        !self.broken
    }

    fn gone(&self) -> bool {
        self.broken
    }
}

/// 一个订阅者的状态：策略、读法上下文、计数
pub struct Watcher {
    policy: Policy,
    cx: SpeechCtx,
    format: Format,
    clock: fn() -> i64,
    /// 这个订阅从什么时候起（结尾行的 `elapsed_ms`）
    started: Instant,
    events: u64,
    pub(super) alerts: u64,
}

impl Watcher {
    pub fn new(
        cx: SpeechCtx,
        format: Format,
        open_chat: Option<ChatId>,
        clock: fn() -> i64,
    ) -> Self {
        let mut policy = Policy::new(Config::default());
        policy.set_open_chat(open_chat);
        Self {
            policy,
            cx,
            format,
            clock,
            started: Instant::now(),
            events: 0,
            alerts: 0,
        }
    }

    /// 结尾时间从这一刻算（直跑：进程启动）
    pub fn since(mut self, started: Instant) -> Self {
        self.started = started;
        self
    }

    /// 中途接入：执行方早已补发完，接着的都算新消息
    pub fn mark_synced(&mut self) {
        self.policy.mark_synced((self.clock)());
    }

    /// 中途接入时执行方正在重连补发：补发结束时汇总这个订阅者接入后看到的
    pub fn mark_reconnecting(&mut self) {
        self.policy.mark_reconnecting();
    }

    /// 一条事件 → 要写出的行（JSON 一行；文字模式只写要给人看的那几种，可能没有）
    pub fn render(&mut self, store: &Store, seq: u64, event: &CoreEvent) -> Option<String> {
        let line = self.render_inner(store, seq, event)?;
        // 只数真写出去的事件行（文字模式没出的不算），转发那头按同一口径数（`forward::count_line`）
        self.events += 1;
        if line.1 {
            self.alerts += 1;
        }
        Some(line.0)
    }

    /// （这一行，要不要提示）
    fn render_inner(
        &mut self,
        store: &Store,
        seq: u64,
        event: &CoreEvent,
    ) -> Option<(String, bool)> {
        let judged = self.policy.observe(store, event, (self.clock)());
        let verdict = judged.as_ref().map(|j| match j {
            Judged::Message { verdict, .. }
            | Judged::Typing { verdict, .. }
            | Judged::Summary { verdict, .. } => *verdict,
        });
        // 重连补发结束的汇总（人定 2026-10-26）：断网期间本该提示的条数、会话数
        let missed = match &judged {
            Some(Judged::Summary {
                messages, chats, ..
            }) => Some((*messages, *chats)),
            _ => None,
        };
        let alert = verdict.is_some_and(|v| v.alert);
        let (mut data, doc) = event_data(store, event);
        let brief = match (&doc, event) {
            (Some(d), _) => Some(atg_core::speech::brief(d, &self.cx)),
            (None, CoreEvent::ChatAction { .. }) if verdict.is_some_and(|v| v.alert) => {
                Some(TYPING_ZH.to_owned())
            }
            (None, CoreEvent::SyncDone) => missed.map(|(n, m)| missed_zh(n, m)),
            _ => None,
        };
        match self.format {
            Format::Json => {
                if let (Some(d), Some(o)) = (&doc, data.as_object_mut()) {
                    // 与 history 同一段：没展开时 data 里的剧透原文遮掉（`json::value` 里做）
                    let v = atg_core::json::value(d, &self.cx);
                    o.insert("message".into(), v["data"].clone());
                }
                if let (Some(v), Some(o)) = (verdict, data.as_object_mut()) {
                    o.insert("notify".into(), json!(v));
                }
                if let (Some((n, m)), Some(o)) = (missed, data.as_object_mut()) {
                    o.insert("missed".into(), json!({ "messages": n, "chats": m }));
                }
                let mut line = envelope("event", data);
                if let Some(o) = line.as_object_mut() {
                    o.insert("seq".into(), json!(seq));
                    if let Some(b) = &brief {
                        o.insert("speech".into(), json!({ "brief": b }));
                    }
                }
                Some((line.to_string(), alert))
            }
            // 文字模式：每条新消息一句读法（要提示的前面加「新消息，」）；正在输入播的那次一句；补发结束一句
            Format::Text => match event {
                CoreEvent::SyncDone => Some(match brief {
                    // 重连汇总：出声的那一行，后面接补发结束那句
                    Some(b) => (format!("{b}。{SYNC_DONE_ZH}"), true),
                    None => (SYNC_DONE_ZH.to_owned(), false),
                }),
                CoreEvent::ChatAction { .. } => brief.map(|b| (b, alert)),
                _ => match (verdict, brief) {
                    (Some(v), Some(b)) if v.alert => Some((format!("{NEW_PREFIX_ZH}{b}"), true)),
                    (Some(_), Some(b)) => Some((b, false)),
                    _ => None,
                },
            },
        }
    }

    pub fn result_value(&self) -> Value {
        envelope(
            "result",
            json!({
                "command": "watch",
                "reason": "interrupted",
                "events": self.events,
                "alerts": self.alerts,
                "elapsed_ms": crate::elapsed_ms(self.started),
            }),
        )
    }

    /// 出错收尾的那一行（按这个订阅者自己的格式），连同退出码
    pub fn error_line(&self, f: &Fail) -> (String, u8) {
        let mut s = Collect::default();
        let code = crate::out::fail_with_to(&mut s, self.format, f);
        (s.0.unwrap_or_default(), code)
    }

    /// 结尾行（文字模式一句统计）
    pub fn result_line(&self) -> String {
        match self.format {
            Format::Json => self.result_value().to_string(),
            Format::Text => format!(
                "已停止；写出了 {} 条事件，其中 {} 条要提示",
                self.events, self.alerts
            ),
        }
    }
}

/// 一个挂着的 `watch`
pub struct Sub {
    pub out: Box<dyn LineOut>,
    pub watcher: Watcher,
    /// 替它打开的会话（关时配对 `chat_close`）
    pub chat: Option<ChatId>,
    /// 读方走了
    pub dead: bool,
}

/// 所有挂着的 `watch`。空的时候 [`Feed`] 按原来的成批方式喂，不多花一次拷贝
#[derive(Default)]
pub struct Hub {
    subs: Vec<Sub>,
    /// 替谁开着会话（订阅者各占一份；以后转写也从这里要）
    leases: Arc<Leases>,
}

impl Hub {
    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.subs.len()
    }

    pub fn add(&mut self, sub: Sub) {
        self.subs.push(sub);
    }

    /// 各订阅者判到时还没设置的论坛话题（去重）：调用方去取，应答喂 store
    pub fn take_missing_topics(&mut self) -> Vec<(ChatId, i64)> {
        let mut v: Vec<(ChatId, i64)> = self
            .subs
            .iter_mut()
            .flat_map(|s| s.watcher.policy.take_missing_topics())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// 会话租约（`--chat` 经 [`lease::open`] 要一份）。读方走了先标 `dead`，到摘掉（reap / 收尾）时才放回
    pub fn leases(&self) -> &Arc<Leases> {
        &self.leases
    }

    pub fn on_event(&mut self, store: &Store, seq: u64, event: &CoreEvent) {
        for s in &mut self.subs {
            if s.dead {
                continue;
            }
            if let Some(line) = s.watcher.render(store, seq, event)
                && !s.out.line(&line)
            {
                s.dead = true;
            }
        }
    }

    /// 摘掉读方已走的；回它们替开的会话里**已没人再开着**的那些（调用方去 `chat_close`）
    pub fn reap(&mut self) -> Vec<ChatId> {
        for s in &mut self.subs {
            if s.out.gone() {
                s.dead = true;
            }
        }
        let (gone, live): (Vec<Sub>, Vec<Sub>) = self.subs.drain(..).partition(|s| s.dead);
        self.subs = live;
        self.give_back(gone)
    }

    /// 全部正常收尾：各写结尾行 `result` + 退出码 0，交出它们替开过的会话
    pub fn finish_all(&mut self) -> Vec<ChatId> {
        self.end_all(|w| (w.result_line(), 0))
    }

    /// 全部出错收尾：各按自己的格式写同一个错误，交出替开过的会话
    pub fn fail_all(&mut self, f: &Fail) -> Vec<ChatId> {
        self.end_all(|w| w.error_line(f))
    }

    fn end_all(&mut self, end: impl Fn(&Watcher) -> (String, u8)) -> Vec<ChatId> {
        let mut subs = std::mem::take(&mut self.subs);
        for s in subs.iter_mut().filter(|s| !s.dead) {
            let (line, code) = end(&s.watcher);
            let _ = s.out.line(&line);
            s.out.finish(code);
        }
        self.give_back(subs)
    }

    /// 摘掉的订阅者各放回它那份租约；回放完已没人开着的会话（调用方去 `chat_close`）
    fn give_back(&self, gone: Vec<Sub>) -> Vec<ChatId> {
        let mut close: Vec<ChatId> = gone
            .into_iter()
            .filter_map(|s| s.chat)
            .filter(|c| self.leases.give(*c))
            .collect();
        close.sort_unstable();
        close
    }

    /// 只给最后挂上的那个写错误、摘掉它（`watch` 接入途中出错，如 `--chat` 找不到）；回退出码。
    /// 只在 [`Hub::open_chat`] 之前调（还没占租约，开失败的已退掉）：不用放；`super::fail_unless_stopped`（模块根 `watch.rs`）同理
    pub fn fail_last(&mut self, f: &Fail) -> u8 {
        let Some(mut s) = self.subs.pop() else {
            return 1;
        };
        debug_assert!(s.chat.is_none(), "fail_last 摘了已占租约的订阅者");
        let (line, code) = s.watcher.error_line(f);
        let _ = s.out.line(&line);
        s.out.finish(code);
        code
    }

    /// 最后挂上的那个订阅者打开了这个会话（它也是正在输入要看的「当前会话」）。调用方已替它要过一份租约，摘掉时放回
    pub fn open_chat(&mut self, chat: ChatId) {
        if let Some(s) = self.subs.last_mut() {
            s.chat = Some(chat);
            s.watcher.policy.set_open_chat(Some(chat));
        }
    }
}
