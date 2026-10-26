# 交接 · 手下 login · issue #17（第一刀第 4 块：`atg-cli login`）

> 2026-10-25 · 交代 `_tmp/20261025-b13/login.md` · **终版**：代码完成，自审 P1/P2 清零，本机检查全绿；测试服务器上**正确验证码被拒、登录与登出都未验成**（已知问题，见下），按大总管定的 B：代码照合，另开 issue；codex 补审记 #32

## 一句话

`atg-cli login`（交互）、`auth state|phone|code|password|email|email-code|resend`（分步）、`logout`（终端确认）接通；`atg-core` 两份后端都有登录动作。秘密只从 `/dev/tty` 读、不回显，标准输出只有步骤名。

## 设计

- **核心** `src/atg-core/src/auth.rs`：`AuthPrompt`（步骤 + 给人看的提示：验证码去向、位数、能否重发；密码提示语、邮箱样式装 `Secret`，`Debug` 不打印；TDLib 给的手机号根本不取）· `prepare_phone`（步骤 → 形状 → 档案：测试档案只收 `99966`，正式档案不收）· `test_code_for`（`99966XYYYY` → X×5）
- `link.rs`：收线程收到授权更新时先存提示、再改授权线（`Shared::set_auth` / `auth_prompt`）
- `Backend` 特征加 `auth_prompt` `wait_auth` `auth_submit_{phone,code,email,email_code,password}` `auth_resend_code` `auth_register`（只测试档案）`logout`。输入一律 `Secret`（规约写 `String`，这里更严）
- `FakeBackend`：`set_auth(FakeAuth)` 假账号（号码、验证码、邮箱支、密码 + 提示、要注册、已登录）；`fail_next(动作, 错误)`；记账 `Call::Auth("code")` 只记动作名不记值
- **CLI** `login.rs`：流程对任何 `Backend` + `Prompter`；每步提交后等授权线离开这一步（30 秒）；人可重输的错同一步 3 次后结束；测试档案上要注册就自动注册；正式档案在 tmux 里登录给提醒
- `tty.rs`：`/dev/tty` 不回显逐字节读，自己处理回车 / 退格 / Ctrl+U / Ctrl+C（→ `interrupted` 130）/ Ctrl+D（→ `cancelled`）；**先切终端再出提示**（否则切换时会吞掉看到提示就敲的输入）；守卫 `Drop` 恢复终端；上限 256 字节、预分配不重分配
- `session.rs`：选档案（`--test-dc` 默认 `test` 并断言测试档案；否则 `--profile`，`main` 首次自动建正式档案）；起后端 → 跑 → 关；没编 TDLib 时在碰档案目录前就报 `not_implemented`
- `out.rs`：`CoreError` → `error.data`（码、说明、retryable、附加字段、`td`）与退出码表；panic 钩子只说「内部错误」

## 文件

`src/atg-core/src/{auth.rs(新), backend.rs, fake.rs, td.rs, link.rs, config.rs(load_test_phone), error.rs(EMAIL_CODE_INVALID), lib.rs}` · `examples/live_wait_phone.rs`（`wait_auth` 签名）· `tests/errors.rs` · `src/atg-cli/src/{login.rs, session.rs, tty.rs (新), main.rs, args.rs, out.rs}` · `Cargo.toml`（serde、libc、`tdlib` 特性）· `tests/cli.rs` · `README.md` · `live/login.exp`（新）

## 验证（第一次名额）

fmt ✓ · clippy -D warnings 三套（默认 / atg-core tdlib-api / atg-cli tdlib）✓ · check-no-debug ✓ · `cargo test --workspace` 全过：atg-cli 35 单测（登录各分支、各错误、分步、登出、起停、暗号）+ 10 个跑二进制的外壳测试；atg-core lib 36（新增 auth 9）

## 冒烟结果（2026-10-25，第二次名额）

- ✓ 编译、签名（`atg.dev`）不弹框；连上测试服务器；交号 → `wait_code`（短信、5 位）；**输错验证码 → 终端「验证码不对，请重新输入」、JSON `auth_code_invalid` + `td{400, PHONE_CODE_INVALID}`、退出码 3**；`auth state` / `auth phone` 正常，`wait_code` 跨进程保留
- ✗ 正确验证码也被拒：`ATG_TEST_PHONE=9996621234` 按官方规则（core.telegram.org/api/auth，已核）是 `22222`，交互与分步都回 `PHONE_CODE_INVALID`；输入核过无误。登录成功、登出两项**未验**。这个号今天已用约 4 次尝试，停手等定（换号 / 先合 / 别的线索）
- api_id / api_hash 出现次数：冒烟输出、分步 JSON、`tdlib-test.log`（空，600）都是 0
- 坑：`/usr/bin/expect` 是系统程序，macOS 启动它时删掉 `DYLD_LIBRARY_PATH`——库目录改用 `ATG_TDLIB_LIB` 传进脚本再设回去

## 自审（/atg-review，3 视角）→ 已改，重跑检查全绿

| 级 | 发现 | 处理 |
|---|---|---|
| P1 | `auth phone` 在 `wait_code` 换号重交：下一条还是 `wait_code`，只比步骤名等不到 → 30 秒后误报 `network`（脚本会重试、触发限流） | 授权更新编号（`Shared::auth_gen` / `Backend::auth_generation` + `wait_auth_update`），提交前记、提交后等编号变；交互登录重发也改用它（去掉 500ms 空等）。测试 `stepwise_phone_again_at_wait_code` |
| P2 | `AuthWrongState.expected` 实为「当前步骤」，字面会读反 | 字段加注释（名字照规约） |
| P2 | 交互与分步两套提问，反馈不一致 | 合成一个 `ask_step` + `submit`，两条路共用；分步在问人之前先查步骤（测试 `stepwise_wrong_state_is_reported_before_asking`） |
| P2 | 五个动作的允许步骤两份后端各写一遍 | `auth::AuthAction::allowed/check` 唯一一份 |
| P2 | 退出码按字符串前缀匹配 | 按变体穷举的 `match`，`EXIT_AUTH` 唯一出处 |
| P2 | `version` 固定说没接 TDLib | 编进 TDLib 时报真实版本（同步 `getOption version`） |
| P3 | 假后端未 start 时报错与真实不一致；代号表缺 T01 / 收线程；`Global` 过时注释；session 死分支；示例 `//` 注释；`normalize_code` 改名 `strip_spaces`；脚本环境变量与程序同名 | 已改（脚本改用 `ATG_SMOKE_PHONE`） |
| P3 | README 讲签名因果、`ATG_LIVE`、测试号规则；`store` 术语；`AuthCmd`/`AuthSub` 两份枚举；`Fragment` 注释 | 未改（文档/小重构，留后续） |

## 第三次名额（修完自审后）

- fmt ✓ · clippy -D warnings 三套 ✓ · check-no-debug ✓ · `cargo test --workspace` 全过：atg-cli 37 单测 + 10 个二进制外壳测试；atg-core lib 36、config 11（新增 `ATG_TEST_PHONE` 读取）
- `atg-cli version` 现报 `"tdlib":"1.8.69"`
- **P1 修复实测**：档案停在 `wait_code` 时 `auth phone` 换新号 → 立即成功，仍是 `wait_code`（05:07:05Z）
- 输错验证码再验一次：终端「验证码不对，请重新输入」✓

## ⚠️ 已知问题：测试服务器上正确验证码被拒（登录、登出都未验成）

| 时间（UTC，2026-10-26） | 号码 | 交的码 | 结果 |
|---|---|---|---|
| ~04:5x（交互 login）、05:04:17（`auth code`） | `9996621234`（DC 2） | `22222` | `400 PHONE_CODE_INVALID` |
| ~05:08（交互 login）、05:11:49（`auth code`） | `9996627391`（DC 2） | `22222` | `400 PHONE_CODE_INVALID` |

- 规则已核（core.telegram.org/api/auth：99966XYYYY → XXXXX），推出的值核过（tclsh 与程序内都是 `22222`、收到 5 个字符）
- 服务器每次说验证码走「短信、5 位」（`authenticationCodeTypeSms`）。没验证的猜测：测试服务器这几天的行为变了，或 DC 2 有问题；下次可试 DC 1 / 3 的号
- 两个号今天各用了约 3–4 次尝试，停手不再耗号
- 另：`live/login.exp` 在**第二次**验证码提示处会挂住，连自己的超时都不触发（两次都是），原因没查清；冒烟时输对的码改用 `auth code`（不用终端）。下次排查先看它

## 冒烟怎么跑

```sh
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo build -p atg-cli --features tdlib
codesign -s atg-dev -i atg.dev --force target/debug/atg-cli
ATG_LIVE=1 ATG_SMOKE_PHONE="$(grep '^ATG_TEST_PHONE=' .env.local | cut -d= -f2)" \
  ATG_TDLIB_LIB="$LOCAL_TDLIB_PATH/lib" expect src/atg-cli/live/login.exp target/debug/atg-cli
```

## 规约缺口 → 人定「全按建议，把现状写进契约」（已办）

1. 新码 `auth_phone_not_for_profile`（退出码 3，无 `td` 附件），代码里替换掉暂用的 `forbidden`；测试 `phone_for_wrong_profile_ends_with_its_own_code`
2. 不支持的授权步骤 → `auth_wrong_state` + 专门说明：写进 `errors.md` 第一节第 3 小节
3. `EMAIL_CODE_INVALID` → `auth_code_invalid`（原文不登记）：写进 `errors.md` 映射第 7 条
4. `internal` 带 `what`：写进 `errors.md` 码表
5. 注册名字写死 `Atg Test`：写进 `actions.md`、`testing.md`
6. `flood.json` 登录计数：不做，大总管另开 issue

三份契约各加了修订记录。另把「登录输入用 `Secret`」「提交后按下一条授权更新判断」「`auth_submit_phone` 可收的步骤」写进了 `actions.md`。

## 收尾（第四次名额）

- fmt ✓ · clippy -D warnings 三套 ✓ · check-no-debug ✓ · `cargo test --workspace` 全过（atg-cli 38 单测 + 10 二进制测试）
- 新增 `src/atg-cli/live/login-main.sh`：给人在「终端」App 里亲手跑，用自己的账号登录正式服务器（档案 main）。编译 + 签名 + 设库路径 → `atg-cli --format text login`（号码、验证码、密码由人从 /dev/tty 输入）→ `atg-cli auth state` 报结果 → **不登出**，末尾打印退出命令。前置：要真终端、不在 tmux 里、有 `.env.local`。不写号码、不碰会话与消息。**AI 没有运行它**（只做了 `bash -n` 语法检查）
- 契约里的命令叫 `auth state`，没有 `auth status`，脚本用的是 `auth state`
