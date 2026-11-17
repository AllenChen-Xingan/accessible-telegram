//! 发文件的本机一侧（#12）：哪些文件许发、大小上限、交出的快照副本、按图片还是按文件发（都是人定 2026-11-17，题面与选项见 `state/handoff/msg2.md` #12 节）。
//! 都不碰 TDLib，单测直接在临时目录里跑。

use crate::error::{CoreError, InvalidSendWhy};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// 文件最大多少字节（人定 2026-11-17：50 MB）
pub const FILE_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// 只许发这三个文件夹里的（人定 2026-11-17，按解析符号链接后的真实路径判）
pub const ALLOWED_DIRS: [&str; 3] = ["Downloads", "Desktop", "Documents"];

/// 按图片发的上限（TDLib `inputMessagePhoto`，`td_api.tl:6429`）：≤ 10 MB、宽 + 高 ≤ 10000、长宽比 ≤ 20
const PHOTO_MAX_BYTES: u64 = 10 * 1024 * 1024;
const PHOTO_MAX_SUM: u64 = 10_000;
const PHOTO_MAX_RATIO: u64 = 20;

/// 快照留多久：结果未知的那份留给 TDLib 下次启动续发时读，过了这么久就清（`config-privacy.md` `outbox/`）
pub const OUTBOX_KEEP_S: u64 = 24 * 3600;

/// 许发的路径要对照的几处：家目录（三个许可文件夹在它下面）、一律不许的目录（本应用的数据根与档案目录；TDLib 的库 `db/` 在档案目录里），
/// 快照放哪（档案目录下的 `outbox/`）
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct FileRoots {
    pub home: PathBuf,
    pub forbidden: Vec<PathBuf>,
    pub outbox: PathBuf,
}

impl FileRoots {
    /// 真机上的几处：家目录、本应用的数据根（档案目录的上两级，`config::default_root` 或测试给的根）与这个档案、
    /// 快照放档案目录下的 `outbox/`
    pub fn for_profile(profile: &crate::config::Profile) -> Result<Self, CoreError> {
        let dir = profile.dir().to_path_buf();
        let mut forbidden = vec![dir.clone()];
        if let Some(root) = dir.parent().and_then(Path::parent) {
            forbidden.push(root.to_path_buf());
        }
        Ok(Self {
            home: crate::config::home()?,
            forbidden,
            outbox: dir.join("outbox"),
        })
    }
}

fn bad(why: InvalidSendWhy) -> CoreError {
    CoreError::InvalidSend { why }
}

/// 许发、已打开的文件：判许可时与打开的是同一个文件实体，之后只从这个句柄读（不再按路径打开）
#[cfg_attr(test, derive(Debug))]
pub(super) struct Allowed {
    pub name: String,
    pub file: fs::File,
}

/// 只看路径的那部分规则：在三个许可文件夹之一里、不在一律不许的目录里、许可文件夹以下每一级名字都不像密钥
fn path_ok(real: &Path, roots: &FileRoots) -> bool {
    // 数据根、档案（TDLib 的库 `db/` 与下载的 `files/` 都在里面）：即使挪进（或链进）许可的文件夹也不许
    if roots
        .forbidden
        .iter()
        .any(|f| real.starts_with(fs::canonicalize(f).unwrap_or_else(|_| f.clone())))
    {
        return false;
    }
    let Some(rel) = ALLOWED_DIRS
        .iter()
        .filter_map(|d| fs::canonicalize(roots.home.join(d)).ok())
        .find_map(|root| real.strip_prefix(&root).ok().map(Path::to_path_buf))
    else {
        return false;
    };
    !rel.as_os_str().is_empty()
        && !rel
            .components()
            .any(|c| secret_name(&c.as_os_str().to_string_lossy()))
}

/// 许不许发（人定 2026-11-17「哪些文件能发」）并打开它：
/// 1. 解析符号链接后的真实路径过 [`path_ok`]。解析不了（不存在）的看它所在文件夹的真实路径：在许可文件夹里才说
///    `not_a_file`，否则一律 `file_not_allowed`——不让人借这条命令探别处有什么文件
/// 2. 按真实路径打开、**不跟随符号链接**（判完到打开之间最后一段被换成链接就打不开），打开的那个文件要和判许可时
///    看到的是同一个（设备号 + inode），且只有一个名字（硬链接数为 1：硬链接能让许可文件夹里的名字指向 `~/.ssh` 里的
///    私钥，真实路径看不出来）；否则 `file_not_allowed`
/// 3. 普通文件（否则 `not_a_file`）、≤ 50 MB（否则 `file_too_large`）
pub(super) fn allowed(p: &Path, roots: &FileRoots) -> Result<Allowed, CoreError> {
    use std::os::unix::fs::MetadataExt;
    let not_allowed = || bad(InvalidSendWhy::FileNotAllowed);
    let Ok(real) = fs::canonicalize(p) else {
        let inside = p
            .parent()
            .and_then(|d| {
                fs::canonicalize(if d.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    d
                })
                .ok()
            })
            .zip(p.file_name())
            .is_some_and(|(d, n)| path_ok(&d.join(n), roots));
        return Err(if inside {
            bad(InvalidSendWhy::NotAFile)
        } else {
            not_allowed()
        });
    };
    if !path_ok(&real, roots) {
        return Err(not_allowed());
    }
    let seen = fs::metadata(&real).map_err(|_| bad(InvalidSendWhy::NotAFile))?;
    if !seen.is_file() {
        return Err(bad(InvalidSendWhy::NotAFile));
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&real)
        .map_err(|_| not_allowed())?;
    let meta = file.metadata().map_err(|_| bad(InvalidSendWhy::NotAFile))?;
    if meta.dev() != seen.dev() || meta.ino() != seen.ino() || meta.nlink() != 1 {
        return Err(not_allowed());
    }
    if !meta.is_file() {
        return Err(bad(InvalidSendWhy::NotAFile));
    }
    if meta.len() > FILE_MAX_BYTES {
        return Err(bad(InvalidSendWhy::FileTooLarge));
    }
    let name = real
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or(bad(InvalidSendWhy::NotAFile))?;
    Ok(Allowed { name, file })
}

/// 像密钥、凭据或 Telegram 本地库的名字（不分大小写；文件名与它所在的各级文件夹名都查）。这份名单是正本，契约只举例、指向这里
pub(super) fn secret_name(name: &str) -> bool {
    const PREFIX: [&str; 6] = [
        ".env",
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        "credentials",
    ];
    const SUFFIX: [&str; 9] = [
        ".pem",
        ".key",
        ".p12",
        ".pfx",
        ".keystore",
        ".jks",
        ".session",
        ".binlog",
        ".kdbx",
    ];
    const EXACT: [&str; 6] = [".netrc", ".pgpass", ".ssh", ".gnupg", "tdata", "tdlib_db"];
    let n = name.to_lowercase();
    PREFIX.iter().any(|p| n.starts_with(p))
        || SUFFIX.iter().any(|s| n.ends_with(s))
        || EXACT.contains(&n.as_str())
}

/// 交出的快照副本：`outbox/<随机>/<原文件名>`（对方看到的文件名就是原名）
#[cfg_attr(test, derive(Debug))]
pub(super) struct Snapshot {
    /// 快照所在的那个随机目录（删快照删整个目录）
    pub dir: PathBuf,
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

/// 复制一份快照（目录 0700、文件 0600），从已打开的句柄读、边复制边算 SHA-256；读到超过上限就停、删掉、报
/// `file_too_large`（打开后文件还在长）。顺手清掉放了超过 [`OUTBOX_KEEP_S`] 的旧快照
pub(super) fn snapshot(src: Allowed, outbox: &Path) -> Result<Snapshot, CoreError> {
    let io = |_| CoreError::Internal {
        what: "outbox_write",
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(outbox)
        .map_err(io)?;
    sweep(outbox, OUTBOX_KEEP_S);
    let Allowed { name, mut file } = src;
    let dir = outbox.join(random_hex()?);
    fs::DirBuilder::new().mode(0o700).create(&dir).map_err(io)?;
    let path = dir.join(&name);
    match copy_hashed(&mut file, &path) {
        Ok((size, sha256)) => Ok(Snapshot {
            dir,
            path,
            name,
            size,
            sha256,
        }),
        Err(e) => {
            let _ = fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

fn copy_hashed(src: &mut fs::File, to: &Path) -> Result<(u64, [u8; 32]), CoreError> {
    let io = |_| CoreError::Internal {
        what: "outbox_write",
    };
    let mut dst = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(to)
        .map_err(io)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = src
            .read(&mut buf)
            .map_err(|_| bad(InvalidSendWhy::NotAFile))?;
        if n == 0 {
            break;
        }
        size += n as u64;
        // 打开后文件还在长：照样按上限拦
        if size > FILE_MAX_BYTES {
            return Err(bad(InvalidSendWhy::FileTooLarge));
        }
        h.update(&buf[..n]);
        dst.write_all(&buf[..n]).map_err(io)?;
    }
    dst.sync_all().map_err(io)?;
    Ok((size, h.finalize().into()))
}

/// 重算一个文件的 SHA-256（交出前复核快照没被换）
pub(super) fn sha256_of(p: &Path) -> Option<[u8; 32]> {
    let mut f = fs::File::open(p).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            return Some(h.finalize().into());
        }
        h.update(&buf[..n]);
    }
}

/// 清掉 `outbox/` 里放了超过 `keep_s` 秒的快照目录（尽力而为，失败不报）
pub(super) fn sweep(outbox: &Path, keep_s: u64) {
    let Ok(rd) = fs::read_dir(outbox) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age.as_secs() >= keep_s);
        if old {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

fn random_hex() -> Result<String, CoreError> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| CoreError::Internal { what: "random" })?;
    Ok(crate::gate::hex(&b))
}

/// 按图片发还是按文件发（人定 2026-11-17：看扩展名，按图片发不了的自动退回按文件发）
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub enum SendAs {
    /// `inputMessagePhoto`：Telegram 会压缩，对方直接看到图
    Photo { width: u32, height: u32 },
    /// `inputMessageDocument`：原样发。`fallback` 是本来该按图片发、为什么退回按文件发
    Document { fallback: Option<&'static str> },
}

impl SendAs {
    /// 留痕与计划行里的类型
    pub fn kind(self) -> &'static str {
        match self {
            Self::Photo { .. } => "photo",
            Self::Document { .. } => "document",
        }
    }
}

/// 看扩展名（`jpg` `jpeg` `png` `webp`）、大小与图片头里的宽高定怎么发
pub(super) fn send_as(snap: &Snapshot) -> SendAs {
    let ext = Path::new(&snap.name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return SendAs::Document { fallback: None };
    }
    let doc = |why| SendAs::Document {
        fallback: Some(why),
    };
    if snap.size > PHOTO_MAX_BYTES {
        return doc("photo_too_large");
    }
    // 读不出宽高（头坏了、扩展名不对）：按图片发多半被服务器拒，退回按文件发
    let Some((w, h)) = image_size(&snap.path) else {
        return doc("photo_unreadable");
    };
    let (a, b) = (u64::from(w.max(h)), u64::from(w.min(h).max(1)));
    if u64::from(w) + u64::from(h) > PHOTO_MAX_SUM {
        return doc("photo_too_big");
    }
    if a > b * PHOTO_MAX_RATIO {
        return doc("photo_too_narrow");
    }
    SendAs::Photo {
        width: w,
        height: h,
    }
}

/// 从文件头读宽高：PNG、JPEG、WebP（VP8 / VP8L / VP8X）。读不出回 `None`
pub(super) fn image_size(p: &Path) -> Option<(u32, u32)> {
    let mut f = fs::File::open(p).ok()?;
    let mut head = Vec::new();
    Read::by_ref(&mut f)
        .take(512 * 1024)
        .read_to_end(&mut head)
        .ok()?;
    let d = head.as_slice();
    let be16 = |i: usize| -> Option<u32> {
        Some(u32::from(u16::from_be_bytes([*d.get(i)?, *d.get(i + 1)?])))
    };
    let le16 = |i: usize| -> Option<u32> {
        Some(u32::from(u16::from_le_bytes([*d.get(i)?, *d.get(i + 1)?])))
    };
    let le24 = |i: usize| -> Option<u32> {
        Some(
            u32::from(*d.get(i)?)
                | u32::from(*d.get(i + 1)?) << 8
                | u32::from(*d.get(i + 2)?) << 16,
        )
    };
    let dims = |w: u32, h: u32| (w > 0 && h > 0).then_some((w, h));
    if d.starts_with(b"\x89PNG\r\n\x1a\n") && d.get(12..16)? == b"IHDR" {
        let w = u32::from_be_bytes(d.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(d.get(20..24)?.try_into().ok()?);
        return dims(w, h);
    }
    if d.starts_with(b"RIFF") && d.get(8..12)? == b"WEBP" {
        return match d.get(12..16)? {
            b"VP8 " => dims(le16(26)? & 0x3fff, le16(28)? & 0x3fff),
            b"VP8L" => {
                let b = |i: usize| d.get(21 + i).map(|x| u32::from(*x));
                let w = 1 + (b(0)? | (b(1)? & 0x3f) << 8);
                let h = 1 + (b(1)? >> 6 | b(2)? << 2 | (b(3)? & 0x0f) << 10);
                dims(w, h)
            }
            b"VP8X" => dims(1 + le24(24)?, 1 + le24(27)?),
            _ => None,
        };
    }
    if d.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        while i + 3 < d.len() {
            if d[i] != 0xff {
                return None;
            }
            let m = d[i + 1];
            // 填充字节
            if m == 0xff {
                i += 1;
                continue;
            }
            // SOF0–SOF15（除了 DHT C4、JPG C8、DAC CC）带宽高
            if (0xc0..=0xcf).contains(&m) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
                return dims(be16(i + 7)?, be16(i + 5)?);
            }
            i += 2 + usize::try_from(be16(i + 2)?).ok()?;
        }
    }
    None
}
