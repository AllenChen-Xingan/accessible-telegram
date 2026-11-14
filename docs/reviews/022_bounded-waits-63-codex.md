# 评审 022 · #63 #73 #78 #79 CLI 等待都有期限、Ctrl+C 能收尾（codex · gpt-6-astra · read-only）

> 对象：`git diff main...HEAD`（worktree `atg-wt/fix`，分支 `fix/63-bounded-waits`，已合进含 #61 的 main），依据 `specs/contracts/cli.md`、`specs/contracts/errors.md`。**最新一轮：第 1 轮（2026-11-13）· 总评「修改」· 停止线：连续第 1 轮无 P0/P1**。
> 累计处理：P0×0、P1×0、P2×5、P3×0 逐条回源码核实，属实 5、驳回 0、无法核实 0；5 条都已修（#2 起后端那一半按理由保留现状），没有留作 issue 的。

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
