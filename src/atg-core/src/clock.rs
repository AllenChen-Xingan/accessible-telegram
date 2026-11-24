//! 「现在」与系统时区：全工作区共用这一份（#60 R8；`docs/plan/010` 第二节 S10）。CLI 和 GUI 都从这里取。
//!
//! 不放在 `tree` 里：`tree` 只放数据（见 `tree/mod.rs` 头），这里要读系统时钟、调 libc。
//! 只做「取值」，不做换算：Unix 秒 ↔ RFC3339、本地钟点仍是 `tree::Time` 的方法。

// 溢出时一律取 `i64::MAX`（#60 R8 统一；原 cli `speech::now` 取 0）。秒要到约 2920 亿年、毫秒要到约 2.9 亿年才溢出，
// 实际碰不到，所以这两处边界的差别（还有 cli `out::now_rfc3339_ms` 溢出后毫秒位固定为 807）不算行为变化，只在这里记一笔

/// 现在（Unix 秒）。时钟早于 1970 年按 0
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// 现在（Unix 毫秒）。时钟早于 1970 年按 0
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 系统时区在某个时间点（Unix 秒）的偏移（秒）；取不到按 0
pub fn local_offset_at(t: i64) -> i32 {
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
    fn local_offset_is_sane() {
        assert!(local_offset_at(now_s()).abs() <= 14 * 3600);
        // 冬夏各取一个时间点都能算（有夏令时的时区两者不同，这里只要求不出错、在范围内）
        for t in [1_767_225_600, 1_782_864_000] {
            assert!(local_offset_at(t).abs() <= 14 * 3600);
        }
    }
}
