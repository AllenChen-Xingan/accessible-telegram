# 评审 022 · #63 #73 #78 #79 CLI 等待都有期限、Ctrl+C 能收尾（codex · gpt-6-astra · read-only）

> 对象：`git diff main...HEAD`（worktree `atg-wt/fix`，分支 `fix/63-bounded-waits`，已合进含 #61 的 main），依据 `specs/contracts/cli.md`、`specs/contracts/errors.md`。**最新一轮：第 2 轮（2026-11-13）· 总评「修改」· 已达停止线（连续两轮无 P0/P1）**。
> 累计处理：P0×0、P1×0、P2×9、P3×0 逐条回源码核实，属实 9、驳回 0、无法核实 0；8 条已修（第 1 轮 #2 起后端那一半按理由保留现状），第 2 轮 #2 留作 issue。

开审前的多视角自审（atg-review，3 个视角）：P0/P1 无；P2×7 都已处理——登出确认与转发 `auth state` 两处 Ctrl+C 被吞（确认改逐字节读、自己认 Ctrl+C；信号处理挪到转发之后装）、`download` 传了立不起来的 `signals::STOP`（改传永不立起的标志、测试注释改准）、`within` / `bounded` 用法口径写进 `deadlines.rs` 头部、契约 `cli.md` 补「最多等多久」、两条测试走不到自己声称的路径（补断言、改成确认途中立标志）；分支落后 main 的假差异由大总管合并 main 解决。P3×10 里注释、出处、use 位置、「150 秒」文案随常量等 9 条照改；`AUTH_POLL` 与 `STOP_POLL` 两个片长不合并（授权线有条件变量、醒得早），在 `deadlines.rs` 头部写明两处各管什么。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-022/diff-63-bounded-codex-1.md`（不入库）。effort medium，耗时 52 秒。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜src/atg-cli/src/tty.rs:105 读秘密的出错出口没清零**（第 1 轮 #1）
  - 核实：属实。重构后 `read_raw` 只在成功时交回缓冲区，Ctrl+C / 读错误时 `Vec` 直接丢掉，没经 `Secret`；旧版不论成败都先包成 `Secret`
  - 处理：`read_raw` 改回 `(Vec<u8>, Result<(), ReadEnd>)`，`read_secret` 先 `Secret::new(buf)` 再判结果
- **P2｜src/atg-cli/src/watch.rs:783 Ctrl+C 之后仍可能以错误结束**（第 1 轮 #2）
  - 核实：属实（开订阅前那段）。`resolve` / `chat_open` 按期限等、不看 Ctrl+C，等的时候按了、随后失败或到点，直接出 `error`，与「按了就出 `result`、退 0」不符
  - 处理：加 `fail_unless_stopped`：已按过 Ctrl+C 就 `finish_all` 出 `result`、退 0，否则照实报错。新测 `watch/tests/ctrl_c.rs` 两种都测。起后端那一步不改：后端没起来就还没开始订阅、没有可收尾的，失败照实报错比假装正常结束更有用；`watch.rs` 注释与 `cli.md`「最多等多久」写明（`serve` 同）
- **P2｜src/atg-cli/src/serve.rs:540 执行方收尾会打断转来的 `auth state`**（第 1 轮 #3）
  - 核实：属实。`Ctx.stop` 传进登录流程，执行方一按 Ctrl+C，转来的 `auth state` 报 `interrupted`，与 `cli.md`「先做完手上的请求」不符
  - 处理：改传一个永远立不起来的标志（只按期限等），删掉 `Ctx.stop`
- **P2｜src/atg-cli/src/login/tests/bounded.rs:35 挂起测试靠固定延时，慢机器会偶发失败**（第 1 轮 #4）
  - 核实：属实。300 毫秒后立停止标志，主线程那时可能还没把请求交出去，`phone` 交出的断言就会红；voice 下载、watch 话题两条同类
  - 处理：改成等假后端记下那次请求（`thread::scope` 里轮询 `calls()`，5 秒上限）再立标志：login 三条（`stop_once_sent`）、voice、watch 各一条
- **P2｜src/atg-cli/src/tty.rs:129 回显的退格按字数擦，没按显示宽度**（第 1 轮 #5）
  - 核实：属实。汉字占 2 列，退格只擦 1 列；Ctrl+U 同
  - 处理：加 `cols`（中日韩、全角、表情 2 列，组合附加符号 0 列，半个字按字节数），按它擦；直接在缓冲区上算、不拷贝（读秘密也走这里）。测试补宽字符、组合符号、半个字

### 引用抽查

- codex 列的「不符」：1 条（watch.rs:708 注释说起好就能正常停，起后端超时却直接报错）——并入 #2 处理，注释已改准
- codex 列的「核实属实」：4 条
- 无法核实：codex 未编译、未跑测试

### 是否再审

P2 全部属实已修 → 开第 2 轮，带本轮原文与本记录。

## 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews-022/diff-63-bounded-codex-2.md`（不入库）。effort medium，耗时 70 秒，模型 gpt-6-astra，只读沙箱。对象 `main...HEAD` 到 f48541b。
本轮 P0/P1：0 条 · 停止线：**满足**（第 1、2 轮都无 P0/P1）· 留作 issue 的 P2：1 条（#2）

- **P2｜src/atg-cli/src/login.rs:87 等授权线时按的 Ctrl+C 可能被跨过，接着停在下一个输入提示**（第 2 轮 #1）
  - 核实：属实。`wait_line` 每片开头才看停止标志；那一片里按了、新状态又同时到了，就照常返回下一步，`login` 接着问人
  - 处理：片里等到新状态后再看一次停止标志；`login` 问人之前也看一次。补测 `stop_during_a_slice_wins_over_a_new_state`（停止线后修，未再过 codex，作者实跑测试）
- **P2｜src/atg-cli/src/watch.rs:764 起好后端之后、等授权线期间按 Ctrl+C，仍出 `error`**（第 2 轮 #2）
  - 核实：属实。`watch_loop` 开头的 `ensure_ready` 在出 `status`、挂上订阅者之前，失败走 `fail_line`；那时还没有订阅者可以 `finish_all`
  - 处理：留作 issue（要把订阅者提前挂上或另写一条收尾行，改动比本条大；只影响「起好后端、授权线 10 秒内还没定」这一小段里按 Ctrl+C 的结尾行，退出码非 0、不多发请求）
- **P2｜src/atg-cli/src/chats.rs:229 等授权线到点报 `internal{auth_state_timeout}`，与新契约「到点报 network」不符**（第 2 轮 #3）
  - 核实：属实（main 上原有的行为，本批新写的契约把它说成了 `network`）。这个 `what` 没在 `errors.md` 登记过
  - 处理：改走 `deadlines::stalled`（内核故障 → `disconnected`，否则 `network`），`chats` / `history` / `links` / `send` / `download` / `play` 共用（停止线后修，未再过 codex）
- **P2｜src/atg-cli/src/tty.rs:133 退格删组合附加符号不擦屏**（第 2 轮 #4）
  - 核实：属实。零宽符号删了不输出擦除，屏幕仍显示 é，测试还认了它
  - 处理：删到零宽符号就连前面的基字一起删、按基字宽度擦；测试改为 `e` + U+0301 退格后缓冲区里没有 `e`、擦一格（停止线后修，未再过 codex）

### 引用抽查

- codex 列的「不符」：1 条（本记录第 1 轮 #5 说补了组合字符测试，没证明显示正确）——即本轮 #4，已改
- codex 列的「核实属实」：5 条（第 1 轮的秘密清零、serve 转发不随执行方停止、测试改等调用记录、等待原语、信号安装位置）
- 无法核实：codex 未编译、未跑测试

### 是否再审

已达停止线，不再审。第 2 轮 #1、#3、#4 改动小、范围清楚，在停止线后修掉并实跑测试；#2 开 issue。
