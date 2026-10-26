//! `session`：选档案、起后端、跑一件事、关后端。登录这组命令（`login` · `auth *` · `logout`）用它。
//!
//! 选档案（`cli.md` 全局选项；`config-privacy.md` 第一节第 2 小节）：
//! - `--test-dc`：档案名默认 `test`（`--profile` 另给名字就用那个，如第二个测试号的 `test2`），并断言它是测试
//!   服务器档案——没有就按测试档案新建，已有的是正式档案就拒绝启动（`internal{profile_kind_mismatch}`）
//! - 不带 `--test-dc`：打开 `--profile`（默认 `main`）。`main` 第一次用时按正式档案新建；别的名字必须已存在
//!   （档案种类只由档案自己的标记定，不由选项定）
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
use atg_core::link::block_on;
use atg_core::secret::Secret;
use atg_core::status::AuthStep;
use serde_json::Value;

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
    if g.test_dc {
        // 已有的是正式档案：`create` 核对种类不符，报 `internal{profile_kind_mismatch}`，不启动
        return Profile::create(&root, profile_name(g), ServerKind::Test).map_err(Fail::Core);
    }
    match Profile::open(&root, &g.profile) {
        Ok(p) => Ok(p),
        Err(_) if g.profile == "main" => {
            Profile::create(&root, "main", ServerKind::Main).map_err(Fail::Core)
        }
        Err(e) => Err(Fail::Core(e)),
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

/// 真实后端（本进程唯一的一个）
#[cfg(feature = "tdlib")]
pub fn td_backend() -> Result<atg_core::td::TdBackend, Fail> {
    use atg_core::error::Holder;
    use atg_core::keystore::Keychain;
    use atg_core::td::{TdBackend, TdConfig};
    let log_dir = config::default_log_dir().map_err(Fail::Core)?;
    TdBackend::new(TdConfig {
        env_file: config::default_env_file(),
        keystore: Box::new(Keychain),
        log_dir,
        holder: Holder::Cli,
    })
    .map_err(Fail::Core)
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
