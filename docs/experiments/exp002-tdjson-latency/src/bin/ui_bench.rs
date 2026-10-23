//! 界面段：假事件模拟「一条新消息到达」，量到 GPUI 重绘完、无障碍树里能查到这条消息。
//!
//!   ui_bench [--items N] [--samples N] [--interval-ms N]   分段计时（默认 50 项、300 个样本、间隔 120ms）
//!   ui_bench --cold [--items N]                             冷启动模式：打印一行 COLD 后退出
//!
//! 一条样本的时间点（同一进程 Instant）：
//!   t0  后台线程（扮演 TDLib 收线程）把消息文字塞进 testCallString 发给 TDLib
//!   tA  后台线程收到 TDLib 应答并解析完（真实的 tdjson 往返，离线方法）
//!   t1  主线程从通道里拿到消息，写进视图状态，cx.notify()
//!   t2  下一帧 render() 开始
//!   t3  这一帧画到最后一个元素（之后 GPUI 同步发无障碍树更新与通知：window.rs:3789-3813 → gpui_macos window.rs:2359-2369）
//!   t4  这一帧 draw + present 返回后（App::defer，在同一次主线程调度里）
//!   t5  进程内查无障碍树（NSAccessibility，等同读屏来问）找到这条消息的节点
//!
//! 无障碍树只在辅助技术来问过之后才建（window.rs:1679-1720）。本程序开窗后自己问一次
//! 内容视图的 accessibilityChildren，AccessKit 就激活了（accesskit_macos subclass.rs:92-99），
//! 不需要「辅助功能」权限。

#[path = "../common.rs"]
mod common;

use common::*;
use gpui::{
    App, Bounds, Context, Role, SharedString, Window, WindowBounds, WindowOptions, canvas, div,
    prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Args {
    cold: bool,
    items: usize,
    samples: usize,
    warmup: usize,
    interval_ms: u64,
}

fn parse_args() -> Args {
    let mut a = Args { cold: false, items: 50, samples: 300, warmup: 20, interval_ms: 120 };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        match v[i].as_str() {
            "--cold" => a.cold = true,
            "--items" => {
                i += 1;
                a.items = v[i].parse().unwrap()
            }
            "--samples" => {
                i += 1;
                a.samples = v[i].parse().unwrap()
            }
            "--interval-ms" => {
                i += 1;
                a.interval_ms = v[i].parse().unwrap()
            }
            other => panic!("unknown arg {other}"),
        }
        i += 1;
    }
    a
}

struct Incoming {
    seq: u64,
    text: SharedString,
    t0: Instant,
    t_a: Instant,
}

#[derive(Default, Clone)]
struct Sample {
    seq: u64,
    t0: Option<Instant>,
    t_a: Option<Instant>,
    t1: Option<Instant>,
    t2: Option<Instant>,
    t3: Option<Instant>,
    t4: Option<Instant>,
    t5: Option<Instant>,
    found: bool,
    ax_nodes: usize,
    window_active: bool,
}

struct Msg {
    seq: u64,
    text: SharedString,
}

struct Bench {
    args: Args,
    msgs: VecDeque<Msg>,
    pending: Option<Sample>,
    done: Vec<Sample>,
    not_found: usize,
    // 冷启动用
    cold: Rc<RefCell<ColdTimes>>,
    _tasks: Vec<gpui::Task<()>>,
}

#[derive(Default)]
struct ColdTimes {
    main: u128,
    window_opened: u128,
    first_render: u128,
    first_paint: u128,
    first_present: u128,
    ax_ready: u128,
    ax_polls: usize,
    td_ready: u128,
}

fn list_label(n: usize) -> String {
    format!("消息列表，{n} 条")
}

impl Bench {
    fn new(args: Args, cold: Rc<RefCell<ColdTimes>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let msgs = (0..args.items as u64)
            .map(|i| Msg { seq: i, text: format!("旧消息 {i}：{}", SAMPLE_TEXT.chars().take(10).collect::<String>()).into() })
            .collect();
        let mut tasks = vec![];
        let bg = cx.background_executor().clone();

        if args.cold {
            // 冷启动：开窗后立刻反复问无障碍树，直到能查到列表节点（等同读屏一启动就来问）
            let label = list_label(args.items);
            let cold2 = cold.clone();
            tasks.push(cx.spawn_in(window, async move |_this, cx| {
                loop {
                    let found = cx.update(|_, _| ax::find(&label)).unwrap();
                    cold2.borrow_mut().ax_polls += 1;
                    if found.found {
                        cold2.borrow_mut().ax_ready = epoch_ns();
                        break;
                    }
                    bg.timer(Duration::from_micros(500)).await;
                }
                // 等 TDLib 也就绪再退出
                loop {
                    if cold2.borrow().td_ready != 0 {
                        break;
                    }
                    bg.timer(Duration::from_millis(1)).await;
                }
                let c = cold2.borrow();
                println!(
                    "COLD ui_main={} window_opened={} first_render={} first_paint={} first_present={} ax_ready={} ax_polls={} td_ready={}",
                    c.main, c.window_opened, c.first_render, c.first_paint, c.first_present, c.ax_ready, c.ax_polls, c.td_ready
                );
                drop(c);
                let _ = cx.update(|_, cx| cx.quit());
            }));
        } else {
            // 分段计时：先激活无障碍树，等它真的激活，再放后台线程开始送消息
            let (tx, rx) = async_channel::unbounded::<Incoming>();
            let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
            let a = args.clone();
            std::thread::spawn(move || source_thread(a, tx, start_rx));

            tasks.push(cx.spawn_in(window, async move |this, cx| {
                loop {
                    let active = cx
                        .update(|window, _| {
                            ax::touch();
                            window.is_a11y_active()
                        })
                        .unwrap();
                    if active {
                        break;
                    }
                    bg.timer(Duration::from_millis(5)).await;
                }
                eprintln!("# a11y active, starting source");
                start_tx.send(()).unwrap();
                while let Ok(m) = rx.recv().await {
                    let t1 = Instant::now();
                    let ok = this.update_in(cx, |this, window, cx| {
                        this.msgs.push_back(Msg { seq: m.seq, text: m.text.clone() });
                        while this.msgs.len() > this.args.items {
                            this.msgs.pop_front();
                        }
                        this.pending = Some(Sample {
                            seq: m.seq,
                            t0: Some(m.t0),
                            t_a: Some(m.t_a),
                            t1: Some(t1),
                            window_active: window.is_window_active(),
                            ..Default::default()
                        });
                        cx.notify();
                    });
                    if ok.is_err() {
                        break;
                    }
                }
            }));
        }

        Self { args, msgs, pending: None, done: vec![], not_found: 0, cold, _tasks: tasks }
    }

    fn complete(&mut self, seq: u64, t3: Instant, t4: Instant, t5: Instant, found: bool, nodes: usize, cx: &mut Context<Self>) {
        let Some(p) = self.pending.as_mut() else { return };
        if p.seq != seq {
            return;
        }
        if !found {
            // 这一帧之后树里还没有它：记下来，不算样本（理论上不该发生）
            self.not_found += 1;
            self.pending = None;
            return;
        }
        p.t3 = Some(t3);
        p.t4 = Some(t4);
        p.t5 = Some(t5);
        p.found = found;
        p.ax_nodes = nodes;
        let s = self.pending.take().unwrap();
        self.done.push(s);
        if self.done.len() >= self.args.warmup + self.args.samples {
            self.finish(cx);
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let s: Vec<&Sample> = self.done.iter().skip(self.args.warmup).collect();
        let d = |f: &dyn Fn(&Sample) -> (Option<Instant>, Option<Instant>)| -> Vec<f64> {
            s.iter()
                .map(|x| {
                    let (a, b) = f(x);
                    ms(b.unwrap() - a.unwrap())
                })
                .collect()
        };
        println!("## UI segment · items={} samples={} interval={}ms", self.args.items, s.len(), self.args.interval_ms);
        println!("# window active in {}/{} samples; not-found-after-frame: {}; AX nodes walked (last): {}",
            s.iter().filter(|x| x.window_active).count(), s.len(), self.not_found, s.last().map(|x| x.ax_nodes).unwrap_or(0));
        report("t0→tA  TDLib round trip (testCallString)", &d(&|x| (x.t0, x.t_a)));
        report("tA→t1  bg thread → main thread", &d(&|x| (x.t_a, x.t1)));
        report("t1→t2  notify → next render starts", &d(&|x| (x.t1, x.t2)));
        report("t2→t3  render+layout+paint", &d(&|x| (x.t2, x.t3)));
        report("t3→t4  a11y update+notifs+present", &d(&|x| (x.t3, x.t4)));
        report("t4→t5  in-process AX query finds node", &d(&|x| (x.t4, x.t5)));
        report("t1→t4  state change → frame done (UI)", &d(&|x| (x.t1, x.t4)));
        report("t0→t4  TDLib send → tree updated+notified", &d(&|x| (x.t0, x.t4)));
        report("t0→t5  TDLib send → AX query sees it", &d(&|x| (x.t0, x.t5)));
        cx.quit();
    }
}

/// 扮演 TDLib 收线程：每隔 interval 把一条「新消息」走一趟真实的 tdjson 往返，再丢给主线程
fn source_thread(a: Args, tx: async_channel::Sender<Incoming>, start: std::sync::mpsc::Receiver<()>) {
    quiet_tdlib();
    let client = tdlib_rs::create_client();
    let _ = raw_roundtrip(client, r#"{"@type":"testCallEmpty"}"#, 1_000_000);
    start.recv().unwrap();
    let total = (a.warmup + a.samples) as u64 + 50; // 多送一些，防止个别样本被丢
    for seq in 0..total {
        std::thread::sleep(Duration::from_millis(a.interval_ms));
        let text = format!("新消息 #{seq}：{}", SAMPLE_TEXT.chars().take(20).collect::<String>());
        let t0 = Instant::now();
        let req = format!(r#"{{"@type":"testCallString","x":{}}}"#, serde_json::to_string(&text).unwrap());
        let (resp, _) = raw_roundtrip(client, &req, 2_000_000 + seq);
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        let back = v["value"].as_str().unwrap().to_string();
        let t_a = Instant::now();
        if tx.send_blocking(Incoming { seq: 1_000_000 + seq, text: back.into(), t0, t_a }).is_err() {
            break;
        }
    }
}

impl Render for Bench {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if self.args.cold {
            let mut c = self.cold.borrow_mut();
            if c.first_render == 0 {
                c.first_render = epoch_ns();
            }
        }
        if let Some(p) = self.pending.as_mut()
            && p.t2.is_none()
        {
            p.t2 = Some(now);
        }
        let pending = self.pending.as_ref().filter(|p| p.t3.is_none()).map(|p| p.seq);
        let pending_label = pending.and_then(|seq| {
            self.msgs.iter().find(|m| m.seq == seq).map(|m| m.text.to_string())
        });
        let n = self.msgs.len();
        let weak = cx.entity().downgrade();
        let cold = self.args.cold.then(|| self.cold.clone());

        div()
            .id("root")
            .role(Role::Application)
            .aria_label("exp002 latency bench")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .text_sm()
            .child(
                div()
                    .id("chat")
                    .role(Role::List)
                    .aria_label(SharedString::from(list_label(self.args.items)))
                    .flex()
                    .flex_col()
                    .children(self.msgs.iter().enumerate().map(|(i, m)| {
                        div()
                            .id(("msg", m.seq))
                            .role(Role::ListItem)
                            .aria_label(m.text.clone())
                            .aria_position_in_set(i + 1)
                            .aria_size_of_set(n)
                            .child(m.text.clone())
                    })),
            )
            // 最后一个元素：画到它时记 t3，然后用 defer 拿到「这一帧 draw+present 结束」的时刻
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, _, cx: &mut App| {
                        let t3 = Instant::now();
                        if let Some(cold) = cold {
                            let mut c = cold.borrow_mut();
                            if c.first_paint == 0 {
                                c.first_paint = epoch_ns();
                                drop(c);
                                cx.defer(move |_| {
                                    let mut c = cold.borrow_mut();
                                    if c.first_present == 0 {
                                        c.first_present = epoch_ns();
                                    }
                                });
                            }
                            return;
                        }
                        let (Some(seq), Some(label)) = (pending, pending_label) else { return };
                        cx.defer(move |cx| {
                            let t4 = Instant::now();
                            let f = ax::find(&label);
                            let t5 = Instant::now();
                            let _ = weak.update(cx, |this, cx| {
                                this.complete(seq, t3, t4, t5, f.found, f.nodes, cx)
                            });
                        });
                    },
                )
                .size(px(1.)),
            )
    }
}

fn main() {
    let t_main = epoch_ns();
    let args = parse_args();
    let cold = Rc::new(RefCell::new(ColdTimes { main: t_main, ..Default::default() }));

    if args.cold {
        // 真实应用会在启动时并行拉起 TDLib：放一个后台线程做「建客户端 + 第一次应答」
        let (tx, rx) = std::sync::mpsc::channel::<u128>();
        std::thread::spawn(move || {
            quiet_tdlib();
            let client = tdlib_rs::create_client();
            let _ = raw_roundtrip(client, r#"{"@type":"testCallEmpty"}"#, 1);
            let _ = tx.send(epoch_ns());
        });
        let cold2 = cold.clone();
        application().run(move |cx: &mut App| {
            open(cx, args.clone(), cold2.clone());
            // 主线程上轮询 TDLib 就绪的时刻
            let cold3 = cold2.clone();
            let bg = cx.background_executor().clone();
            cx.spawn(async move |_| {
                loop {
                    if let Ok(t) = rx.try_recv() {
                        cold3.borrow_mut().td_ready = t;
                        break;
                    }
                    bg.timer(Duration::from_millis(1)).await;
                }
            })
            .detach();
        });
    } else {
        application().run(move |cx: &mut App| open(cx, args.clone(), cold.clone()));
    }
}

fn open(cx: &mut App, args: Args, cold: Rc<RefCell<ColdTimes>>) {
    let height = (args.items as f32 * 20.0 + 40.0).clamp(300.0, 1000.0);
    let bounds = Bounds::centered(None, size(px(700.), px(height)), cx);
    let cold2 = cold.clone();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("exp002 latency bench".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| Bench::new(args, cold2, window, cx)),
    )
    .unwrap();
    cold.borrow_mut().window_opened = epoch_ns();
    cx.activate(true);
}
