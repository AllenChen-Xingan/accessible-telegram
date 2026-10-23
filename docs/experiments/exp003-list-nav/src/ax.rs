//! 在本进程里直接调 NSAccessibility 方法，看 macOS 那一层（VoiceOver 实际会问到的）长什么样。
//!
//! 调的是本进程自己窗口的 contentView，不经过 AX 跨进程接口，所以**不需要辅助功能权限**。
//! 顺带作用：accesskit_macos 的 SubclassingAdapter 在 `accessibilityFocusedUIElement`
//! 被调用时激活（`subclass.rs:187`），于是没开 VoiceOver 也能让 GPUI 建树。

use std::ffi::{CStr, c_char};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, Bool, Sel};
use objc2::{class, msg_send, sel};
use serde_json::{Value, json};

fn responds(obj: &AnyObject, s: Sel) -> bool {
    let b: Bool = unsafe { msg_send![obj, respondsToSelector: s] };
    b.as_bool()
}

fn nsstring(obj: Option<Retained<AnyObject>>) -> Option<String> {
    let obj = obj?;
    let p: *const c_char = unsafe { msg_send![&obj, UTF8String] };
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

fn get_str(obj: &AnyObject, s: Sel) -> Option<String> {
    if !responds(obj, s) {
        return None;
    }
    let v: Option<Retained<AnyObject>> = unsafe { msg_send![obj, performSelector: s] };
    nsstring(v)
}

fn get_obj(obj: &AnyObject, s: Sel) -> Option<Retained<AnyObject>> {
    if !responds(obj, s) {
        return None;
    }
    unsafe { msg_send![obj, performSelector: s] }
}

fn array_len(arr: &Option<Retained<AnyObject>>) -> Option<usize> {
    arr.as_ref().map(|a| unsafe { msg_send![&**a, count] })
}

fn content_view() -> Option<Retained<AnyObject>> {
    unsafe {
        let app: Option<Retained<AnyObject>> = msg_send![class!(NSApplication), sharedApplication];
        let app = app?;
        let windows: Option<Retained<AnyObject>> = msg_send![&app, windows];
        let windows = windows?;
        let count: usize = msg_send![&windows, count];
        for i in 0..count {
            let win: Option<Retained<AnyObject>> = msg_send![&windows, objectAtIndex: i];
            let Some(win) = win else { continue };
            let view: Option<Retained<AnyObject>> = msg_send![&win, contentView];
            if view.is_some() {
                return view;
            }
        }
        None
    }
}

fn focused() -> Option<Retained<AnyObject>> {
    let view = content_view()?;
    unsafe { msg_send![&view, accessibilityFocusedUIElement] }
}

/// 唤醒 AccessKit；返回一句说明
pub fn activate() -> String {
    autoreleasepool(|_| match content_view() {
        None => "找不到 contentView".into(),
        Some(view) => {
            let _f: Option<Retained<AnyObject>> = unsafe { msg_send![&view, accessibilityFocusedUIElement] };
            "contentView.accessibilityFocusedUIElement（进程内，无需权限）".into()
        }
    })
}

/// 本窗口是不是 key window（AccessKit 只在窗口是 key 时才报焦点）
pub fn window_is_key() -> Option<bool> {
    autoreleasepool(|_| unsafe {
        let view = content_view()?;
        let win: Option<Retained<AnyObject>> = msg_send![&view, window];
        let b: Bool = msg_send![&win?, isKeyWindow];
        Some(b.as_bool())
    })
}

/// 读出 macOS 层的焦点元素：角色、标题、帮助、选中、位置相关 API 是否存在、父列表信息
pub fn probe() -> Value {
    autoreleasepool(|_| {
        let Some(f) = focused() else {
            return json!({"focused": null, "window_is_key": window_is_key()});
        };
        let selected = if responds(&f, sel!(isAccessibilitySelected)) {
            let b: Bool = unsafe { msg_send![&f, isAccessibilitySelected] };
            Some(b.as_bool())
        } else {
            None
        };
        // 「第几项共几项」在 macOS 上可能走的几个口子，记下有没有
        let pos_apis: Vec<&str> = [
            ("accessibilityIndex", sel!(accessibilityIndex)),
            ("accessibilityRowIndexRange", sel!(accessibilityRowIndexRange)),
            ("accessibilityRowCount", sel!(accessibilityRowCount)),
        ]
        .into_iter()
        .filter(|(_, s)| {
            // PlatformNode 用 isAccessibilitySelectorAllowed: 决定 VoiceOver 能不能用
            responds(&f, *s)
                && (!responds(&f, sel!(isAccessibilitySelectorAllowed:)) || {
                    let b: Bool = unsafe { msg_send![&f, isAccessibilitySelectorAllowed: *s] };
                    b.as_bool()
                })
        })
        .map(|(n, _)| n)
        .collect();
        let parent = get_obj(&f, sel!(accessibilityParent));
        let parent_info = parent.as_ref().map(|p| {
            let children = get_obj(p, sel!(accessibilityChildren));
            let sel_children = get_obj(p, sel!(accessibilitySelectedChildren));
            let rows = get_obj(p, sel!(accessibilityRows));
            json!({
                "role": get_str(p, sel!(accessibilityRole)),
                "title": get_str(p, sel!(accessibilityTitle)),
                "children": array_len(&children),
                "selected_children": array_len(&sel_children),
                "rows": array_len(&rows),
            })
        });
        json!({
            "window_is_key": window_is_key(),
            "focused": {
                "class": format!("{:?}", f.class()),
                "role": get_str(&f, sel!(accessibilityRole)),
                "role_description": get_str(&f, sel!(accessibilityRoleDescription)),
                "title": get_str(&f, sel!(accessibilityTitle)),
                "help": get_str(&f, sel!(accessibilityHelp)),
                "selected": selected,
                "position_apis_available": pos_apis,
            },
            "parent": parent_info,
        })
    })
}

// ================================================================ exp003b 追加
//
// 1. 真实按键：用 CGEvent 生成键盘事件，转成 NSEvent 投进本进程的 NSApp 事件队列。
//    之后走的是 AppKit 的真实路径：NSApp sendEvent → key window → GPUIView keyDown:
//    → 输入法 inputContext → GPUI 按键绑定。不经过 GPUI 内部的 dispatch_keystroke。
//    只投给本进程，不需要辅助功能权限；但也因此**不经过 VoiceOver**（VO 在系统事件层）。
// 2. 按键监视：NSEvent 本地监视器，记下本进程收到的每个按键；可选「绕过输入法」：
//    方向键类直接交给 GPUI（见 main.rs 的 key_fix）。
// 3. 模拟 VoiceOver 的 VO+→：在 macOS 层拿列表的子元素，按 VO 落到一项时会做的事
//    （AXScrollToVisible、设键盘焦点）逐项往下走，看能不能走出可见区。

use std::ffi::c_void;
use std::sync::Mutex;

use block2::RcBlock;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: *const c_void, keycode: u16, keydown: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CFRelease(cf: *const c_void);
}

const CG_FLAG_CMD: u64 = 0x0010_0000;
pub const NS_FLAG_CMD: usize = 1 << 20;
pub const NS_FLAG_SHIFT: usize = 1 << 17;
pub const NS_FLAG_CTRL: usize = 1 << 18;
pub const NS_FLAG_OPT: usize = 1 << 19;

/// 键名 → (macOS 虚拟键码, 是否带 Command)
pub fn keycode_for(key: &str) -> Option<(u16, bool)> {
    Some(match key {
        "down" => (125, false),
        "up" => (126, false),
        "home" => (115, false),
        "end" => (119, false),
        "pageup" => (116, false),
        "pagedown" => (121, false),
        "cmd-up" => (126, true),
        "cmd-down" => (125, true),
        _ => return None,
    })
}

/// 键码 + 修饰键 → 本实验用的键名（与 main.rs 的绑定一致）
pub fn key_name(code: u16, flags: usize) -> Option<&'static str> {
    let cmd = flags & NS_FLAG_CMD != 0;
    if flags & (NS_FLAG_CTRL | NS_FLAG_OPT | NS_FLAG_SHIFT) != 0 {
        return None; // 带 Ctrl/Option/Shift 的（含 VO 键 Ctrl+Option）一律不碰
    }
    Some(match (code, cmd) {
        (125, false) => "down",
        (126, false) => "up",
        (115, false) => "home",
        (119, false) => "end",
        (116, false) => "pageup",
        (121, false) => "pagedown",
        (126, true) => "cmd-up",
        (125, true) => "cmd-down",
        _ => return None,
    })
}

/// 投一次真实按键（按下 + 抬起）到本进程 NSApp 队列。返回是否投成功
pub fn post_key(key: &str) -> bool {
    let Some((code, cmd)) = keycode_for(key) else { return false };
    autoreleasepool(|_| unsafe {
        let Some(app): Option<Retained<AnyObject>> = msg_send![class!(NSApplication), sharedApplication] else {
            return false;
        };
        for down in [true, false] {
            let cg = CGEventCreateKeyboardEvent(std::ptr::null(), code, down);
            if cg.is_null() {
                return false;
            }
            if cmd {
                CGEventSetFlags(cg, CG_FLAG_CMD);
            }
            let ev: Option<Retained<AnyObject>> = msg_send![class!(NSEvent), eventWithCGEvent: cg];
            CFRelease(cg);
            let Some(ev) = ev else { return false };
            let _: () = msg_send![&app, postEvent: &*ev, atStart: Bool::NO];
        }
        true
    })
}

/// 让本进程成为前台、窗口成为 key window（自测前调；系统可能不允许）
pub fn make_key() {
    autoreleasepool(|_| unsafe {
        let Some(app): Option<Retained<AnyObject>> = msg_send![class!(NSApplication), sharedApplication] else { return };
        let _: () = msg_send![&app, activateIgnoringOtherApps: Bool::YES];
        if let Some(view) = content_view() {
            let win: Option<Retained<AnyObject>> = msg_send![&view, window];
            if let Some(win) = win {
                let nil: *mut AnyObject = std::ptr::null_mut();
                let _: () = msg_send![&win, makeKeyAndOrderFront: nil];
            }
        }
    })
}

/// 监视器看到的一次按键
#[derive(Clone, Debug)]
pub struct RawKey {
    pub code: u16,
    pub flags: usize,
    pub chars: Option<String>,
    pub repeat: bool,
    pub window_is_key: Option<bool>,
}

/// 装 NSEvent 本地监视器（只看按下）。`f` 返回 true = 吞掉这个事件（不再交给 AppKit/GPUI）
pub fn install_key_monitor(f: impl Fn(&RawKey) -> bool + 'static) {
    let block = RcBlock::new(move |ev: *mut AnyObject| -> *mut AnyObject {
        if ev.is_null() {
            return ev;
        }
        let ev_ref: &AnyObject = unsafe { &*ev };
        let code: u16 = unsafe { msg_send![ev_ref, keyCode] };
        let flags: usize = unsafe { msg_send![ev_ref, modifierFlags] };
        let repeat: Bool = unsafe { msg_send![ev_ref, isARepeat] };
        let chars: Option<Retained<AnyObject>> = unsafe { msg_send![ev_ref, charactersIgnoringModifiers] };
        let raw = RawKey {
            code,
            flags,
            chars: nsstring(chars).map(|s| s.chars().map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" ")),
            repeat: repeat.as_bool(),
            window_is_key: window_is_key(),
        };
        if f(&raw) { std::ptr::null_mut() } else { ev }
    });
    unsafe {
        let mask: u64 = 1 << 10; // NSEventMaskKeyDown
        let monitor: Option<Retained<AnyObject>> =
            msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask, handler: &*block];
        std::mem::forget(monitor); // 进程活多久，监视器就留多久
    }
    std::mem::forget(block);
}

// ---------------------------------------------------------------- 模拟 VO+←/→

fn children_of(obj: &AnyObject) -> Vec<Retained<AnyObject>> {
    let Some(arr) = get_obj(obj, sel!(accessibilityChildren)) else { return vec![] };
    let n: usize = unsafe { msg_send![&arr, count] };
    (0..n)
        .filter_map(|i| unsafe { msg_send![&arr, objectAtIndex: i] })
        .collect()
}

fn find_role(obj: &AnyObject, role: &str, depth: usize) -> Option<Retained<AnyObject>> {
    if depth > 8 {
        return None;
    }
    for c in children_of(obj) {
        if get_str(&c, sel!(accessibilityRole)).as_deref() == Some(role) {
            return Some(c);
        }
        if let Some(found) = find_role(&c, role, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// macOS 层列表（AXList）的子元素标题，按 VoiceOver 的浏览顺序
pub fn list_child_titles() -> Option<Vec<String>> {
    autoreleasepool(|_| {
        let view = content_view()?;
        let list = find_role(&view, "AXList", 0)?;
        Some(
            children_of(&list)
                .iter()
                .map(|c| get_str(c, sel!(accessibilityTitle)).unwrap_or_default())
                .collect(),
        )
    })
}

/// 模拟 VO 光标落到标题为 `title` 的那一项：VO 会做的两件事，凡是该项支持的都做。
/// 返回实际做了哪些；找不到该项返回 None
pub fn vo_land_on(title: &str) -> Option<Vec<&'static str>> {
    autoreleasepool(|_| unsafe {
        let view = content_view()?;
        let list = find_role(&view, "AXList", 0)?;
        let item = children_of(&list)
            .into_iter()
            .find(|c| get_str(c, sel!(accessibilityTitle)).as_deref() == Some(title))?;
        let mut did = Vec::new();
        if let Some(names) = get_obj(&item, sel!(accessibilityActionNames)) {
            let n: usize = msg_send![&names, count];
            for i in 0..n {
                let name: Option<Retained<AnyObject>> = msg_send![&names, objectAtIndex: i];
                if let Some(name) = name {
                    if nsstring(Some(name.clone())).as_deref() == Some("AXScrollToVisible") {
                        let _: () = msg_send![&item, accessibilityPerformAction: &*name];
                        did.push("AXScrollToVisible");
                    }
                }
            }
        }
        let allowed: Bool = msg_send![&item, isAccessibilitySelectorAllowed: sel!(setAccessibilityFocused:)];
        if allowed.as_bool() {
            let _: () = msg_send![&item, setAccessibilityFocused: Bool::YES];
            did.push("setAccessibilityFocused");
        }
        Some(did)
    })
}

// ---------------------------------------------------------------- 现场记录

static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn open_log(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    *LOG.lock().unwrap() = std::fs::File::create(path).ok();
}

/// 记一行 JSON（带毫秒时间戳）；没开记录就什么都不做
pub fn log_event(mut v: Value) {
    use std::io::Write;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    v["t_ms"] = json!(ms);
    if let Some(f) = LOG.lock().unwrap().as_mut() {
        writeln!(f, "{v}").ok();
    }
}
