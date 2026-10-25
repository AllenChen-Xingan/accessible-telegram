//! macOS 这一层：截获无障碍通知、直发公告、播系统音、进程内探查（都不需要辅助功能权限）。
//!
//! 截获思路同 exp001：本文件导出与 AppKit 同名的两个 C 函数，链接时 accesskit_macos 的调用
//! 先落到这里，记一笔再经 `dlsym(RTLD_NEXT)` 转给真正的 AppKit。

use std::ffi::{CStr, c_void};
use std::sync::Mutex;
use std::time::Instant;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};

/// 一条记录：系统通知，或本程序自己做的非通知动作（播音、改标题、改提示语）
#[derive(Clone, Debug)]
pub struct Event {
    /// 距程序启动的秒数
    pub t: f64,
    pub name: String,
    pub text: String,
    pub priority: isize,
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static EPOCH: Mutex<Option<Instant>> = Mutex::new(None);

pub fn set_epoch(t: Instant) {
    *EPOCH.lock().unwrap() = Some(t);
}

pub fn now() -> f64 {
    EPOCH.lock().unwrap().map(|e| e.elapsed().as_secs_f64()).unwrap_or(0.0)
}

pub fn mark() -> usize {
    EVENTS.lock().unwrap().len()
}

pub fn since(mark: usize) -> Vec<Event> {
    EVENTS.lock().unwrap()[mark..].to_vec()
}

/// 记一条（通知或本程序动作）
pub fn record(name: &str, text: &str, priority: isize) {
    let t = now();
    if std::env::var_os("EXP004_QUIET").is_none() {
        eprintln!("[记录 {t:8.3}s] {name} {text:?} prio={priority}");
    }
    EVENTS.lock().unwrap().push(Event { t, name: name.into(), text: text.into(), priority });
}

unsafe fn ns_to_string(obj: *const AnyObject) -> String {
    if obj.is_null() {
        return String::new();
    }
    unsafe { (*(obj as *const NSString)).to_string() }
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSAccessibilityAnnouncementKey: *const AnyObject;
    static NSAccessibilityPriorityKey: *const AnyObject;
    static NSAccessibilityAnnouncementRequestedNotification: *const AnyObject;
}

const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;

unsafe fn real_fn(name: &CStr) -> *mut c_void {
    let p = unsafe { libc::dlsym(RTLD_NEXT, name.as_ptr()) };
    assert!(!p.is_null(), "dlsym(RTLD_NEXT, {name:?}) 找不到");
    p
}

type PostFn = unsafe extern "C" fn(*mut AnyObject, *const AnyObject);
type PostInfoFn = unsafe extern "C" fn(*mut AnyObject, *const AnyObject, *const AnyObject);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn NSAccessibilityPostNotification(el: *mut AnyObject, n: *const AnyObject) {
    record(&unsafe { ns_to_string(n) }, "", 0);
    unsafe {
        let real: PostFn = std::mem::transmute(real_fn(c"NSAccessibilityPostNotification"));
        real(el, n);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn NSAccessibilityPostNotificationWithUserInfo(
    el: *mut AnyObject,
    n: *const AnyObject,
    info: *const AnyObject,
) {
    let name = unsafe { ns_to_string(n) };
    let (mut text, mut prio) = (String::new(), 0isize);
    if !info.is_null() {
        unsafe {
            let t: *const AnyObject = msg_send![info, objectForKey: NSAccessibilityAnnouncementKey];
            text = ns_to_string(t);
            let p: *const AnyObject = msg_send![info, objectForKey: NSAccessibilityPriorityKey];
            if !p.is_null() {
                prio = msg_send![p, integerValue];
            }
        }
    }
    record(&name, &text, prio);
    unsafe {
        let real: PostInfoFn = std::mem::transmute(real_fn(c"NSAccessibilityPostNotificationWithUserInfo"));
        real(el, n, info);
    }
}

/// 不经 GPUI/AccessKit，直接在 NSWindow 上发公告。
/// NSAccessibilityPriorityLevel：Low=10 · Medium=50 · High=90。AccessKit 只会发 50 和 90，所以「低」只能走这里。
pub fn announce_direct(text: &str, priority: isize) {
    let window = key_window();
    if window.is_null() {
        eprintln!("[exp004] 直发失败：拿不到 NSWindow");
        return;
    }
    unsafe {
        let info = NSMutableDictionary::<AnyObject, AnyObject>::new();
        let s = NSString::from_str(text);
        let n = NSNumber::new_isize(priority);
        let _: () = msg_send![&*info, setObject: &*s, forKey: NSAccessibilityAnnouncementKey];
        let _: () = msg_send![&*info, setObject: &*n, forKey: NSAccessibilityPriorityKey];
        NSAccessibilityPostNotificationWithUserInfo(
            window,
            NSAccessibilityAnnouncementRequestedNotification,
            Retained::as_ptr(&info) as *const AnyObject,
        );
    }
}

/// 播一声系统音（/System/Library/Sounds 下的名字，如 Pop、Tink）。不走无障碍接口，所以截获不到，自己记一笔
pub fn play_sound(name: &str) {
    unsafe {
        let cls = AnyClass::get(c"NSSound").expect("NSSound");
        let s = NSString::from_str(name);
        let sound: *mut AnyObject = msg_send![cls, soundNamed: &*s];
        if sound.is_null() {
            record("声音（找不到）", name, 0);
            return;
        }
        // soundNamed 返回共享实例，上一声没播完时 play 会失败，所以每次复制一份
        let sound: *mut AnyObject = msg_send![sound, copy];
        let ok: bool = msg_send![sound, play];
        record(if ok { "声音" } else { "声音（播放失败）" }, name, 0);
    }
}

fn ns_windows() -> Vec<*mut AnyObject> {
    unsafe {
        let cls = AnyClass::get(c"NSApplication").expect("NSApplication");
        let app: *mut AnyObject = msg_send![cls, sharedApplication];
        let windows: *mut AnyObject = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        (0..count)
            .map(|i| {
                let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
                w
            })
            .collect()
    }
}

fn key_window() -> *mut AnyObject {
    unsafe {
        let cls = AnyClass::get(c"NSApplication").expect("NSApplication");
        let app: *mut AnyObject = msg_send![cls, sharedApplication];
        let key: *mut AnyObject = msg_send![app, keyWindow];
        if !key.is_null() {
            return key;
        }
    }
    ns_windows().into_iter().next().unwrap_or(std::ptr::null_mut())
}

/// 进程内问一次 accessibilityFocusedUIElement：accesskit_macos 由此懒激活（等同读屏第一次来查询）
pub fn activate_in_process() {
    for w in ns_windows() {
        unsafe {
            let view: *mut AnyObject = msg_send![w, contentView];
            if !view.is_null() {
                let _: *mut AnyObject = msg_send![view, accessibilityFocusedUIElement];
            }
        }
    }
}

/// 把窗口摆到最上层但不激活应用：人的按键仍进他正在用的应用；窗口不被遮挡，GPUI 才会出新帧
pub fn order_front_without_activating() {
    for w in ns_windows() {
        unsafe {
            let _: () = msg_send![w, orderFrontRegardless];
        }
    }
}

/// macOS 层（VoiceOver 实际查询的那一层）现在的样子
#[derive(Debug, Default)]
pub struct Probe {
    pub window_title: String,
    pub focused_title: String,
    pub focused_help: String,
}

pub fn probe() -> Probe {
    let mut p = Probe::default();
    let w = key_window();
    if w.is_null() {
        return p;
    }
    unsafe {
        let t: *const AnyObject = msg_send![w, title];
        p.window_title = ns_to_string(t);
        let view: *mut AnyObject = msg_send![w, contentView];
        if view.is_null() {
            return p;
        }
        let f: *mut AnyObject = msg_send![view, accessibilityFocusedUIElement];
        if f.is_null() {
            return p;
        }
        let t: *const AnyObject = msg_send![f, accessibilityTitle];
        p.focused_title = ns_to_string(t);
        let h: *const AnyObject = msg_send![f, accessibilityHelp];
        p.focused_help = ns_to_string(h);
    }
    p
}
