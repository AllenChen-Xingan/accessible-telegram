//! exp003-list-nav：1000 条假消息的虚拟化列表，由应用自己的方向键驱动。
//!
//! 容器持焦点 + 选中项 `aria_active_descendant`；按键先 `scroll_to_item` 再选中，
//! 两件事落在同一帧里（uniform_list 在 prepaint 时处理延迟滚动，见
//! `uniform_list.rs:440-492`），所以被选中的那一项一定已被渲染、进了树。
//!
//! exp003b 追加：
//! - 默认读法不再带「第几条 / 第几项共几项」；`--pos-in-label` 留作对照
//! - `--key-fix on|off`：方向键类按键在 AppKit 层截下、直接交给 GPUI，绕过输入法（默认 on）
//! - `--sync none|scroll|focus|both`：VoiceOver 光标落到某项时，应用怎么跟（默认 both）
//!     scroll = 接住 AXScrollToVisible（AccessKit 的 ScrollIntoView）
//!     focus  = 接住设键盘焦点（AccessKit 的 Focus），同步为选中项
//!   两者都会在该项靠近可见区边缘时把它滚到中间，让下一屏进树
//! - `--overdraw K`：可见区上下各多渲染 K 项（裁掉不显示，但在无障碍树里）
//! - 人工试用时把收到的按键、GPUI 认出的按键、读屏发来的动作逐条记到 `out/live-*.jsonl`
//!
//! 运行：
//!   cargo run                               # 人工试用
//!   cargo run -- --selftest out/x           # 自测：GPUI 内部模拟按键（exp003 原法）
//!   cargo run -- --selftest-real out/x      # 自测：AppKit 真实按键事件
//!   cargo run -- --selftest-vo out/x        # 自测：模拟 VO+→ / VO+← 逐项走

mod ax;

use std::{
    cell::Cell,
    path::PathBuf,
    rc::Rc,
    sync::OnceLock,
    time::{Duration, Instant},
};

use gpui::{
    AccessibleAction, App, Bounds, Context, FocusHandle, KeyBinding, Keystroke, Role,
    ScrollStrategy, SharedString, UniformListScrollHandle, Window, WindowBounds, WindowOptions,
    actions, div, prelude::*, px, rgb, size, uniform_list,
};
use gpui_platform::application;
use serde_json::{Value, json};

actions!(
    exp003,
    [SelectNext, SelectPrev, SelectFirst, SelectLast, PageDown, PageUp, FocusNext, FocusPrev]
);

/// 焦点是否在消息列表上。AppKit 监视器（不在 GPUI 上下文里）靠它决定要不要截下 ↑/↓：
/// 只有列表有焦点时才绕过输入法，别的控件（以后的真输入框）照常交给输入法
static LIST_FOCUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const N: usize = 1000;
const ITEM_H: f32 = 28.0;
const VISIBLE: usize = 12;
/// 翻一页移动的条数：可见条数减一，留一条上下文
const PAGE: usize = VISIBLE - 1;
/// VO 光标离可见区边缘不到这么多项时，把它滚到中间
const EDGE: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
enum SelfTest {
    /// GPUI 内部 dispatch_keystroke（exp003 原法）
    Internal,
    /// AppKit 真实按键事件
    Real,
    /// 模拟 VoiceOver 逐项浏览
    Vo,
    /// exp003b-v2：Tab 进出列表、列表内 ↑/↓、VO 把焦点设到列表
    Tab,
}

#[derive(Clone, Debug)]
struct Config {
    listbox: bool,
    pos_in_label: bool,
    /// 列表名称带「共 N 条」（只在进入列表时读一次）；默认不带
    list_count: bool,
    key_fix: bool,
    sync_scroll: bool,
    sync_focus: bool,
    overdraw: usize,
    selftest: Option<(SelfTest, PathBuf)>,
    log: Option<PathBuf>,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut cfg = Config {
            listbox: false,
            pos_in_label: false,
            list_count: false,
            key_fix: true,
            // 拍板 3 改为「专门列表 + ↑/↓」后，跟随 VO 光标降为次要，默认关
            sync_scroll: false,
            sync_focus: false,
            overdraw: 0,
            selftest: None,
            log: None,
        };
        let next = |i: &mut usize| {
            *i += 1;
            args.get(*i).cloned().unwrap_or_default()
        };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--listbox" => cfg.listbox = true,
                "--pos-in-label" => cfg.pos_in_label = true,
                "--list-count" => cfg.list_count = true,
                "--key-fix" => cfg.key_fix = next(&mut i) != "off",
                "--sync" => {
                    let v = next(&mut i);
                    cfg.sync_scroll = v == "scroll" || v == "both";
                    cfg.sync_focus = v == "focus" || v == "both";
                }
                "--overdraw" => cfg.overdraw = next(&mut i).parse().unwrap_or(0),
                "--log" => cfg.log = Some(PathBuf::from(next(&mut i))),
                "--selftest" => cfg.selftest = Some((SelfTest::Internal, PathBuf::from(next(&mut i)))),
                "--selftest-real" => cfg.selftest = Some((SelfTest::Real, PathBuf::from(next(&mut i)))),
                "--selftest-vo" => cfg.selftest = Some((SelfTest::Vo, PathBuf::from(next(&mut i)))),
                "--selftest-tab" => cfg.selftest = Some((SelfTest::Tab, PathBuf::from(next(&mut i)))),
                other => eprintln!("忽略未知参数 {other}"),
            }
            i += 1;
        }
        cfg
    }

    fn sync_name(&self) -> &'static str {
        match (self.sync_scroll, self.sync_focus) {
            (false, false) => "none",
            (true, false) => "scroll",
            (false, true) => "focus",
            (true, true) => "both",
        }
    }

    fn mode_name(&self) -> String {
        format!(
            "{}+{}{}+keyfix-{}+sync-{}+overdraw-{}",
            if self.listbox { "listbox" } else { "list" },
            if self.pos_in_label { "pos-in-label" } else { "no-pos" },
            if self.list_count { "+list-count" } else { "" },
            if self.key_fix { "on" } else { "off" },
            self.sync_name(),
            self.overdraw
        )
    }
}

const SENDERS: [&str; 5] = ["王小明", "李华", "张伟", "Alice", "Bob"];
const BODIES: [&str; 7] = [
    "今晚一起吃饭吗",
    "收到，谢谢",
    "文件我发你邮箱了",
    "明天几点开会",
    "好的",
    "路上堵车，晚十分钟",
    "看到了",
];

/// 默认读法：「发送人：内容，时间」——不读位置（人定 2026-10-23）。
/// 时间每条不同（1000 < 1440 分钟），兼作自测里的唯一标识
fn base_label(ix: usize) -> String {
    let sender = SENDERS[ix % SENDERS.len()];
    let body = BODIES[(ix * 3) % BODIES.len()];
    let minutes = ix % (24 * 60);
    format!("{sender}：{body}，{:02}:{:02}", minutes / 60, minutes % 60)
}

fn label_for(cfg: &Config, ix: usize) -> String {
    if cfg.pos_in_label {
        format!("{}，第 {} 项，共 {} 项", base_label(ix), ix + 1, N)
    } else {
        base_label(ix)
    }
}

/// 按键对选中项的作用；界面和自测共用这一份，自测拿它算期望值
fn apply_key(sel: usize, key: &str) -> usize {
    match key {
        "down" => (sel + 1).min(N - 1),
        "up" => sel.saturating_sub(1),
        "home" | "cmd-up" => 0,
        "end" | "cmd-down" => N - 1,
        "pagedown" => (sel + PAGE).min(N - 1),
        "pageup" => sel.saturating_sub(PAGE),
        _ => sel,
    }
}

/// 绕过输入法时，按键从 AppKit 监视器经这条通道交给 GPUI
static KEY_TX: OnceLock<async_channel::Sender<&'static str>> = OnceLock::new();

struct MessageList {
    cfg: Config,
    /// 列表本身（Tab 顺序第 3 个）
    focus: FocusHandle,
    /// 列表之外的几个可聚焦控件：会话标题、搜索、输入框（占位）、发送
    header: FocusHandle,
    search: FocusHandle,
    input: FocusHandle,
    send: FocusHandle,
    scroll: UniformListScrollHandle,
    selected: usize,
    /// 上一帧 uniform_list 渲染的行范围（行号 = 消息序号 + overdraw，含上下垫行）
    rendered_rows: Rc<Cell<(usize, usize)>>,
    _subs: Vec<gpui::Subscription>,
}

impl MessageList {
    fn new(cfg: Config, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Tab 顺序：会话标题 → 搜索 → 消息列表 → 输入框 → 发送 →（回到）会话标题
        let header = cx.focus_handle().tab_index(0).tab_stop(true);
        let search = cx.focus_handle().tab_index(1).tab_stop(true);
        let focus = cx.focus_handle().tab_index(2).tab_stop(true);
        let input = cx.focus_handle().tab_index(3).tab_stop(true);
        let send = cx.focus_handle().tab_index(4).tab_stop(true);
        // 旧的三种自测假定一开始就在列表上；人工试用和 Tab 自测从会话标题开始
        let start_in_list = matches!(
            cfg.selftest,
            Some((SelfTest::Internal | SelfTest::Real | SelfTest::Vo, _))
        );
        window.focus(if start_in_list { &focus } else { &header }, cx);
        LIST_FOCUSED.store(start_in_list, std::sync::atomic::Ordering::SeqCst);
        let subs = vec![
            cx.on_focus(&focus, window, |_, _, _| {
                LIST_FOCUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                ax::log_event(json!({"ev": "focus", "to": "消息列表"}));
            }),
            cx.on_blur(&focus, window, |_, _, _| {
                LIST_FOCUSED.store(false, std::sync::atomic::Ordering::SeqCst);
                ax::log_event(json!({"ev": "blur", "from": "消息列表"}));
            }),
        ];
        Self {
            cfg,
            focus,
            header,
            search,
            input,
            send,
            scroll: UniformListScrollHandle::new(),
            selected: 0,
            rendered_rows: Rc::new(Cell::new((0, 0))),
            _subs: subs,
        }
    }

    /// 先把目标项滚进可见区，再选中；两步在同一帧生效。
    /// overdraw = K 时行号 = 消息号 + K，可见区 = uniform_list 视口去掉上下各 K 行：
    /// 往下走保证「行号 + K」在视口里，往上走保证「行号 - K」在视口里。K = 0 时就是 Nearest
    fn move_by_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let next = apply_key(self.selected, key);
        let k = self.cfg.overdraw;
        if next >= self.selected {
            self.scroll.scroll_to_item(next + 2 * k, ScrollStrategy::Nearest);
        } else {
            self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        }
        ax::log_event(json!({"ev": "app_key", "key": key, "from": self.selected + 1, "to": next + 1}));
        self.selected = next;
        cx.notify();
    }

    /// VoiceOver 光标落到第 ix 项（读屏发来 ScrollIntoView 或 Focus）
    fn on_vo(&mut self, ix: usize, what: &str, cx: &mut Context<Self>) {
        let k = self.cfg.overdraw;
        let (r0, r1) = self.rendered_rows.get();
        // 可见的消息号范围 [vis.0, vis.1)
        let vis = (r0.min(N), r1.saturating_sub(2 * k).min(N));
        let near_edge = ix < vis.0 + EDGE || ix + EDGE >= vis.1;
        if near_edge {
            self.scroll.scroll_to_item_strict(ix + k, ScrollStrategy::Center);
        }
        let mut synced = false;
        if what == "Focus" && ix != self.selected {
            self.selected = ix;
            synced = true;
        }
        ax::log_event(json!({"ev": "a11y_action", "action": what, "item": ix + 1,
                             "visible": [vis.0 + 1, vis.1], "scrolled": near_edge, "selection_synced": synced}));
        cx.notify();
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (list_role, item_role) = if self.cfg.listbox {
            (Role::ListBox, Role::ListBoxOption)
        } else {
            (Role::List, Role::ListItem)
        };
        let selected = self.selected;
        let k = self.cfg.overdraw;
        let weak = cx.entity().downgrade();
        let list_name = if self.cfg.list_count {
            format!("消息，共 {N} 条")
        } else {
            "消息".to_string()
        };
        // 列表之外的可聚焦控件：有角色、有名称、在 Tab 顺序里，获焦时画个框
        let control = |id: &'static str, role: Role, name: &'static str, handle: &FocusHandle| {
            div()
                .id(id)
                .role(role)
                .aria_label(SharedString::from(name))
                .track_focus(handle)
                .px_2()
                .border_1()
                .border_color(rgb(0x1e1e2e))
                .focus(|s| s.border_color(rgb(0xf9e2af)))
                .child(SharedString::from(name))
        };
        div()
            .id("root")
            .size_full()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, window, cx| window.focus_prev(cx)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(control("chat-title", Role::Button, "会话：王小明", &self.header))
                    .child(control("search", Role::Button, "搜索", &self.search)),
            )
            .child(
                div()
                    .id("msg-list")
                    .role(list_role)
                    .aria_label(SharedString::from(list_name))
                    .border_1()
                    .border_color(rgb(0x1e1e2e))
                    .focus(|s| s.border_color(rgb(0xf9e2af)))
                    .key_context("MessageList")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_by_key("down", cx)))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_by_key("up", cx)))
                    .on_action(cx.listener(|this, _: &SelectFirst, _, cx| this.move_by_key("home", cx)))
                    .on_action(cx.listener(|this, _: &SelectLast, _, cx| this.move_by_key("end", cx)))
                    .on_action(cx.listener(|this, _: &PageDown, _, cx| this.move_by_key("pagedown", cx)))
                    .on_action(cx.listener(|this, _: &PageUp, _, cx| this.move_by_key("pageup", cx)))
                    .relative()
                    .overflow_hidden()
                    .w_full()
                    .h(px(ITEM_H * VISIBLE as f32))
                    .child(
                        // overdraw：uniform_list 比可见区上下各高 K 行，外层裁掉
                        div()
                            .absolute()
                            .left_0()
                            .top(px(-(ITEM_H * k as f32)))
                            .w_full()
                            .h(px(ITEM_H * (VISIBLE + 2 * k) as f32))
                            .child(
                                uniform_list(
                                    "msg-uniform",
                                    N + 2 * k,
                                    cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                                        this.rendered_rows.set((range.start, range.end));
                                        range
                                            .map(|row| {
                                                if row < k || row >= N + k {
                                                    // 垫行：让第 1 条 / 最后一条也能落在可见区里
                                                    return div().id(("pad", row)).h(px(ITEM_H)).into_any_element();
                                                }
                                                let ix = row - k;
                                                let label = label_for(&this.cfg, ix);
                                                let is_sel = ix == this.selected;
                                                let w1 = weak.clone();
                                                let w2 = weak.clone();
                                                div()
                                                    .id(("msg", ix))
                                                    .role(item_role)
                                                    .aria_label(SharedString::from(label.clone()))
                                                    .aria_position_in_set(ix + 1)
                                                    .aria_size_of_set(N)
                                                    .aria_selected(is_sel)
                                                    .when(is_sel, |d| d.aria_active_descendant().bg(rgb(0x45475a)))
                                                    .when(this.cfg.sync_scroll, |d| {
                                                        d.on_a11y_action(AccessibleAction::ScrollIntoView, move |_, _, cx| {
                                                            w1.update(cx, |this, cx| this.on_vo(ix, "ScrollIntoView", cx)).ok();
                                                        })
                                                    })
                                                    .when(this.cfg.sync_focus, |d| {
                                                        d.on_a11y_action(AccessibleAction::Focus, move |_, _, cx| {
                                                            w2.update(cx, |this, cx| this.on_vo(ix, "Focus", cx)).ok();
                                                        })
                                                    })
                                                    .h(px(ITEM_H))
                                                    .px_2()
                                                    .child(SharedString::from(label))
                                                    .into_any_element()
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .track_scroll(&self.scroll)
                                .h_full(),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(control("composer", Role::TextInput, "输入消息（占位）", &self.input).flex_1())
                    .child(control("send", Role::Button, "发送", &self.send)),
            )
            .child(
                div()
                    .px_2()
                    .child(SharedString::from(format!("选中 {} / {}", selected + 1, N))),
            )
    }
}

// ---------------------------------------------------------------- 自测：按键

fn find_list_and_ad(tree: &Value) -> (Option<String>, Option<&Value>) {
    let nodes = tree["nodes"].as_object();
    let list_key = nodes.and_then(|m| {
        m.iter().find_map(|(k, n)| {
            let role = n["aria"]["role"].as_str().unwrap_or("");
            ((role == "List" || role == "ListBox")
                && n["element_id"].as_str().is_some_and(|e| e.contains("\"msg-list\"")))
                .then(|| k.clone())
        })
    });
    let ad = tree["active_descendant_focus"]
        .as_str()
        .and_then(|k| nodes.and_then(|m| m.get(k)));
    (list_key, ad)
}

fn ad_pos(window: &Window) -> Option<u64> {
    read_tree(window).and_then(|t| find_list_and_ad(&t).1.and_then(|n| n["aria"]["position_in_set"].as_u64()))
}

/// 检查一帧的树：返回发现的问题（空 = 通过）以及摘要
fn check_tree(cfg: &Config, tree: &Value, expect: usize) -> (Vec<String>, Value) {
    let mut problems = Vec::new();
    let (list_key, ad) = find_list_and_ad(tree);
    match &list_key {
        None => problems.push("树里找不到 msg-list 容器".into()),
        Some(k) => {
            if tree["gpui_focus"].as_str() != Some(k.as_str()) {
                problems.push(format!("键盘焦点不在列表容器上：gpui_focus={}", tree["gpui_focus"]));
            }
        }
    }
    let want_label = label_for(cfg, expect);
    match ad {
        None => problems.push("没有 active_descendant".into()),
        Some(n) => {
            let a = &n["aria"];
            if a["label"].as_str() != Some(want_label.as_str()) {
                problems.push(format!("名称不对：{} ≠ {want_label}", a["label"]));
            }
            if a["position_in_set"].as_u64() != Some(expect as u64 + 1) {
                problems.push(format!("position_in_set={} 应为 {}", a["position_in_set"], expect + 1));
            }
            if a["selected"].as_bool() != Some(true) {
                problems.push("active_descendant 没标 selected".into());
            }
        }
    }
    let mut selected_count = 0;
    let mut positions: Vec<u64> = Vec::new();
    if let Some(m) = tree["nodes"].as_object() {
        for n in m.values() {
            let a = &n["aria"];
            if let Some(p) = a["position_in_set"].as_u64() {
                positions.push(p);
                if a["selected"].as_bool() == Some(true) {
                    selected_count += 1;
                }
            }
        }
    }
    if selected_count != 1 {
        problems.push(format!("selected 的项有 {selected_count} 个"));
    }
    let summary = json!({
        "node_count": tree["frame"]["node_count"],
        "frame_number": tree["frame"]["frame_number"],
        "rendered_items": positions.len(),
        "rendered_min": positions.iter().min(),
        "rendered_max": positions.iter().max(),
        "ad_label": ad.map(|n| n["aria"]["label"].clone()),
        "ad_pos": ad.map(|n| n["aria"]["position_in_set"].clone()),
    });
    (problems, summary)
}

fn key_sequence() -> Vec<&'static str> {
    let mut keys = Vec::new();
    keys.push("up"); // 顶端再上：应停在第 1 项
    keys.extend(std::iter::repeat_n("down", N - 1)); // 一路走到底
    keys.push("down"); // 底端再下：应停在最后一项
    keys.extend(std::iter::repeat_n("up", 30));
    keys.push("home");
    keys.push("end");
    keys.push("home");
    keys.extend(std::iter::repeat_n("pagedown", 95)); // 超过 1000/11，验证夹在末尾
    keys.extend(std::iter::repeat_n("pageup", 5));
    keys.push("cmd-up");
    keys.push("cmd-down");
    keys
}

fn read_tree(window: &Window) -> Option<Value> {
    window
        .debug_a11y_tree_json()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn frame_no(tree: &Option<Value>) -> u64 {
    tree.as_ref()
        .and_then(|t| t["frame"]["frame_number"].as_u64())
        .unwrap_or(0)
}

fn out_path(out: &PathBuf, suffix: &str) -> PathBuf {
    out.with_file_name(format!("{}{suffix}", out.file_name().unwrap().to_string_lossy()))
}

fn pct(v: &mut [f64], p: f64) -> Option<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.get(((v.len() as f64 - 1.0) * p) as usize).copied()
}

fn finish(out: &PathBuf, log: &[Value], summary: Value, fails: usize) {
    std::fs::create_dir_all(out.parent().unwrap_or(out)).ok();
    std::fs::write(out_path(out, ".jsonl"), log.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n")).ok();
    std::fs::write(out_path(out, "-summary.json"), serde_json::to_string_pretty(&summary).unwrap()).ok();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    for v in log.iter().filter(|v| v["problems"].as_array().is_some_and(|a| !a.is_empty())).take(10) {
        println!("FAIL step {} key {}: {}", v["step"], v["key"], v["problems"]);
    }
    std::process::exit(if fails == 0 { 0 } else { 1 });
}

fn run_key_selftest(cfg: Config, kind: SelfTest, out: PathBuf, window: &mut Window, cx: &mut Context<MessageList>) {
    cx.spawn_in(window, async move |_this, cx| {
        let started = Instant::now();
        let exec = cx.background_executor().clone();

        // 唤醒 AccessKit（进程内，无需权限），等树建好
        cx.update(|_, _| ax::activate()).unwrap();
        let mut ready = false;
        for _ in 0..100 {
            exec.timer(Duration::from_millis(30)).await;
            if cx.update(|w, _| w.is_a11y_active() && ad_pos(w).is_some()).unwrap() {
                ready = true;
                break;
            }
        }
        if !ready {
            eprintln!("[exp003b] 树没建起来，放弃");
            std::process::exit(2);
        }
        cx.update(|_, _| ax::make_key()).unwrap();
        exec.timer(Duration::from_millis(300)).await;

        let mut log: Vec<Value> = Vec::new();
        let mut fails = 0usize;
        let mut expect = 0usize;
        let mut latencies_ms: Vec<f64> = Vec::new();
        let mut not_key_steps = 0usize;
        let mut no_effect_steps = 0usize;
        let mut ax_mismatch = 0usize;
        let mut max_rendered = 0u64;

        for (step, key) in key_sequence().into_iter().enumerate() {
            let prev_frame = cx.update(|w, _| frame_no(&read_tree(w))).unwrap();
            let next = apply_key(expect, key);
            let is_key = cx.update(|_, _| ax::window_is_key()).unwrap();
            if is_key != Some(true) {
                not_key_steps += 1;
            }
            let t0 = Instant::now();
            if kind == SelfTest::Real {
                cx.update(|_, _| ax::post_key(key)).unwrap();
            } else {
                cx.update(|w, cx| {
                    w.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                })
                .unwrap();
            }
            let changed = next != expect;
            let mut new_frame = false;
            if changed {
                for _ in 0..200 {
                    exec.timer(Duration::from_millis(5)).await;
                    let ok = cx
                        .update(|w, _| frame_no(&read_tree(w)) > prev_frame && ad_pos(w) == Some(next as u64 + 1))
                        .unwrap();
                    if ok {
                        new_frame = true;
                        break;
                    }
                }
                latencies_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            } else {
                exec.timer(Duration::from_millis(30)).await;
            }
            // 没等到就以应用实际状态为准继续，免得一步错步步错
            if changed && !new_frame {
                no_effect_steps += 1;
                expect = cx.update(|w, _| ad_pos(w)).unwrap().map(|p| p as usize - 1).unwrap_or(expect);
            } else {
                expect = next;
            }
            let Some(tree) = cx.update(|w, _| read_tree(w)).unwrap() else {
                fails += 1;
                log.push(json!({"step": step, "key": key, "problems": ["拿不到树"]}));
                continue;
            };
            let (mut problems, summary) = check_tree(&cfg, &tree, expect);
            if changed && !new_frame {
                problems.push(format!("1 秒内没等到选中第 {} 项（按键没生效）", next + 1));
            }
            max_rendered = max_rendered.max(summary["rendered_items"].as_u64().unwrap_or(0));
            let probe = cx.update(|_, _| ax::probe()).unwrap();
            if probe["focused"]["title"].as_str() != Some(label_for(&cfg, expect).as_str()) {
                ax_mismatch += 1;
            }
            if !problems.is_empty() {
                fails += 1;
            }
            log.push(json!({"step": step, "key": key, "expect": expect + 1, "window_is_key": is_key,
                            "problems": problems, "tree": summary, "ax_title": probe["focused"]["title"]}));
        }

        let summary = json!({
            "mode": cfg.mode_name(),
            "key_source": if kind == SelfTest::Real { "AppKit 真实事件（CGEvent→NSEvent→NSApp postEvent）" } else { "GPUI 内部 dispatch_keystroke" },
            "steps": log.len(),
            "failed_steps": fails,
            "keys_without_effect": no_effect_steps,
            "window_not_key_steps": not_key_steps,
            "max_rendered_items_in_any_frame": max_rendered,
            "macos_focused_title_mismatch_steps": ax_mismatch,
            "key_to_frame_ms": {"n": latencies_ms.len(), "p50": pct(&mut latencies_ms, 0.5), "p95": pct(&mut latencies_ms, 0.95), "max": pct(&mut latencies_ms, 1.0)},
            "elapsed_s": started.elapsed().as_secs_f64(),
        });
        finish(&out, &log, summary, fails);
    })
    .detach();
}

// ---------------------------------------------------------------- 自测：模拟 VO+→ / VO+←

/// 从第 1 项起模拟 VO+→ 逐项往下，走不动为止；再从停下处模拟 VO+← 往回走。
/// 每落到一项，照 VoiceOver 的做法：该项支持 AXScrollToVisible 就执行，可设焦点就设焦点。
/// 「走得动」= 当前项在 macOS 层列表的子元素里还有下一个兄弟。
fn run_vo_selftest(cfg: Config, out: PathBuf, window: &mut Window, cx: &mut Context<MessageList>) {
    cx.spawn_in(window, async move |this, cx| {
        let started = Instant::now();
        let exec = cx.background_executor().clone();
        cx.update(|_, _| ax::activate()).unwrap();
        let mut ready = false;
        for _ in 0..100 {
            exec.timer(Duration::from_millis(30)).await;
            if cx.update(|w, _| w.is_a11y_active() && ad_pos(w).is_some()).unwrap() {
                ready = true;
                break;
            }
        }
        if !ready {
            eprintln!("[exp003b] 树没建起来，放弃");
            std::process::exit(2);
        }
        cx.update(|_, _| ax::make_key()).unwrap();
        exec.timer(Duration::from_millis(300)).await;

        let index_of = |t: &str| (0..N).find(|&i| label_for(&cfg, i) == t);
        let mut log: Vec<Value> = Vec::new();
        let mut calls = std::collections::BTreeMap::<String, usize>::new();
        let mut max_children = 0usize;
        let mut ends = [0usize; 2];
        let mut steps_by_dir = [0usize; 2];
        let mut cur = 0usize;

        for (d, dir) in [1i64, -1].into_iter().enumerate() {
            let mut steps = 0usize;
            loop {
                let titles = cx.update(|_, _| ax::list_child_titles()).unwrap().unwrap_or_default();
                max_children = max_children.max(titles.len());
                let here = label_for(&cfg, cur);
                let Some(pos) = titles.iter().position(|t| *t == here) else {
                    log.push(json!({"dir": dir, "at": cur + 1, "stop": "当前项已不在 macOS 层列表里", "children": titles.len()}));
                    break;
                };
                let np = pos as i64 + dir;
                if np < 0 || np as usize >= titles.len() {
                    log.push(json!({"dir": dir, "at": cur + 1, "stop": "列表里没有下一个兄弟了", "children": titles.len()}));
                    break;
                }
                let next_title = titles[np as usize].clone();
                let did = cx.update(|_, _| ax::vo_land_on(&next_title)).unwrap().unwrap_or_default();
                for c in &did {
                    *calls.entry(c.to_string()).or_default() += 1;
                }
                exec.timer(Duration::from_millis(40)).await; // 等一两帧，让滚动 / 同步生效
                let Some(nix) = index_of(&next_title) else {
                    log.push(json!({"dir": dir, "at": cur + 1, "stop": format!("不认识的标题 {next_title}")}));
                    break;
                };
                cur = nix;
                steps += 1;
                if steps % 100 == 0 || steps <= 2 {
                    let sel = this.update(cx, |v, _| v.selected).unwrap_or(usize::MAX);
                    log.push(json!({"dir": dir, "step": steps, "at": cur + 1, "did": did,
                                    "children": titles.len(), "app_selected": sel.wrapping_add(1)}));
                }
                if steps > N + 5 {
                    break;
                }
            }
            ends[d] = cur;
            steps_by_dir[d] = steps;
        }

        let selected_end = this.update(cx, |v, _| v.selected).unwrap_or(usize::MAX);
        let fails = usize::from(ends[0] != N - 1) + usize::from(ends[1] != 0);
        let summary = json!({
            "mode": cfg.mode_name(),
            "what": "模拟 VO+→ 从第 1 项走到走不动，再 VO+← 走回去",
            "forward_reached": ends[0] + 1,
            "forward_steps": steps_by_dir[0],
            "backward_reached": ends[1] + 1,
            "backward_steps": steps_by_dir[1],
            "vo_calls_made": calls,
            "max_macos_list_children": max_children,
            "app_selected_at_end": selected_end.wrapping_add(1),
            "passed": fails == 0,
            "elapsed_s": started.elapsed().as_secs_f64(),
        });
        finish(&out, &log, summary, fails);
    })
    .detach();
}

// ---------------------------------------------------------------- 自测：Tab 进出列表 + ↑/↓ + VO 落到列表

/// 当前 GPUI 焦点所在节点的名称，和列表的选中项（1 起）
fn focus_state(window: &Window) -> (Option<String>, Option<u64>) {
    let Some(t) = read_tree(window) else { return (None, None) };
    let label = t["gpui_focus"]
        .as_str()
        .and_then(|k| t["nodes"][k]["aria"]["label"].as_str())
        .map(str::to_string);
    let sel = t["nodes"]
        .as_object()
        .and_then(|m| m.values().find(|n| n["aria"]["selected"].as_bool() == Some(true)))
        .and_then(|n| n["aria"]["position_in_set"].as_u64());
    (label, sel)
}

/// 一步：做什么 → 期望焦点名称、期望选中项
enum TabStep {
    Key(&'static str),
    VoFocusList,
}

fn tab_script(list_name: &str) -> Vec<(TabStep, String, usize, &'static str)> {
    use TabStep::*;
    let l = list_name.to_string();
    let (h, sr, inp, snd) = ("会话：王小明".to_string(), "搜索".to_string(), "输入消息（占位）".to_string(), "发送".to_string());
    let mut v = vec![
        (Key("tab"), sr.clone(), 1, "Tab 1：会话标题 → 搜索"),
        (Key("tab"), l.clone(), 1, "Tab 2：→ 消息列表"),
        (Key("tab"), inp.clone(), 1, "Tab 3：→ 输入框（离开列表）"),
        (Key("tab"), snd.clone(), 1, "Tab 4：→ 发送"),
        (Key("tab"), h.clone(), 1, "Tab 5：绕回会话标题"),
        (Key("shift-tab"), snd.clone(), 1, "Shift+Tab：反向 → 发送"),
        (Key("shift-tab"), inp.clone(), 1, "Shift+Tab：→ 输入框"),
        (Key("shift-tab"), l.clone(), 1, "Shift+Tab：→ 回到列表"),
    ];
    let mut sel = 0usize;
    let mut keys: Vec<&'static str> = std::iter::repeat_n("down", 20).collect();
    keys.extend(["up", "up", "up", "pagedown", "end", "down", "home", "up", "pagedown"]);
    for k in keys {
        sel = apply_key(sel, k);
        v.push((Key(k), l.clone(), sel + 1, "列表内按键"));
    }
    v.push((Key("tab"), inp.clone(), sel + 1, "Tab 离开列表 → 输入框"));
    v.push((Key("down"), inp.clone(), sel + 1, "焦点在输入框时按 ↓：不该动列表"));
    v.push((Key("shift-tab"), l.clone(), sel + 1, "Shift+Tab 回列表：选中项保持"));
    v.push((Key("down"), l.clone(), apply_key(sel, "down") + 1, "回来后 ↓ 接着走"));
    sel = apply_key(sel, "down");
    v.push((Key("tab"), inp.clone(), sel + 1, "再离开到输入框"));
    v.push((VoFocusList, l.clone(), sel + 1, "模拟 VO 光标落到列表（键盘焦点跟随）"));
    v.push((Key("down"), l.clone(), apply_key(sel, "down") + 1, "VO 落到列表后 ↓ 生效"));
    v
}

fn run_tab_selftest(cfg: Config, out: PathBuf, window: &mut Window, cx: &mut Context<MessageList>) {
    cx.spawn_in(window, async move |_this, cx| {
        let started = Instant::now();
        let exec = cx.background_executor().clone();
        cx.update(|_, _| ax::activate()).unwrap();
        let mut ready = false;
        for _ in 0..100 {
            exec.timer(Duration::from_millis(30)).await;
            if cx.update(|w, _| w.is_a11y_active() && focus_state(w).0.is_some()).unwrap() {
                ready = true;
                break;
            }
        }
        if !ready {
            eprintln!("[exp003b] 树没建起来，放弃");
            std::process::exit(2);
        }
        cx.update(|_, _| ax::make_key()).unwrap();
        exec.timer(Duration::from_millis(300)).await;

        let list_name = if cfg.list_count { format!("消息，共 {N} 条") } else { "消息".to_string() };
        let stops = cx.update(|_, _| ax::vo_stops()).unwrap();
        let list_is_stop = stops.iter().any(|(r, t)| r == "AXList" && *t == list_name);
        let mut log: Vec<Value> = vec![json!({"step": -1, "what": "VO+←/→ 在窗口里会经过的元素（macOS 层）",
            "stops": stops.iter().map(|(r, t)| format!("{r}「{t}」")).collect::<Vec<_>>(),
            "problems": if list_is_stop { json!([]) } else { json!(["列表不是 VO 的一站"]) }})];
        let mut fails = usize::from(!list_is_stop);
        let mut not_key = 0usize;
        let mut lat: Vec<f64> = Vec::new();

        for (step, (act, want_focus, want_sel, what)) in tab_script(&list_name).into_iter().enumerate() {
            if cx.update(|_, _| ax::window_is_key()).unwrap() != Some(true) {
                not_key += 1;
            }
            let before = cx.update(|w, _| focus_state(w)).unwrap();
            let t0 = Instant::now();
            let act_name = match act {
                TabStep::Key(k) => {
                    cx.update(|_, _| ax::post_key(k)).unwrap();
                    k.to_string()
                }
                TabStep::VoFocusList => {
                    let r = cx.update(|_, _| ax::vo_focus_list()).unwrap();
                    format!("vo_focus_list(allowed={})", r.0)
                }
            };
            let want = (Some(want_focus.clone()), Some(want_sel as u64));
            let mut got = before.clone();
            let mut ok = false;
            for _ in 0..200 {
                exec.timer(Duration::from_millis(5)).await;
                got = cx.update(|w, _| focus_state(w)).unwrap();
                if got == want {
                    ok = true;
                    break;
                }
            }
            if ok && got != before {
                lat.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            // 期望「不变」的步：多等一会儿，确认确实没动
            if ok && got == before {
                exec.timer(Duration::from_millis(80)).await;
                got = cx.update(|w, _| focus_state(w)).unwrap();
                ok = got == want;
            }
            let problems: Vec<String> = if ok { vec![] } else {
                vec![format!("期望 焦点「{}」选中 {}，实际 焦点「{}」选中 {:?}", want_focus, want_sel,
                             got.0.clone().unwrap_or_default(), got.1)]
            };
            if !ok {
                fails += 1;
            }
            log.push(json!({"step": step, "key": act_name, "what": what, "focus": got.0, "selected": got.1,
                            "list_focused_flag": LIST_FOCUSED.load(std::sync::atomic::Ordering::SeqCst),
                            "problems": problems}));
        }
        let summary = json!({
            "mode": cfg.mode_name(),
            "key_source": "AppKit 真实事件（CGEvent→NSEvent→NSApp postEvent），不经过 VoiceOver",
            "vo_stops": stops.iter().map(|(r, t)| format!("{r}「{t}」")).collect::<Vec<_>>(),
            "list_is_vo_stop": list_is_stop,
            "steps": log.len(),
            "failed_steps": fails,
            "window_not_key_steps": not_key,
            "key_to_frame_ms": {"n": lat.len(), "p50": pct(&mut lat, 0.5), "p95": pct(&mut lat, 0.95), "max": pct(&mut lat, 1.0)},
            "elapsed_s": started.elapsed().as_secs_f64(),
        });
        finish(&out, &log, summary, fails);
    })
    .detach();
}

// ---------------------------------------------------------------- 入口

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("gpui", log::LevelFilter::Info)
        .init();
    let mut cfg = Config::from_args();
    if cfg.selftest.is_none() && cfg.log.is_none() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        cfg.log = Some(PathBuf::from(format!("out/live-{ts}.jsonl")));
    }
    if let Some(p) = &cfg.log {
        ax::open_log(p);
        eprintln!("[exp003b] 现场记录 → {}", p.display());
    }
    eprintln!("[exp003b] 模式 {}", cfg.mode_name());
    ax::log_event(json!({"ev": "start", "mode": cfg.mode_name()}));

    application().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("down", SelectNext, Some("MessageList")),
            KeyBinding::new("up", SelectPrev, Some("MessageList")),
            KeyBinding::new("home", SelectFirst, Some("MessageList")),
            KeyBinding::new("end", SelectLast, Some("MessageList")),
            KeyBinding::new("cmd-up", SelectFirst, Some("MessageList")),
            KeyBinding::new("cmd-down", SelectLast, Some("MessageList")),
            KeyBinding::new("pagedown", PageDown, Some("MessageList")),
            KeyBinding::new("pageup", PageUp, Some("MessageList")),
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
        // GPUI 认出了哪个按键、落到了哪个动作
        cx.observe_keystrokes(|ev, _, _| {
            ax::log_event(json!({"ev": "gpui_keystroke", "key": ev.keystroke.key.clone(),
                                 "cmd": ev.keystroke.modifiers.platform,
                                 "action": ev.action.as_ref().map(|a| a.name())}));
        })
        .detach();

        // AppKit 层看到的每个按键；key_fix 时把方向键类直接交给 GPUI，不经过输入法
        let (tx, rx) = async_channel::unbounded::<&'static str>();
        KEY_TX.set(tx).ok();
        let key_fix = cfg.key_fix;
        ax::install_key_monitor(move |raw| {
            let name = ax::key_name(raw.code, raw.flags);
            // Tab / Shift+Tab 任何时候都截；↑/↓ 等只在列表有焦点时截
            let applies = match name {
                Some("tab" | "shift-tab") => true,
                Some(_) => LIST_FOCUSED.load(std::sync::atomic::Ordering::SeqCst),
                None => false,
            };
            let take = key_fix && applies && raw.window_is_key == Some(true);
            ax::log_event(json!({"ev": "raw_key", "code": raw.code, "flags": format!("{:#x}", raw.flags),
                                 "chars": raw.chars, "repeat": raw.repeat, "window_is_key": raw.window_is_key,
                                 "name": name, "bypass_ime": take}));
            if take
                && let (Some(tx), Some(n)) = (KEY_TX.get(), name)
            {
                tx.try_send(n).ok();
            }
            take
        });

        let bounds = Bounds::centered(None, size(px(520.), px(ITEM_H * VISIBLE as f32 + 40.)), cx);
        let cfg2 = cfg.clone();
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("exp003 消息列表".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let view = MessageList::new(cfg2.clone(), window, cx);
                        match cfg2.selftest.clone() {
                            Some((SelfTest::Vo, out)) => run_vo_selftest(cfg2.clone(), out, window, cx),
                            Some((SelfTest::Tab, out)) => run_tab_selftest(cfg2.clone(), out, window, cx),
                            Some((kind, out)) => run_key_selftest(cfg2.clone(), kind, out, window, cx),
                            None => {}
                        }
                        view
                    })
                },
            )
            .unwrap();
        cx.spawn(async move |cx| {
            while let Ok(key) = rx.recv().await {
                // 用 AnyWindowHandle：不借出根视图，否则动作回调里再更新 MessageList 会重入
                cx.update_window(handle.into(), |_, w, cx| {
                    w.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                })
                .ok();
            }
        })
        .detach();
        cx.activate(true);
    });
}
