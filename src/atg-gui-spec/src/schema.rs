//! 给 agent 的界面自描述（`docs/plan/012_gui-ssot.md` 第二节第 14 行）：由本 crate 的表生成，
//! `cargo run -p atg-gui-spec --bin gui-schema` 输出（不编 GPUI；`atg-gui --schema` 将来原样转这份）。
//!
//! 带版本号 `v`（[`GUI_SCHEMA_V`]）和内容哈希 `x-content-hash`（[`GUI_SCHEMA_HASH`]）。门槛两道：
//! - 金样：`tests/golden/gui-schema.json` 与生成结果逐字比（`tests/golden.rs`）
//! - 盖章：`python3 scripts/schema-stamp.py check src/atg-gui-spec/tests/golden/gui-schema.json`（CI）——内容变了
//!   哈希没跟着改就红，逼人有意识地决定要不要升 `v`
//!
//! 改了表之后的更新步骤：① 按下面的口径决定升不升 `GUI_SCHEMA_V`；② `cargo run -p atg-gui-spec --bin gui-schema >
//! src/atg-gui-spec/tests/golden/gui-schema.json`；③ `python3 scripts/schema-stamp.py stamp` 同一文件，把打印出的哈希抄进
//! [`GUI_SCHEMA_HASH`]；④ 再跑一次 ②，金样测试与盖章都绿。
//!
//! 升版本的口径同 `specs/content-tree.md` 第八节：加字段、加取值不加一；改名、删、改义加一。

use crate::a11y::{Role, TABLE};
use crate::aid::IdKind;
use crate::text::TextId;
use serde_json::{Value, json};

/// 界面自描述的版本号
pub const GUI_SCHEMA_V: u32 = 1;

/// 内容哈希（`scripts/schema-stamp.py` 的算法：去掉本字段后按键排序、紧凑 JSON 的 sha256 前 16 位）
pub const GUI_SCHEMA_HASH: &str = "6a3834991b1e76a7";

/// 生成自描述
pub fn schema() -> Value {
    json!({
        "v": GUI_SCHEMA_V,
        "x-content-hash": GUI_SCHEMA_HASH,
        "ids": IdKind::ALL.iter().map(|k| json!({ "kind": k.prefix(), "arity": k.arity() })).collect::<Vec<_>>(),
        "roles": Role::ALL.iter().map(|r| r.name()).collect::<Vec<_>>(),
        "elements": TABLE.iter().map(|s| json!({
            "key": s.key,
            "role": s.role.name(),
            "id": s.id.prefix(),
            "name": s.name.key(),
            "role_description": s.role_description.map(|t| t.key()),
            "operable": s.operable,
        })).collect::<Vec<_>>(),
        "texts": TextId::ALL.iter().map(|t| json!({ "key": t.key(), "zh": t.zh() })).collect::<Vec<_>>(),
    })
}

/// 输出的文字（金样里存的就是它）：两格缩进、末尾一个换行
pub fn render() -> String {
    // 自描述只含字符串、数字、布尔，序列化不会失败；万一失败就出空串，金样测试会红
    let mut s = serde_json::to_string_pretty(&schema()).unwrap_or_default();
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::NameFrom;

    #[test]
    fn every_text_is_referenced_by_the_table() {
        // 012 第二节第 4 行：文案表里每条都被至少一处引用（未引用即红）
        for t in TextId::ALL {
            let used = TABLE.iter().any(|s| {
                s.role_description == Some(t)
                    || matches!(s.name, NameFrom::Text(x) | NameFrom::WindowTitle(x) if x == t)
            });
            assert!(used, "{t:?} 没有被契约表引用");
        }
    }

    #[test]
    fn every_role_is_used() {
        for r in Role::ALL {
            assert!(TABLE.iter().any(|s| s.role == r), "{r:?} 没有元素用");
        }
    }
}
