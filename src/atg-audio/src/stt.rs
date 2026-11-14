//! 端侧语音转文字（#59）：PCM → 每个语种的结果 → 按规则选一份。起 Swift 辅助程序 `atg-stt`（`stt-helper/`），
//! PCM 经管道喂过去（不落盘）。选语种表、协议、生命周期的正本见 `docs/experiments/exp008-local-stt/design.md`
//! 第三节第 1、6、7 条。错误只带码，不带辅助程序输出原文（可能含转写文字）。
//! 不按平台区分：非 macOS 上找不到辅助程序，报 [`SttError::Unavailable`]。

use crate::decode::{Pcm, SAMPLE_RATE};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// 协议版本。Swift 那边的 `PROTO` 由测试 `proto_matches_swift` 比对
pub const PROTO: u32 = 1;
/// 辅助程序报「这个语种的模型没装 / 不支持」用的错误码（同上，测试比对 Swift 那边的写法）
const UNAVAILABLE: &str = "unavailable";
/// 握手期限：起来后这么久还没出版本行 → 不可用
const HELLO_WITHIN: Duration = Duration::from_secs(5);
/// 总期限 = 这个基数 + 音频时长
const TOTAL_BASE: Duration = Duration::from_secs(10);
/// 辅助程序自己的期限比看门线程晚这么多（见 [`transcribe`] 里传参处）
const BACKSTOP: Duration = Duration::from_secs(1);
/// 看门线程多久看一次取消与期限
const POLL: Duration = Duration::from_millis(50);

/// 一段转写（片段边界由系统分析器给）
#[derive(Clone, PartialEq, Deserialize)]
pub struct Segment {
    pub text: String,
    pub start_s: f64,
    pub end_s: f64,
}

/// 选中的那一份
#[derive(Clone, PartialEq)]
pub enum Recognition {
    /// 有字。`conf`：0 到 1，辅助程序按带置信度的文字段取的平均；**只拿来在语种之间挑**（不同模型的值不严格可比，
    /// 也不是给人看的准确率）
    Text {
        locale: String,
        conf: f64,
        segments: Vec<Segment>,
    },
    /// 跑成功了但没听出内容
    NoSpeech,
}

// 含转写文字：只打印形状（specs/contracts/config-privacy.md 第三节验收判据）
impl std::fmt::Debug for Recognition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Recognition::Text { segments, .. } => {
                write!(f, "Recognition::Text {{ segments: {} }}", segments.len())
            }
            Recognition::NoSpeech => write!(f, "Recognition::NoSpeech"),
        }
    }
}
impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Segment {{ <redacted> }}")
    }
}

/// 转写出错的种类。映射到 CLI 的失败只在 atg-cli 写一处（同 [`crate::AudioError`]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttError {
    /// 找不到辅助程序、起不来、版本不对、握手超时、模型缺失 → 单开的码 `stt_unavailable`（人要去系统设置装语言模型；
    /// 码的正本随 `transcribe` 命令进 atg-core 的错误表，design 第三节第 4 条）
    Unavailable,
    /// 总期限到了
    Timeout,
    /// 非零退出、协议不完整、坏 JSON
    HelperFailed,
    /// 调用方取消（CLI 的 Ctrl+C）→ CLI 照常报 `interrupted`、退出码 130
    Cancelled,
}

impl SttError {
    /// 进 `internal{what}` 的码；`Unavailable`、`Cancelled` 各有自己的顶层码，不走 `internal`，回 `None`
    pub fn what(self) -> Option<&'static str> {
        match self {
            SttError::Timeout => Some("stt_timeout"),
            SttError::HelperFailed => Some("stt_helper_failed"),
            SttError::Unavailable | SttError::Cancelled => None,
        }
    }
}

/// 辅助程序每个语种的那一行
#[derive(Deserialize)]
struct LocaleLine {
    locale: String,
    status: String,
    #[serde(default)]
    conf: Option<f64>,
    #[serde(default)]
    segments: Vec<Segment>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Line {
    Hello { proto: u32 },
    Locale(LocaleLine),
    Done { count: usize },
}

/// 一个语种的三态（design.md 第三节第 1 条的表）
enum Outcome {
    Text(f64, Vec<Segment>),
    Empty,
    Failed { unavailable: bool },
}

/// 按表选：至少一个有字 → 置信度最高（相等取 `locales` 里排前的）；否则有空 → 没听出内容；全败 → 错误
fn choose(locales: &[String], got: Vec<(String, Outcome)>) -> Result<Recognition, SttError> {
    let mut best: Option<(usize, f64, String, Vec<Segment>)> = None;
    let mut any_empty = false;
    let mut any_unavailable = false;
    for (loc, o) in got {
        // read_protocol 已保证只有请求过的语种；找不到就排最后（到不了）
        let rank = locales.iter().position(|l| *l == loc).unwrap_or(usize::MAX);
        match o {
            Outcome::Text(conf, segs) => {
                let better = match &best {
                    None => true,
                    Some((r, c, _, _)) => conf > *c || (conf == *c && rank < *r),
                };
                if better {
                    best = Some((rank, conf, loc, segs));
                }
            }
            Outcome::Empty => any_empty = true,
            Outcome::Failed { unavailable } => any_unavailable |= unavailable,
        }
    }
    match best {
        Some((_, conf, locale, segments)) => Ok(Recognition::Text {
            locale,
            conf,
            segments,
        }),
        None if any_empty => Ok(Recognition::NoSpeech),
        None if any_unavailable => Err(SttError::Unavailable),
        None => Err(SttError::HelperFailed),
    }
}

/// 读完整的协议输出：恰好一个 hello → 每个请求的语种恰好一行 → done(count) → EOF。多行、缺行、乱序都是进程失败。
/// 读到版本对得上的 hello 时调一次 `on_hello`（调用方在定终态的锁里记下握手时刻）
fn read_protocol(
    r: impl BufRead,
    locales: &[String],
    mut on_hello: impl FnMut(),
) -> Result<Vec<(String, Outcome)>, SttError> {
    let mut hello_seen = false;
    let mut got: Vec<(String, Outcome)> = Vec::new();
    let mut done = None;
    for line in r.lines() {
        let line = line.map_err(|_| SttError::HelperFailed)?;
        if done.is_some() {
            return Err(SttError::HelperFailed);
        }
        // 解析失败只报码：serde 的错误会带出原文
        let parsed: Line = serde_json::from_str(&line).map_err(|_| SttError::HelperFailed)?;
        match parsed {
            Line::Hello { .. } if hello_seen => {
                return Err(SttError::HelperFailed);
            }
            Line::Hello { proto } => {
                if proto != PROTO {
                    return Err(SttError::Unavailable);
                }
                hello_seen = true;
                on_hello();
            }
            Line::Locale(l) => {
                if !hello_seen
                    || !locales.contains(&l.locale)
                    || got.iter().any(|(x, _)| *x == l.locale)
                {
                    return Err(SttError::HelperFailed);
                }
                let o = match l.status.as_str() {
                    // 没给置信度按 0（仍是有字）；文字全是空白算空（design 第三节第 1 条）
                    "text" if l.segments.iter().any(|s| !s.text.trim().is_empty()) => {
                        Outcome::Text(l.conf.unwrap_or(0.0), l.segments)
                    }
                    "text" | "empty" => Outcome::Empty,
                    "failed" => Outcome::Failed {
                        unavailable: l.error.as_deref() == Some(UNAVAILABLE),
                    },
                    _ => return Err(SttError::HelperFailed),
                };
                got.push((l.locale, o));
            }
            Line::Done { count } => done = Some(count),
        }
    }
    match done {
        Some(n) if n == locales.len() && got.len() == locales.len() => Ok(got),
        _ => Err(SttError::HelperFailed),
    }
}

/// 找辅助程序：环境变量 `ATG_STT_HELPER` > 可执行文件旁边的 `atg-stt` > `<repo_root>/_build/*/atg-stt` 最新一份
/// （arch 定 2026-11-13）。`repo_root` 是仓库根（`_build` 的上一级），不知道就传 `None`
pub fn locate(repo_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ATG_STT_HELPER") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("atg-stt")))
        && p.is_file()
    {
        return Some(p);
    }
    let dir = repo_root?.join("_build");
    let mut runs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path().join("atg-stt"))
        .filter(|p| p.is_file())
        .collect();
    runs.sort();
    runs.pop()
}

/// 锁住（某线程 panic 过也照样拿到：收尾必须做）
fn lock<T>(c: &Mutex<T>) -> MutexGuard<'_, T> {
    c.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 这一次转写的终态：「主线程收完」与「看门线程动手」只能有一个赢（同一把锁里转换，赢了的才算数）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Running,
    /// 主线程拿到了结果（或放弃了），看门线程不再动手
    Finished,
    /// 看门线程动了手，原因就是最终的错误
    Killed(SttError),
}

impl Phase {
    /// 看门线程要动手：还在跑才算数
    fn kill(&mut self, why: SttError) -> bool {
        let won = *self == Phase::Running;
        if won {
            *self = Phase::Killed(why);
        }
        won
    }
    /// 主线程收完：还在跑才算数（已被杀则保留被杀的原因）
    fn finish(&mut self) -> bool {
        let won = *self == Phase::Running;
        if won {
            *self = Phase::Finished;
        }
        won
    }
    /// 主线程定终态（协议坏了、或拿到退出码）：期限已到或已取消，就按看门线程的原因定（它可能还没轮上执行）。
    /// 辅助程序到兜底期限以 3 退出、看门线程迟到时，结果仍是超时而不是进程失败
    fn settle(&mut self, due: Option<SttError>) {
        match due {
            Some(w) => {
                self.kill(w);
            }
            None => {
                self.finish();
            }
        }
    }
}

/// 该不该停、为什么停：看门线程与主线程共用这一处判据（取消 > 握手期限 > 总期限）。
/// 握手按「握手时刻是否在期限内」判，不按「现在有没有收到」：迟到的 hello 改不了超期的事实，
/// 判得晚也不会把及时的 hello 判成超期。时刻全以 `t0` 为起点；调用方在定终态的锁里取 `now` 与握手时刻
fn overdue(
    cancelled: bool,
    hello_at: Option<Duration>,
    now: Duration,
    limits: Limits,
    total: Duration,
) -> Option<SttError> {
    let hello_late = match hello_at {
        Some(at) => at > limits.hello,
        None => now > limits.hello,
    };
    if cancelled {
        Some(SttError::Cancelled)
    } else if hello_late {
        Some(SttError::Unavailable)
    } else if now > total {
        Some(SttError::Timeout)
    } else {
        None
    }
}

#[cfg(test)]
thread_local! {
    /// 测试用：本线程收尾时回收了几个子进程（`Stop::drop` 里 `wait` 成功一次加一）
    static REAPED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// 统一收尾（design 第三节第 6 条）：终态定下（没定的按收完）→ kill（已退出则无害）→ wait 回收。
/// 在 `thread::scope` 闭包里、起任何线程之前建好，所以任何退出（含线程起不来、panic 展开）都走它；
/// 它先于 scope 的 join 执行，子进程一死，写线程的 write 报管道断开返回，join 不会卡住
struct Stop<'a> {
    phase: &'a Mutex<Phase>,
    child: &'a Mutex<Child>,
}

impl Drop for Stop<'_> {
    fn drop(&mut self) {
        lock(self.phase).finish();
        let mut c = lock(self.child);
        let _ = c.kill();
        let _reaped = c.wait().is_ok();
        #[cfg(test)]
        if _reaped {
            REAPED.with(|n| n.set(n.get() + 1));
        }
    }
}

/// 转写。
/// - `pcm`：[`SAMPLE_RATE`] 单声道（[`crate::decode`] 出的那份）
/// - `locales`：要跑的语种（如 `zh-CN`、`en-US`），至少一个；**顺序有意义**：置信度相等时取排前的
/// - `cancel`：调用方立（CLI 是 Ctrl+C 的标志，GUI 传自己的）
///
/// 所有线程都在 `thread::scope` 里，函数返回前必定全部结束；子进程在任何路径上都被 kill + wait
pub fn transcribe(
    helper: &Path,
    pcm: &Pcm,
    locales: &[String],
    cancel: &AtomicBool,
) -> Result<Recognition, SttError> {
    transcribe_with(helper, pcm, locales, cancel, Limits::DEFAULT)
}

/// 期限。只有测试改它：握手、总期限的测试不必真等 5 秒、10 秒
#[derive(Clone, Copy)]
struct Limits {
    hello: Duration,
    /// 总期限 = `base` + 音频时长
    base: Duration,
}

impl Limits {
    const DEFAULT: Limits = Limits {
        hello: HELLO_WITHIN,
        base: TOTAL_BASE,
    };
}

fn transcribe_with(
    helper: &Path,
    pcm: &Pcm,
    locales: &[String],
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<Recognition, SttError> {
    if locales.is_empty() {
        return Err(SttError::HelperFailed);
    }
    let total = limits.base + Duration::from_secs_f64(pcm.duration_s());
    let bytes: Vec<u8> = pcm.samples.iter().flat_map(|x| x.to_le_bytes()).collect();
    // 计时起点在起子进程之前：辅助程序的兜底期限从它自己启动算起，这边的起点只会更早，
    // 到期总是这边先判（超时报 Timeout，不会因为它先以 3 退出被判成进程失败）
    let t0 = Instant::now();
    let mut child = Command::new(helper)
        // 辅助程序一起来就核对父进程是不是这个：本进程在它起来之前就没了，它也能立刻发现
        .arg("--parent-pid")
        .arg(std::process::id().to_string())
        .arg("--rate")
        .arg(SAMPLE_RATE.to_string())
        // 辅助程序自己的期限只是兜底：第二次 Ctrl+C 时 CLI 直接 _exit、不跑析构（atg-cli/src/signals.rs），
        // 来不及杀它。比看门线程晚一秒，正常的超时由这边判
        .arg("--deadline-s")
        .arg((total + BACKSTOP).as_secs_f64().to_string())
        .args(locales.iter().flat_map(|l| ["--locale", l.as_str()]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| SttError::Unavailable)?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let child = Mutex::new(child);
    let phase = Mutex::new(Phase::Running);
    // 握手时刻（距 t0）。只在持有 `phase` 锁时读写（锁序：phase → hello_at / child），
    // 所以「记下握手」与「按期限定终态」不会交错
    let hello_at: Mutex<Option<Duration>> = Mutex::new(None);

    // 只在持有 `phase` 锁时调
    let due = || {
        overdue(
            cancel.load(Ordering::SeqCst),
            *lock(&hello_at),
            t0.elapsed(),
            limits,
            total,
        )
    };

    let read = std::thread::scope(|s| {
        let _stop = Stop {
            phase: &phase,
            child: &child,
        };
        let Some(stdout) = stdout else {
            return (Err(SttError::HelperFailed), None);
        };
        // 写线程：写完关管道；子进程被杀时 write 报管道断开、返回
        if let Some(mut w) = stdin {
            s.spawn(move || {
                let _ = w.write_all(&bytes);
            });
        }
        // 看门线程：取消、握手期限、总期限 → 记下原因并杀子进程（主线程的读随之拿到 EOF）
        // 判与定在同一把锁里：判完到定之间，主线程插不进来
        s.spawn(|| {
            loop {
                {
                    let mut ph = lock(&phase);
                    if *ph != Phase::Running {
                        return;
                    }
                    if let Some(w) = due() {
                        ph.kill(w);
                        let _ = lock(&child).kill();
                        return;
                    }
                }
                std::thread::sleep(POLL);
            }
        });
        let r = read_protocol(BufReader::new(stdout), locales, || {
            let _ph = lock(&phase);
            *lock(&hello_at) = Some(t0.elapsed());
        });
        // 协议坏了不等：立刻收尾；没到期、没取消就报进程失败（不让它拖到总期限被判成超时），
        // 已到期或已取消则按那个原因（同一判据 `overdue`，在定终态的锁里看）
        if r.is_err() {
            lock(&phase).settle(due());
            return (r, None);
        }
        // 协议读完：等它自己退出（看门线程仍在看期限，卡住会被杀），拿退出码。拿到退出码与定终态在同一把锁里，
        // 也用同一判据：它出完 done 却被兜底以 3 结束、看门线程还没轮上，照样定成超时；定下之后看门线程再到点也不算数
        let status = loop {
            {
                let mut ph = lock(&phase);
                if *ph != Phase::Running {
                    break None;
                }
                match lock(&child).try_wait() {
                    Ok(Some(st)) => {
                        ph.settle(due());
                        break Some(st);
                    }
                    Ok(None) => {}
                    Err(_) => break None,
                }
            }
            std::thread::sleep(POLL);
        };
        (r, status)
    });
    if let Phase::Killed(w) = *lock(&phase) {
        return Err(w);
    }
    match read {
        (Ok(got), Some(st)) if st.success() => choose(locales, got),
        (Err(e), _) => Err(e),
        _ => Err(SttError::HelperFailed),
    }
}

#[cfg(test)]
mod tests;
