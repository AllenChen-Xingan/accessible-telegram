//! 选档案（#61）：缺标记时只有全新的 main 才新建；留着数据的、读不懂标记的都拒绝，且不动盘

use super::pick_profile;
use atg_core::config::{Profile, ServerKind};
use atg_core::error::CoreError;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "atg-pick-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn internal(what: &'static str) -> Option<CoreError> {
    Some(CoreError::Internal { what })
}

/// 正式档案留着库却丢了标记
fn main_without_marker(root: &Path) -> PathBuf {
    let p = Profile::create(root, "main", ServerKind::Main).unwrap();
    std::fs::write(p.db_dir().join("stub"), "x").unwrap();
    std::fs::remove_file(p.dir().join("server-kind")).unwrap();
    std::fs::set_permissions(p.dir(), std::fs::Permissions::from_mode(0o750)).unwrap();
    p.dir().to_path_buf()
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn first_main_is_created_as_main() {
    let root = temp_root("first");
    let p = pick_profile(&root, false, "main").unwrap();
    assert_eq!(p.kind(), ServerKind::Main);
}

#[test]
fn other_names_must_exist() {
    let root = temp_root("other");
    assert_eq!(
        pick_profile(&root, false, "work").err(),
        internal("profile_marker_missing")
    );
    assert!(!root.join("profiles").join("work").exists());
}

/// 正式档案丢了标记：不带 `--test-dc` 和带 `--test-dc` 都拒绝，不补写、连权限都不改
#[test]
fn main_with_data_but_no_marker_is_refused_either_way() {
    let root = temp_root("relabel");
    let dir = main_without_marker(&root);
    for (test_dc, name) in [(false, "main"), (true, "main")] {
        assert_eq!(
            pick_profile(&root, test_dc, name).err(),
            internal("profile_marker_missing"),
            "test_dc={test_dc}"
        );
        assert!(!dir.join("server-kind").exists());
        assert_eq!(mode(&dir), 0o750);
    }
}

/// 读不懂的标记原样报，不转去 `create`（不建子目录、不改标记）
#[test]
fn garbled_marker_is_reported_without_side_effects() {
    let root = temp_root("garbled");
    let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
    std::fs::write(p.dir().join("server-kind"), "maybe").unwrap();
    std::fs::remove_dir(p.files_dir()).unwrap();
    assert_eq!(
        pick_profile(&root, false, "main").err(),
        internal("profile_marker_unreadable")
    );
    assert!(!p.files_dir().exists());
    assert_eq!(
        std::fs::read_to_string(p.dir().join("server-kind")).unwrap(),
        "maybe"
    );
}

/// `--test-dc` 撞上正式档案：先报种类不符，不先动子目录（`db` 是普通文件也照样报 mismatch）
#[test]
fn test_dc_on_main_profile_is_kind_mismatch_before_touching_disk() {
    let root = temp_root("mismatch");
    let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
    std::fs::remove_dir(p.db_dir()).unwrap();
    std::fs::write(p.db_dir(), "x").unwrap();
    assert_eq!(
        pick_profile(&root, true, "main").err(),
        internal("profile_kind_mismatch")
    );
    assert!(p.db_dir().is_file());
}
