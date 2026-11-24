//! 进程内问 macOS 无障碍层（不需要辅助功能权限），并截获本进程发出的无障碍通知。
//!
//! 截获的办法同 exp001：本二进制导出与 AppKit 同名的 `NSAccessibilityPostNotification*`，
//! accesskit_macos 的调用会先到这里，记下后经 `dlsym(RTLD_NEXT)` 转给 AppKit。

use std::ffi::{CStr, c_char, c_void};
use std::sync::Mutex;

use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{msg_send, sel};

#[derive(Clone, Debug)]
pub struct Note {
    pub name: String,
    /// 公告文字（只有 AXAnnouncementRequested 有）
    pub text: String,
    /// 有没有带 userInfo
    pub has_user_info: bool,
}

static NOTES: Mutex<Vec<Note>> = Mutex::new(Vec::new());

pub fn mark() -> usize {
    NOTES.lock().unwrap().len()
}

pub fn since(mark: usize) -> Vec<Note> {
    NOTES.lock().unwrap()[mark..].to_vec()
}

unsafe fn ns_to_string(obj: *const AnyObject) -> String {
    if obj.is_null() {
        return String::new();
    }
    let p: *const c_char = unsafe { msg_send![obj, UTF8String] };
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSAccessibilityAnnouncementKey: *const AnyObject;
}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn class_addMethod(cls: *const AnyClass, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
    fn object_getClass(obj: *const AnyObject) -> *const AnyClass;
    fn class_getName(cls: *const AnyClass) -> *const c_char;
}

const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;

type PostFn = unsafe extern "C" fn(*mut AnyObject, *const AnyObject);
type PostInfoFn = unsafe extern "C" fn(*mut AnyObject, *const AnyObject, *const AnyObject);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn NSAccessibilityPostNotification(el: *mut AnyObject, n: *const AnyObject) {
    let name = unsafe { ns_to_string(n) };
    NOTES.lock().unwrap().push(Note { name, text: String::new(), has_user_info: false });
    unsafe {
        let real: PostFn = std::mem::transmute(dlsym(RTLD_NEXT, c"NSAccessibilityPostNotification".as_ptr()));
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
    let text = if info.is_null() {
        String::new()
    } else {
        unsafe {
            let t: *const AnyObject = msg_send![info, objectForKey: NSAccessibilityAnnouncementKey];
            ns_to_string(t)
        }
    };
    NOTES.lock().unwrap().push(Note { name, text, has_user_info: !info.is_null() });
    unsafe {
        let real: PostInfoFn =
            std::mem::transmute(dlsym(RTLD_NEXT, c"NSAccessibilityPostNotificationWithUserInfo".as_ptr()));
        real(el, n, info);
    }
}

fn app() -> *mut AnyObject {
    let cls = AnyClass::get(c"NSApplication").expect("NSApplication");
    unsafe { msg_send![cls, sharedApplication] }
}

fn first_window() -> *mut AnyObject {
    unsafe {
        let windows: *mut AnyObject = msg_send![app(), windows];
        let count: usize = msg_send![windows, count];
        if count == 0 {
            return std::ptr::null_mut();
        }
        msg_send![windows, objectAtIndex: 0usize]
    }
}

fn content_view() -> *mut AnyObject {
    let w = first_window();
    if w.is_null() {
        return w;
    }
    unsafe { msg_send![w, contentView] }
}

/// 进程内问一次 contentView 的子元素：AccessKit 懒激活（同 exp001 / exp003）
pub fn activate() -> usize {
    let v = content_view();
    if v.is_null() {
        return 0;
    }
    unsafe {
        let children: *mut AnyObject = msg_send![v, accessibilityChildren];
        if children.is_null() { 0 } else { msg_send![children, count] }
    }
}

pub fn window_is_key() -> bool {
    let w = first_window();
    !w.is_null() && unsafe { msg_send![w, isKeyWindow] }
}

fn role_and_title(obj: *mut AnyObject) -> String {
    if obj.is_null() {
        return "null".into();
    }
    unsafe {
        let r: *const AnyObject = msg_send![obj, accessibilityRole];
        let responds: bool = msg_send![obj, respondsToSelector: sel!(accessibilityTitle)];
        let t = if responds {
            let t: *const AnyObject = msg_send![obj, accessibilityTitle];
            ns_to_string(t)
        } else {
            String::new()
        };
        format!("{} {:?}", ns_to_string(r), t)
    }
}

/// AccessKit 子类化的 contentView 答的焦点元素
pub fn focus_from_content_view() -> String {
    let v = content_view();
    if v.is_null() {
        return "no view".into();
    }
    role_and_title(unsafe { msg_send![v, accessibilityFocusedUIElement] })
}

/// 应用级答的焦点元素（VoiceOver 切到本应用时问的就是它，见上游 issue #66691）
pub fn focus_from_app() -> String {
    role_and_title(unsafe { msg_send![app(), accessibilityFocusedUIElement] })
}

/// 第一响应者的类名（GPUI 自己的 GPUIView）
pub fn first_responder_class() -> String {
    let w = first_window();
    if w.is_null() {
        return "no window".into();
    }
    unsafe {
        let fr: *mut AnyObject = msg_send![w, firstResponder];
        if fr.is_null() {
            return "null".into();
        }
        CStr::from_ptr(class_getName(object_getClass(fr))).to_string_lossy().into_owned()
    }
}

/// #66691 评论里的权宜做法：给第一响应者的类加 `accessibilityFocusedUIElement`，转问父视图
/// （父视图就是 AccessKit 子类化的 contentView）。父视图答空时退回 NSView 的默认答法——
/// 这里简化为返回 self。只在本实验里用。
extern "C" fn forwarded_focus(this: *mut AnyObject, _cmd: Sel) -> *mut AnyObject {
    unsafe {
        let sup: *mut AnyObject = msg_send![this, superview];
        if sup.is_null() {
            return this;
        }
        let f: *mut AnyObject = msg_send![sup, accessibilityFocusedUIElement];
        if f.is_null() { this } else { f }
    }
}

pub fn install_focus_workaround() -> bool {
    let w = first_window();
    if w.is_null() {
        return false;
    }
    unsafe {
        let fr: *mut AnyObject = msg_send![w, firstResponder];
        if fr.is_null() {
            return false;
        }
        let cls = object_getClass(fr);
        class_addMethod(
            cls,
            sel!(accessibilityFocusedUIElement),
            forwarded_focus as *const c_void,
            c"@@:".as_ptr(),
        )
    }
}

/// 系统里 VoiceOver 开没开（NSWorkspace.isVoiceOverEnabled）
pub fn voiceover_enabled() -> bool {
    let cls = AnyClass::get(c"NSWorkspace").expect("NSWorkspace");
    unsafe {
        let ws: *mut AnyObject = msg_send![cls, sharedWorkspace];
        msg_send![ws, isVoiceOverEnabled]
    }
}
