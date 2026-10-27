//! `ipc`：本机接口——占着档案的进程（执行方）在 `profiles/<档案>/atg.sock` 收命令，后起的 `atg-cli` 把命令转过去。
//!
//! 规约正本：`specs/contracts/config-privacy.md` 第一节第 2 小节「本机接口」（含协议 v0）。本模块只管传输：
//! 套接字路径、绑定（含上次残留的文件）、对端核对、分帧。命令怎么执行、输出长什么样归调用方（现在是 `atg-cli`）。
//!
//! - **谁能连**：档案目录 `0700`、套接字 `0600`；两头连上后都用 `getpeereid` 核对对端的用户 id，不是本用户即断
//! - **残留文件**：执行方先拿到档案锁才绑定，锁在手就说明没有别的活执行方——残留的 `atg.sock` 是上次没正常关
//!   留下的，删掉重绑。保险起见删之前先试连一次，连得上就不删、报错
//! - **路径上限**：macOS 的 `sun_path` 104 字节含结尾的 0，路径最多 103 字节；超了不绑（`socket_path_too_long`）
//! - **协议 v0**：一连一请求。请求一行 JSON（[`Request`]）；应答若干行 [`Frame`]：`out`（原样写标准输出）·
//!   `err`（原样写标准错误）· 最后恰好一行 `exit`（退出码）
//!
//! 隐私：请求里的参数以后会带内容（如 `send --text`），所以 [`Request`] 不派生 `Debug`，本模块也不记日志。

use crate::config::Profile;
use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 协议版本 v0（与 CLI 外壳的 `v` 无关：这是两个进程之间的线格式）
pub const PROTOCOL_V: u32 = 0;
/// 套接字文件名
pub const SOCKET: &str = "atg.sock";
/// 路径最多几个字节（macOS `sun_path` 104 字节，含结尾的 0）
pub const SOCKET_PATH_MAX: usize = 103;
/// 一行最长多少字节：防对端发一行永不结束的东西把内存吃光
pub const MAX_LINE: usize = 1 << 20;
/// 执行方等请求那一行最多多久（连上不说话的不许占着执行方）
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// 执行方写一帧最多等多久（发起方不读了，比如被挂起）
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(60);

/// 某个档案的套接字路径
pub fn socket_path(profile: &Profile) -> PathBuf {
    profile.dir().join(SOCKET)
}

/// 路径长度在上限内
pub fn check_path_len(path: &Path) -> Result<(), CoreError> {
    if path.as_os_str().len() > SOCKET_PATH_MAX {
        return Err(CoreError::Internal {
            what: "socket_path_too_long",
        });
    }
    Ok(())
}

fn io_err(_: io::Error) -> CoreError {
    CoreError::Internal { what: "ipc_io" }
}

// ───────────────────────── 对端核对 ─────────────────────────

/// 本进程的有效用户 id
pub fn my_uid() -> u32 {
    // SAFETY: geteuid 没有前置条件，不会失败
    unsafe { libc::geteuid() }
}

/// 对端进程的有效用户 id（`getpeereid`）
pub fn peer_uid(s: &UnixStream) -> io::Result<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: fd 在 `s` 活着期间有效；两个出参指向本栈上的变量
    let r = unsafe { libc::getpeereid(s.as_raw_fd(), &raw mut uid, &raw mut gid) };
    if r == 0 {
        Ok(uid)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// 对端已经关了连接（不读、不等：看一眼有没有读到结尾）。流式订阅时执行方靠它发现 `watch` 已走，
/// 不必等到下一次写失败。对端还发着别的东西、或暂时没东西，都算没关
pub fn peer_closed(s: &UnixStream) -> bool {
    let mut b = [0u8; 1];
    // SAFETY: fd 在 `s` 活着期间有效；缓冲在本栈上、长度 1；MSG_PEEK 不取走数据，MSG_DONTWAIT 不阻塞
    let n = unsafe {
        libc::recv(
            s.as_raw_fd(),
            b.as_mut_ptr().cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    match n {
        0 => true,
        n if n > 0 => false,
        _ => !matches!(
            io::Error::last_os_error().kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        ),
    }
}

/// 对端是本用户才放行；查不到也不放行
pub fn peer_allowed(peer: io::Result<u32>, me: u32) -> bool {
    matches!(peer, Ok(uid) if uid == me)
}

// ───────────────────────── 执行方 ─────────────────────────

/// 执行方在听的套接字。丢掉时删掉套接字文件
pub struct Listener {
    inner: UnixListener,
    path: PathBuf,
}

impl Listener {
    /// 在档案目录里绑定 `atg.sock`。**调用方须已拿到这个档案的锁**（真实后端 `start` 成功即是；锁在后端里，
    /// 拿不出来当参数，只能靠这条约定——GUI 当执行方时照此办理）：锁在手就没有别的活执行方，残留文件可以删
    pub fn bind(profile: &Profile) -> Result<Self, CoreError> {
        Self::bind_at(&socket_path(profile))
    }

    /// 同 [`Listener::bind`]，直接给路径（单测用）
    pub fn bind_at(path: &Path) -> Result<Self, CoreError> {
        check_path_len(path)?;
        match std::fs::symlink_metadata(path) {
            Ok(m) => {
                // 只删套接字；同名的别的东西不碰
                if !m.file_type().is_socket() {
                    return Err(CoreError::Internal {
                        what: "socket_path_occupied",
                    });
                }
                // 按理不会有活的（锁在我们手里）；万一连得上，宁可不删
                if UnixStream::connect(path).is_ok() {
                    return Err(CoreError::Internal {
                        what: "socket_in_use",
                    });
                }
                std::fs::remove_file(path).map_err(io_err)?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_err(e)),
        }
        let inner = UnixListener::bind(path).map_err(io_err)?;
        let listener = Self {
            inner,
            path: path.to_owned(),
        };
        // 目录已是 0700；文件再收紧到 0600
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(io_err)?;
        listener.inner.set_nonblocking(true).map_err(io_err)?;
        Ok(listener)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 等一个连接，最多 `timeout`。没有 → `Ok(None)`；对端不是本用户 → 直接断开，回 [`Accepted::Refused`]
    pub fn accept(&self, timeout: Duration) -> io::Result<Option<Accepted>> {
        if !poll_readable(self.inner.as_raw_fd(), timeout)? {
            return Ok(None);
        }
        let s = match self.inner.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(e) => return Err(e),
        };
        if !peer_allowed(peer_uid(&s), my_uid()) {
            let _ = s.shutdown(std::net::Shutdown::Both);
            return Ok(Some(Accepted::Refused));
        }
        s.set_nonblocking(false)?;
        s.set_read_timeout(Some(REQUEST_TIMEOUT))?;
        s.set_write_timeout(Some(WRITE_TIMEOUT))?;
        Ok(Some(Accepted::Conn(Conn::new(s)?)))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// [`Listener::accept`] 的结果
pub enum Accepted {
    Conn(Conn),
    /// 对端不是本用户（或查不到），已断开
    Refused,
}

/// 等 fd 可读，最多 `timeout`
fn poll_readable(fd: i32, timeout: Duration) -> io::Result<bool> {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: 指向一个本栈上的 pollfd，个数 1
    let r = unsafe { libc::poll(&raw mut p, 1, ms) };
    match r {
        0 => Ok(false),
        r if r > 0 => Ok(true),
        _ => {
            let e = io::Error::last_os_error();
            // 被信号打断（如 Ctrl+C）：当作这一轮没有，调用方回去看退出标志
            if e.kind() == io::ErrorKind::Interrupted {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

// ───────────────────────── 发起方 ─────────────────────────

/// 连执行方的结果
pub enum Connect {
    Ok(Conn),
    /// 没有文件、没人听（上次残留）、路径超长、对端不是本用户……一律当连不上
    Unreachable,
}

/// 连档案的套接字，并核对对端是本用户
pub fn connect(profile: &Profile) -> Connect {
    connect_at(&socket_path(profile))
}

/// 同 [`connect`]，直接给路径（单测用）
pub fn connect_at(path: &Path) -> Connect {
    if check_path_len(path).is_err() {
        return Connect::Unreachable;
    }
    let Ok(s) = UnixStream::connect(path) else {
        return Connect::Unreachable;
    };
    if !peer_allowed(peer_uid(&s), my_uid()) {
        return Connect::Unreachable;
    }
    match Conn::new(s) {
        Ok(c) => Connect::Ok(c),
        Err(_) => Connect::Unreachable,
    }
}

// ───────────────────────── 分帧 ─────────────────────────

/// 发起方是谁（L1 照发起方的规则，`config-privacy.md` 第一节第 2 小节「闸在执行方」）
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Cli,
}

/// 一个请求：命令名 + 发起方已解析好的参数。参数形状归调用方，这里不认
#[derive(Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    pub origin: Origin,
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

/// 应答的一帧
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Frame {
    /// 原样写标准输出的一行（不含换行）
    Out { line: String },
    /// 原样写标准错误的一行（不含换行）
    Err { line: String },
    /// 最后一帧：退出码
    Exit { code: u8 },
}

/// 读到的一行出了什么事
pub enum ReadError {
    /// 对端关了（读到结尾）
    Closed,
    /// 一行太长、不是合法 JSON、形状不对
    Malformed,
    /// 超时
    TimedOut,
    /// 其他读错误
    Io,
}

/// 一条连接：按行读写 JSON
pub struct Conn {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Conn {
    fn new(s: UnixStream) -> io::Result<Self> {
        let writer = s.try_clone()?;
        Ok(Self {
            reader: BufReader::new(s),
            writer,
        })
    }

    /// 写一行 JSON
    pub fn send<T: Serialize>(&mut self, msg: &T) -> io::Result<()> {
        let mut line = serde_json::to_vec(msg).map_err(|_| io::Error::other("encode"))?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()
    }

    /// 读一行 JSON
    pub fn recv<T: for<'de> Deserialize<'de>>(&mut self) -> Result<T, ReadError> {
        let mut buf = Vec::new();
        let n = (&mut self.reader)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut buf)
            .map_err(|e| match e.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ReadError::TimedOut,
                _ => ReadError::Io,
            })?;
        if n == 0 {
            return Err(ReadError::Closed);
        }
        if buf.last() != Some(&b'\n') {
            // 太长，或没写完就断了
            return Err(if buf.len() > MAX_LINE {
                ReadError::Malformed
            } else {
                ReadError::Closed
            });
        }
        serde_json::from_slice(&buf).map_err(|_| ReadError::Malformed)
    }

    /// 发起方等应答时，两帧之间最多等多久（命令本身可能要几十秒，执行方还可能在处理别人的请求）
    pub fn set_read_timeout(&self, t: Duration) -> io::Result<()> {
        self.reader.get_ref().set_read_timeout(Some(t))
    }

    /// 底下的连接（单测用：核对对端）
    pub fn stream(&self) -> &UnixStream {
        self.reader.get_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// 短路径的临时目录（套接字路径有上限，`temp_dir()` 在 macOS 上已经很长）
    fn short_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atgipc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        crate::config::ensure_private_dir(&d).unwrap();
        d
    }

    #[test]
    fn path_length_limit() {
        let ok = PathBuf::from(format!("/{}", "a".repeat(SOCKET_PATH_MAX - 1)));
        assert!(check_path_len(&ok).is_ok());
        let long = PathBuf::from(format!("/{}", "a".repeat(SOCKET_PATH_MAX)));
        let e = check_path_len(&long).unwrap_err();
        assert_eq!(
            e,
            CoreError::Internal {
                what: "socket_path_too_long"
            }
        );
        assert!(matches!(connect_at(&long), Connect::Unreachable));
        assert!(Listener::bind_at(&long).is_err());
    }

    #[test]
    fn real_profile_paths_fit() {
        // 本机默认档案根下，最长的档案名（32 个字符）也放得下
        if let Ok(root) = crate::config::default_root() {
            let p = root.join("profiles").join("a".repeat(32)).join(SOCKET);
            assert!(
                p.as_os_str().len() <= SOCKET_PATH_MAX,
                "{}",
                p.as_os_str().len()
            );
        }
    }

    #[test]
    fn socket_is_private_and_removed_on_drop() {
        let d = short_dir("perm");
        let p = d.join(SOCKET);
        {
            let l = Listener::bind_at(&p).unwrap();
            let mode = std::fs::metadata(l.path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dmode = std::fs::metadata(&d).unwrap().permissions().mode() & 0o777;
            assert_eq!(dmode, 0o700);
        }
        assert!(!p.exists(), "丢掉后应删掉套接字文件");
    }

    #[test]
    fn stale_socket_is_replaced() {
        let d = short_dir("stale");
        let p = d.join(SOCKET);
        // 造一个没人听的残留：绑定后把 listener 忘掉（不走 Drop，文件留着）
        let raw = UnixListener::bind(&p).unwrap();
        drop(raw);
        assert!(p.exists());
        assert!(matches!(connect_at(&p), Connect::Unreachable));
        let l = Listener::bind_at(&p).unwrap();
        assert!(matches!(connect_at(&p), Connect::Ok(_)));
        drop(l);
    }

    #[test]
    fn live_socket_is_not_stolen() {
        let d = short_dir("live");
        let p = d.join(SOCKET);
        let _first = Listener::bind_at(&p).unwrap();
        let e = Listener::bind_at(&p).err().unwrap();
        assert_eq!(
            e,
            CoreError::Internal {
                what: "socket_in_use"
            }
        );
    }

    #[test]
    fn non_socket_file_is_left_alone() {
        let d = short_dir("file");
        let p = d.join(SOCKET);
        std::fs::write(&p, b"x").unwrap();
        let e = Listener::bind_at(&p).err().unwrap();
        assert_eq!(
            e,
            CoreError::Internal {
                what: "socket_path_occupied"
            }
        );
        assert_eq!(std::fs::read(&p).unwrap(), b"x");
    }

    #[test]
    fn other_users_are_refused() {
        let me = my_uid();
        assert!(peer_allowed(Ok(me), me));
        assert!(!peer_allowed(Ok(me.wrapping_add(1)), me));
        assert!(!peer_allowed(Err(io::Error::other("x")), me));
    }

    #[test]
    fn missing_socket_is_unreachable() {
        let d = short_dir("none");
        assert!(matches!(connect_at(&d.join(SOCKET)), Connect::Unreachable));
    }

    #[test]
    fn round_trip_and_peer_is_me() {
        let d = short_dir("rt");
        let p = d.join(SOCKET);
        let l = Listener::bind_at(&p).unwrap();
        let t = std::thread::spawn(move || {
            let Some(Accepted::Conn(mut c)) = l.accept(Duration::from_secs(5)).unwrap() else {
                panic!("没接到");
            };
            assert_eq!(peer_uid(c.stream()).unwrap(), my_uid());
            let req: Request = c.recv().ok().unwrap();
            assert_eq!(req.command, "chats");
            assert_eq!(req.args["limit"], 3);
            c.send(&Frame::Out {
                line: "{\"a\":1}".into(),
            })
            .unwrap();
            c.send(&Frame::Exit { code: 0 }).unwrap();
        });
        let Connect::Ok(mut c) = connect_at(&p) else {
            panic!("连不上");
        };
        c.send(&Request {
            v: PROTOCOL_V,
            origin: Origin::Cli,
            command: "chats".into(),
            args: serde_json::json!({"limit": 3}),
        })
        .unwrap();
        let f: Frame = c.recv().ok().unwrap();
        assert!(
            f == Frame::Out {
                line: "{\"a\":1}".into()
            }
        );
        let f: Frame = c.recv().ok().unwrap();
        assert!(f == Frame::Exit { code: 0 });
        assert!(matches!(c.recv::<Frame>(), Err(ReadError::Closed)));
        t.join().unwrap();
    }

    #[test]
    fn accept_times_out_quietly() {
        let d = short_dir("idle");
        let l = Listener::bind_at(&d.join(SOCKET)).unwrap();
        assert!(l.accept(Duration::from_millis(20)).unwrap().is_none());
    }

    #[test]
    fn overlong_line_is_malformed() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut c = Conn::new(a).unwrap();
        let t = std::thread::spawn(move || {
            let big = vec![b'x'; MAX_LINE + 10];
            let _ = b.write_all(&big);
        });
        assert!(matches!(c.recv::<Frame>(), Err(ReadError::Malformed)));
        t.join().unwrap();
    }

    #[test]
    fn truncated_line_is_closed() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut c = Conn::new(a).unwrap();
        b.write_all(b"{\"t\":\"out\"").unwrap();
        drop(b);
        assert!(matches!(c.recv::<Frame>(), Err(ReadError::Closed)));
    }
}
