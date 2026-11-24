//! 给 agent 的界面自描述（`docs/plan/012_gui-ssot.md` 第二节第 14 行）：由本 crate 的表生成，
//! `cargo run -p atg-gui-spec --bin gui-schema` 输出（不编 GPUI；`atg-gui --schema` 将来原样转这份）。
//!
//! 带版本号 `v`（[`GUI_SCHEMA_V`]）和内容哈希 `x-content-hash`（[`GUI_SCHEMA_HASH`]）。门槛两道：
//! - 金样：`tests/golden/gui-schema.json` 与生成结果逐字比（`tests/golden.rs`）
//! - 盖章：`python3 scripts/schema-stamp.py check src/atg-gui-spec/tests/golden/gui-schema.json`（CI：`arch.yml` 单独一步）——内容变了
//!   哈希没跟着改就红，逼人有意识地决定要不要升 `v`
//!
//! 改了表之后的更新步骤：① 按下面的口径决定升不升 `GUI_SCHEMA_V`；② `cargo run -p atg-gui-spec --bin gui-schema >
//! src/atg-gui-spec/tests/golden/gui-schema.json`；③ `python3 scripts/schema-stamp.py stamp` 同一文件，把打印出的哈希抄进
//! [`GUI_SCHEMA_HASH`]；④ 再跑一次 ②，金样测试与盖章都绿。
//!
//! 为什么哈希要手抄成常量、不让程序自己算：手抄这一步是故意加的一道关。程序自己算的话，内容一变哈希就跟着变，
//! 金样重生成就绿了，没人会停下来想 `v` 要不要加一；手抄逼改表的人在抄的那一刻做这个决定。第 ④ 步重跑 ②，
//! 是因为金样里的哈希取自常量，常量改了金样要跟着重出。
//!
//! 升版本的口径同 `specs/content-tree.md` 第八节：加字段、加取值不加一；改名、删、改义加一。

use crate::a11y::{NameFrom, Role, TABLE};
use crate::aid::IdKind;
use crate::text::TextId;
use serde_json::{Value, json};

/// 界面自描述的版本号
pub const GUI_SCHEMA_V: u32 = 1;

/// 内容哈希（`scripts/schema-stamp.py` 的算法：去掉本字段后按键排序、紧凑 JSON 的 sha256 前 16 位）
pub const GUI_SCHEMA_HASH: &str = "8c023670543d95a4";

/// 生成自描述
pub fn schema() -> Value {
    json!({
        "v": GUI_SCHEMA_V,
        "x-content-hash": GUI_SCHEMA_HASH,
        "about": {
            "v": "本自描述的版本号：改名、删字段、改义时加一；加字段、加取值不加",
            "x-content-hash": "内容哈希（去掉本字段后按键排序、紧凑 JSON 的 sha256 前 16 位），内容变了它就变",
            "ids": "稳定标识（macOS 的 AXIdentifier）的种类。写法：kind，后面跟 arity 个十进制整数，每个前面加一个冒号，如 msg:-100123:4567；数字是什么见 specs/gui/a11y-tree.md 第二节",
            "roles": "树里会出现的角色（AccessKit 的角色名）",
            "elements": "每类元素：role 角色、id 用哪种标识、name 名称从哪来、role_description 角色描述取哪条文案（null 为不设）、operable 能不能获焦或被选中",
            "elements.name": "text:<文案>：就是这条文案；text+alert:<文案>：文案加新消息提醒（提醒还没接上时只有文案）；speech:<读法>：由 atg-core 的这条读法现算；window_title:<文案>+alert+status：根节点不带名称，窗口名在 NSWindow 标题上（文案 + 新消息提醒 + 用户状态）",
            "texts": "固定文案表：key 与中文"
        },
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

/// `specs/gui/a11y-tree.md` 第三节里由本表生成的那一段的起止标记（金样比对按它们取段）
pub const MD_BEGIN: &str =
    "<!-- 由 `cargo run -p atg-gui-spec --bin gui-schema -- --md-elements` 生成，别手改：开始 -->";
pub const MD_END: &str = "<!-- 生成段结束 -->";

fn name_md(n: NameFrom) -> String {
    match n {
        NameFrom::WindowTitle(t) => format!(
            "无（窗口名走 NSWindow 标题：文案 `{}`「{}」+ 新消息提醒 + 用户状态）",
            t.key(),
            t.zh()
        ),
        NameFrom::Text(t) => format!("文案 `{}`「{}」", t.key(), t.zh()),
        NameFrom::TextWithAlert(t) => format!("文案 `{}`「{}」+ 新消息提醒", t.key(), t.zh()),
        NameFrom::ChatBrief => "`speech::chat_brief`".into(),
        NameFrom::Brief => "`speech::brief`".into(),
    }
}

/// 契约表的 Markdown 形式（含起止标记），整段贴进 `a11y-tree.md` 第三节；`tests/golden.rs` 逐字比
pub fn elements_md() -> String {
    let mut s = format!(
        "{MD_BEGIN}\n\n| 元素 | 角色 | 标识种类 | 名称来自 | 角色描述 | 能操作 |\n|---|---|---|---|---|---|\n"
    );
    for e in TABLE.iter() {
        let roledesc = e.role_description.map_or("—".to_owned(), |t| {
            format!("文案 `{}`「{}」", t.key(), t.zh())
        });
        let id = if e.id.arity() == 0 {
            format!("`{}`", e.id.prefix())
        } else {
            format!("`{}:…`（{} 个数字）", e.id.prefix(), e.id.arity())
        };
        s.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} |\n",
            e.key,
            e.role.name(),
            id,
            name_md(e.name),
            roledesc,
            if e.operable { "是" } else { "否" }
        ));
    }
    s.push_str(&format!("\n{MD_END}\n"));
    s
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

    #[test]
    fn every_text_is_referenced_by_the_table() {
        // 012 第二节第 4 行：文案表里每条都被至少一处引用（未引用即红）
        for t in TextId::ALL {
            let used = TABLE.iter().any(|s| {
                s.role_description == Some(t)
                    || matches!(s.name, NameFrom::Text(x) | NameFrom::TextWithAlert(x) | NameFrom::WindowTitle(x) if x == t)
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
