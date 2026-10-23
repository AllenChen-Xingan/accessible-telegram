//! 截获本进程发给 macOS 的无障碍通知（不需要辅助功能权限）。见 main.rs 顶部说明。

use std::ffi::{CStr, c_void};
use std::sync::Mutex;
use std::time::Instant;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};

#[derive(Clone, Debug)]
pub struct Event {
    pub name: String,
    pub text: String,
    pub priority: isize,
    /// 从「模型改值」到「通知发出」的毫秒数（只对公告有意义）
    pub latency_ms: Option<f64>,
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static LAST_CHANGE: Mutex<Option<Instant>> = Mutex::new(None);
static EPOCH: Mutex<Option<Instant>> = Mutex::new(None);

pub fn set_epoch(t: Instant) {
    *EPOCH.lock().unwrap() = Some(t);
}

pub fn mark_model_change(_value: &str) {
    *LAST_CHANGE.lock().unwrap() = Some(Instant::now());
}

pub fn mark() -> usize {
    EVENTS.lock().unwrap().len()
}

pub fn since(mark: usize) -> Vec<Event> {
    EVENTS.lock().unwrap()[mark..].to_vec()
}

fn verbose() -> bool {
    std::env::var_os("EXP001_QUIET").is_none()
}

unsafe fn ns_to_string(obj: *const AnyObject) -> String {
    if obj.is_null() {
        return String::new();
    }
    let s: &NSString = unsafe { &*(obj as *const NSString) };
    s.to_string()
}

fn record(name: String, text: String, priority: isize) {
    let latency_ms = if name == "AXAnnouncementRequested" {
        LAST_CHANGE
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_secs_f64() * 1000.0)
    } else {
        None
    };
    if verbose() {
        let t = EPOCH
            .lock()
            .unwrap()
            .map(|e| e.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        eprintln!("[通知 {t:8.3}s] {name} {text:?} prio={priority}");
    }
    EVENTS.lock().unwrap().push(Event {
        name,
        text,
        priority,
        latency_ms,
    });
}

// ---- AppKit 里的常量（自己声明，免得依赖 objc2-app-kit 的 feature 名） ----
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

/// 与 AppKit 同名：accesskit_macos 的调用会绑定到这里
#[unsafe(no_mangle)]
pub unsafe extern "C" fn NSAccessibilityPostNotification(
    element: *mut AnyObject,
    notification: *const AnyObject,
) {
    record(unsafe { ns_to_string(notification) }, String::new(), 0);
    unsafe {
        let real: PostFn = std::mem::transmute(real_fn(c"NSAccessibilityPostNotification"));
        real(element, notification);
    }
}

/// 与 AppKit 同名：accesskit_macos 的公告走这里
#[unsafe(no_mangle)]
pub unsafe extern "C" fn NSAccessibilityPostNotificationWithUserInfo(
    element: *mut AnyObject,
    notification: *const AnyObject,
    user_info: *const AnyObject,
) {
    let name = unsafe { ns_to_string(notification) };
    let (mut text, mut priority) = (String::new(), 0isize);
    if !user_info.is_null() {
        unsafe {
            let t: *const AnyObject =
                msg_send![user_info, objectForKey: NSAccessibilityAnnouncementKey];
            text = ns_to_string(t);
            let p: *const AnyObject = msg_send![user_info, objectForKey: NSAccessibilityPriorityKey];
            if !p.is_null() {
                priority = msg_send![p, integerValue];
            }
        }
    }
    record(name, text, priority);
    unsafe {
        let real: PostInfoFn =
            std::mem::transmute(real_fn(c"NSAccessibilityPostNotificationWithUserInfo"));
        real(element, notification, user_info);
    }
}

/// 兜底路径：不经 GPUI/AccessKit，直接在 NSWindow 上发公告（与 accesskit_macos 发的同一个通知）
pub fn post_announcement_direct(window: *mut AnyObject, text: &str, high: bool) {
    if window.is_null() {
        eprintln!("[exp001] 兜底直发失败：拿不到 NSWindow");
        return;
    }
    mark_model_change(text);
    unsafe {
        let info = NSMutableDictionary::<AnyObject, AnyObject>::new();
        let s = NSString::from_str(text);
        // NSAccessibilityPriorityLevel：Low=10 Medium=50 High=90
        let n = NSNumber::new_isize(if high { 90 } else { 50 });
        let _: () = msg_send![&*info, setObject: &*s, forKey: NSAccessibilityAnnouncementKey];
        let _: () = msg_send![&*info, setObject: &*n, forKey: NSAccessibilityPriorityKey];
        NSAccessibilityPostNotificationWithUserInfo(
            window,
            NSAccessibilityAnnouncementRequestedNotification,
            Retained::as_ptr(&info) as *const AnyObject,
        );
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

pub fn first_ns_window() -> *mut AnyObject {
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

/// 在进程内调用一次 contentView 的 accessibilityChildren——accesskit_macos 的
/// SubclassingAdapter 就是在这里懒激活的，等同读屏第一次来查询。
pub fn activate_in_process() -> usize {
    let mut n = 0;
    for w in ns_windows() {
        unsafe {
            let view: *mut AnyObject = msg_send![w, contentView];
            if view.is_null() {
                continue;
            }
            let children: *mut AnyObject = msg_send![view, accessibilityChildren];
            if !children.is_null() {
                let c: usize = msg_send![children, count];
                n += c;
            }
        }
    }
    n
}
