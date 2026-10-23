//! exp001-announce：GPUI + AccessKit 的 Live 播报在 macOS 上到底发不发、发什么。
//!
//! 两种跑法：
//! - 手动：`cargo run`，开着 VoiceOver 按键试听（见 人工试用.md）
//! - 自动：`cargo run -- --auto`，程序自己激活无障碍、按脚本走一遍，
//!   截获本进程发给 macOS 的无障碍通知，打印结果表后退出。
//!
//! 截获原理：本文件导出与 AppKit 同名的两个 C 函数
//! `NSAccessibilityPostNotification` / `NSAccessibilityPostNotificationWithUserInfo`。
//! 链接时 accesskit_macos 对它们的调用会绑定到这里（目标文件里的定义优先于动态库），
//! 我们记一笔后再经 `dlsym(RTLD_NEXT)` 转给真正的 AppKit。不需要辅助功能权限。

mod intercept;

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Role, SharedString, Window, WindowBounds,
    WindowOptions, accesskit::Live, actions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

actions!(
    exp001,
    [NewMsg, RepeatMsg, ToggleLive, ToggleWorkaround, Fallback, Delayed]
);

/// 同一句话连续出现时的绕法
#[derive(Clone, Copy, Debug, PartialEq)]
enum Workaround {
    /// 不绕：同一个节点、同一个值
    None,
    /// 每次播报换一个节点 id（AccessKit 走 node_added 分支）
    NewNode,
    /// 交替在末尾加零宽空格，让值「变了」
    Zwsp,
}

impl Workaround {
    fn next(self) -> Self {
        match self {
            Self::None => Self::NewNode,
            Self::NewNode => Self::Zwsp,
            Self::Zwsp => Self::None,
        }
    }
    fn zh(self) -> &'static str {
        match self {
            Self::None => "不绕",
            Self::NewNode => "换节点",
            Self::Zwsp => "零宽字符",
        }
    }
}

fn live_zh(live: Live) -> &'static str {
    match live {
        Live::Assertive => "打断式",
        Live::Polite => "礼貌式",
        Live::Off => "关",
    }
}

const SENDERS: [&str; 4] = ["张三", "李四", "王五", "赵六"];
const BODIES: [&str; 4] = ["晚上一起吃饭吗", "文件我发你邮箱了", "收到，谢谢", "明天几点开会"];

struct Demo {
    focus_handle: FocusHandle,
    messages: Vec<String>,
    /// 播报节点当前的值；None = 节点没有值，不会播
    announce: Option<String>,
    live: Live,
    workaround: Workaround,
    /// 换节点绕法用的序号
    node_seq: usize,
    zwsp_flip: bool,
    counter: usize,
}

impl Demo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            messages: vec![
                "张三：今天下午的会改到四点了".into(),
                "李四：好的".into(),
                "王五：这是一条很长的消息，用来让 VoiceOver 读上好一阵子。我们想知道，当它正在朗读这一段的时候，如果来了一条新消息，礼貌式播报会不会等它读完再说，打断式播报会不会直接把它掐断。请把这段从头听到尾，中途留意有没有被插话。".into(),
            ],
            announce: None,
            live: Live::Polite,
            workaround: Workaround::None,
            node_seq: 0,
            zwsp_flip: false,
            counter: 0,
        }
    }

    /// 让播报节点说一句话（按当前绕法处理）
    fn say(&mut self, text: &str, cx: &mut Context<Self>) {
        let mut value = text.to_string();
        match self.workaround {
            Workaround::None => {}
            Workaround::NewNode => self.node_seq += 1,
            Workaround::Zwsp => {
                self.zwsp_flip = !self.zwsp_flip;
                if self.zwsp_flip {
                    value.push('\u{200B}');
                }
            }
        }
        intercept::mark_model_change(&value);
        self.announce = Some(value);
        cx.notify();
    }

    fn new_message(&mut self, cx: &mut Context<Self>) {
        let i = self.counter;
        self.counter += 1;
        let msg = format!("{}：{}", SENDERS[i % 4], BODIES[i % 4]);
        self.messages.push(msg.clone());
        self.say(&format!("新消息，{msg}"), cx);
    }

    fn repeat_message(&mut self, cx: &mut Context<Self>) {
        // 上一次播报的原文（去掉零宽字符）
        let last = self
            .announce
            .clone()
            .unwrap_or_else(|| "还没有消息".into())
            .replace('\u{200B}', "");
        self.say(&last, cx);
    }

    /// 切换模式：不经过 Live 节点（否则 live 变化本身会触发一次播报），用兜底直发告知
    fn set_mode(&mut self, live: Live, workaround: Workaround, announce_change: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.live = live;
        self.workaround = workaround;
        self.announce = None;
        self.node_seq += 1;
        if announce_change {
            intercept::post_announcement_direct(
                window_ptr(window),
                &format!("{}，{}", live_zh(live), workaround.zh()),
                live == Live::Assertive,
            );
        }
        cx.notify();
    }
}

/// 取当前 GPUI 窗口对应的 NSWindow（本实验只开一个窗口，取 keyWindow 或第一个）
fn window_ptr(_window: &Window) -> *mut objc2::runtime::AnyObject {
    intercept::first_ns_window()
}

impl Render for Demo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.messages.len();
        let live = self.live;
        let value = self.announce.clone();
        let announcer_key = match self.workaround {
            Workaround::NewNode => self.node_seq,
            _ => 0,
        };
        let shown = value.clone().unwrap_or_default().replace('\u{200B}', "");

        div()
            .id("root")
            .role(Role::Application)
            .aria_label("exp001 播报实验")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &NewMsg, _, cx| this.new_message(cx)))
            .on_action(cx.listener(|this, _: &RepeatMsg, _, cx| this.repeat_message(cx)))
            .on_action(cx.listener(|this, _: &ToggleLive, window, cx| {
                let live = if this.live == Live::Polite { Live::Assertive } else { Live::Polite };
                this.set_mode(live, this.workaround, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleWorkaround, window, cx| {
                this.set_mode(this.live, this.workaround.next(), true, window, cx)
            }))
            .on_action(cx.listener(|_, _: &Fallback, window, _| {
                intercept::post_announcement_direct(window_ptr(window), "兜底直发：这句话不经过 GPUI", false);
            }))
            .on_action(cx.listener(|_, _: &Delayed, window, cx| {
                cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(3)).await;
                    this.update(cx, |this, cx| this.new_message(cx)).ok();
                })
                .detach();
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(format!(
                "模式：{} · 重复绕法：{}　（n 新消息 · r 重复 · p 切换礼貌/打断 · w 切换绕法 · f 兜底直发 · l 三秒后来消息）",
                live_zh(live),
                self.workaround.zh()
            ))
            .child(
                div()
                    .id("messages")
                    .role(Role::List)
                    .aria_label("消息")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.messages.iter().enumerate().map(|(i, m)| {
                        div()
                            .id(("msg", i))
                            .role(Role::ListItem)
                            .aria_label(SharedString::from(m.clone()))
                            .aria_position_in_set(i + 1)
                            .aria_size_of_set(n)
                            .child(m.clone())
                    })),
            )
            // 播报节点：Role::Status，经公开出口 a11y_synthetic_children 拿到自己的
            // AccessKit 节点，设 Live + value
            .child(
                div()
                    .id(("announcer", announcer_key))
                    .role(Role::Status)
                    .a11y_synthetic_children(move |builder| {
                        let node = builder.parent_node();
                        node.set_live(live);
                        match &value {
                            Some(v) => node.set_value(v.clone()),
                            None => node.clear_value(),
                        }
                    })
                    .text_color(rgb(0xa6e3a1))
                    .child(format!("播报区：{shown}")),
            )
    }
}

// ---------------- 自动脚本 ----------------

struct Row {
    label: String,
    expect: usize,
    got: Vec<intercept::Event>,
    focus_events: usize,
    other: Vec<String>,
}

async fn run_auto(this: gpui::WeakEntity<Demo>, cx: &mut gpui::AsyncWindowContext) {
    let ex = cx.background_executor().clone();
    let wait = move |ms| ex.timer(Duration::from_millis(ms));
    // 1. 在进程内激活无障碍（等同读屏第一次来问「你有哪些子元素」）
    wait(500).await;
    let activated = intercept::activate_in_process();
    eprintln!("[exp001] 进程内激活：{activated}");
    wait(1000).await;

    let tree_before = this
        .update_in(cx, |_, window, _| (window.is_a11y_active(), window.debug_a11y_tree_json()))
        .unwrap();
    eprintln!("[exp001] GPUI 无障碍已激活：{}", tree_before.0);
    let focus_before = gpui_focus(tree_before.1.as_deref());

    let mut rows: Vec<Row> = Vec::new();

    for live in [Live::Polite, Live::Assertive] {
        for wa in [Workaround::None, Workaround::NewNode, Workaround::Zwsp] {
            let tag = format!("{} · {}", live_zh(live), wa.zh());
            // 切换模式本身不应播报
            let mark = intercept::mark();
            this.update_in(cx, |this, window, cx| this.set_mode(live, wa, false, window, cx)).unwrap();
            wait(600).await;
            rows.push(collect(format!("{tag} · 切换模式（值清空）"), 0, mark));

            let text = format!("新消息，{tag}");
            let mark = intercept::mark();
            this.update(cx, |this, cx| this.say(&text, cx)).unwrap();
            wait(600).await;
            rows.push(collect(format!("{tag} · 第一次说"), 1, mark));

            let mark = intercept::mark();
            this.update(cx, |this, cx| this.say(&text, cx)).unwrap();
            wait(600).await;
            rows.push(collect(format!("{tag} · 同一句再说"), 1, mark));

            let mark = intercept::mark();
            this.update(cx, |this, cx| this.say(&format!("另一句，{tag}"), cx)).unwrap();
            wait(600).await;
            rows.push(collect(format!("{tag} · 换一句"), 1, mark));

            let mark = intercept::mark();
            this.update(cx, |_, cx| cx.notify()).unwrap();
            wait(600).await;
            rows.push(collect(format!("{tag} · 只重绘不改值"), 0, mark));
        }
    }

    // 2. 兜底：直接发 NSAccessibilityAnnouncementRequestedNotification，同一句两次
    for i in 0..2 {
        let mark = intercept::mark();
        this.update_in(cx, |_, window, _| {
            intercept::post_announcement_direct(window_ptr(window), "兜底直发，同一句", false)
        })
        .unwrap();
        wait(400).await;
        rows.push(collect(format!("兜底直发 · 第 {} 次", i + 1), 1, mark));
    }

    let tree_after = this
        .update_in(cx, |_, window, _| window.debug_a11y_tree_json())
        .unwrap();
    let focus_after = gpui_focus(tree_after.as_deref());

    // 输出
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
    std::fs::create_dir_all(&out_dir).ok();
    if let Some(j) = &tree_before.1 {
        std::fs::write(out_dir.join("tree-before.json"), j).ok();
    }
    if let Some(j) = &tree_after {
        std::fs::write(out_dir.join("tree-after.json"), j).ok();
    }

    let mut md = String::new();
    md.push_str("| 步骤 | 期望公告数 | 实际公告 | 优先级 | 改值→发通知 ms | 焦点变化通知 | 其他通知 | 判定 |\n|---|---|---|---|---|---|---|---|\n");
    let mut pass = 0;
    for r in &rows {
        let texts: Vec<String> = r.got.iter().map(|e| format!("「{}」", e.text.replace('\u{200B}', "⁠<ZW>"))).collect();
        let prios: Vec<String> = r.got.iter().map(|e| e.priority.to_string()).collect();
        let lat: Vec<String> = r
            .got
            .iter()
            .map(|e| e.latency_ms.map(|l| format!("{l:.1}")).unwrap_or("-".into()))
            .collect();
        let ok = r.got.len() == r.expect && r.focus_events == 0;
        if ok {
            pass += 1;
        }
        md.push_str(&format!(
            "| {} | {} | {} {} | {} | {} | {} | {} | {} |\n",
            r.label,
            r.expect,
            r.got.len(),
            texts.join(" "),
            prios.join(","),
            lat.join(","),
            r.focus_events,
            r.other.join(","),
            if ok { "✅" } else { "❌" }
        ));
    }
    md.push_str(&format!(
        "\n合计 {pass}/{} 步符合期望。GPUI 焦点节点：前 {:?} → 后 {:?}。全程截获通知 {} 条。\n",
        rows.len(),
        focus_before,
        focus_after,
        intercept::mark()
    ));
    std::fs::write(out_dir.join("result.md"), &md).ok();
    println!("{md}");
    eprintln!("[exp001] 结果已写入 {}", out_dir.display());

    this.update(cx, |_, cx| cx.quit()).ok();
}

fn collect(label: String, expect: usize, mark: usize) -> Row {
    let evs = intercept::since(mark);
    let mut got = Vec::new();
    let mut focus_events = 0;
    let mut other = Vec::new();
    for e in evs {
        if e.name == "AXAnnouncementRequested" {
            got.push(e);
        } else if e.name == "AXFocusedUIElementChanged" {
            focus_events += 1;
        } else {
            other.push(e.name.clone());
        }
    }
    Row { label, expect, got, focus_events, other }
}

fn gpui_focus(json: Option<&str>) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json?).ok()?;
    let key = v.get("gpui_focus")?.as_str()?.to_string();
    let role = v
        .get("nodes")
        .and_then(|n| n.get(&key))
        .and_then(|n| n.get("role"))
        .map(|r| r.to_string())
        .unwrap_or_default();
    Some(format!("{key} {role}"))
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("gpui", log::LevelFilter::Info)
        .init();
    let auto = std::env::args().any(|a| a == "--auto");

    application().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("n", NewMsg, None),
            KeyBinding::new("r", RepeatMsg, None),
            KeyBinding::new("p", ToggleLive, None),
            KeyBinding::new("w", ToggleWorkaround, None),
            KeyBinding::new("f", Fallback, None),
            KeyBinding::new("l", Delayed, None),
        ]);
        let bounds = Bounds::centered(None, size(px(720.), px(480.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("exp001 播报实验".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let started = Instant::now();
                intercept::set_epoch(started);
                cx.new(|cx| {
                    if auto {
                        cx.spawn_in(window, async move |this, cx| run_auto(this, cx).await)
                            .detach();
                    }
                    Demo::new(window, cx)
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
