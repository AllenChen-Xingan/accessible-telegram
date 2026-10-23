//! exp003-list-nav：1000 条假消息的虚拟化列表，由应用自己的方向键驱动。
//!
//! 容器持焦点 + 选中项 `aria_active_descendant`；按键先 `scroll_to_item` 再选中，
//! 两件事落在同一帧里（uniform_list 在 prepaint 时处理延迟滚动，见
//! `uniform_list.rs:440-492`），所以被选中的那一项一定已被渲染、进了树。
//!
//! 运行：
//!   cargo run                         # 人工试用（默认：List/ListItem，位置只放 aria 属性）
//!   cargo run -- --pos-in-label       # 对照：把「第几项，共几项」也写进名称
//!   cargo run -- --listbox            # 对照：ListBox/ListBoxOption 角色
//!   cargo run -- --selftest out/x     # 自动验证：模拟按键，逐步导出树并检查，结果写 out/x*

mod ax;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Keystroke, Role, ScrollStrategy, SharedString,
    UniformListScrollHandle, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px,
    rgb, size, uniform_list,
};
use gpui_platform::application;
use serde_json::{Value, json};

actions!(
    exp003,
    [SelectNext, SelectPrev, SelectFirst, SelectLast, PageDown, PageUp]
);

const N: usize = 1000;
const ITEM_H: f32 = 28.0;
const VISIBLE: usize = 12;
/// 翻一页移动的条数：可见条数减一，留一条上下文
const PAGE: usize = VISIBLE - 1;

#[derive(Clone, Debug)]
struct Config {
    listbox: bool,
    pos_in_label: bool,
    selftest: Option<PathBuf>,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut cfg = Config {
            listbox: false,
            pos_in_label: false,
            selftest: None,
        };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--listbox" => cfg.listbox = true,
                "--pos-in-label" => cfg.pos_in_label = true,
                "--selftest" => {
                    i += 1;
                    cfg.selftest = Some(PathBuf::from(
                        args.get(i).cloned().unwrap_or_else(|| "out/selftest".into()),
                    ));
                }
                other => eprintln!("忽略未知参数 {other}"),
            }
            i += 1;
        }
        cfg
    }

    fn mode_name(&self) -> String {
        format!(
            "{}+{}",
            if self.listbox { "listbox" } else { "list" },
            if self.pos_in_label { "pos-in-label" } else { "pos-attr" }
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

/// 最简读法：「发送人，内容，时间」
fn base_label(ix: usize) -> String {
    let sender = SENDERS[ix % SENDERS.len()];
    let body = BODIES[(ix * 3) % BODIES.len()];
    let minutes = ix % (24 * 60);
    format!(
        "{sender}，第{}条：{body}，{:02}:{:02}",
        ix + 1,
        minutes / 60,
        minutes % 60
    )
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

struct MessageList {
    cfg: Config,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    selected: usize,
}

impl MessageList {
    fn new(cfg: Config, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            cfg,
            focus,
            scroll: UniformListScrollHandle::new(),
            selected: 0,
        }
    }

    /// 先把目标项滚进可见区，再选中；两步在同一帧生效
    fn move_by_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let next = apply_key(self.selected, key);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        if next != self.selected {
            self.selected = next;
            cx.notify();
        }
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
        div()
            .id("root")
            .size_full()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(
                div()
                    .id("msg-list")
                    .role(list_role)
                    .aria_label(SharedString::from(format!("消息列表，共 {N} 条")))
                    .key_context("MessageList")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_by_key("down", cx)))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_by_key("up", cx)))
                    .on_action(cx.listener(|this, _: &SelectFirst, _, cx| this.move_by_key("home", cx)))
                    .on_action(cx.listener(|this, _: &SelectLast, _, cx| this.move_by_key("end", cx)))
                    .on_action(cx.listener(|this, _: &PageDown, _, cx| this.move_by_key("pagedown", cx)))
                    .on_action(cx.listener(|this, _: &PageUp, _, cx| this.move_by_key("pageup", cx)))
                    .w_full()
                    .h(px(ITEM_H * VISIBLE as f32))
                    .child(
                        uniform_list(
                            "msg-uniform",
                            N,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                                range
                                    .map(|ix| {
                                        let label = label_for(&this.cfg, ix);
                                        let is_sel = ix == this.selected;
                                        div()
                                            .id(("msg", ix))
                                            .role(item_role)
                                            .aria_label(SharedString::from(label.clone()))
                                            .aria_position_in_set(ix + 1)
                                            .aria_size_of_set(N)
                                            .aria_selected(is_sel)
                                            .when(is_sel, |d| d.aria_active_descendant().bg(rgb(0x45475a)))
                                            .h(px(ITEM_H))
                                            .px_2()
                                            .child(SharedString::from(label))
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .h_full(),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .child(SharedString::from(format!("选中 {} / {}", selected + 1, N))),
            )
    }
}

// ---------------------------------------------------------------- 自测

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

/// 检查一帧的树：返回发现的问题（空 = 通过）以及摘要
fn check_tree(cfg: &Config, tree: &Value, expect: usize) -> (Vec<String>, Value) {
    let mut problems = Vec::new();
    let (list_key, ad) = find_list_and_ad(tree);
    match &list_key {
        None => problems.push("树里找不到 msg-list 容器".into()),
        Some(k) => {
            if tree["gpui_focus"].as_str() != Some(k.as_str()) {
                problems.push(format!(
                    "键盘焦点不在列表容器上：gpui_focus={}",
                    tree["gpui_focus"]
                ));
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
            if a["size_of_set"].as_u64() != Some(N as u64) {
                problems.push(format!("size_of_set={} 应为 {N}", a["size_of_set"]));
            }
            if a["selected"].as_bool() != Some(true) {
                problems.push("active_descendant 没标 selected".into());
            }
        }
    }
    // 选中项唯一；顺便记下本帧渲染了哪些项（验证虚拟化）
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
        "ad_size": ad.map(|n| n["aria"]["size_of_set"].clone()),
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

fn run_selftest(cfg: Config, out: PathBuf, window: &mut Window, cx: &mut Context<MessageList>) {
    cx.spawn_in(window, async move |this, cx| {
        let started = Instant::now();
        let mut log: Vec<Value> = Vec::new();
        let exec = cx.background_executor().clone();
        let timer = move |ms: u64| exec.timer(Duration::from_millis(ms));

        // 0. 不开读屏时树是否已建？
        let before = cx.update(|w, _| (w.is_a11y_active(), w.debug_a11y_tree_json().is_some())).unwrap();
        eprintln!("[exp003] 激活前：is_a11y_active={} 有树={}", before.0, before.1);

        // 1. 用本进程内的 NSAccessibility 调用唤醒 AccessKit（不需要辅助功能权限）
        let activated_by = cx.update(|_, _| ax::activate()).unwrap();
        let mut active = false;
        for _ in 0..100 {
            timer(30).await;
            let ok = cx
                .update(|w, _| w.is_a11y_active() && read_tree(w).map(|t| find_list_and_ad(&t).1.is_some()).unwrap_or(false))
                .unwrap();
            if ok {
                active = true;
                break;
            }
        }
        eprintln!("[exp003] 激活：{activated_by}；树已就绪={active}");
        if !active {
            eprintln!("[exp003] 树没建起来，放弃");
            std::process::exit(2);
        }

        let first_tree = cx.update(|w, _| read_tree(w)).unwrap();
        let mut fails = 0usize;
        let mut expect = 0usize;
        let mut latencies_ms: Vec<f64> = Vec::new();
        let mut ax_mismatch = 0usize;
        let mut ax_sample: Option<Value> = None;
        let mut max_rendered = 0u64;
        let dumps_at = [0usize, 500, 1000];
        std::fs::create_dir_all(out.parent().unwrap_or(&out)).ok();
        if let Some(t) = &first_tree {
            let (p, s) = check_tree(&cfg, t, 0);
            log.push(json!({"step": -1, "key": "(start)", "expect": 0, "problems": p, "tree": s}));
            fails += p.len().min(1);
        }

        for (step, key) in key_sequence().into_iter().enumerate() {
            let prev_frame = cx.update(|w, _| frame_no(&read_tree(w))).unwrap();
            let next = apply_key(expect, key);
            let t0 = Instant::now();
            cx.update(|w, cx| {
                w.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
            })
            .unwrap();
            // 选中项变了才会有新帧；等它出来（最多 1 秒）
            let mut new_frame = false;
            let changed = next != expect;
            if changed {
                // 等到「有新帧且 active descendant 已是目标项」；别的原因触发的帧不算
                for _ in 0..200 {
                    timer(5).await;
                    let ok = cx
                        .update(|w, _| {
                            let t = read_tree(w);
                            frame_no(&t) > prev_frame
                                && t.as_ref().and_then(|t| find_list_and_ad(t).1.map(|n| n["aria"]["position_in_set"].as_u64()))
                                    == Some(Some(next as u64 + 1))
                        })
                        .unwrap();
                    if ok {
                        new_frame = true;
                        break;
                    }
                }
                latencies_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            } else {
                timer(30).await;
            }
            expect = next;
            let tree = cx.update(|w, _| read_tree(w)).unwrap();
            let Some(tree) = tree else {
                fails += 1;
                log.push(json!({"step": step, "key": key, "problems": ["拿不到树"]}));
                continue;
            };
            let (mut problems, summary) = check_tree(&cfg, &tree, expect);
            if changed && !new_frame {
                problems.push("1 秒内没等到选中目标项的新帧".into());
            }
            max_rendered = max_rendered.max(summary["rendered_items"].as_u64().unwrap_or(0));

            // macOS 那一层：VoiceOver 实际会问到的东西
            let probe = cx.update(|_, _| ax::probe()).unwrap();
            let want = label_for(&cfg, expect);
            if probe["focused"]["title"].as_str() != Some(want.as_str()) {
                ax_mismatch += 1;
            }
            if ax_sample.is_none() || step == 500 {
                ax_sample = Some(probe.clone());
            }
            if dumps_at.contains(&step) {
                let p = out.with_file_name(format!(
                    "{}-tree-step{step}.json",
                    out.file_name().unwrap().to_string_lossy()
                ));
                std::fs::write(&p, serde_json::to_string_pretty(&tree).unwrap()).ok();
            }
            if !problems.is_empty() {
                fails += 1;
            }
            log.push(json!({"step": step, "key": key, "expect": expect + 1,
                            "problems": problems, "tree": summary, "ax": probe}));
        }

        latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f64| latencies_ms.get(((latencies_ms.len() as f64 - 1.0) * p) as usize).copied();
        let summary = json!({
            "mode": cfg.mode_name(),
            "steps": log.len(),
            "failed_steps": fails,
            "a11y_active_before_probe": before.0,
            "tree_before_probe": before.1,
            "activated_by": activated_by,
            "max_rendered_items_in_any_frame": max_rendered,
            "macos_focused_title_mismatch_steps": ax_mismatch,
            "macos_probe_sample": ax_sample,
            "key_to_frame_ms": {"n": latencies_ms.len(), "p50": pct(0.5), "p95": pct(0.95), "max": pct(1.0)},
            "elapsed_s": started.elapsed().as_secs_f64(),
        });
        let log_path = out.with_file_name(format!("{}.jsonl", out.file_name().unwrap().to_string_lossy()));
        let sum_path = out.with_file_name(format!("{}-summary.json", out.file_name().unwrap().to_string_lossy()));
        std::fs::write(&log_path, log.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n")).ok();
        std::fs::write(&sum_path, serde_json::to_string_pretty(&summary).unwrap()).ok();
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        // 失败步列出前 10 条
        for v in log.iter().filter(|v| v["problems"].as_array().is_some_and(|a| !a.is_empty())).take(10) {
            println!("FAIL step {} key {}: {}", v["step"], v["key"], v["problems"]);
        }
        let _ = this;
        std::process::exit(if fails == 0 { 0 } else { 1 });
    })
    .detach();
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("gpui", log::LevelFilter::Info)
        .init();
    let cfg = Config::from_args();
    eprintln!("[exp003] 模式 {}", cfg.mode_name());
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
        ]);
        let bounds = Bounds::centered(None, size(px(520.), px(ITEM_H * VISIBLE as f32 + 40.)), cx);
        let cfg2 = cfg.clone();
        cx.open_window(
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
                    if let Some(out) = cfg2.selftest.clone() {
                        run_selftest(cfg2.clone(), out, window, cx);
                    }
                    view
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
