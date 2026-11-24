//! exp009 · Zed 主干 15d136b 上 GPUI 无障碍的五个小问题，进程内自测（假数据；本程序不去开 VoiceOver，VO 开着时 run.sh 拒跑）。
//!
//! 1. 播报：只设值 / 只设名称 / 两者都设，在 accesskit_macos 0.26.3 下各发不发公告
//! 2. 变高 `list` + 先滚后选 + active descendant：选中项每次都进树、焦点在列表
//! 3. 应用级焦点（上游 #66691）：contentView 与 NSApp 各答什么；装上权宜做法后 NSApp 答什么
//! 4. 输入框（TextInput + TextRun + 选区）：移光标发不发 AXSelectedTextChanged、带不带 userInfo
//! 5. 缓存视图（上游 #66690）：只重画根视图后，缓存视图里的按钮还在不在树里
//!
//! 跑法见 README。结果写 `out/result-<模式>.json`，最后一帧的树写 `out/tree-end-<模式>.json`。

mod ax;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, AsyncWindowContext, Bounds, Context, Entity, FocusHandle, ListAlignment, ListOffset, ListState, Role,
    SharedString, StyleRefinement, WeakEntity, Window, WindowBounds, WindowOptions, accesskit, div,
    list, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::sync::Mutex;

/// 外部无障碍客户端发来的「设焦点」请求：(距启动毫秒, 目标)
static A11Y_FOCUS: Mutex<Vec<(u128, &'static str)>> = Mutex::new(Vec::new());
static START: Mutex<Option<std::time::Instant>> = Mutex::new(None);

fn log_focus(target: &'static str) {
    let ms = START.lock().unwrap().map_or(0, |t| t.elapsed().as_millis());
    A11Y_FOCUS.lock().unwrap().push((ms, target));
}
use serde_json::{Value, json};

const MSG_COUNT: usize = 300;
const COMPOSER_TEXT: &str = "你好 world 测试一下";

#[derive(Clone, Copy, Debug)]
enum Field {
    Value,
    Label,
    Both,
}

struct Pane;

impl Render for Pane {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("cached-btn")
            .role(Role::Button)
            .aria_label("缓存视图里的按钮")
            .child("缓存视图里的按钮")
    }
}

struct Demo {
    mode: &'static str,
    list_focus: FocusHandle,
    composer_focus: FocusHandle,
    list_state: ListState,
    selected: Rc<RefCell<usize>>,
    caret: usize,
    say: Option<(String, Field, usize)>,
    pane: Entity<Pane>,
    ticks: usize,
}

fn msg_label(ix: usize) -> String {
    format!("发送人{}：第 {} 条假消息", ix % 7, ix + 1)
}

impl Demo {
    fn new(mode: &'static str, cx: &mut Context<Self>) -> Self {
        let overdraw = if mode == "step" { px(200.) } else { px(0.) };
        let state = ListState::new(MSG_COUNT, ListAlignment::Top, overdraw);
        let state = if mode == "measure" { state.measure_all() } else { state };
        Self {
            mode,
            list_focus: cx.focus_handle(),
            composer_focus: cx.focus_handle(),
            list_state: state,
            selected: Rc::new(RefCell::new(0)),
            caret: 0,
            say: None,
            pane: cx.new(|_| Pane),
            ticks: 0,
        }
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        *self.selected.borrow_mut() = ix;
        if self.mode == "scrollto" {
            self.list_state.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
        } else {
            self.list_state.scroll_to_reveal_item(ix);
        }
        cx.notify();
    }
}

impl Render for Demo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let list_fh = self.list_focus.clone();
        let comp_fh = self.composer_focus.clone();
        let selected = self.selected.clone();
        let say = self.say.clone();
        let caret = self.caret;
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(format!("exp009 · tick {}", self.ticks))
            // 1. 播报节点：键随每次播报变（换节点绕法），按模式只设值 / 只设名称 / 两者
            .child(
                div()
                    .id(("announcer", say.as_ref().map_or(0, |s| s.2)))
                    .role(Role::Status)
                    .a11y_synthetic_children(move |b| {
                        let n = b.parent_node();
                        n.set_live(accesskit::Live::Polite);
                        if let Some((text, field, _)) = &say {
                            match field {
                                Field::Value => n.set_value(text.clone()),
                                Field::Label => n.set_label(text.clone()),
                                Field::Both => {
                                    n.set_value(text.clone());
                                    n.set_label(text.clone());
                                }
                            }
                        }
                    })
                    .child("播报区"),
            )
            // 2. 变高消息列表：容器持焦点，选中项 active descendant
            .child(
                div()
                    .id("messages")
                    .role(Role::List)
                    .aria_label("消息")
                    .track_focus(&self.list_focus)
                    .on_a11y_action(gpui::AccessibleAction::Focus, move |_, window, cx| {
                        log_focus("list");
                        window.focus(&list_fh, cx);
                    })
                    .h(px(220.))
                    .child(
                        list(self.list_state.clone(), move |ix, _w, _cx| {
                            let sel = *selected.borrow() == ix;
                            let lines = ix % 3 + 1;
                            div()
                                .id(("msg", ix))
                                .role(Role::ListItem)
                                .aria_label(SharedString::from(msg_label(ix)))
                                .aria_selected(sel)
                                .when(sel, |d| d.aria_active_descendant().bg(rgb(0x45475a)))
                                .children((0..lines).map(move |l| {
                                    div().child(if l == 0 { msg_label(ix) } else { format!("第 {} 行", l + 1) })
                                }))
                                .into_any_element()
                        })
                        .size_full(),
                    ),
            )
            // 4. 输入框：照 Zed 设置页的结构自写（TextInput + 一个 TextRun + 选区）
            .child(
                div()
                    .id("composer")
                    .role(Role::TextInput)
                    .aria_label("输入消息")
                    .aria_value(COMPOSER_TEXT)
                    .track_focus(&self.composer_focus)
                    .on_a11y_action(gpui::AccessibleAction::Focus, move |_, window, cx| {
                        log_focus("composer");
                        window.focus(&comp_fh, cx);
                    })
                    .a11y_synthetic_children(move |b| {
                        let mut run = accesskit::Node::new(Role::TextRun);
                        run.set_value(COMPOSER_TEXT);
                        run.set_character_lengths(
                            COMPOSER_TEXT.chars().map(|c| c.len_utf8() as u8).collect::<Vec<u8>>(),
                        );
                        let id = b.synthetic_node_id(0u64);
                        b.push_child(id, run);
                        let pos = accesskit::TextPosition { node: id, character_index: caret };
                        b.parent_node().set_text_selection(accesskit::TextSelection { anchor: pos, focus: pos });
                    })
                    .child(format!("输入框（光标 {caret}）：{COMPOSER_TEXT}")),
            )
            // 5. 缓存视图
            .child(self.pane.clone().cached(StyleRefinement::default()))
    }
}

fn tree(this: &WeakEntity<Demo>, cx: &mut AsyncWindowContext) -> Value {
    let s = this
        .update_in(cx, |_, window, _| window.debug_a11y_tree_json())
        .ok()
        .flatten()
        .unwrap_or_default();
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn has_label(t: &Value, label: &str) -> bool {
    t["nodes"].as_object().is_some_and(|m| m.values().any(|n| n["aria"]["label"] == label))
}

/// GPUI 认为的焦点节点（active descendant 优先）的名称
fn focus_label(t: &Value) -> Option<String> {
    let key = t["active_descendant_focus"].as_str().or(t["gpui_focus"].as_str())?;
    Some(t["nodes"][key]["aria"]["label"].as_str().unwrap_or("").to_string())
}

async fn run(this: WeakEntity<Demo>, cx: &mut AsyncWindowContext) {
    let ex = cx.background_executor().clone();
    let wait = move |ms| ex.timer(Duration::from_millis(ms));
    let mut out = serde_json::Map::new();
    wait(800).await;
    out.insert("activate_children".into(), json!(ax::activate()));
    wait(500).await;
    out.insert("window_is_key".into(), json!(ax::window_is_key()));
    out.insert("voiceover_enabled".into(), json!(ax::voiceover_enabled()));

    // 1. 播报
    let mut announce = vec![];
    for (k, field) in [Field::Value, Field::Label, Field::Both].into_iter().enumerate() {
        let text = format!("播报测试 {field:?}");
        let m = ax::mark();
        this.update(cx, |d, cx| {
            d.say = Some((text.clone(), field, k + 1));
            cx.notify();
        })
        .ok();
        wait(400).await;
        let got: Vec<_> = ax::since(m).into_iter().filter(|n| n.name == "AXAnnouncementRequested").map(|n| n.text).collect();
        announce.push(json!({"field": format!("{field:?}"), "announcements": got}));
    }
    out.insert("announce".into(), json!(announce));

    // 2. 变高 list：先滚后选
    this.update_in(cx, |d, window, cx| window.focus(&d.list_focus, cx)).ok();
    wait(200).await;
    let mut steps = vec![];
    let mut fails = 0;
    let mode = this.update(cx, |d, _| d.mode).unwrap_or("reveal");
    let seq: Vec<usize> = if mode == "step" { (1..=60).chain((0..60).rev()).collect() } else { vec![1, 5, 40, 41, 120, 299, 298, 150, 0] };
    out.insert("list_mode".into(), json!(mode));
    for ix in seq {
        this.update(cx, |d, cx| d.select(ix, cx)).ok();
        wait(120).await;
        let t = tree(&this, cx);
        let ok = focus_label(&t).as_deref() == Some(msg_label(ix).as_str()) && has_label(&t, &msg_label(ix));
        fails += usize::from(!ok);
        if !ok || mode != "step" {
            steps.push(json!({"t_ms": START.lock().unwrap().map_or(0, |t| t.elapsed().as_millis()), "select": ix, "ok": ok, "focus": focus_label(&t), "gpui_focus": t["gpui_focus"], "ad": t["active_descendant_focus"], "item_in_tree": has_label(&t, &msg_label(ix)), "nodes": t["frame"]["node_count"]}));
        }
    }
    out.insert("list".into(), json!({"steps": steps, "fails": fails}));

    // 3. 应用级焦点（#66691）
    this.update_in(cx, |d, window, cx| window.focus(&d.composer_focus, cx)).ok();
    wait(300).await;
    let before = json!({"content_view": ax::focus_from_content_view(), "app": ax::focus_from_app(), "first_responder": ax::first_responder_class()});
    let installed = ax::install_focus_workaround();
    wait(100).await;
    let after = json!({"content_view": ax::focus_from_content_view(), "app": ax::focus_from_app()});
    out.insert("app_focus".into(), json!({"before": before, "workaround_installed": installed, "after": after}));

    // 4. 输入框移光标
    let m = ax::mark();
    for c in [1usize, 2, 3, 5] {
        this.update(cx, |d, cx| {
            d.caret = c;
            cx.notify();
        })
        .ok();
        wait(150).await;
    }
    let sel: Vec<_> = ax::since(m).into_iter().filter(|n| n.name.contains("SelectedText")).map(|n| json!({"name": n.name, "user_info": n.has_user_info})).collect();
    out.insert("caret_moves".into(), json!({"moves": 4, "notifications": sel}));

    // 5. 缓存视图（#66690）
    let t0 = has_label(&tree(&this, cx), "缓存视图里的按钮");
    this.update(cx, |d, cx| {
        d.ticks += 1;
        cx.notify();
    })
    .ok();
    wait(200).await;
    let t1 = has_label(&tree(&this, cx), "缓存视图里的按钮");
    this.update(cx, |d, cx| d.pane.update(cx, |_, cx| cx.notify())).ok();
    wait(200).await;
    let t2 = has_label(&tree(&this, cx), "缓存视图里的按钮");
    out.insert("cached_view".into(), json!({"initial": t0, "after_root_redraw": t1, "after_pane_redraw": t2}));
    out.insert("external_focus_requests".into(), json!(A11Y_FOCUS.lock().unwrap().clone()));
    out.insert("voiceover_enabled_end".into(), json!(ax::voiceover_enabled()));

    let end = tree(&this, cx);
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/out");
    let mode = this.update(cx, |d, _| d.mode).unwrap_or("reveal");
    std::fs::create_dir_all(dir).ok();
    std::fs::write(format!("{dir}/tree-end-{mode}.json"), serde_json::to_string_pretty(&end).unwrap()).ok();
    let result = Value::Object(out);
    std::fs::write(format!("{dir}/result-{mode}.json"), serde_json::to_string_pretty(&result).unwrap()).ok();
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    this.update(cx, |_, cx| cx.quit()).ok();
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("gpui", log::LevelFilter::Info)
        .init();
    let mode: &'static str = match std::env::args().skip_while(|a| a != "--list-mode").nth(1).as_deref() {
        Some("measure") => "measure",
        Some("scrollto") => "scrollto",
        Some("step") => "step",
        _ => "reveal",
    };
    *START.lock().unwrap() = Some(std::time::Instant::now());
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.), px(460.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions { title: Some("exp009".into()), ..Default::default() }),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    cx.spawn_in(window, async move |this, cx| run(this, cx).await).detach();
                    Demo::new(mode, cx)
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
