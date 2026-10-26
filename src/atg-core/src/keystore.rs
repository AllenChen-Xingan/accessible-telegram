//! `keystore`：本地库加密钥匙放哪（拍板 001-8：加密，钥匙放钥匙串）。
//!
//! 规约：`specs/contracts/config-privacy.md` 第一节第 3 小节——32 字节随机数，首次建档时用系统安全随机源生成，
//! 存进钥匙串一条「通用密码」：服务名 `atg.tdlib-db-key`，账户名 = 档案名。不进环境变量、文件、日志、命令行参数。
//!
//! [`KeyStore`] 特征让测试换成内存版（CI 绝不碰钥匙串）。真钥匙串 [`Keychain`] 用原生接口：条目的访问名单
//! 记的是**建它的那个程序**的签名要求，所以开发版要用同一张开发证书、同一个标识签名（`codesign -s atg-dev
//! -i atg.dev`；证书由 `scripts/setup-dev-codesign.sh` 建），重编后才不弹框（005 N2：开发证书也管正式档案的钥匙）。
//!
//! 钥匙丢了（条目没了但本地库还在）：这里会生成新钥匙，TDLib 打开旧库时报 `Wrong database encryption key`
//! → `db_key_invalid`，由人决定是否清缓存。**绝不自动删库**。

use crate::error::{CoreError, KeySource};
use crate::secret::Secret;
use std::collections::HashMap;
use std::sync::Mutex;

/// 钥匙串条目的服务名
pub const SERVICE: &str = "atg.tdlib-db-key";
/// 钥匙长度（字节）
pub const KEY_LEN: usize = 32;

pub trait KeyStore: Send + Sync {
    /// 取档案的钥匙；没有就生成一把存进去再返回
    fn get_or_create(&self, profile: &str) -> Result<Secret, CoreError>;
}

fn random_key() -> Result<Secret, CoreError> {
    let mut buf = vec![0u8; KEY_LEN];
    getrandom::fill(&mut buf).map_err(|_| CoreError::Internal { what: "no_random" })?;
    Ok(Secret::new(buf))
}

/// 测试用：钥匙只在内存里
#[derive(Default)]
pub struct MemoryKeyStore {
    keys: Mutex<HashMap<String, Vec<u8>>>,
}

impl KeyStore for MemoryKeyStore {
    fn get_or_create(&self, profile: &str) -> Result<Secret, CoreError> {
        let mut keys = self.keys.lock().map_err(|_| CoreError::Internal {
            what: "keystore_poisoned",
        })?;
        if let Some(k) = keys.get(profile) {
            return Ok(Secret::new(k.clone()));
        }
        let k = random_key()?;
        keys.insert(profile.to_owned(), k.expose().to_vec());
        Ok(k)
    }
}

/// macOS 钥匙串（登录钥匙串里的通用密码条目）
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct Keychain;

#[cfg(target_os = "macos")]
impl KeyStore for Keychain {
    fn get_or_create(&self, profile: &str) -> Result<Secret, CoreError> {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        // errSecItemNotFound
        const NOT_FOUND: i32 = -25300;
        match get_generic_password(SERVICE, profile) {
            Ok(bytes) if bytes.len() == KEY_LEN => Ok(Secret::new(bytes)),
            // 条目在但长度不对：不覆盖（覆盖等于丢掉原钥匙），按钥匙不对报
            Ok(bytes) => {
                drop(Secret::new(bytes));
                Err(CoreError::DbKeyInvalid {
                    source: KeySource::WrongKey,
                })
            }
            Err(e) if e.code() == NOT_FOUND => {
                let key = random_key()?;
                set_generic_password(SERVICE, profile, key.expose()).map_err(|_| {
                    CoreError::DbKeyInvalid {
                        source: KeySource::KeychainDenied,
                    }
                })?;
                Ok(key)
            }
            Err(_) => Err(CoreError::DbKeyInvalid {
                source: KeySource::KeychainDenied,
            }),
        }
    }
}
