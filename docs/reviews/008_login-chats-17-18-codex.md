# 评审 008 · #17 login / #18 chats 补审（codex · gpt-6-astra · read-only）

> 对象：PR #37、#39 合并范围 `b8c8401..9087ac4`（`atg-cli` login / session / tty / out / chats，`atg-core` auth / link / fake 相关改动）。#32 补审第 2 块。**最新一轮：修复后（2026-11-13）· #61 部分经 PR #91、#63 部分经分支 fix/63-bounded-waits 各两轮无 P0/P1 · 已达停止线（P2 → #64、#97–#100）**。
> 累计处理：P1×6、P2×1 逐条回当前源码核实：属实有效 P1×2、属实降 P2×2、与 007 重复 1、已被后续 PR 修掉 1、P2 属实 1，驳回 0。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/login-chats-17-18-codex-1.md`（不入库）。diff 类型，effort high。行号指当前文件（codex 声明）。
本轮 P0/P1：6 条（核实后有效 2 条）· 停止线：未满足 · 留作 issue 的 P2：#1、#5、#7 → #64

- **P1｜login.rs:196 提交请求本身无超时**（第 1 轮 #4）
  - 核实：属实。`submit` 用 `block_on(b.auth_submit_*)`，`logout`（:385）、`resend`（:244、:336）同样；`STEP_TIMEOUT` 只管提交之后等状态。断网时 TDLib 把请求挂着等网络，`login` / 分步 `auth *` 会无限期卡住并占着档案锁；分步命令是给 AI 和脚本用的，调用方会被一直挂住。违反 `errors.md:28`「请求超时、网络不可用时请求失败 → `network`」。
  - 处理：报大总管派修。建议：提交与等状态共用一个截止时间（`link::block_on_timeout` 已有），超时报 `network` 并照常收尾关闭。
- **P1｜tty.rs:83 只有读秘密那一刻屏蔽信号，其余时间 Ctrl-C 直接杀进程**（第 1 轮 #6）
  - 核实：属实。`read_secret` 结束即恢复 `ISIG`；`login` / `auth *` 不调 `signals::install()`（只有 watch / serve 调），等网络、登出确认时按 Ctrl-C 走默认处理直接终止：不跑 `shutdown`、不出结尾行、运行标记残留。违反 `cli.md:128`「Ctrl+C 中断 → 退出码 130、`interrupted`」。
  - 处理：报大总管派修。建议登录全程接管 SIGINT（复用 `signals.rs`），统一走收尾；补伪终端测试。
- **P1→P2｜tests/cli.rs:173 秘密参数测试只查 stdout，clap 原文在 stderr 回显秘密**（第 1 轮 #1）
  - 核实：属实。`main.rs:442` `eprint!("{}", e.render())`，`auth password hunter2` 的 clap 报错含 `hunter2`；测试注释写「不回显」却只查 stdout。降为 P2：秘密是用户自己放进命令行参数的（已进 shell 历史与进程列表），回显到 stderr 不扩大太多暴露面，但在 tmux 里会被 capture-pane 读到。
  - 处理：转 issue。建议用法错误统一给固定提示，测试断言两路输出都不含秘密。
- **P1｜main.rs:167 原版 `chats` 忽略 `--redact`**（第 1 轮 #2）
  - 核实：属实于合并版；**已修**：当前 `main.rs:211` 未实现时拒绝。
- **P1｜session.rs:51 已有档案缺标记被重新初始化**（第 1 轮 #3）
  - 核实：属实，与 007 第 1 轮 #2 同根（`Profile::create` 补写标记），已开 #61，修时一并覆盖 `open` 失败转 `create` 这条路径。
- **P1→P2｜link.rs:173 `set_auth` 分两次加锁更新代次与步骤**（第 1 轮 #5）
  - 核实：属实。`auth_gen += 1` 与 `lines.auth = step` 在两次加锁里；等待方若在两次之间被唤醒（虚假唤醒或别的线改动触发的 `notify_all`），会拿新代次配旧步骤，登录循环再要一次上一步的输入。窗口极窄、后果可恢复（报 `auth_wrong_state`，重跑即可），降为 P2。
  - 处理：转 issue。建议同一把锁里改提示、代次、步骤，再唤醒。
- **P2｜login.rs:358 分步登录落到正式档案的 `wait_registration` 仍报成功**（第 1 轮 #7）
  - 核实：属实。`auth_step` 只对测试档案处理注册，正式档案输出 `step:"wait_registration"` 并以成功结尾；`errors.md:82` 要求以 `auth_wrong_state`、退出码 3 结尾。交互 `login` 由 `advance`（:99）处理，没问题。修时顺带核分步路径对 `unsupported`、`wait_other_device` 是否同样漏判。
  - 处理：转 issue。

### 引用抽查

- codex 列的「核实属实」：4 条（STEP_TIMEOUT 30 秒、main.rs:211 no_redact、config.rs:188 补写标记、cli.rs:173 只读 stdout）；无「不符」。

### 是否再审

有有效 P1×2（请求无超时、Ctrl-C 不收尾）→ 修完后开第 2 轮，提示词附本记录。

### 修复后：#61 部分（2026-11-13，与 007 合跑，审 main 上 PR #91 `517e70d^1..517e70d`）· 「修改」→「通过」

原文：`_tmp/20261113-b18/codex-work/fix-61-codex-1.md`、`-2.md`（不入库）。名 `fix-61`。两轮无 P0/P1，停止线**满足**。
- 与 #61 同根的 `session.rs` 打开档案路径：login、chats 等都经 `open_profile` → `pick_profile`（`session.rs:47`、`:54`），缺标记拒绝且不补写；测试 `main_with_data_but_no_marker_is_refused_either_way`。手下抽核属实。剩余 P2（并发首次建档误报）记在 007。

### 修复后：#63 部分（2026-11-13，与 012 合跑，审分支 `fix/63-bounded-waits` 的 `main...HEAD`，worktree `atg-wt/fix` 提交 `d1df09d`）· 「修改」→「通过」×2

原文：`_tmp/20261113-b18/codex-work/fix-63-codex-1.md`、`-2.md`、`-3.md`（不入库）。名 `fix-63`，上一轮原文、本记录、012、修复分支自身的评审 022 作依据。effort high。
第 1 轮 codex 报 P1×1、P2×2（手下降 P1 为 P2）；第 2、3 轮「未发现 P0–P2 问题」，均确认降级无推翻证据 · 停止线：**满足**（脚本 `stop-line:` 以第 2、3 轮判定）· 留作 issue 的 P2：3 条 → #98、#99、#100

- #63（第 1 轮 P1×2）：已修。`login.rs:281` 提交与等授权线共用 30 秒期限（`deadlines.rs` 定数，与 `cli.md`「最多等多久」一致），到点报 `network`、内核故障报 `disconnected`；Ctrl+C 停止等待并照常关后端。测试 `hung_request_past_deadline_is_network`、`ctrl_c_while_submit_hangs_is_interrupted_and_shut_down`。手下抽核属实。
- **P1→P2｜tty.rs:88 读秘密 / 登出确认期间不看停止标志**（修复后第 1 轮 #1）
  - 核实：现象属实，降为 P2。人按 Ctrl+C 时终端已切 `~ISIG`，作为 0x03 被认成 `Interrupted`（`tty.rs:133`），照常收尾——#63 原题已修。剩下的：提示前微秒级窗口里按下（再按一次即收尾）；外部 SIGTERM 不打断读（`libc::signal` 在 macOS 自动重启系统调用），要等人敲键；第二次 SIGTERM 走 `_exit(130)` 跳过终端恢复，终端停在不回显。login 只在有人在场的控制终端上进行。
  - 处理：转 issue。建议读前复查 STOP、读用 `poll` 切片看 STOP、raw 模式下第二次信号先恢复终端（→ #98，降级经大总管接受）。
- **P2｜serve.rs:375 `chat_open` 到点后迟到成功，漏配 `chat_close`**（修复后第 1 轮 #2）
  - 核实：属实。会话在 TDLib 里停在「打开」直到 serve 退出（多收更新；不标已读）。需 10 秒到点且随后成功，罕见。处理：转 issue（保留已交出请求的清理责任）→ #99。
- **P2｜login.rs:87 授权线故障后空转到 30 秒期限**（修复后第 1 轮 #3）
  - 核实：属实。`wait_auth` / `wait_auth_after` 遇 `Faulted` 立即回 `None`（`link.rs:207`、`:328`），`wait_line` 不休眠地重试到期限再经 `stalled` 报 `disconnected`：码对，但晚 30 秒并占满 CPU，比原版立即报退步。处理：转 issue（片后遇故障即返回，补测试）。 → #100
- 无法核实：`Secret` 实际擦除范围、tdlib-rs 丢弃接收方后迟到应答的回收（超出对象范围）。

是否再审：**停**。剩余 P2 见 #64、#97（并发建档，记在 007）、#98、#99、#100。
