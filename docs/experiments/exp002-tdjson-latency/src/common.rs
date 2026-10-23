//! 三个程序共用：统计、直连 tdjson 的 C 接口、进程内查无障碍树。
#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char, c_double, c_int};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------- 统计 ----------

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

pub fn epoch_ns() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    // 最近秩法
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// 打印一行：名称 · 样本数 · 中位数 · p95 · p99 · 最小 · 最大（单位 ms）
pub fn report(name: &str, samples: &[f64]) -> String {
    let mut s = samples.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let line = if s.is_empty() {
        format!("{name:<44} n=0")
    } else {
        format!(
            "{name:<44} n={:<5} median={:>9.3}ms  p95={:>9.3}ms  p99={:>9.3}ms  min={:>8.3}ms  max={:>9.3}ms",
            s.len(),
            pct(&s, 50.0),
            pct(&s, 95.0),
            pct(&s, 99.0),
            s[0],
            s[s.len() - 1]
        )
    };
    println!("{line}");
    line
}

pub fn median(samples: &[f64]) -> f64 {
    let mut s = samples.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pct(&s, 50.0)
}

// ---------- 直连 tdjson（库由 tdlib-rs 的 download-tdlib 负责下载和链接） ----------

unsafe extern "C" {
    fn td_send(client_id: c_int, request: *const c_char);
    fn td_receive(timeout: c_double) -> *const c_char;
    fn td_execute(request: *const c_char) -> *const c_char;
}

pub fn raw_send(client_id: i32, request: &str) {
    let c = CString::new(request).unwrap();
    unsafe { td_send(client_id, c.as_ptr()) }
}

pub fn raw_receive(timeout_s: f64) -> Option<String> {
    unsafe {
        let p = td_receive(timeout_s);
        if p.is_null() {
            None
        } else {
            Some(CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
}

pub fn raw_execute(request: &str) -> Option<String> {
    let c = CString::new(request).unwrap();
    unsafe {
        let p = td_execute(c.as_ptr());
        if p.is_null() {
            None
        } else {
            Some(CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
}

/// 关掉 TDLib 默认的大量日志，免得写 stderr 干扰计时
pub fn quiet_tdlib() {
    raw_execute(r#"{"@type":"setLogVerbosityLevel","new_verbosity_level":1}"#);
}

/// 发一个请求并在本线程收回带同一 @extra 的应答；途中收到的更新计数返回。
/// 要求：此时没有别的线程在调 td_receive。
pub fn raw_roundtrip(client_id: i32, request_without_extra: &str, extra: u64) -> (String, usize) {
    // request_without_extra 形如 {"@type":"testCallEmpty"}，在末尾 } 前塞 @extra
    let body = request_without_extra.trim_end();
    let req = format!("{},\"@extra\":{extra}}}", &body[..body.len() - 1]);
    raw_send(client_id, &req);
    let mut updates = 0;
    loop {
        let Some(resp) = raw_receive(5.0) else {
            panic!("TDLib 5 秒没回应：{req}");
        };
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        if v.get("@extra").and_then(|e| e.as_u64()) == Some(extra) {
            return (resp, updates);
        }
        updates += 1;
    }
}

// ---------- 进程内查无障碍树（不走跨进程 AX 接口，不需要「辅助功能」权限） ----------

pub mod ax {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSArray, NSString};

    pub struct Found {
        /// 找到标签等于目标的节点
        pub found: bool,
        /// 一共走过多少个节点
        pub nodes: usize,
        /// 若找到，它的兄弟数（即所在列表的项数）
        pub siblings: usize,
    }

    fn content_view() -> Option<Retained<AnyObject>> {
        unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let windows: Retained<NSArray<AnyObject>> = msg_send![&*app, windows];
            for i in 0..windows.count() {
                let w = windows.objectAtIndex(i);
                let v: Option<Retained<AnyObject>> = msg_send![&*w, contentView];
                if v.is_some() {
                    return v;
                }
            }
            None
        }
    }

    fn children(obj: &AnyObject) -> Option<Retained<NSArray<AnyObject>>> {
        unsafe { msg_send![obj, accessibilityChildren] }
    }

    fn title(obj: &AnyObject) -> Option<String> {
        let t: Option<Retained<NSString>> = unsafe { msg_send![obj, accessibilityTitle] };
        t.map(|s| s.to_string())
    }

    /// 从窗口内容视图起深度优先找标签为 `target` 的节点。
    /// 第一次调用会让 AccessKit 激活（等同于读屏第一次来问）。
    pub fn find(target: &str) -> Found {
        let mut out = Found { found: false, nodes: 0, siblings: 0 };
        let Some(view) = content_view() else { return out };
        fn walk(obj: &AnyObject, target: &str, out: &mut Found, depth: usize) {
            if depth > 64 || out.found {
                return;
            }
            let Some(kids) = children(obj) else { return };
            let n = kids.count();
            for i in 0..n {
                let k = kids.objectAtIndex(i);
                out.nodes += 1;
                if title(&k).as_deref() == Some(target) {
                    out.found = true;
                    out.siblings = n;
                    return;
                }
                walk(&k, target, out, depth + 1);
                if out.found {
                    return;
                }
            }
        }
        walk(&view, target, &mut out, 0);
        out
    }

    /// 只碰一下根，让 AccessKit 激活
    pub fn touch() -> usize {
        content_view()
            .and_then(|v| children(&v))
            .map(|k| k.count())
            .unwrap_or(0)
    }
}

// ---------- 一条像样的「新消息」更新（TDLib 1.8.61 的 JSON 形状） ----------

pub fn sample_update_new_message(id: i64, text: &str) -> String {
    let text_json = serde_json::to_string(text).unwrap();
    format!(
        r#"{{"@type":"updateNewMessage","message":{{"@type":"message","id":{id},"sender_id":{{"@type":"messageSenderUser","user_id":777000123}},"chat_id":-1001234567890,"is_outgoing":false,"is_pinned":false,"is_from_offline":false,"can_be_saved":true,"has_timestamped_media":false,"is_channel_post":false,"is_paid_star_suggested_post":false,"is_paid_ton_suggested_post":false,"contains_unread_mention":false,"date":1792700000,"edit_date":0,"unread_reactions":[],"self_destruct_in":0.0,"auto_delete_in":0.0,"via_bot_user_id":0,"sender_business_bot_user_id":0,"sender_boost_count":0,"paid_message_star_count":0,"author_signature":"","media_album_id":"0","effect_id":"0","summary_language_code":"","content":{{"@type":"messageText","text":{{"@type":"formattedText","text":{text_json},"entities":[{{"@type":"textEntity","offset":0,"length":4,"type":{{"@type":"textEntityTypeBold"}}}},{{"@type":"textEntity","offset":10,"length":6,"type":{{"@type":"textEntityTypeItalic"}}}}]}}}}}},"@client_id":1}}"#
    )
}

pub const SAMPLE_TEXT: &str = "今晚七点在老地方见，记得带上上次说的那本书。另外周末的计划我改了一下：周六上午去爬山，下午回来一起做饭，周日休息。你看看行不行，不行我们再商量。顺便问一句，猫粮还有吗？没有的话我路上买。";
