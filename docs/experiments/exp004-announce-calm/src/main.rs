//! exp004-announce-calm：新消息来了，用户要知道，但正在读的内容不能被打断。
//!
//! 七种提示模式（数字键 1–7 切换），同一个消息列表：
//! 1 礼貌式（对照，exp001 原样：Live::Polite，优先级 50，立刻发）
//! 2 低优先级（直发公告，优先级 10；AccessKit 只有 50/90 两档）
//! 3 提示音（只播一声系统音，不说话；按 m 再听）
//! 4 静候再说（用户停手 N 秒后，说一句汇总「2 条新消息，最新：……」）
//! 5 估算朗读时长再说（等「当前选中项按字数估算的朗读时间 + 1 秒」过去再汇总）
//! 6 写进名称（不说话；窗口标题和列表名称带上「2 条新消息」）
//! 7 写进提示语（不说话；选中项的 aria_description = macOS 的 AXHelp，VoiceOver 读完名称停顿后才念）
//!
//! 两种跑法：`cargo run`（人开 VoiceOver 试用）· `cargo run -- --auto`（自动走七种模式，打印结果表后退出）

mod ax;

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Keystroke, Role, SharedString, Window,
    WindowBounds, WindowOptions, accesskit::Live, actions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

actions!(
    exp004,
    [
        SelectNext, SelectPrev, SelectLast, ReadUnread, OneLater, BurstLater, Mode1, Mode2, Mode3,
        Mode4, Mode5, Mode6, Mode7
    ]
);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Polite,
    Low,
    Sound,
    Idle,
    Estimate,
    Name,
    Hint,
}

const MODES: [Mode; 7] =
    [Mode::Polite, Mode::Low, Mode::Sound, Mode::Idle, Mode::Estimate, Mode::Name, Mode::Hint];

impl Mode {
    fn zh(self) -> &'static str {
        match self {
            Mode::Polite => "模式 1，礼貌式",
            Mode::Low => "模式 2，低优先级",
            Mode::Sound => "模式 3，提示音",
            Mode::Idle => "模式 4，静候再说",
            Mode::Estimate => "模式 5，估算朗读时长再说",
            Mode::Name => "模式 6，写进名称",
            Mode::Hint => "模式 7，写进提示语",
        }
    }
    fn deferred(self) -> bool {
        matches!(self, Mode::Idle | Mode::Estimate)
    }
}

fn env_ms(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

const LONG: &str = "王五：这是一条很长的消息，用来让 VoiceOver 读上好一阵子。我们想知道，当它正在朗读这一段的时候，如果来了一条新消息，这一种提示方式会不会插进来、会不会把它掐断，还是安安静静地等它读完。请把这段从头听到尾，中途留意有没有被插话，有没有听到提示音。";

const INCOMING: [&str; 6] = [
    "赵六：晚上一起吃饭吗",
    "钱七：文件我发你邮箱了",
    "孙八：收到，谢谢",
    "周九：明天几点开会",
    "吴十：好的",
    "郑一：路上堵车，晚十分钟",
];

const BASE_TITLE: &str = "exp004 新消息提示";

struct Demo {
    focus: FocusHandle,
    messages: Vec<String>,
    selected: usize,
    mode: Mode,
    unread: usize,
    latest: Option<String>,
    /// 模式 4/5：有待说的汇总
    pending: bool,
    /// 用户最近一次按键（应用收得到的键）的时间
    last_input: Instant,
    /// Live 播报节点：值 + 换节点序号（exp001 的「换节点」绕法，避免同一句不播）
    announce: Option<String>,
    announce_seq: usize,
    incoming_ix: usize,
    idle_ms: u64,
    ms_per_char: u64,
    /// 上一次写给窗口的标题，变了才写
    shown_title: String,
    shown_hint: String,
}

impl Demo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // 记最近按键：应用能收到的键都算（VoiceOver 自己吃掉的 VO 组合键收不到，见 README）
        cx.observe_keystrokes(|this, e, _, _| {
            this.last_input = Instant::now();
            ax::record("按键", &e.keystroke.key, 0);
        })
        .detach();
        // 调试：Demo 被释放时打印是谁释放的
        cx.on_release(|_, _| {
            eprintln!("[exp004] Demo 被释放 t={:.3}\n{}", ax::now(), std::backtrace::Backtrace::force_capture());
        })
        .detach();
        // 模式 4/5 的计时器：每 100 ms 看一次该不该说
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        Self {
            focus,
            messages: vec![
                "张三：今天下午的会改到四点了".into(),
                "李四：好的".into(),
                LONG.into(),
                "张三：会议室在三楼".into(),
                "李四：我先过去占位置".into(),
                "张三：带上投影线".into(),
                "李四：没问题".into(),
                "张三：顺便买咖啡".into(),
                "李四：几杯".into(),
                "张三：四杯".into(),
            ],
            selected: 0,
            mode: Mode::Polite,
            unread: 0,
            latest: None,
            pending: false,
            last_input: Instant::now(),
            announce: None,
            announce_seq: 0,
            incoming_ix: 0,
            idle_ms: env_ms("EXP004_IDLE_MS", 3000),
            ms_per_char: env_ms("EXP004_MS_PER_CHAR", 180),
            shown_title: String::new(),
            shown_hint: String::new(),
        }
    }

    fn touch(&mut self) {
        self.last_input = Instant::now();
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.touch();
        self.selected = ix.min(self.messages.len() - 1);
        // 走到最后一条 = 新消息都看到了
        if self.selected == self.messages.len() - 1 {
            self.clear_unread();
        }
        cx.notify();
    }

    fn clear_unread(&mut self) {
        self.unread = 0;
        self.pending = false;
    }

    fn summary(&self) -> String {
        let latest = self.latest.clone().unwrap_or_default();
        match self.unread {
            0 => "没有新消息".into(),
            1 => format!("新消息，{latest}"),
            n => format!("{n} 条新消息，最新：{latest}"),
        }
    }

    /// 经 GPUI 的 Live 节点说一句（礼貌式，换节点）
    fn say_live(&mut self, text: String, cx: &mut Context<Self>) {
        self.announce_seq += 1;
        self.announce = Some(text);
        cx.notify();
    }

    /// 一条新消息到达。不动 selected——焦点和当前读到的位置都不变
    fn arrive(&mut self, cx: &mut Context<Self>) {
        let msg = INCOMING[self.incoming_ix % INCOMING.len()].to_string();
        self.incoming_ix += 1;
        ax::record("（新消息到达）", &msg, 0);
        self.messages.push(msg.clone());
        self.unread += 1;
        self.latest = Some(msg.clone());
        match self.mode {
            Mode::Polite => self.say_live(format!("新消息，{msg}"), cx),
            Mode::Low => ax::announce_direct(&format!("新消息，{msg}"), 10),
            Mode::Sound => ax::play_sound("Pop"),
            Mode::Idle | Mode::Estimate => self.pending = true,
            Mode::Name | Mode::Hint => {} // render 里写进名称 / 提示语
        }
        cx.notify();
    }

    /// 模式 4/5 什么时候可以说
    fn deadline(&self) -> Instant {
        match self.mode {
            Mode::Estimate => {
                let chars = self.messages[self.selected].chars().count() as u64;
                self.last_input + Duration::from_millis(chars * self.ms_per_char + 1000)
            }
            _ => self.last_input + Duration::from_millis(self.idle_ms),
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.pending && std::env::var_os("EXP004_DEBUG").is_some() {
            eprintln!("[tick] mode={:?} 距 deadline {:?}", self.mode, self.deadline().checked_duration_since(Instant::now()));
        }
        if self.pending && self.mode.deferred() && Instant::now() >= self.deadline() {
            self.pending = false;
            let s = self.summary();
            self.say_live(s, cx);
        }
    }

    /// m 键：用户主动要听，直接说（高优先级），并清零
    fn read_unread(&mut self) {
        self.touch();
        ax::announce_direct(&self.summary(), 90);
        self.clear_unread();
    }

    fn set_mode(&mut self, mode: Mode, speak: bool, cx: &mut Context<Self>) {
        self.mode = mode;
        self.clear_unread();
        self.announce = None;
        self.announce_seq += 1;
        if speak {
            ax::announce_direct(mode.zh(), 90);
        }
        cx.notify();
    }

    fn later(&mut self, count: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.touch();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            for i in 0..count {
                if i > 0 {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                }
                this.update(cx, |this, cx| this.arrive(cx)).ok();
            }
        })
        .detach();
    }

    fn unread_note(&self) -> Option<String> {
        (self.unread > 0).then(|| format!("{} 条新消息，按 M 收听", self.unread))
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 模式 6：窗口标题 + 列表名称
        let name_note = (self.mode == Mode::Name).then(|| self.unread_note()).flatten();
        let title = match &name_note {
            Some(n) => format!("{BASE_TITLE}，{n}"),
            None => BASE_TITLE.to_string(),
        };
        if title != self.shown_title {
            window.set_window_title(&title);
            ax::record("改窗口标题", &title, 0);
            self.shown_title = title;
        }
        let list_label = match &name_note {
            Some(n) => format!("消息，{n}"),
            None => "消息".to_string(),
        };
        // 模式 7：选中项的提示语
        let hint = (self.mode == Mode::Hint).then(|| self.unread_note()).flatten();
        let hint_s = hint.clone().unwrap_or_default();
        if hint_s != self.shown_hint {
            ax::record("改选中项提示语", &hint_s, 0);
            self.shown_hint = hint_s;
        }

        let n = self.messages.len();
        let selected = self.selected;
        let value = self.announce.clone();
        let shown = value.clone().unwrap_or_default();

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(format!(
                "{}　未读 {}　（↑↓ 浏览 · 1–7 切模式 · l 三秒后来 1 条 · k 三秒后来 3 条 · m 听新消息）",
                self.mode.zh(),
                self.unread
            ))
            .child(
                div()
                    .id("msg-list")
                    .role(Role::List)
                    .aria_label(SharedString::from(list_label))
                    .key_context("MessageList")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select(this.selected + 1, cx)))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.select(this.selected.saturating_sub(1), cx)))
                    .on_action(cx.listener(|this, _: &SelectLast, _, cx| this.select(usize::MAX, cx)))
                    .on_action(cx.listener(|this, _: &ReadUnread, _, _| this.read_unread()))
                    .on_action(cx.listener(|this, _: &OneLater, w, cx| this.later(1, w, cx)))
                    .on_action(cx.listener(|this, _: &BurstLater, w, cx| this.later(3, w, cx)))
                    .on_action(cx.listener(|this, _: &Mode1, _, cx| this.set_mode(Mode::Polite, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode2, _, cx| this.set_mode(Mode::Low, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode3, _, cx| this.set_mode(Mode::Sound, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode4, _, cx| this.set_mode(Mode::Idle, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode5, _, cx| this.set_mode(Mode::Estimate, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode6, _, cx| this.set_mode(Mode::Name, true, cx)))
                    .on_action(cx.listener(|this, _: &Mode7, _, cx| this.set_mode(Mode::Hint, true, cx)))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.messages.iter().enumerate().map(|(i, m)| {
                        let is_sel = i == selected;
                        div()
                            .id(("msg", i))
                            .role(Role::ListItem)
                            .aria_label(SharedString::from(m.clone()))
                            .aria_position_in_set(i + 1)
                            .aria_size_of_set(n)
                            .aria_selected(is_sel)
                            .when(is_sel, |d| d.aria_active_descendant().bg(rgb(0x45475a)))
                            .when_some(hint.clone().filter(|_| is_sel), |d, h| d.aria_description(h))
                            .px_2()
                            .child(m.clone())
                    })),
            )
            // Live 播报节点（模式 1/4/5 用）
            .child(
                div()
                    .id(("announcer", self.announce_seq))
                    .role(Role::Status)
                    .a11y_synthetic_children(move |b| {
                        let node = b.parent_node();
                        node.set_live(Live::Polite);
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

struct ModeResult {
    mode: Mode,
    /// 浏览期间（第一次按键 → 最后一次按键后 0.5 s）的公告
    during: Vec<ax::Event>,
    sounds: usize,
    /// 停手后的公告，以及距最后一次按键几秒
    after: Vec<(ax::Event, f64)>,
    selected_ok: bool,
    probe: ax::Probe,
    list_label: String,
    /// GPUI 树里选中项的 description（= macOS AXHelp）
    tree_hint: String,
}

async fn run_auto(this: gpui::WeakEntity<Demo>, cx: &mut gpui::AsyncWindowContext) {
    let ex = cx.background_executor().clone();
    let wait = move |ms: u64| ex.timer(Duration::from_millis(ms));
    wait(500).await;
    ax::order_front_without_activating();
    ax::activate_in_process();
    wait(800).await;
    let active = this.update_in(cx, |_, w, _| w.is_a11y_active()).unwrap();
    eprintln!("[exp004] GPUI 无障碍已激活：{active}");

    let mut results = Vec::new();
    for mode in MODES {
        this.update(cx, |this, cx| {
            this.set_mode(mode, false, cx);
            this.selected = 0;
        })
        .unwrap();
        wait(400).await;

        // 浏览：每 400 ms 按一次 ↓，共 8 次；第 2、5 次按键后各来一条新消息
        let mark = ax::mark();
        let t_first = ax::now();
        for i in 0..8 {
            cx.update(|w, cx| w.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx)).unwrap();
            if i == 1 || i == 4 {
                wait(100).await;
                this.update(cx, |this, cx| this.arrive(cx)).unwrap();
                wait(300).await;
            } else {
                wait(400).await;
            }
        }
        let t_last = ax::now() - 0.4;
        // 停手 7 秒
        wait(7000).await;
        let probe = ax::probe();
        let (selected_ok, tree) = this
            .update_in(cx, |this, w, _| (this.selected == 8, w.debug_a11y_tree_json()))
            .unwrap();
        let list_label = list_label_from_tree(tree.as_deref());
        let tree_hint = selected_hint_from_tree(tree.as_deref());

        let evs = ax::since(mark);
        let ann = |e: &&ax::Event| e.name == "AXAnnouncementRequested";
        let during = evs.iter().filter(ann).filter(|e| e.t >= t_first && e.t <= t_last + 0.5).cloned().collect();
        let after = evs
            .iter()
            .filter(ann)
            .filter(|e| e.t > t_last + 0.5)
            .map(|e| (e.clone(), e.t - t_last))
            .collect();
        let sounds = evs.iter().filter(|e| e.name == "声音").count();
        results.push(ModeResult { mode, during, sounds, after, selected_ok, probe, list_label, tree_hint });
        // 每跑完一种模式就写一次盘：中途被关窗也留得下已跑完的部分
        write_result(&report(&results));

        // 按 m 听新消息（清零），让下一个模式从干净的状态开始
        cx.update(|w, cx| w.dispatch_keystroke(Keystroke::parse("m").unwrap(), cx)).unwrap();
        wait(400).await;
    }

    let md = report(&results);
    let out = write_result(&md);
    if let Some(j) = this.update_in(cx, |_, w, _| w.debug_a11y_tree_json()).unwrap() {
        std::fs::write(out.join("tree-end.json"), j).ok();
    }
    println!("{md}");
    eprintln!("[exp004] 结果已写入 {}", out.display());
    this.update(cx, |_, cx| cx.quit()).ok();
}

fn write_result(md: &str) -> std::path::PathBuf {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
    std::fs::create_dir_all(&out).ok();
    std::fs::write(out.join("result.md"), md).ok();
    out
}

fn selected_hint_from_tree(json: Option<&str>) -> String {
    let Some(v) = json.and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()) else {
        return String::new();
    };
    v["nodes"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .find(|n| n["aria"]["selected"] == true)
        .and_then(|n| n["aria"]["description"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn list_label_from_tree(json: Option<&str>) -> String {
    let Some(v) = json.and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()) else {
        return String::new();
    };
    v["nodes"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .find(|n| n["aria"]["role"] == "List")
        .and_then(|n| n["aria"]["label"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn report(rs: &[ModeResult]) -> String {
    let mut md = String::from(
        "| 模式 | 浏览中公告（条 · 优先级） | 浏览中提示音 | 停手后公告（距最后按键 s · 文本 · 优先级） | 选中项没被挪动 | macOS 窗口标题 | 列表名称（GPUI 树） | 选中项提示语（GPUI 树 description） | 选中项 AXHelp（macOS 层，应用非前台时多为空） | 判定 |\n|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut pass = 0;
    for r in rs {
        let during_n = r.during.len();
        let prios: Vec<String> = r.during.iter().map(|e| e.priority.to_string()).collect();
        let after: Vec<String> = r
            .after
            .iter()
            .map(|(e, dt)| format!("{dt:.1} s「{}」{}", e.text, e.priority))
            .collect();
        let has2 = |s: &str| s.contains("2 条新消息");
        let ok = r.selected_ok
            && match r.mode {
                Mode::Polite => during_n == 2 && r.during.iter().all(|e| e.priority == 50),
                Mode::Low => during_n == 2 && r.during.iter().all(|e| e.priority == 10),
                Mode::Sound => during_n == 0 && r.sounds == 2,
                Mode::Idle | Mode::Estimate => {
                    during_n == 0 && r.after.len() == 1 && has2(&r.after[0].0.text)
                }
                Mode::Name => {
                    during_n == 0 && r.after.is_empty() && has2(&r.probe.window_title) && has2(&r.list_label)
                }
                Mode::Hint => during_n == 0 && r.after.is_empty() && has2(&r.tree_hint),
            };
        if ok {
            pass += 1;
        }
        md.push_str(&format!(
            "| {} | {} · {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.mode.zh(),
            during_n,
            prios.join(","),
            r.sounds,
            after.join("；"),
            if r.selected_ok { "是（第 9 条）" } else { "否" },
            r.probe.window_title,
            r.list_label,
            r.tree_hint,
            r.probe.focused_help,
            if ok { "✅" } else { "❌" }
        ));
    }
    md.push_str(&format!(
        "\n合计 {pass}/{} 种模式符合期望。macOS 层焦点元素（停手时）：见各行；浏览 8 次 ↓，每次间隔 0.4 s，第 2、5 次后各来一条新消息，停手 7 s。\n",
        rs.len()
    ));
    for r in rs {
        md.push_str(&format!("- {}：macOS 焦点元素标题「{}」\n", r.mode.zh(), r.probe.focused_title));
    }
    md
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("gpui", log::LevelFilter::Info)
        .init();
    let auto = std::env::args().any(|a| a == "--auto");

    application().run(move |cx: &mut App| {
        let c = Some("MessageList");
        cx.bind_keys([
            KeyBinding::new("down", SelectNext, c),
            KeyBinding::new("up", SelectPrev, c),
            KeyBinding::new("end", SelectLast, c),
            KeyBinding::new("m", ReadUnread, c),
            KeyBinding::new("l", OneLater, c),
            KeyBinding::new("k", BurstLater, c),
            KeyBinding::new("1", Mode1, c),
            KeyBinding::new("2", Mode2, c),
            KeyBinding::new("3", Mode3, c),
            KeyBinding::new("4", Mode4, c),
            KeyBinding::new("5", Mode5, c),
            KeyBinding::new("6", Mode6, c),
            KeyBinding::new("7", Mode7, c),
        ]);
        let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(BASE_TITLE.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                ax::set_epoch(Instant::now());
                if auto {
                    // 自动模式：人可能正在用电脑，不抢前台；人关窗就让他关，已跑完的模式已逐个写盘，直接退出
                    window.on_window_should_close(cx, |_, cx| {
                        eprintln!("[exp004] 窗口被外部关闭，自动脚本中止；已跑完的模式见 out/result.md");
                        cx.quit();
                        true
                    });
                }
                cx.new(|cx| {
                    if auto {
                        cx.spawn_in(window, async move |this, cx| run_auto(this, cx).await).detach();
                    }
                    Demo::new(window, cx)
                })
            },
        )
        .unwrap();
        if !auto {
            cx.activate(true);
        }
    });
}
