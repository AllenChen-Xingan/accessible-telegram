//! `tty`：登录与登出时跟人说话、读人输入——只经 `/dev/tty`，不经标准输入输出。
//!
//! `cli.md`：秘密值（手机号、验证码、密码、邮箱）不进命令行参数、不从标准输入读（标准输入可能是 AI 拼的管道），
//! 从 `/dev/tty` 读、**不回显**；给人看的话（提示语、密码提示、出错原因）也只写 `/dev/tty`，标准输出只放 JSON 行。
//!
//! 读秘密（和登出确认）时把终端切成「不回显、逐字节、Ctrl+C 不发信号」（`~ECHO ~ICANON ~ISIG ~IEXTEN`），自己处理：
//! 回车结束、退格删一个字、Ctrl+U 清空、Ctrl+C → 中断、空行上 Ctrl+D → 取消。这样中途 Ctrl+C 也能先把
//! 终端设置恢复（否则人的终端会一直不回显）。恢复靠守卫的 `Drop`，panic 展开时同样执行。
//! 输入上限 [`MAX_INPUT`] 字节，预先分配好，读的过程中不会重新分配（重新分配会留下没清零的旧副本）；
//! 退格、Ctrl+U 删掉的字节当场清零（#40）。
//!
//! 读的途中看停止标志（#98）：不靠 `read` 被信号打断（`signals::install` 装的处理函数自带 `SA_RESTART`，
//! 信号来了操作系统会把 `read` 重启、接着等键），而是 `select` 一片片等（每片 [`SLICE_MS`]），片间看标志——
//! 外部 SIGTERM、提示出来之前那一瞬按的 Ctrl+C 都收得到。不用 `poll`：macOS 的 `poll` 不支持 `/dev/tty`
//! 这个间接设备，一调就回 `POLLNVAL`、不等（本机伪终端实测；`select` 按期等满）。
//! 切终端之前先登记恢复信息（[`crate::signals::arm_tty`]）：第二次信号直接 `_exit` 之前先把终端放回去。

use atg_core::secret::Secret;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};

/// 等输入时每片最多多久（片间看停止标志）
const SLICE_MS: libc::suseconds_t = 100;

/// 一次输入最多多少字节（号码、验证码、邮箱、密码都远小于它）
pub const MAX_INPUT: usize = 256;

/// 读的时候人怎么结束的（不是正常回车）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadEnd {
    /// Ctrl+C，或停止标志立起（外部 SIGTERM、读之前已按的 Ctrl+C）
    Interrupted,
    /// 空行上 Ctrl+D，或终端没了
    Cancelled,
}

pub struct Tty {
    file: File,
    /// 停止标志（正式是 [`crate::signals::STOP`]）
    stop: &'static AtomicBool,
}

/// 读完把终端设置原样放回
struct Restore {
    fd: i32,
    orig: libc::termios,
}

impl Drop for Restore {
    fn drop(&mut self) {
        // SAFETY: fd 是本进程打开着的 /dev/tty；orig 是 tcgetattr 填好的
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSAFLUSH, &self.orig);
        }
        // 先放回再解除：中间来第二次信号，处理函数再放一次也无妨
        crate::signals::disarm_tty();
    }
}

impl Tty {
    /// 打开控制终端。没有（在管道、cron、CI 里跑）→ 错
    pub fn open() -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        Ok(Self {
            file,
            stop: &crate::signals::STOP,
        })
    }

    /// 对人说一句（独占一行）
    pub fn say(&mut self, s: &str) {
        let _ = writeln!(self.file, "{s}");
        let _ = self.file.flush();
    }

    /// 读一行，不回显。回车结束；结果装进 [`Secret`]
    pub fn read_secret(&mut self, prompt: &str) -> Result<Secret, ReadEnd> {
        // 不论怎么结束，缓冲区都先装进 Secret（丢掉时清零），出错路径也一样
        let (buf, end) = self.read_raw(prompt, false);
        let secret = Secret::new(buf);
        end.map(|()| secret)
    }

    /// 读一行，照常回显（登出确认这类不是秘密的回答）。返回去掉首尾空白的小写形式。
    /// 终端同样切成逐字节、Ctrl+C 不发信号、自己回显：登录这组命令装了信号处理，规范模式下 Ctrl+C 只立标志、
    /// 读会被内核重启，提示停着不动；这里自己认 Ctrl+C，结果同读秘密（`interrupted`）
    pub fn read_visible(&mut self, prompt: &str) -> Result<String, ReadEnd> {
        let (line, end) = self.read_raw(prompt, true);
        end.map(|()| String::from_utf8_lossy(&line).trim().to_lowercase())
    }

    /// 切好终端（`~ECHO ~ICANON ~ISIG ~IEXTEN`）再出提示、读一行；`echo` = 自己把敲的字写回终端。
    /// 缓冲区不论成败都交回（读秘密的那边要清零）
    fn read_raw(&mut self, prompt: &str, echo: bool) -> (Vec<u8>, Result<(), ReadEnd>) {
        let fd = self.file.as_raw_fd();
        // SAFETY: termios 是纯数据结构，全零是合法初值，随即由 tcgetattr 填满
        let mut orig: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: fd 有效，orig 可写
        if unsafe { libc::tcgetattr(fd, &mut orig) } != 0 {
            return (Vec::new(), Err(ReadEnd::Cancelled));
        }
        // 先登记再切：切好之后、登记之前来第二次信号也不会把终端留在不回显（用原设置登记无害）
        crate::signals::arm_tty(fd, &orig);
        let mut raw = orig;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        // SAFETY: 同上
        if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } != 0 {
            crate::signals::disarm_tty();
            return (Vec::new(), Err(ReadEnd::Cancelled));
        }
        let guard = Restore { fd, orig };
        // 先切好终端再出提示：切换时会丢掉已敲进来的输入，提示在前的话，看到提示就敲的人（或脚本）会被吞掉
        let _ = write!(self.file, "{prompt}");
        let _ = self.file.flush();
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let mut r = Polled {
            file: &self.file,
            stop: self.stop,
        };
        let mut w = &self.file;
        let end = read_raw_line(&mut r, &mut buf, &mut |bytes| {
            if echo {
                let _ = w.write_all(bytes);
                let _ = w.flush();
            }
        });
        drop(guard);
        // 回车不回显，补一个换行，下一句另起一行
        let _ = writeln!(self.file);
        (buf, end)
    }
}

/// 一片片 `select` 着读。约定：**交出 `ErrorKind::Interrupted` 只表示停止标志立起来了**（`read_raw_line` 认作
/// Ctrl+C）；真的 `EINTR`（被信号打断）在这里吞掉、当这一片结束，回头再看标志，不往上交。
/// fd 超出 `select` 能管的范围（`FD_SETSIZE`）时退回直接 `read`（不再看标志，不该发生：本进程 fd 很少）
struct Polled<'a> {
    file: &'a File,
    stop: &'a AtomicBool,
}

impl Read for Polled<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let fd = self.file.as_raw_fd();
            if usize::try_from(fd).is_ok_and(|f| f < libc::FD_SETSIZE) {
                // SAFETY: fd_set 是纯数据，全零后由 FD_ZERO / FD_SET 设好；fd 在 FD_SETSIZE 以内
                let mut set: libc::fd_set = unsafe { std::mem::zeroed() };
                let mut tv = libc::timeval {
                    tv_sec: 0,
                    tv_usec: SLICE_MS * 1000,
                };
                // SAFETY: set、tv 都是本栈上的有效对象；只关心可读
                let n = unsafe {
                    libc::FD_ZERO(&mut set);
                    libc::FD_SET(fd, &mut set);
                    libc::select(
                        fd + 1,
                        &mut set,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &mut tv,
                    )
                };
                match n {
                    0 => continue,
                    n if n < 0 => {
                        let e = io::Error::last_os_error();
                        if e.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(e);
                    }
                    _ => {}
                }
            }
            match (&mut &*self.file).read(out) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                r => return r,
            }
        }
    }
}

/// 逐字节读到回车。源交出 `ErrorKind::Interrupted` = 停止标志立起（见 [`Polled`]），结局同 Ctrl+C。`buf` 预先分配了 [`MAX_INPUT`]，超出的字节丢掉，不会重新分配。
/// 收下的字、删掉的字经 `echo` 交回（回显时写回终端；读秘密时丢掉）
fn read_raw_line(
    src: &mut impl Read,
    buf: &mut Vec<u8>,
    echo: &mut dyn FnMut(&[u8]),
) -> Result<(), ReadEnd> {
    let mut b = [0u8; 1];
    loop {
        match src.read(&mut b) {
            // 停止标志立起（见 [`Polled`]）：同 Ctrl+C
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(ReadEnd::Interrupted),
            Ok(0) | Err(_) => return Err(ReadEnd::Cancelled),
            Ok(_) => {}
        }
        match b[0] {
            b'\r' | b'\n' => return Ok(()),
            0x03 => return Err(ReadEnd::Interrupted),
            0x04 if buf.is_empty() => return Err(ReadEnd::Cancelled),
            // 退格：删掉最后一个字（UTF-8 的续字节一起删）；删到的是组合附加符号（零宽）就连它前面的基字一起删，
            // 按基字的宽度擦，屏幕上不留半个字（codex 022 第 2 轮 #4）
            0x7f | 0x08 => {
                let mut start = buf.len();
                let mut width = 0;
                while width == 0 && start > 0 {
                    start = buf[..start]
                        .iter()
                        .rposition(|x| x & 0xC0 != 0x80)
                        .unwrap_or(0);
                    width = cols(&buf[start..]);
                }
                for _ in 0..width {
                    echo(b"\x08 \x08");
                }
                wipe_from(buf, start);
            }
            // Ctrl+U：清空这一行
            0x15 => {
                for _ in 0..cols(buf) {
                    echo(b"\x08 \x08");
                }
                wipe_from(buf, 0);
            }
            c if c >= 0x20 && buf.len() < MAX_INPUT => {
                buf.push(c);
                echo(&[c]);
            }
            _ => {}
        }
    }
}

/// 删掉 `start` 之后的字节，删之前先清零：截短后它们还留在容量区，`Secret` 丢掉时虽也整块清，
/// 读的途中就不该留着（#40）
fn wipe_from(buf: &mut Vec<u8>, start: usize) {
    atg_core::secret::wipe(&mut buf[start..]);
    buf.truncate(start);
}

/// 这几个字节在终端上占几列（回显时退格按它擦）。不拷贝：读秘密时也走这里。
/// 中日韩、全角、表情占 2 列，组合附加符号占 0 列，其余 1 列；解不出来（半个字）按字节数
fn cols(bytes: &[u8]) -> usize {
    std::str::from_utf8(bytes).map_or(bytes.len(), |s| s.chars().map(char_cols).sum())
}

fn char_cols(c: char) -> usize {
    match u32::from(c) {
        0x0300..=0x036F | 0x200B..=0x200F | 0xFE00..=0xFE0F => 0,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &[u8]) -> (Result<(), ReadEnd>, Vec<u8>) {
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let r = read_raw_line(&mut &input[..], &mut buf, &mut |_| {});
        (r, buf)
    }

    #[test]
    fn line_editing() {
        assert_eq!(read(b"12345\r"), (Ok(()), b"12345".to_vec()));
        assert_eq!(read(b"12x\x7f345\n"), (Ok(()), b"12345".to_vec()));
        assert_eq!(read(b"abc\x15123\r"), (Ok(()), b"123".to_vec()));
        // 退格删掉整个汉字（3 字节）
        assert_eq!(
            read("密码码\x7f\r".as_bytes()),
            (Ok(()), "密码".as_bytes().to_vec())
        );
        // 控制字符丢掉
        assert_eq!(read(b"1\x012\r"), (Ok(()), b"12".to_vec()));
    }

    /// 回显：收下的字原样交回；退格、Ctrl+U 按显示宽度交回「退一格」（汉字两格、组合符号零格）
    #[test]
    fn echo_follows_editing() {
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let mut seen = Vec::new();
        let bs = "\x08 \x08";
        let r = read_raw_line(
            &mut &"ab\x7f码\x15y码\x7fe\u{301}\x7f\r".as_bytes()[..],
            &mut buf,
            &mut |b| seen.extend_from_slice(b),
        );
        assert_eq!(r, Ok(()));
        assert_eq!(buf, "y".as_bytes());
        // a b ⌫(b 一格) 码 ⌃U(a 一格 + 码 两格) y 码 ⌫(两格) e ◌́ ⌫(连基字 e 一起删，擦一格)
        let want = format!("ab{bs}码{}y码{}e\u{301}{bs}", bs.repeat(3), bs.repeat(2));
        assert_eq!(String::from_utf8(seen).unwrap(), want);
        assert_eq!(cols("码".as_bytes()), 2);
        assert_eq!(cols(&"码".as_bytes()[..2]), 2, "半个字按字节数");
    }

    #[test]
    fn ends() {
        assert_eq!(read(b"12\x03").0, Err(ReadEnd::Interrupted));
        assert_eq!(read(b"\x04").0, Err(ReadEnd::Cancelled));
        assert_eq!(read(b"1\x042\r"), (Ok(()), b"12".to_vec()));
        assert_eq!(read(b"no newline").0, Err(ReadEnd::Cancelled));
    }

    #[test]
    fn input_is_capped_without_reallocating() {
        let long = vec![b'7'; MAX_INPUT * 2];
        let mut input = long.clone();
        input.push(b'\r');
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let cap = buf.capacity();
        read_raw_line(&mut &input[..], &mut buf, &mut |_| {}).unwrap();
        assert_eq!(buf.len(), MAX_INPUT);
        assert_eq!(buf.capacity(), cap);
    }

    /// 退格、Ctrl+U 删掉的字节当场清零：容量区里不留（#40）
    #[test]
    fn deleted_bytes_are_wiped() {
        for input in [&b"ab\x7f\r"[..], &b"ab\x15\r"[..]] {
            let mut buf = Vec::with_capacity(MAX_INPUT);
            read_raw_line(&mut &input[..], &mut buf, &mut |_| {}).unwrap();
            // SAFETY: 读的是本 Vec 自己的容量区前两个字节，读的过程中写过（先 a b、再清零）
            let raw = unsafe { std::slice::from_raw_parts(buf.as_ptr(), 2) };
            assert_eq!(raw[1], 0, "{input:?}");
        }
    }

    /// 读的途中源报停止（`Interrupted`）→ 同 Ctrl+C，已读的交回
    #[test]
    fn stop_while_reading_is_interrupted() {
        struct Then<'a>(&'a [u8]);
        impl Read for Then<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                match self.0.split_first() {
                    Some((b, rest)) => {
                        out[0] = *b;
                        self.0 = rest;
                        Ok(1)
                    }
                    None => Err(io::ErrorKind::Interrupted.into()),
                }
            }
        }
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let r = read_raw_line(&mut Then(b"12"), &mut buf, &mut |_| {});
        assert_eq!((r, buf), (Err(ReadEnd::Interrupted), b"12".to_vec()));
    }

    /// `select` 切片：没人敲键时另一线程立停止标志，一两片之内就回来；标志一开始就立着则一片都不等（#98）
    #[test]
    fn polled_read_sees_the_stop_flag() {
        use std::os::fd::FromRawFd;
        use std::time::{Duration, Instant};
        let mut fds = [0; 2];
        // SAFETY: fds 可写，两个元素
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: 两个 fd 刚由 pipe 建好，交给 File 各管一个
        let (rd, wr) = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
        let stop: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
        let t0 = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                stop.store(true, Ordering::SeqCst);
            });
            let mut b = [0u8; 1];
            let e = Polled { file: &rd, stop }.read(&mut b).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::Interrupted);
        });
        assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
        // 有字节可读时照常交回（标志放下）
        stop.store(false, Ordering::SeqCst);
        (&wr).write_all(b"x").unwrap();
        let mut b = [0u8; 1];
        assert_eq!(Polled { file: &rd, stop }.read(&mut b).unwrap(), 1);
        assert_eq!(&b, b"x");
    }

    /// 真终端（`/dev/tty`）上同样按片等、看得到停止标志：macOS 的 `poll` 在 `/dev/tty` 上不等（自审 P1），
    /// 管道测不出来。没有控制终端（CI、管道里跑）就跳过；本地用 `script -q /dev/null cargo test …` 跑得到
    #[test]
    fn polled_read_waits_on_the_real_tty() {
        use std::time::{Duration, Instant};
        let Ok(tty) = OpenOptions::new().read(true).open("/dev/tty") else {
            return;
        };
        let stop: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
        let t0 = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                stop.store(true, Ordering::SeqCst);
            });
            let mut b = [0u8; 1];
            let r = Polled { file: &tty, stop }.read(&mut b);
            assert_eq!(r.map_err(|e| e.kind()), Err(io::ErrorKind::Interrupted));
        });
        let took = t0.elapsed();
        assert!(
            took >= Duration::from_millis(250) && took < Duration::from_secs(2),
            "{took:?}"
        );
    }
}
