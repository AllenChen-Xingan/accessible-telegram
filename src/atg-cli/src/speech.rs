//! `speech --fixture PATH [--level brief|chat_brief]`：夹具 → 树 → 读法（第一刀第 6 块，#19）。
//! 不联网，也不碰档案（本地 TDLib 数据库、钥匙串、档案锁都不开）。
//!
//! 夹具 = TDLib 更新对象的 JSON 数组，按顺序喂进 store（`atg_core::tdjson::World`，与快照测试同一个入口）。
//! - `brief`（默认）：按到达顺序，每条消息 / 系统消息 / 撤回占位 / 状态一行，`speech.brief` 是消息读法
//! - `chat_brief`：主列表（TDLib `chatListMain`；归档、文件夹里的不算）的会话，一行一个，`speech.brief` 是会话项读法
//!
//! 文字模式每行一句读法。加 `--tz +08:00` 时与 `src/atg-core/tests/snapshots/*brief*.snap` 逐行一致，
//! 只差快照里 GUI 才有的「按 D 展开」（`tests/cli.rs` 测住）。

use crate::args::{Format, SpeechArgs, SpeechLevel};
use crate::out::{self, Code, emit, envelope, fail, text_line};
use atg_core::speech::{SpeechCtx, brief};
use atg_core::tdjson::World;
use serde_json::json;
use std::process::ExitCode;
use std::time::Instant;

pub fn run(a: &SpeechArgs, tz: Option<&str>, format: Format, started: Instant) -> ExitCode {
    // 不给 --tz：跟系统时区，每个钟点按它自己的时间点取偏移（夏令时前后不一样）；给了：一律用它
    let (utc_offset, offset_at) = match tz {
        None => (
            local_offset_at(now()),
            Some(local_offset_at as fn(i64) -> i32),
        ),
        Some(s) => match parse_tz(s) {
            Some(o) => (o, None),
            None => {
                return fail(
                    format,
                    Code::Usage,
                    "--tz 不对：写成 +08:00、-05:30、+0800、+8、Z 或 UTC",
                    json!({}),
                );
            }
        },
    };
    // 读不了、不是 JSON、顶层不是数组：用法错误。说明是写死的话，不带文件内容和路径——标准输出常被贴进 issue、
    // 交给 AI，和 `main.rs` 的 `parse_failed` 一样，用户给的东西不进标准输出（`.42cog/real.md`「隐私」）
    let Ok(src) = std::fs::read_to_string(&a.fixture) else {
        return fail(format, Code::Usage, "--fixture 指的文件读不了", json!({}));
    };
    let Ok(world) = World::from_json(&src) else {
        return fail(
            format,
            Code::Usage,
            "--fixture 不是 TDLib 更新的 JSON 数组",
            json!({}),
        );
    };
    // CLI 没有按键：提醒不读「按 M 收听」，隐藏内容不读「按 D 展开」
    let cx = SpeechCtx {
        utc_offset,
        offset_at,
        listen_key: None,
        expand_key: None,
        reveal: a.reveal,
    };
    let docs = match a.level {
        SpeechLevel::Brief => world.docs(),
        SpeechLevel::ChatBrief => world.chat_items(),
    };
    for d in &docs {
        match format {
            Format::Json => {
                let mut line = atg_core::json::value(d, &cx);
                if let Some(o) = line.as_object_mut() {
                    o.insert("at".into(), json!(out::now_rfc3339_ms()));
                }
                emit(&line);
            }
            // 与 JSON 的 `speech.brief` 同一个函数（会话项由 `brief` 转给 `chat_brief`）
            Format::Text => text_line(&brief(d, &cx)),
        }
    }
    if format == Format::Json {
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        emit(&envelope(
            "result",
            json!({
                "command": "speech",
                "level": a.level.name(),
                "count": docs.len(),
                // 给了 --tz：所有钟点都按这个固定偏移；跟系统时区：每个钟点各取各的偏移（夏令时前后不同），
                // 没有一个对所有行都成立的值，所以是 null
                "tz": if offset_at.is_some() { "system" } else { "fixed" },
                "utc_offset_s": if offset_at.is_some() { None } else { Some(utc_offset) },
                "elapsed_ms": elapsed_ms,
            }),
        ));
    }
    ExitCode::SUCCESS
}

/// `+08:00` · `-05:30` · `+8` · `+0800` · `Z` / `UTC` → 秒。范围 ±14 小时
pub(crate) fn parse_tz(s: &str) -> Option<i32> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("z") || s.eq_ignore_ascii_case("utc") {
        return Some(0);
    }
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let (h, m) = match rest.split_once(':') {
        Some((h, m)) => (h, m),
        None if rest.len() == 4 => rest.split_at(2),
        None => (rest, "0"),
    };
    let all_digits =
        |x: &str| !x.is_empty() && x.len() <= 2 && x.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(h) || !all_digits(m) {
        return None;
    }
    let (h, m): (i32, i32) = (h.parse().ok()?, m.parse().ok()?);
    if h > 14 || m > 59 || (h == 14 && m > 0) {
        return None;
    }
    Some(sign * (h * 3600 + m * 60))
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// 系统时区在某个时间点（Unix 秒）的偏移（秒）；取不到按 0
pub(crate) fn local_offset_at(t: i64) -> i32 {
    // macOS 上 `time_t` 就是 i64
    let t: libc::time_t = t;
    // SAFETY: `localtime_r` 只读 `t`、只写我们给的 `tm`，返回空指针时不读它
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        i32::try_from(tm.tm_gmtoff).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tz_forms() {
        assert_eq!(parse_tz("+08:00"), Some(8 * 3600));
        assert_eq!(parse_tz("+8"), Some(8 * 3600));
        assert_eq!(parse_tz("+0800"), Some(8 * 3600));
        assert_eq!(parse_tz("-05:30"), Some(-(5 * 3600 + 30 * 60)));
        assert_eq!(parse_tz("Z"), Some(0));
        assert_eq!(parse_tz("utc"), Some(0));
        assert_eq!(parse_tz("+14:00"), Some(14 * 3600));
        for bad in [
            "", "8", "+", "+15", "+14:30", "+08:60", "+a", "+08:0x", "+123", "++8",
        ] {
            assert_eq!(parse_tz(bad), None, "{bad}");
        }
    }

    #[test]
    fn local_offset_is_sane() {
        assert!(local_offset_at(now()).abs() <= 14 * 3600);
        // 冬夏各取一个时间点都能算（有夏令时的时区两者不同，这里只要求不出错、在范围内）
        for t in [1_767_225_600, 1_782_864_000] {
            assert!(local_offset_at(t).abs() <= 14 * 3600);
        }
    }
}
