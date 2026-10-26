# atg-cli

无障碍 Telegram 的命令行入口，也是 AI 和脚本的首选接口：标准输出一行一个 JSON（外壳、结尾行、退出码照 `specs/contracts/cli.md`），给人看加 `--format text`。依赖 `atg-core`，不依赖 GPUI。

**已做**：`version`（或 `--version`）、`--help`；登录这组（第一刀第 4 块，T01）：

- `login`：交互登录，手机号 → 验证码 → 二步验证密码（服务器要邮箱就先问邮箱、邮箱验证码）。全部从 `/dev/tty` 读、不回显；提示语、密码提示、出错原因只写终端。每走到一步标准输出一行 `{"kind":"auth","data":{"step":"wait_code"}}`，最后一行 `result` / `error`。同一步输错 3 次就以那个错误结束
- `auth state|phone|code|password|email|email-code|resend`：分步登录，给 AI 和脚本，每步一行 `auth`，出错直接是结尾行。测试档案（`--test-dc`）上号码取 `.env.local` 的 `ATG_TEST_PHONE`，验证码按测试号规则推出，不问人
- `logout`：在终端当场输入 `yes` 确认后登出

测试服务器上号码没注册（会定期清空）时自动注册。

列会话（第一刀第 5 块，T02）：

- `chats [--list main|archive] [-n N] [--unread] [--cursor C]`：一行一个 `{"kind":"chat","data":<ChatItem>,"speech":{"brief":"会话项读法"}}`，顺序同官方客户端；`-n` 默认 100。最后一行 `result`：`count` · `more` · `next_cursor`（`more` 为真时才有，下一页用 `--cursor` 接着列）· `synced`（等到服务器把离线期间漏掉的更新补送完了没有，最多等 10 秒；`false` 时未读数可能是旧的，断网时就是这样）· `load_timed_out`（加载 10 秒没应答提前停了，可能断网）· `first_chat_ms`（进程启动到第一条会话出来的毫秒数）· `elapsed_ms`。`--format text` 每行一句读法，统计写标准错误。只读：不打开会话、不标已读

读法定标（第一刀第 6 块，#19）：

- `speech --fixture PATH [--level brief|chat_brief] [--tz OFFSET]`：夹具（TDLib 更新对象的 JSON 数组，读法夹具在 `src/atg-core/fixtures/speech/`）→ 内容树 → 读法，不联网、不碰档案。`brief`（默认）按到达顺序每条消息一行（`kind:"message"`，状态行是 `event`）；`chat_brief` 主列表每个会话一行（`kind:"chat"`）。读法在 `speech.brief`，CLI 没有按键，不读「按 D 展开」。`--tz` 写 `+08:00`、`-05:30`、`+0800`、`+8`、`Z` 或 `UTC`，默认系统时区。`--format text` 每行一句读法；加 `--tz +08:00` 时与 `src/atg-core/tests/snapshots/*brief*.snap` 逐行一致，只差快照里 GUI 才有的「按 D 展开」（`tests/cli.rs` 管着）。「定标」= 先把读法产出来给人实际听，人改定的那一版才是标准说法；这个子命令就是给人试听、校准用的

其余子命令还只占名，输出一行 `not_implemented` 错误、退出码 1。全局选项里 `--fake`（场景目录）、`--redact`、`--wait`、`--unsafe-direct-send` 还没接上；`--tz` 现在只有 `speech` 用。

跑法：真实后端要编进 TDLib（`--features tdlib`），运行前提（`.env.local`、库路径、签名）见 `src/README.md`（`state/handoff/login.md` 是登录那块的过程记录，里面的 `codesign -s atg-dev …` 已作废，签名以 `src/README.md` 为准）：

```sh
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo run -p atg-cli --features tdlib -- --test-dc login   # runner 自动签名、带上库目录（scripts/dev-sign-run.sh）
# 或分开做：cargo build -p atg-cli --features tdlib && bash scripts/dev-sign.sh target/debug/atg-cli
#           DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib" target/debug/atg-cli --test-dc login
```

测试：`cargo test -p atg-cli`（登录、列会话用假后端单测走全部分支；另跑真二进制对照契约检查外壳与退出码）。本机冒烟脚本 `live/login.exp`（测试服务器、测试号，用 expect 把输入喂进终端）。
