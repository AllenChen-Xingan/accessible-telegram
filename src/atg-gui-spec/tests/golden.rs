//! 金样：生成的界面自描述与已提交的 `tests/golden/gui-schema.json` 逐字比。
//! 不一致时照 `atg_gui_spec::schema` 文件头的更新步骤重生成（并决定升不升 `GUI_SCHEMA_V`）。

#[test]
fn gui_schema_matches_golden() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/gui-schema.json");
    let golden = std::fs::read_to_string(path).unwrap_or_default();
    let now = atg_gui_spec::schema::render();
    assert!(
        golden == now,
        "界面自描述变了：跑 `cargo run -p atg-gui-spec --bin gui-schema > src/atg-gui-spec/tests/golden/gui-schema.json`，\
         再按 atg_gui_spec::schema 文件头的步骤盖章、抄哈希"
    );
}

/// `specs/gui/a11y-tree.md` 第三节的契约表段由 `gui-schema --md-elements` 生成（正本移交，012 第一节第 3 条）
#[test]
fn a11y_tree_md_elements_match_table() {
    use atg_gui_spec::schema::{MD_BEGIN, MD_END, elements_md};
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/gui/a11y-tree.md");
    let md = std::fs::read_to_string(path).unwrap_or_default();
    let got = match (md.find(MD_BEGIN), md.find(MD_END)) {
        (Some(b), Some(e)) if b < e => &md[b..e + MD_END.len()],
        _ => "",
    };
    assert!(
        format!("{got}\n") == elements_md(),
        "a11y-tree.md 第三节的生成段与契约表不一致（或找不到起止标记）：跑 \
         `cargo run -p atg-gui-spec --bin gui-schema -- --md-elements`，把输出替换起止标记之间那段"
    );
}
