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
