//! Unix 秒 ↔ RFC3339 UTC，以及读法要的「本地钟点」。不引日期库：算法照 Howard Hinnant 的 civil_from_days。

use super::Time;
use serde::{Serialize, Serializer};

impl Serialize for Time {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.rfc3339())
    }
}

impl Time {
    /// `2026-10-25T08:05:00Z`
    pub fn rfc3339(self) -> String {
        let days = self.0.div_euclid(86_400);
        let secs = self.0.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        )
    }

    /// 加上时区偏移（秒）后的 (时, 分)
    pub fn local_hm(self, utc_offset: i32) -> (u32, u32) {
        let secs = self
            .0
            .saturating_add(i64::from(utc_offset))
            .rem_euclid(86_400);
        ((secs / 3600) as u32, (secs % 3600 / 60) as u32)
    }

    /// 加上时区偏移（秒）后的 (年, 月, 日)
    pub fn local_ymd(self, utc_offset: i32) -> (i64, u32, u32) {
        let local = self.0.saturating_add(i64::from(utc_offset));
        civil_from_days(local.div_euclid(86_400))
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_points() {
        assert_eq!(Time(0).rfc3339(), "1970-01-01T00:00:00Z");
        assert_eq!(Time(951_782_400).rfc3339(), "2000-02-29T00:00:00Z");
        assert_eq!(Time(1_792_700_000).rfc3339(), "2026-10-22T20:13:20Z");
        assert_eq!(Time(-1).rfc3339(), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn local_hm_wraps_day() {
        // 2026-10-22T20:13:20Z，东八区是第二天 04:13
        assert_eq!(Time(1_792_700_000).local_hm(8 * 3600), (4, 13));
        assert_eq!(Time(0).local_hm(-3600), (23, 0));
        // 极端值不溢出
        let _ = Time(i64::MAX).local_hm(8 * 3600);
        let _ = Time(i64::MIN).rfc3339();
    }

    #[test]
    fn local_ymd_follows_offset() {
        // 2026-10-22T20:13:20Z：东八区已是 23 日，西五区还是 22 日
        assert_eq!(Time(1_792_700_000).local_ymd(8 * 3600), (2026, 10, 23));
        assert_eq!(Time(1_792_700_000).local_ymd(-5 * 3600), (2026, 10, 22));
        let _ = Time(i64::MAX).local_ymd(8 * 3600);
        let _ = Time(i64::MIN).local_ymd(-8 * 3600);
    }
}
