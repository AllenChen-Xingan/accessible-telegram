# 评审 065 · #13 13b-2 按按钮（`press`，直发；「只有闸能发」清单加 `get_callback_query_answer`）（codex · gpt-6-astra · read-only · diff 类）

> **最新一轮：第 3 轮（2026-11-24）· 总评「修改」· 无 P0/P1**。第 1 轮 P1×1（改标签绕过防重复），已改；停止线从第 2 轮起算，第 2、3 轮连续无 P0/P1，**满足，收敛**。剩余 P2×1（第 3 轮 #1）交大总管开 issue。

> 对象：`origin/main...HEAD`（三点；已合 main，含 #179 #181）。重点写在 `_tmp/reviews-065/focus.md`（不入库）。
> 之前做了多视角自审 4 轮（atg-review）：
> - 第 1 轮：无 P0/P1，P2×5，主要一条是群里按键盘按钮要回复键盘那条，先红后绿。
> - 第 2 轮：**P1×1**，「机器人没回话」原先认的是 `BOT_RESPONSE_TIMEOUT`，TDLib 会改写成 502，按 TDLib 源码核实后先红后绿改了。
> - 第 3 轮：无 P0/P1，认不出的按钮区改报 `button_not_supported`，先红后绿。
> - 第 4 轮：无 P0/P1。
> - 第 3、4 轮连续无 P0/P1，自审收敛。**第 4 轮三个视角只回了两个**：「挑刺」一路返回空结果，所以已在 focus 里请 codex 重点补这一路。
> 
> 遗留：#175、#182。
> 机器验收：`cargo test -p atg-core -p atg-cli` 731 过 0 败；两种 clippy -D warnings、rustdoc 都是 0；fmt、check-no-debug、check-file-size、check-sources 都过。
> 属一级评审，人本人看。闸的拦截逻辑不动；交出口加 `SealedKind::Callback`。
> 隐私扫描：密钥类命中 24 条，都是误报，大总管逐行核过（2026-11-24），准加 `--force-secrets`，只限这 24 条：
> - 20 处 `token: ConfirmToken,` 类型标注，分布在 send/file、forward、press、react、resending、text、vote。其中 file、forward、react、resending、text 是因为本块在它们的 `Answer` match 里各补了一个分支，扫描就把整个文件读了一遍。
> - 3 条是 press.rs 那几行在 diff 里的重复。
> - 1 条是 `tdjson/interactive.rs:166` 那行 TDLib 类型名（035、062 核过的同一行，挪了行号）。

## 第 1 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-065/press-13b-codex-1.md`（不入库）。模型 gpt-6-astra，read-only，effort medium（`result: P0×0 P1×1 P2×0 P3×0`）。行号指评审时的版本。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：0 条

- **P1｜src/atg-core/src/send/press.rs:79 按钮改了字就绕过防重复**（第 1 轮 #1）
  - 核实：属实。`intent()` 直接用了含按钮文字的请求摘要。机器人按下后把同一位置的「提交」改成「处理中」、数据照旧，再跑同一条命令就是新指纹，不加 `--again` 也能再按；结果未知时同样丢掉 24 小时的拦截。这条正好就是自审第 4 轮「挑刺」那一路本该找的东西。
  - 处理：防重复意图和凭据摘要分开。意图只装会话、消息、第几个按钮，凭据摘要照旧绑文字。先红后绿：`send::press::tests::relabelled_button_is_still_the_same_button`。契约 actions.md 第 5 小节、cli.md 同步改了。

### 引用抽查

- 核实属实：5 条
  - `verify_again` → `recheck` → `before_submit_again` 之后才造 `SealedKind::Callback`；
  - `callback_data` 与 `Keyboard::nth` 数法一致；
  - `openable` 只认 http/https、拒空白与控制字符，`system_open` 用独立参数、不经 shell；
  - 键盘文字按钮在不能回复时报 `reply_not_allowed`；
  - 重复按的测试只覆盖了文字没变的情况。
- 不符：0 条
- 无法核实：1 条。TDLib 的 31 秒编辑窗口不在 codex 可读范围内；作者在自审第 2 轮已对照本机 TDLib 源码 `CallbackQueriesManager.cpp` 的 91–97 行核实过。

### 是否再审

要再审：改完开第 2 轮。

## 第 2 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-065/press-13b-codex-2.md`（不入库）。范围含 dea88de，`--previous` 带第 1 轮原文、`--response` 带本记录。模型 gpt-6-astra，read-only，effort medium（`result: P0×0 P1×0 P2×1 P3×0`）。隐私扫描 27 条：第 1 轮那 24 条挪了行号，另 3 条是本记录第 15 行引的类型标注文字。大总管逐条核过，准 `--force-secrets`，只限这 27 条。
本轮 P0/P1：0 条 · 停止线：从第 2 轮起算，连续第 1 轮 · 留作 issue 的 P2：0 条

- **P2｜src/atg-cli/src/press.rs:298 打开网址失败仍报 `opened:true`**（第 2 轮 #1）
  - 核实：属实。`system_open` 丢掉了启动错误和退出码，`answer` 不管结果一律报「已交给系统打开」。
  - 处理：打开出口改成回 `bool`，系统 `open` 起来且退出码为 0 才算交出去。失败时 `pressed:true`、`opened:false`，文字版说「网址没能交给系统打开（按下已送到，别为这个再按）」。补测试 `failed_open_is_reported_and_press_still_counts`。原来的出口签名表达不了失败，修前写不出会红的测试，这里如实记下。cli.md 同步改了。

### 引用抽查

- 核实属实：6 条
  - 第 1 轮修法：意图与凭据摘要已分开；
  - 改标签的测试；
  - `verify_again` → `recheck` → `before_submit_again` 的顺序；
  - 编号数法一致；
  - td 层核对与 `bot_silent`；
  - 开网址的限定。
- 不符：0 条
- 无法核实：1 条，同第 1 轮（TDLib 源码不在 codex 可读范围内）

### 是否再审

要再审：改完开第 3 轮确认。

## 第 3 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-065/press-13b-codex-3.md`（不入库）。范围含 f39c438，`--previous` 带第 2 轮原文、`--response` 带本记录。模型 gpt-6-astra，read-only，effort medium（`result: P0×0 P1×0 P2×1 P3×0`）。隐私扫描同第 2 轮那 27 条，同一批文件、同一批行号，大总管准过。
本轮 P0/P1：0 条 · 停止线：**满足**（第 2、3 轮连续无 P0/P1）· 留作 issue 的 P2：1 条

- **P2｜src/atg-cli/src/press.rs:107 回调按钮结果未知时，套用了消息的「下次启动会自动补发」**（第 3 轮 #1）
  - 核实：属实。`fail_line` 直接用 `CoreError::message_zh()`，而 `send_outcome_unknown` 的通用说法是「没发出的下次启动会自动补发」。回调按钮没有待发队列，也不会补按，这句会让人干等。errors.md 那一行「是否可重试」一栏已写了 press 的下一步，但中文提示没跟上。
  - 处理：停止线已满足，这条按规矩交大总管开 issue，不在本块改：要给 press 专门写一句结果未知的说法，并限定 errors.md 里「自动补发」的适用范围。issue 草稿在 MAIN `_tmp/20261124-b21/issue-press-unknown-wording.md`。

### 引用抽查

- 核实属实：6 条
  - 第 2 轮修法：`system_open` 回成败，`opened` 据此设置；
  - 失败出口的测试；
  - 意图与凭据摘要分开；
  - `verify_again` → `recheck` → `before_submit_again`；
  - 编号数法一致；
  - 开网址的限定。
- 不符：0 条
- 无法核实：1 条，同前两轮

### 是否再审

不再审：收敛。

