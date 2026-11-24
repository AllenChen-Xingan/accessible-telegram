//! `--fake <场景>` 的共用一步：解析场景 → 建假后端、装数据 → 建一次性测试档案（arch 审定，2026-11-24）。
//! 场景格式与加载器的正本在 `atg_core::fake::Scenario`；现在只有 `chats`、`history` 走这里，别的命令照旧 `not_implemented`。
//!
//! 一次性档案：系统临时目录下新建的私有目录（0700），`Profile::create(dir, "main", Test)`，跑这件事的时候照常拿它的
//! `atg.lock`；[`FakeRun`] 一丢（跑完、出错、提前返回）目录就删。不碰人的正式档案目录。
//! 档案名照 CLI 默认叫 `main`，但它建在这个临时目录里，和人的档案目录（`profiles/main`）不相干。
//!
//! **这条路不碰钥匙串**。为什么要紧：真实后端起来时要从 macOS 钥匙串取本地库的加密钥匙（「库钥匙」，
//! `specs/contracts/config-privacy.md` 第一节第 3 小节），会动到人的真凭据、可能弹窗。三层证明（arch 审定）：
//! 1. 类型：这里只构造具体类型 [`FakeBackend`]，造不出真实后端；假后端的库钥匙是写死的，不经钥匙串
//! 2. 文本扫描：`tests/fake_scenario.rs` 的 `fake_path_never_mentions_the_keychain_or_real_backend` 扫本文件、假后端全部源码与
//!    `config.rs` 里私有临时目录那一段，出现钥匙串模块、真实后端的名字就红（清单是那个测试文件里的 `FORBIDDEN`；
//!    本注释不写出这几个名字，否则扫描会扫到自己）
//! 3. 调用路径：下面的单测核对假后端记下的调用只有起、订阅、拉列表、关这几步
//!
//! 靠的是类型和文本扫描（加一条调用记录核对），运行时没有去 hook 钥匙串

use crate::out::Fail;
use atg_core::config::{Profile, ServerKind, TempDirGuard};
use atg_core::fake::{FakeBackend, Scenario};
use std::ffi::OsStr;

/// 装好场景的假后端 + 它用的一次性测试档案。字段按声明顺序丢：先后端、再档案，最后删目录
pub struct FakeRun {
    pub backend: FakeBackend,
    pub profile: Profile,
    _dir: TempDirGuard,
}

/// 起一次：场景名（`[a-z0-9-]+`，只在开发构建里认）或场景目录路径
pub fn open(scenario: &OsStr) -> Result<FakeRun, Fail> {
    let sc = Scenario::resolve(scenario)
        .and_then(|dir| Scenario::load(&dir))
        .map_err(Fail::Core)?;
    let backend = FakeBackend::new().map_err(Fail::Core)?;
    sc.install(&backend);
    let dir = TempDirGuard::new("fake").map_err(Fail::Core)?;
    let profile = Profile::create(dir.path(), "main", ServerKind::Test).map_err(Fail::Core)?;
    Ok(FakeRun {
        backend,
        profile,
        _dir: dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use atg_core::fake::Call;
    use atg_core::model::ChatList;

    #[test]
    fn chats_on_basic_scenario_uses_test_profile_and_only_expected_calls() {
        // 用路径，开发构建、发布构建都认（按名字找只在开发构建里有）
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../atg-core/fixtures/scenarios/basic-4-chats"
        );
        let Ok(f) = open(OsStr::new(dir)) else {
            panic!("basic-4-chats 场景应能装");
        };
        assert_eq!(f.profile.kind(), ServerKind::Test);
        let dir = f.profile.dir().to_path_buf();
        let q = crate::chats::Query {
            list: ChatList::Main,
            limit: 50,
            unread: false,
            after: None,
        };
        let listing = match crate::chats::run(&f.backend, &f.profile, &q, std::time::Instant::now())
        {
            Ok(l) => l,
            Err(_) => panic!("basic-4-chats 上 chats 应成功"),
        };
        let ids: Vec<i64> = listing.items.iter().map(|c| c.chat_id).collect();
        assert_eq!(ids, vec![101, -1001, -1002, 9]);
        // 路径层：只有起、订阅、拉列表、关这几步（没有登录动作，更没有任何真实后端才有的步骤）
        for c in f.backend.calls() {
            assert!(
                matches!(
                    c,
                    Call::Start {
                        use_test_dc: true,
                        ..
                    } | Call::Subscribe
                        | Call::ChatsLoad { .. }
                        | Call::Shutdown
                ),
                "意外的调用"
            );
        }
        drop(f);
        assert!(!dir.exists(), "一次性档案目录应已删掉");
    }

    #[test]
    fn missing_scenario_is_fixture_bad() {
        match open(OsStr::new("/nonexistent/atg-scenario")) {
            Err(Fail::Core(atg_core::error::CoreError::Internal { what })) => {
                assert_eq!(what, "fixture_bad")
            }
            _ => panic!("缺场景应报 fixture_bad"),
        }
    }
}
