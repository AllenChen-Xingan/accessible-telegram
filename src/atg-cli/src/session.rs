//! `session`：选档案、起后端、跑一件事、关后端。登录这组命令（`login` · `auth *` · `logout`）用它。
//!
//! 选档案（`cli.md` 全局选项；`config-privacy.md` 第一节第 2 小节）：
//! - `--test-dc`：档案名默认 `test`（`--profile` 另给名字就用那个，如第二个测试号的 `test2`），并断言它是测试
//!   服务器档案——没有就按测试档案新建，已有的是正式档案就拒绝启动（`internal{profile_kind_mismatch}`）
//! - 不带 `--test-dc`：打开 `--profile`（默认 `main`）。`main` 第一次用时按正式档案新建；别的名字必须已存在
//!   （档案种类只由档案自己的标记定，不由选项定）
//! - 两种都一样：档案缺标记、目录里却留着东西（库、文件、发送状态……），拒绝启动（`internal{profile_marker_missing}`），
//!   不按选项补写种类（#61）
//!
//! 后端：编进 TDLib（`--features tdlib`）才有真实后端；`--fake` 的场景目录还没做（要假时钟与时间线，
//! `testing.md` 第一节第 3 小节，留给用到它的块；`chats` 的逻辑由假后端单测覆盖）。
//! 起不来时照样调 `shutdown` 收尾（客户端建好之后才失败的情况，`td.rs` 头部）。

use crate::args::Global;
use crate::login::{self, AuthSub, Prompter};
use crate::out::Fail;
use crate::tty::Tty;
use atg_core::backend::{Backend, SHUTDOWN_TIMEOUT};
use atg_core::config::{self, Profile, ServerKind};
use atg_core::error::CoreError;
use atg_core::link::block_on;
use atg_core::secret::Secret;
use atg_core::status::AuthStep;
use serde_json::Value;
use std::path::Path;

/// 要跑的那件事
#[derive(Clone, Copy)]
pub enum Job {
    Login,
    Auth(AuthSub),
    Logout,
}

/// 按全局选项该用哪个档案名：`--test-dc` 且没另给名字 → `test`（`forward` 探锁也用它，两边必须是同一个档案）
pub fn profile_name(g: &Global) -> &str {
    if g.test_dc && g.profile == "main" {
        "test"
    } else {
        &g.profile
    }
}

/// 按全局选项打开档案
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
pub fn open_profile(g: &Global) -> Result<Profile, Fail> {
    let root = config::default_root().map_err(Fail::Core)?;
    pick_profile(&root, g.test_dc, profile_name(g)).map_err(Fail::Core)
}

/// 选档案的规则本身（模块头），根目录由调用方给，测试才不碰真实档案目录
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
fn pick_profile(root: &Path, test_dc: bool, name: &str) -> Result<Profile, CoreError> {
    if test_dc {
        // 已有的是正式档案：`create` 核对种类不符，报 `internal{profile_kind_mismatch}`，不启动
        return Profile::create(root, name, ServerKind::Test);
    }
    match Profile::open(root, name) {
        Ok(p) => Ok(p),
        // 缺标记一律交给 `create`：全新的 main 就此建好，留着数据的由它拒绝（#61）；读不懂的标记等别的错误原样报
        Err(CoreError::Internal {
            what: "profile_marker_missing",
        }) if name == "main" => Profile::create(root, "main", ServerKind::Main),
        Err(e) => Err(e),
    }
}

/// 终端：第一次要用时才打开（`auth state` 这类不问人的命令在管道里也能跑）
#[derive(Default)]
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
pub struct LazyTty {
    tty: Option<Tty>,
    tried: bool,
}

#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
impl LazyTty {
    fn get(&mut self) -> Option<&mut Tty> {
        if !self.tried {
            self.tried = true;
            self.tty = Tty::open().ok();
        }
        self.tty.as_mut()
    }
}

#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
const NO_TTY: &str = "要在终端里跑：读不到 /dev/tty（手机号、验证码、密码只从终端读）";

impl Prompter for LazyTty {
    fn say(&mut self, s: &str) {
        if let Some(t) = self.get() {
            t.say(s);
        }
    }

    fn ask_secret(&mut self, prompt: &str) -> Result<Secret, Fail> {
        let t = self.get().ok_or(Fail::Usage(NO_TTY))?;
        t.read_secret(prompt).map_err(login::read_end)
    }

    fn confirm(&mut self, prompt: &str) -> Result<bool, Fail> {
        let t = self
            .get()
            .ok_or(Fail::Usage("logout 要在终端里当场确认：读不到 /dev/tty"))?;
        t.read_visible(prompt)
            .map(|a| a == "yes")
            .map_err(login::read_end)
    }
}

/// 起后端 → 跑 → 关。`on_step` 收授权步骤（标准输出那一行）
pub fn run(g: &Global, job: Job, on_step: &mut dyn FnMut(AuthStep)) -> Result<Value, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: job_name(job),
            message_zh: "假后端的场景目录还没做，登录请连测试服务器：--test-dc",
        });
    }
    // 没编进 TDLib 就在碰档案目录之前说清楚
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = on_step;
        Err(Fail::NotImplemented {
            command: job_name(job),
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let mut tty = LazyTty::default();
        real(&profile, job, &mut tty, on_step)
    }
}

pub fn job_name(job: Job) -> &'static str {
    match job {
        Job::Login => "login",
        Job::Auth(_) => "auth",
        Job::Logout => "logout",
    }
}

/// 真实后端（本进程唯一的一个）。`direct`：本进程是不是以 `--unsafe-direct-send` 启动的（只有发送类命令和
/// `serve` 会传真；别的命令不发东西，闸用不到）
#[cfg(feature = "tdlib")]
pub fn td_backend_with(direct: bool) -> Result<atg_core::td::TdBackend, Fail> {
    use atg_core::error::Holder;
    use atg_core::keystore::Keychain;
    use atg_core::td::{TdBackend, TdConfig};
    let log_dir = config::default_log_dir().map_err(Fail::Core)?;
    TdBackend::new(TdConfig {
        env_file: config::default_env_file(),
        keystore: Box::new(Keychain),
        log_dir,
        holder: Holder::Cli,
        gate: atg_core::gate::Gate::new(
            direct,
            Holder::Cli,
            atg_core::gate::system_authenticator(),
        ),
    })
    .map_err(Fail::Core)
}

/// 不发东西的命令用的真实后端（闸照常在，但不是直发）
#[cfg(feature = "tdlib")]
pub fn td_backend() -> Result<atg_core::td::TdBackend, Fail> {
    td_backend_with(false)
}

#[cfg(feature = "tdlib")]
fn real(
    profile: &Profile,
    job: Job,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
) -> Result<Value, Fail> {
    let b = td_backend()?;
    drive(&b, profile, job, p, on_step)
}

/// `chats`：选档案、起真实后端、列（`--fake` 场景目录还没做）
pub fn chats(
    g: &Global,
    q: &crate::chats::Query,
    started: std::time::Instant,
) -> Result<crate::chats::Listing, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "chats",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, started);
        Err(Fail::NotImplemented {
            command: "chats",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        crate::chats::run(&b, &profile, q, started)
    }
}

/// `history`：选档案、起真实后端、取一页（`--fake` 场景目录还没做）
pub fn history(
    g: &Global,
    q: &crate::history::Query,
    started: std::time::Instant,
) -> Result<crate::history::Page, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "history",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, started);
        Err(Fail::NotImplemented {
            command: "history",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        crate::history::run(&b, &profile, q, started)
    }
}

/// `download`：选档案、起真实后端、下这条语音、关。v0 不经本机接口转发（arch 定 2026-11-13）
pub fn download(
    g: &Global,
    q: &crate::voice::VoiceQuery,
    started: std::time::Instant,
) -> Result<crate::voice::VoiceFile, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "download",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, started);
        Err(Fail::NotImplemented {
            command: "download",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        crate::voice::run_download(&b, &profile, q, started)
    }
}

/// `play` 的准备：选档案、起真实后端、找到 → 下载 → 解码 → 出口停着打开 →（要标就）标已听、关后端。v0 不转发
pub fn play_prepare(
    g: &Global,
    q: &crate::voice::VoiceQuery,
    mark: bool,
    started: std::time::Instant,
) -> Result<(atg_audio::Player<atg_audio::Queue>, crate::voice::VoiceFile), Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "play",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, mark, started);
        Err(Fail::NotImplemented {
            command: "play",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        crate::voice::run_prepare_play(
            &b,
            &profile,
            q,
            mark,
            atg_audio::Queue::new(),
            &crate::signals::STOP,
            started,
        )
    }
}

/// `watch`：选档案、起真实后端、订阅到 Ctrl+C（输出由 `watch` 自己写标准输出，回退出码）
pub fn watch(
    g: &Global,
    q: &crate::watch::Query,
    cx: atg_core::speech::SpeechCtx,
    format: crate::args::Format,
    started: std::time::Instant,
) -> Result<u8, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "watch",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, cx, format, started);
        Err(Fail::NotImplemented {
            command: "watch",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        Ok(crate::watch::run(
            &b,
            &profile,
            q,
            cx,
            format,
            &crate::signals::STOP,
            Box::new(crate::watch::StdoutLines::default()),
            crate::watch::now_ms,
            started,
        ))
    }
}

/// `links`：同上
pub fn links(
    g: &Global,
    q: &crate::history::LinksQuery,
    started: std::time::Instant,
) -> Result<crate::history::LinkList, Fail> {
    if g.fake.is_some() {
        return Err(Fail::NotImplemented {
            command: "links",
            message_zh: "假后端的场景目录还没做；要用真实账号或测试服务器（--test-dc）",
        });
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (q, started);
        Err(Fail::NotImplemented {
            command: "links",
            message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
        })
    }
    #[cfg(feature = "tdlib")]
    {
        let profile = open_profile(g)?;
        let b = td_backend()?;
        crate::history::run_links(&b, &profile, q, started)
    }
}

/// 起后端、跑这件事、关后端（任何后端都一样；单测用假后端走这里）
#[cfg_attr(not(feature = "tdlib"), allow(dead_code))]
pub fn drive<B: Backend>(
    b: &B,
    profile: &Profile,
    job: Job,
    p: &mut dyn Prompter,
    on_step: &mut dyn FnMut(AuthStep),
) -> Result<Value, Fail> {
    // 这组命令不读事件流：取出来就丢掉，事件不在无界通道里越攒越多
    drop(b.subscribe());
    if let Err(e) = block_on(b.start(profile)) {
        let _ = b.shutdown(SHUTDOWN_TIMEOUT);
        return Err(Fail::Core(e));
    }
    let kind = profile.kind();
    let r = match job {
        Job::Login => login::login(b, kind, p, on_step),
        Job::Auth(sub) => {
            let test_phone = (kind == ServerKind::Test)
                .then(|| config::load_test_phone(&config::default_env_file()))
                .flatten();
            login::auth_step(b, kind, sub, test_phone.as_ref(), p, on_step)
        }
        Job::Logout => login::logout(b, p, on_step),
    };
    if let Err(e) = b.shutdown(SHUTDOWN_TIMEOUT) {
        // 结尾行仍按这件事本身的结果；关闭出错另在标准错误说一声（只有机读码）
        eprintln!("atg-cli: 关闭时出错（{}）", e.code());
    }
    r
}

/// `send` 直跑：选档案、起真实后端（直发与否看本进程的 `--unsafe-direct-send`）、发、关。输出直接写标准输出
pub fn send(
    g: &Global,
    format: crate::args::Format,
    p: &crate::send::SendParams,
    started: std::time::Instant,
) -> u8 {
    l2_run(g, format, "send", started, &mut |b, stream, feed, sink| {
        crate::send::execute_send(
            b,
            stream,
            feed,
            p,
            sink,
            format,
            started,
            atg_core::send::RESULT_TIMEOUT,
        )
    })
}

/// `resend` 直跑
pub fn resend(
    g: &Global,
    format: crate::args::Format,
    p: &crate::send::ResendParams,
    started: std::time::Instant,
) -> u8 {
    l2_run(
        g,
        format,
        "resend",
        started,
        &mut |b, stream, feed, sink| {
            crate::send::execute_resend(
                b,
                stream,
                feed,
                p,
                sink,
                format,
                started,
                atg_core::send::RESULT_TIMEOUT,
            )
        },
    )
}

/// [`l2_run`] 交给每条发送类命令的那一段：拿到已起好的后端、事件流、Store，输出写进给它的出口，返回退出码
#[cfg(feature = "tdlib")]
type L2Body<'a> = dyn FnMut(
        &atg_core::td::TdBackend,
        &mut atg_core::link::EventStream,
        &mut crate::chats::Feed,
        &mut dyn crate::out::Sink,
    ) -> u8
    + 'a;

/// 不编 TDLib 时只为通过类型检查（`l2_run` 在那种构建里直接报 `not_implemented`，用不到它）
#[cfg(not(feature = "tdlib"))]
type L2Body<'a> = dyn FnMut(
        &atg_core::fake::FakeBackend,
        &mut atg_core::link::EventStream,
        &mut crate::chats::Feed,
        &mut dyn crate::out::Sink,
    ) -> u8
    + 'a;

/// 发送类命令（`send`、`resend`）的直跑外壳：选档案 → 起真实后端（直发与否看本进程的 `--unsafe-direct-send`）→
/// 直发时先出开场提示与 `status` 行 → 跑 `body` → 关后端
fn l2_run(
    g: &Global,
    format: crate::args::Format,
    command: &'static str,
    started: std::time::Instant,
    body: &mut L2Body<'_>,
) -> u8 {
    use crate::out::{Stdio, fail_with_to};
    if g.fake.is_some() {
        return fail_with_to(
            &mut Stdio,
            format,
            &Fail::NotImplemented {
                command,
                message_zh: "假后端的场景目录还没做；要用测试服务器（--test-dc）",
            },
        );
    }
    #[cfg(not(feature = "tdlib"))]
    {
        let _ = (started, body);
        fail_with_to(
            &mut Stdio,
            format,
            &Fail::NotImplemented {
                command,
                message_zh: "这个版本没编进 TDLib：cargo build -p atg-cli --features tdlib",
            },
        )
    }
    #[cfg(feature = "tdlib")]
    {
        let r = open_profile(g).and_then(|profile| {
            let b = td_backend_with(g.unsafe_direct_send)?;
            crate::history::with_backend(&b, &profile, started, |b, stream, feed| {
                if b.gate().direct() {
                    crate::send::direct_banner(b, &mut Stdio, format);
                }
                Ok(body(b, stream, feed, &mut Stdio))
            })
        });
        match r {
            Ok(code) => code,
            Err(f) => fail_with_to(&mut Stdio, format, &f),
        }
    }
}

#[cfg(test)]
mod tests;
