//! `tty`：登录与登出时跟人说话、读人输入——只经 `/dev/tty`，不经标准输入输出。
//!
//! `cli.md`：秘密值（手机号、验证码、密码、邮箱）不进命令行参数、不从标准输入读（标准输入可能是 AI 拼的管道），
//! 从 `/dev/tty` 读、**不回显**；给人看的话（提示语、密码提示、出错原因）也只写 `/dev/tty`，标准输出只放 JSON 行。
//!
//! 读秘密时把终端切成「不回显、逐字节、Ctrl+C 不发信号」（`~ECHO ~ICANON ~ISIG ~IEXTEN`），自己处理：
//! 回车结束、退格删一个字、Ctrl+U 清空、Ctrl+C → 中断、空行上 Ctrl+D → 取消。这样中途 Ctrl+C 也能先把
//! 终端设置恢复（否则人的终端会一直不回显）。恢复靠守卫的 `Drop`，panic 展开时同样执行。
//! 输入上限 [`MAX_INPUT`] 字节，预先分配好，读的过程中不会重新分配（重新分配会留下没清零的旧副本）。

use atg_core::secret::Secret;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;

/// 一次输入最多多少字节（号码、验证码、邮箱、密码都远小于它）
pub const MAX_INPUT: usize = 256;

/// 读的时候人怎么结束的（不是正常回车）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadEnd {
    /// Ctrl+C
    Interrupted,
    /// 空行上 Ctrl+D，或终端没了
    Cancelled,
}

pub struct Tty {
    file: File,
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
    }
}

impl Tty {
    /// 打开控制终端。没有（在管道、cron、CI 里跑）→ 错
    pub fn open() -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        Ok(Self { file })
    }

    /// 对人说一句（独占一行）
    pub fn say(&mut self, s: &str) {
        let _ = writeln!(self.file, "{s}");
        let _ = self.file.flush();
    }

    /// 读一行，不回显。回车结束；结果装进 [`Secret`]
    pub fn read_secret(&mut self, prompt: &str) -> Result<Secret, ReadEnd> {
        let fd = self.file.as_raw_fd();
        // SAFETY: termios 是纯数据结构，全零是合法初值，随即由 tcgetattr 填满
        let mut orig: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: fd 有效，orig 可写
        if unsafe { libc::tcgetattr(fd, &mut orig) } != 0 {
            return Err(ReadEnd::Cancelled);
        }
        let mut raw = orig;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        // SAFETY: 同上
        if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } != 0 {
            return Err(ReadEnd::Cancelled);
        }
        let guard = Restore { fd, orig };
        // 先切好终端再出提示：切换时会丢掉已敲进来的输入，提示在前的话，看到提示就敲的人（或脚本）会被吞掉
        let _ = write!(self.file, "{prompt}");
        let _ = self.file.flush();
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let end = read_raw_line(&mut self.file, &mut buf);
        drop(guard);
        // 回车不回显，补一个换行，下一句另起一行
        let _ = writeln!(self.file);
        let secret = Secret::new(buf);
        end.map(|()| secret)
    }

    /// 读一行，照常回显（登出确认这类不是秘密的回答）。返回去掉首尾空白的小写形式
    pub fn read_visible(&mut self, prompt: &str) -> Result<String, ReadEnd> {
        let _ = write!(self.file, "{prompt}");
        let _ = self.file.flush();
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        loop {
            match self.file.read(&mut b) {
                Ok(0) | Err(_) => return Err(ReadEnd::Cancelled),
                Ok(_) if b[0] == b'\n' => break,
                Ok(_) => {
                    if line.len() < MAX_INPUT {
                        line.push(b[0]);
                    }
                }
            }
        }
        Ok(String::from_utf8_lossy(&line).trim().to_lowercase())
    }
}

/// 逐字节读到回车。`buf` 预先分配了 [`MAX_INPUT`]，超出的字节丢掉，不会重新分配
fn read_raw_line(src: &mut impl Read, buf: &mut Vec<u8>) -> Result<(), ReadEnd> {
    let mut b = [0u8; 1];
    loop {
        match src.read(&mut b) {
            Ok(0) | Err(_) => return Err(ReadEnd::Cancelled),
            Ok(_) => {}
        }
        match b[0] {
            b'\r' | b'\n' => return Ok(()),
            0x03 => return Err(ReadEnd::Interrupted),
            0x04 if buf.is_empty() => return Err(ReadEnd::Cancelled),
            // 退格：删掉最后一个字（UTF-8 的续字节一起删）
            0x7f | 0x08 => {
                while let Some(last) = buf.pop() {
                    if last & 0xC0 != 0x80 {
                        break;
                    }
                }
            }
            // Ctrl+U：清空这一行
            0x15 => {
                for x in buf.iter_mut() {
                    *x = 0;
                }
                buf.clear();
            }
            c if c >= 0x20 && buf.len() < MAX_INPUT => buf.push(c),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &[u8]) -> (Result<(), ReadEnd>, Vec<u8>) {
        let mut buf = Vec::with_capacity(MAX_INPUT);
        let r = read_raw_line(&mut &input[..], &mut buf);
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
        read_raw_line(&mut &input[..], &mut buf).unwrap();
        assert_eq!(buf.len(), MAX_INPUT);
        assert_eq!(buf.capacity(), cap);
    }
}
