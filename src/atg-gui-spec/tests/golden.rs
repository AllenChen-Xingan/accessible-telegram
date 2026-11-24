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
