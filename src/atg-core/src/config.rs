//! `config`：`.env.local`、档案（profile）目录、档案标记、「一库一进程」的锁、上次是否正常关闭。
//!
//! 规约正本：`specs/contracts/config-privacy.md` 第一节第 1、2 小节。
//! - 档案目录：`~/Library/Application Support/atg/profiles/<档案>/`，`db/`、`files/` 都在里面，全部 `0700`，在仓外
//! - `use_test_dc`（连 Telegram 测试服务器，DC = 数据中心）**只**从档案标记来；标记缺失或读不懂就拒绝启动
//! - 起 TDLib 之前先拿档案目录里的 `atg.lock`，拿不到就 `profile_busy`
//! - 路径里不出现手机号、用户名；本模块不打印任何值

use crate::error::{CoreError, Holder};
use crate::secret::Secret;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

// ───────────────────────── .env.local ─────────────────────────

/// `api_id` / `api_hash`（开发期来自仓库根 `.env.local` 的 `ATG_API_ID` / `ATG_API_HASH`）
pub struct ApiCredentials {
    pub api_id: i32,
    pub api_hash: Secret,
}

/// 开发期 `.env.local` 的默认位置：仓库根（按本 crate 的源码位置算，与当前目录无关）
pub fn default_env_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.env.local")
}

/// 读 `.env.local` 里的 `ATG_API_ID` / `ATG_API_HASH`。不依赖 shell 导出；不认环境变量（避免传给子进程）。
/// 出错只说缺什么，不带任何值
pub fn load_api_credentials(path: &Path) -> Result<ApiCredentials, CoreError> {
    let mut raw = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut raw))
        .map_err(|_| CoreError::Internal {
            what: "env_local_unreadable",
        })?;
    let raw = Secret::from_string(raw); // 整份文件也按秘密处理：用完清零
    let mut api_id = None;
    let mut api_hash = None;
    for line in raw.expose_str().lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
        match k.trim() {
            "ATG_API_ID" => api_id = v.parse::<i32>().ok(),
            "ATG_API_HASH" if !v.is_empty() => api_hash = Some(Secret::new(v.as_bytes().to_vec())),
            _ => {}
        }
    }
    match (api_id, api_hash) {
        (Some(api_id), Some(api_hash)) if api_id > 0 => Ok(ApiCredentials { api_id, api_hash }),
        (None, _) | (Some(_), _) => Err(CoreError::Internal {
            what: "env_local_missing_api_credentials",
        }),
    }
}

/// 测试服务器的号码 `ATG_TEST_PHONE`（`.env.local`，`config-privacy.md` 第一节第 1 小节）。
/// 只认 `99966` 开头的；没有、读不到或不是测试号都给 `None`（调用方再去终端问）。不认环境变量
pub fn load_test_phone(path: &Path) -> Option<Secret> {
    let mut raw = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut raw))
        .ok()?;
    let raw = Secret::from_string(raw);
    let found = raw.expose_str().lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (k, v) = line.split_once('=')?;
        (k.trim() == "ATG_TEST_PHONE").then(|| {
            Secret::new(
                v.trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .as_bytes()
                    .to_vec(),
            )
        })
    })?;
    ServerKind::Test
        .phone_allowed(found.expose_str())
        .then_some(found)
}

// ───────────────────────── 档案 ─────────────────────────

/// 档案连的是哪种服务器。测试与正式绝不共用一个档案
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Main,
    Test,
}

impl ServerKind {
    fn marker(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Test => "test",
        }
    }

    pub fn use_test_dc(self) -> bool {
        self == Self::Test
    }

    /// 号码与档案对得上：测试档案只收 `99966` 开头，正式档案拒收 `99966` 开头（第 2 小节）。
    /// `99966X…` 是 Telegram 给测试服务器留的号码段（X = 测试数据中心号，验证码是 X 连写五遍，`testing.md`
    /// 第一节第 5 小节）——用它卡住「拿测试号连正式服务器」或反过来
    pub fn phone_allowed(self, phone: &str) -> bool {
        let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
        digits.starts_with("99966") == (self == Self::Test)
    }
}

const MARKER: &str = "server-kind";
const LOCK: &str = "atg.lock";
const RUNNING: &str = "running";

/// 一个档案：一套独立的本地库 + 文件目录 + 钥匙（钥匙在钥匙串，账户名 = 档案名）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    name: String,
    dir: PathBuf,
    kind: ServerKind,
}

/// 本应用的数据根：`~/Library/Application Support/atg`
pub fn default_root() -> Result<PathBuf, CoreError> {
    home().map(|h| h.join("Library/Application Support/atg"))
}

/// 日志目录：`~/Library/Logs/atg`
pub fn default_log_dir() -> Result<PathBuf, CoreError> {
    home().map(|h| h.join("Library/Logs/atg"))
}

fn home() -> Result<PathBuf, CoreError> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or(CoreError::Internal { what: "no_home" })
}

/// 档案名只许小写字母、数字、连字符，1–32 个（不进路径以外的东西，也防 `..`）
fn valid_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
}

/// 建目录（含上层）并把它收紧到 `0700`
pub fn ensure_private_dir(dir: &Path) -> Result<(), CoreError> {
    let io = |_| CoreError::Internal {
        what: "profile_dir_io",
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(io)
}

impl Profile {
    /// 建一个档案（已有则核对种类）：建目录、写标记。种类对不上就拒绝——测试与正式绝不混用
    pub fn create(root: &Path, name: &str, kind: ServerKind) -> Result<Self, CoreError> {
        if !valid_name(name) {
            return Err(CoreError::InvalidInput { td: None });
        }
        let dir = root.join("profiles").join(name);
        ensure_private_dir(&root.join("profiles"))?;
        ensure_private_dir(&dir)?;
        match read_marker(&dir) {
            Ok(k) if k == kind => {}
            Ok(_) => {
                return Err(CoreError::Internal {
                    what: "profile_kind_mismatch",
                });
            }
            Err(MarkerError::Missing) => write_private(&dir.join(MARKER), kind.marker())?,
            Err(MarkerError::Unreadable) => {
                return Err(CoreError::Internal {
                    what: "profile_marker_unreadable",
                });
            }
        }
        Self::open(root, name)
    }

    /// 打开已有档案。标记缺失或读不懂就拒绝（`use_test_dc` 只从它来）
    pub fn open(root: &Path, name: &str) -> Result<Self, CoreError> {
        if !valid_name(name) {
            return Err(CoreError::InvalidInput { td: None });
        }
        let dir = root.join("profiles").join(name);
        let kind = read_marker(&dir).map_err(|e| CoreError::Internal {
            what: match e {
                MarkerError::Missing => "profile_marker_missing",
                MarkerError::Unreadable => "profile_marker_unreadable",
            },
        })?;
        for sub in ["db", "files"] {
            ensure_private_dir(&dir.join(sub))?;
        }
        ensure_private_dir(&dir)?;
        Ok(Self {
            name: name.to_owned(),
            dir,
            kind,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> ServerKind {
        self.kind
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn db_dir(&self) -> PathBuf {
        self.dir.join("db")
    }
    pub fn files_dir(&self) -> PathBuf {
        self.dir.join("files")
    }

    /// 拿「一库一进程」的锁（起 TDLib **之前**）。拿不到 → `profile_busy`，尽量说清是 CLI 还是 GUI 占着
    pub fn lock(&self, holder: Holder) -> Result<ProfileLock, CoreError> {
        let path = self.dir.join(LOCK);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(|_| CoreError::Internal {
                what: "profile_lock_io",
            })?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut who = String::new();
                let _ = file.read_to_string(&mut who);
                let holder = match who.split_whitespace().next() {
                    Some("cli") => Some(Holder::Cli),
                    Some("gui") => Some(Holder::Gui),
                    _ => None,
                };
                return Err(CoreError::ProfileBusy { holder });
            }
            Err(std::fs::TryLockError::Error(_)) => {
                return Err(CoreError::Internal {
                    what: "profile_lock_io",
                });
            }
        }
        let tag = match holder {
            Holder::Cli => "cli",
            Holder::Gui => "gui",
        };
        let written = file
            .set_len(0)
            .and_then(|()| write!(file, "{tag} {}", std::process::id()));
        written.map_err(|_| CoreError::Internal {
            what: "profile_lock_io",
        })?;
        Ok(ProfileLock { _file: file })
    }

    /// 上次是否没有正常关闭（「运行中」标记还在）。读一次后由 [`Profile::mark_running`] 重新放上
    pub fn previous_unclean(&self) -> bool {
        self.dir.join(RUNNING).exists()
    }

    /// 放上「运行中」标记；正常关闭时 [`Profile::mark_clean`] 拿掉。超时或故障时留着，下次启动提示
    pub fn mark_running(&self) -> Result<(), CoreError> {
        write_private(&self.dir.join(RUNNING), "")
    }

    pub fn mark_clean(&self) {
        let _ = fs::remove_file(self.dir.join(RUNNING));
    }
}

/// 档案锁：活着就占着，丢掉即释放（进程退出时系统也会放）
pub struct ProfileLock {
    _file: File,
}

enum MarkerError {
    Missing,
    Unreadable,
}

fn read_marker(dir: &Path) -> Result<ServerKind, MarkerError> {
    match fs::read_to_string(dir.join(MARKER)) {
        Ok(s) => match s.trim() {
            "main" => Ok(ServerKind::Main),
            "test" => Ok(ServerKind::Test),
            _ => Err(MarkerError::Unreadable),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(MarkerError::Missing),
        Err(_) => Err(MarkerError::Unreadable),
    }
}

fn write_private(path: &Path, content: &str) -> Result<(), CoreError> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(content.as_bytes()))
        .map_err(|_| CoreError::Internal {
            what: "profile_dir_io",
        })
}
