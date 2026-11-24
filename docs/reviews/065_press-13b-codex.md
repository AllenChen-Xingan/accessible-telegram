# 评审 065 · #13 13b-2 按按钮（`press`，直发；「只有闸能发」清单加 `get_callback_query_answer`）（codex · read-only · diff 类）

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
