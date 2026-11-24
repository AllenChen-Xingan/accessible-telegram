# 评审 063 · #42 开发版开强化运行时（codex · gpt-6-astra · read-only · diff 类，effort high，按契约 / 安全口径审）

> 对象：`origin/main...HEAD`（第 1 轮 HEAD = 0dd890e：检查点 4f19c0e、合 main 83f3487、自审修正 0dd890e）。
> - TDLib 构建：OpenSSL 静态链、编完用开发身份给 libtdjson 签名、常驻软链接 `~/.atg-dev/tdlib`（`build.sh link`）
> - `dev-sign.sh` 默认加 `--options runtime`，`ATG_DEV_HARDEN=0` 关
> - `atg-cli`、`atg-core` 新增 `build.rs`：`tdlib` 特性且 macOS 下写 rpath `$HOME/.atg-dev/tdlib/lib`
> - 新脚本 `scripts/check-harden.sh`：注入对照、换库被拒
> - 文档与契约：`src/README.md`、`config-privacy.md`、`testing.md`、exp005、007、004、cog
>
> 类型说明：codex-review 的 `contract` 模板只收文件、不收提交范围，而评审只审已提交的范围（共同规矩 17），所以用 `diff` 类 + effort high，把契约模板的攻击面清单写进 `--focus-file`（本次重点：注入 / 换库是否真挡住、软链接被换、检查器会不会把失败判成通过、身份名与团队号不外泄、CI 不受影响、文档与脚本一致）。
> 依据文件：`specs/contracts/config-privacy.md`、`docs/experiments/exp005-keychain-acl/README.md`。隐私扫描：干净。
> 之前：
> - **多视角自审 1 轮**（83f3487）：P1×1（团队号写进 build-info 并打印，已改），P2×11（改 10 条，build.rs 重复与 rpath 路径写 4 处开 #170），P3×9（全改），修正 0dd890e
> - **实测**（开发机，2026-11-24，领名额跑）：TDLib 重编；fmt、两种 clippy、`cargo test --workspace`（713 过、0 败、5 忽略，经 runner 签成强化运行时版）、check-no-debug、check-sources；`check-harden.sh` 全过；测试服务器 `live_wait_phone` 直接跑到 `wait_phone`；正式档案只读 `chats -n 5` 退出码 0、不弹框（人同意，经大总管）；`me` 报 `not_implemented`（命令本身没做）

## 第 1 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r42-codex/r42-codex-1.md`（不入库）。`result: P0×0 P1×1 P2×3 P3×0`。
本轮 P0/P1：1 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1 #1 属实，已改**：`check-harden.sh` 对任何传进来的程序跑 `<程序> version`。`live_wait_phone` 不认参数，继承了 `ATG_LIVE=1` 就会真起后端、读钥匙串、联网，和脚本头上的「不碰钥匙串、不联网」矛盾。改法：只收 `atg-cli`（它的 `version` 没有副作用），别的程序报准备失败、退出码 2；脚本开头 `unset ATG_LIVE`。用法和 `testing.md` 那一行同步改了。核过：传 `live_wait_phone` 退出码 2
- **P2 #2 属实，已改**：`build.sh` 判断 OpenSSL 有没有静态链时用的是 `otool -L … | grep`，otool 失败时条件为假，照样往下签名，并记 `openssl_link=static`。改成先把输出存进变量、确认 otool 成功、输出里列出了库自己，再匹配 `libssl`、`libcrypto`
- **P2 #3 属实，已改**：`check-harden.sh` 读签名信息用的是 `codesign -dv … || true`。读失败时输出为空，「对照版不带 runtime」会被判成通过。改成读不出、或者输出里没有 `flags=`，都算准备失败、退 2。另外开头的 `mktemp`、`cp` 这些准备步骤失败时也统一退 2（`prep`）
- **P2 #4 属实，已改**：注入检查只看有没有标记文件，强化版要是根本没起来，也会算成「挡住了注入」。现在不管带不带 TDLib，注入检查之前都先核两版能用 `version` 照常起；原先只在 TDLib 段里核「照常起」，已并到前面
- 无法核实 1 条：tdlib-rs 的 `OUT_DIR` rpath 会不会进最终程序。作者核过：`otool -l target/debug/atg-cli` 的 `LC_RPATH` 只有一条 `~/.atg-dev/tdlib/lib`，没有 `OUT_DIR`，也没有 `@loader_path`
- 抽查属实 4 条：`ATG_DEV_HARDEN` 只认精确的 `0`，bash 3.2 下空数组展开正常；先签库、后算哈希，build-info 不写身份名和团队号；已存在的非软链接不覆盖；两个 `build.rs` 只在 `tdlib` 特性且 macOS 下输出 rpath
- 改后已核：`check-harden.sh` 全过（新加的「照常起」两项也过），传非 atg-cli 的程序退 2；`build.sh` 语法检查通过

### 是否再审

本轮有 P1，改完再审一轮（带上第 1 轮原文和本记录）。

## 第 2 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r42-codex/r42-codex-2.md`（不入库）。HEAD = 2dc9ba3。`result: P0×0 P1×0 P2×2 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无（两条都改了）

- 上一轮复核：#1、#2、#4 已落实；#3 签名读取那半已修，准备步骤的退出码还差一处（即本轮 #1）
- **P2 #1 属实，已改**：两段生成 C 源文件的 `cat > … <<'C'` 没接 `prep`，盘满或写不进时 `set -e` 会以 1 退出，和「有一项不符」混在一起。两处都补了 `|| prep`。核过：把 `TMPDIR` 指到只读目录，退出码 2
- **P2 #2 属实，已改**：`src/README.md` 在 `live_wait_phone` 那段里写的是 `check-harden.sh <程序>`，照着传那个例子程序会退 2。改成「`cargo build -p atg-cli --features tdlib` 后传 `target/debug/atg-cli`；只收 atg-cli，上面的例子不收」
- 无法核实 1 条：最终程序是不是只有一条 rpath（产物不在准读范围）。改成让脚本自己核：`check-harden.sh` 的 TDLib 段加一项「rpath 只有这一条」（数 `LC_RPATH`），别的 rpath（如 tdlib-rs 的 `OUT_DIR`、`@loader_path`）一出现就判不符。本机跑过：1 条，通过
- 抽查属实 4 条：同第 1 轮那 4 点（`ATG_DEV_HARDEN` 只认精确的 `0`、先签后算哈希且 build-info 不记身份、非软链接不覆盖、`build.rs` 的条件）
- 改后已核：`check-harden.sh` 全过（共 9 项）

### 是否再审

本轮无 P0/P1，是连续第 1 轮；改完再审一轮，确认是否连续第 2 轮。
