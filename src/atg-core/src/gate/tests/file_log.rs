//! 留痕的 `file` 字段（#12）：新行带类型与字节数，旧行照常读

use super::*;

/// 发文件的计划（#12）：留痕多记 `file{kind,size}`
struct FilePlan(Plan);
impl Sealed for FilePlan {}
impl Gated for FilePlan {
    fn target(&self) -> ChatId {
        self.0.target
    }
    fn digest(&self) -> [u8; 32] {
        self.0.digest
    }
    fn intent(&self) -> [u8; 32] {
        self.0.intent
    }
    fn dedup(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "file"
    }
    fn log_file(&self) -> Option<(&'static str, u64)> {
        Some(("photo", 2048))
    }
}

/// 留痕加了 `file` 字段（#12）：之前写下的旧行（没有这个字段）照常留着、照常按时间整理；新行带类型与字节数、不带路径
#[test]
fn file_field_is_logged_and_old_lines_still_read() {
    let (g, dir) = gate(false);
    let now = unix_now();
    let stale = now - DIRECT_LOG_KEEP_S - 10;
    let old = format!(
        "{{\"at\":{stale},\"proc\":\"cli\",\"pid\":1,\"chat_id\":777,\"temp_id\":-1,\"final_id\":null,\"kind\":\"text\",\"result\":\"succeeded\"}}\n\
         {{\"at\":{now},\"proc\":\"cli\",\"pid\":1,\"chat_id\":777,\"temp_id\":-2,\"final_id\":5,\"kind\":\"text\",\"result\":\"succeeded\"}}\n"
    );
    fs::write(dir.join(DIRECT_LOG_FILE), old).unwrap();
    let p = FilePlan(plan(777, 6));
    let t = g.issue(&p, false).unwrap();
    let mode = g.verify(t, &p).unwrap();
    g.before_submit(mode, &p).unwrap();
    let log = fs::read_to_string(dir.join(DIRECT_LOG_FILE)).unwrap();
    let lines: Vec<serde_json::Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // 过期的旧行整理掉了，没过期的旧行（没有 file 字段）原样留着
    assert_eq!(lines.len(), 2, "{log}");
    assert_eq!(lines[0]["temp_id"], -2);
    assert!(lines[0].get("file").is_none());
    assert_eq!(lines[1]["kind"], "file");
    assert_eq!(
        lines[1]["file"],
        serde_json::json!({"kind": "photo", "size": 2048})
    );
}
