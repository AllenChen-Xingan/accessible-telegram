//! 配置、档案、钥匙、秘密、日志白名单（`specs/contracts/config-privacy.md`）。CI 里跑：不碰钥匙串、不碰真实档案目录

use atg_core::config::{Profile, ServerKind, load_api_credentials};
use atg_core::error::{CoreError, Holder};
use atg_core::keystore::{KEY_LEN, KeyStore, MemoryKeyStore};
use atg_core::logline::{Field, render, safe_td_type};
use atg_core::secret::{Secret, base64};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "atg-cfg-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn profile_dirs_are_private_and_kind_comes_from_marker() {
    let root = temp_root("dirs");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    assert!(p.kind().use_test_dc());
    for d in [p.dir().to_path_buf(), p.db_dir(), p.files_dir()] {
        assert_eq!(mode(&d), 0o700, "{}", d.display());
    }
    // 再开：种类只从标记来
    assert_eq!(
        Profile::open(&root, "test").unwrap().kind(),
        ServerKind::Test
    );
    // 测试档案当正式档案建 → 拒绝
    assert_eq!(
        Profile::create(&root, "test", ServerKind::Main).err(),
        Some(CoreError::Internal {
            what: "profile_kind_mismatch"
        })
    );
}

#[test]
fn missing_or_garbled_marker_refuses_to_start() {
    let root = temp_root("marker");
    assert_eq!(
        Profile::open(&root, "main").err(),
        Some(CoreError::Internal {
            what: "profile_marker_missing"
        })
    );
    let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
    std::fs::write(p.dir().join("server-kind"), "maybe").unwrap();
    assert_eq!(
        Profile::open(&root, "main").err(),
        Some(CoreError::Internal {
            what: "profile_marker_unreadable"
        })
    );
}

/// #61：已有数据却缺标记的档案，`create` 不许按调用方补写种类（正式库被 `--test-dc` 改标成测试档案）。
/// 数据不只 TDLib 的库：档案目录里留着任何落盘状态（如账号被限标记）都算
#[test]
fn create_never_relabels_existing_profile_without_marker() {
    let missing = Some(CoreError::Internal {
        what: "profile_marker_missing",
    });
    for leftover in ["db/stub", "files/stub", "account-limited"] {
        let sub = leftover.replace('/', "-");
        let root = temp_root(&format!("relabel-{sub}"));
        let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
        std::fs::write(p.dir().join(leftover), "x").unwrap();
        std::fs::remove_file(p.dir().join("server-kind")).unwrap();
        for kind in [ServerKind::Test, ServerKind::Main] {
            assert_eq!(Profile::create(&root, "main", kind).err(), missing, "{sub}");
            assert!(!p.dir().join("server-kind").exists(), "{sub}");
        }
        assert_eq!(Profile::open(&root, "main").err(), missing, "{sub}");
    }
}

/// 新档案（目录不存在，或里面只有空的 `db/`、`files/`）照常写标记
#[test]
fn create_writes_marker_only_for_fresh_profile() {
    let root = temp_root("fresh");
    assert_eq!(
        Profile::create(&root, "test", ServerKind::Test)
            .unwrap()
            .kind(),
        ServerKind::Test
    );
    // 目录在、子目录空、缺标记：仍算新档案
    let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
    std::fs::remove_file(p.dir().join("server-kind")).unwrap();
    assert_eq!(
        Profile::create(&root, "main", ServerKind::Main)
            .unwrap()
            .kind(),
        ServerKind::Main
    );
}

/// 读不懂的标记不当缺失处理，`create` 也不覆盖它
#[test]
fn create_keeps_garbled_marker() {
    let root = temp_root("garbled");
    let p = Profile::create(&root, "main", ServerKind::Main).unwrap();
    std::fs::write(p.dir().join("server-kind"), "maybe").unwrap();
    assert_eq!(
        Profile::create(&root, "main", ServerKind::Main).err(),
        Some(CoreError::Internal {
            what: "profile_marker_unreadable"
        })
    );
    assert_eq!(
        std::fs::read_to_string(p.dir().join("server-kind")).unwrap(),
        "maybe"
    );
}

/// `db/` 读不了（不是目录）按「不是新的」处理，宁可拒绝
#[test]
fn unreadable_data_dir_is_not_fresh() {
    let root = temp_root("notdir");
    let dir = root.join("profiles").join("main");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("db"), "x").unwrap();
    assert_eq!(
        Profile::create(&root, "main", ServerKind::Main).err(),
        Some(CoreError::Internal {
            what: "profile_marker_missing"
        })
    );
    assert!(!dir.join("server-kind").exists());
}

#[test]
fn profile_names_cannot_escape() {
    let root = temp_root("names");
    for bad in ["", "../x", "a/b", "Main", "-x", &"a".repeat(33)] {
        assert!(
            Profile::create(&root, bad, ServerKind::Test).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn second_lock_is_profile_busy_and_names_holder() {
    let root = temp_root("lock");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    let held = p.lock(Holder::Gui).unwrap();
    assert_eq!(
        p.lock(Holder::Cli).err(),
        Some(CoreError::ProfileBusy {
            holder: Some(Holder::Gui)
        })
    );
    drop(held);
    assert!(p.lock(Holder::Cli).is_ok());
}

#[test]
fn running_marker_reports_unclean_shutdown() {
    let root = temp_root("unclean");
    let p = Profile::create(&root, "test", ServerKind::Test).unwrap();
    assert!(!p.previous_unclean());
    p.mark_running().unwrap();
    assert!(p.previous_unclean());
    p.mark_clean();
    assert!(!p.previous_unclean());
}

#[test]
fn phone_must_match_profile_kind() {
    assert!(ServerKind::Test.phone_allowed("+999 66 1 2345"));
    assert!(!ServerKind::Test.phone_allowed("+1 555 0100"));
    assert!(ServerKind::Main.phone_allowed("+1 555 0100"));
    assert!(!ServerKind::Main.phone_allowed("99966 1"));
}

#[test]
fn env_local_parses_and_errors_carry_no_values() {
    let root = temp_root("env");
    let f = root.join(".env.local");
    std::fs::write(
        &f,
        "# c\nexport ATG_API_ID=1234\nATG_API_HASH=\"FAKE-HASH-PLACEHOLDER\"\nOTHER=1\n",
    )
    .unwrap();
    let c = load_api_credentials(&f).unwrap();
    assert_eq!(c.api_id, 1234);
    assert_eq!(c.api_hash.expose_str(), "FAKE-HASH-PLACEHOLDER");
    assert_eq!(format!("{:?}", c.api_hash), "<secret>");

    std::fs::write(&f, "ATG_API_HASH=FAKE-HASH-PLACEHOLDER\n").unwrap();
    let e = load_api_credentials(&f).err().unwrap();
    assert!(!format!("{e:?}").contains("FAKE-HASH-PLACEHOLDER"));
    assert!(load_api_credentials(&root.join("nope")).is_err());
}

#[test]
fn memory_keystore_keeps_one_random_key_per_profile() {
    let ks = MemoryKeyStore::default();
    let a1 = ks.get_or_create("test").unwrap();
    let a2 = ks.get_or_create("test").unwrap();
    let b = ks.get_or_create("main").unwrap();
    assert_eq!(a1.len(), KEY_LEN);
    assert_eq!(a1.expose(), a2.expose());
    assert_ne!(a1.expose(), b.expose());
    assert_eq!(format!("{a1:?}"), "<secret>");
}

#[test]
fn base64_matches_known_vectors() {
    for (raw, want) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(raw.as_bytes()).expose_str(), want);
    }
    assert_eq!(base64(&[0xff, 0xfe, 0xfd]).expose_str(), "//79");
    assert_eq!(base64(&[0u8; 32]).len(), 44);
    let _ = Secret::new(vec![1]);
}

#[test]
fn log_fields_are_whitelisted() {
    let line = render(
        "td_start",
        &[
            Field::Num("client_id", 1),
            Field::Name("server", "test"),
            Field::TdType("t", "updateNewMessage"),
            Field::TdType("bad", "晚上 一起吃饭"),
        ],
    );
    assert_eq!(
        line,
        "td_start client_id=1 server=test t=updateNewMessage bad=<invalid>"
    );
    assert_eq!(safe_td_type(&"a".repeat(65)), "<invalid>");
}

#[test]
fn test_phone_only_from_env_file_and_only_test_numbers() {
    use atg_core::config::load_test_phone;
    let dir = std::env::temp_dir().join(format!("atg-test-phone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("env");
    std::fs::write(
        &f,
        "ATG_API_ID=1234\nexport ATG_TEST_PHONE=\"9996621234\"\n",
    )
    .unwrap();
    assert_eq!(
        load_test_phone(&f)
            .map(|s| s.expose_str().to_owned())
            .as_deref(),
        Some("9996621234")
    );
    std::fs::write(&f, "ATG_TEST_PHONE=+8613800000000\n").unwrap();
    assert!(load_test_phone(&f).is_none(), "不是测试号就不认");
    std::fs::write(&f, "ATG_API_ID=1\n").unwrap();
    assert!(load_test_phone(&f).is_none());
    assert!(load_test_phone(&dir.join("missing")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
