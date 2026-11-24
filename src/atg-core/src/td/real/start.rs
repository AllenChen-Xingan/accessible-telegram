//! 真实后端的 `start`：校验、占资源、建客户端、起收线程、交 `setTdlibParameters`（顺序见 `td.rs` 头注释）。
//! 从 `td/real.rs` 拆出（#60 R7）；根上 `Backend::start` 一行转给这里的 `do_start`，函数体照原样。

use super::lifecycle::{self, Claim};
use super::{Running, TdBackend, boot, source};
use crate::config::{self, Profile};
use crate::error::CoreError;
use crate::gate;
use crate::link;
use crate::logline::{self, Field};
use crate::secret;
use crate::status::CoreState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tdlib_rs::{enums, functions, types};

impl TdBackend {
    pub(super) async fn do_start(&self, profile: &Profile) -> Result<(), CoreError> {
        if self.shared.is_faulted() {
            return Err(CoreError::Disconnected);
        }
        let Some(claim) = Claim::take(&self.run) else {
            return Err(CoreError::Internal {
                what: "already_started",
            });
        };
        // 先做纯校验，再占资源（锁、标记、客户端、收线程）
        let db = profile.db_dir();
        let files = profile.files_dir();
        let (Some(db), Some(files)) = (db.to_str(), files.to_str()) else {
            return Err(CoreError::Internal {
                what: "profile_path_not_utf8",
            });
        };
        let profile_lock = profile.lock(self.cfg.holder)?;
        self.previous_unclean
            .store(profile.previous_unclean(), Ordering::SeqCst);
        let creds = config::load_api_credentials(&self.cfg.env_file)?;
        let key = self.cfg.keystore.get_or_create(profile.name())?;
        let key_b64 = secret::base64(key.expose());
        // 直发防重复的指纹密钥：由库钥匙带标签派生，只在内存里（B17-2）
        let guard_key = gate::derive_guard_key(key.expose(), profile.name());
        drop(key);
        boot::quiet_tdlib_log(&self.cfg.log_dir, profile.name())?;
        profile.mark_running()?;

        let use_test_dc = profile.kind().use_test_dc();
        // 闸只认这里实际传给 `setTdlibParameters` 的值
        self.cfg
            .gate
            .set_server(use_test_dc, profile.dir(), guard_key);
        let client_id = tdlib_rs::create_client();
        let receiver = match link::spawn_receiver(self.shared.clone(), source(client_id)) {
            Ok(h) => h,
            Err(e) => {
                // 收线程起不来：交一次 close 不等应答（tdlib-rs 1.4.0 首次 poll 即同步交出，`lib.rs:61-68`；升级要核）
                let _ = link::block_on_timeout(functions::close(client_id), Duration::ZERO);
                lifecycle::release(&mut Some(profile_lock), || profile.mark_clean());
                return Err(e);
            }
        };
        claim.commit(Running {
            client_id,
            use_test_dc,
            profile: profile.clone(),
            lock: Some(profile_lock),
            receiver: Some(receiver),
        });
        logline::info(
            "td_start",
            &[
                Field::Num("client_id", i64::from(client_id)),
                Field::Name("server", if use_test_dc { "test" } else { "main" }),
            ],
        );

        let params = boot::parameters(
            use_test_dc,
            db,
            files,
            key_b64.expose_str(),
            &creds,
            client_id,
        );
        drop(key_b64);
        drop(creds);
        link::guard(self.shared.clone(), params)
            .await?
            .map_err(|e| self.map_error(&e))?;
        self.shared.set_core(CoreState::Running);

        // 别人会看到「在线」：启动时显式设 false，不靠默认值（`actions.md` `set_online`）
        let offline = functions::set_option(
            "online".to_owned(),
            Some(enums::OptionValue::Boolean(types::OptionValueBoolean {
                value: false,
            })),
            client_id,
        );
        if let Err(e) = link::guard(self.shared.clone(), offline)
            .await?
            .map_err(|e| self.map_error(&e))
        {
            logline::warn("td_set_offline_failed", &[Field::Name("code", e.code())]);
        }
        Ok(())
    }
}
