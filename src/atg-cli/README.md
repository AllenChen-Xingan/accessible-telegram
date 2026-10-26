# atg-cli

无障碍 Telegram 的命令行入口，也是 AI 和脚本的首选接口：标准输出一行一个 JSON（外壳、结尾行、退出码照 `specs/contracts/cli.md`），给人看加 `--format text`。依赖 `atg-core`，不依赖 GPUI。

**已做**：`version`（或 `--version`）、`--help`；登录这组（第一刀第 4 块，T01）：

- `login`：交互登录，手机号 → 验证码 → 二步验证密码（服务器要邮箱就先问邮箱、邮箱验证码）。全部从 `/dev/tty` 读、不回显；提示语、密码提示、出错原因只写终端。每走到一步标准输出一行 `{"kind":"auth","data":{"step":"wait_code"}}`，最后一行 `result` / `error`。同一步输错 3 次就以那个错误结束
- `auth state|phone|code|password|email|email-code|resend`：分步登录，给 AI 和脚本，每步一行 `auth`，出错直接是结尾行。测试档案（`--test-dc`）上号码取 `.env.local` 的 `ATG_TEST_PHONE`，验证码按测试号规则推出，不问人
- `logout`：在终端当场输入 `yes` 确认后登出

测试服务器上号码没注册（会定期清空）时自动注册。其余子命令还只占名，输出一行 `not_implemented` 错误、退出码 1。

跑法：真实后端要编进 TDLib（`--features tdlib`），运行前提（`.env.local`、库路径、签名）见 `src/README.md` 与 `state/handoff/login.md`：

```sh
export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
cargo build -p atg-cli --features tdlib
codesign -s atg-dev -i atg.dev --force target/debug/atg-cli
DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib" target/debug/atg-cli --test-dc login
```

测试：`cargo test -p atg-cli`（登录流程用假后端单测走全部分支；另跑真二进制对照契约检查外壳与退出码）。本机冒烟脚本 `live/login.exp`（测试服务器、测试号，用 expect 把输入喂进终端）。
