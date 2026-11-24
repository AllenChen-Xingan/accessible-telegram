# 评审 062 · #13 13b-1 投票（`vote`，直发；「只有闸能发」清单加 `set_poll_answer`）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（三点；已合 main dda9d52，含 #173 #176）。重点写在 `_tmp/reviews-062/focus.md`（不入库）。
> **最新一轮：第 1 轮（2026-11-24）· 总评「修改」· P1×1**。
> 之前：多视角自审 3 轮（atg-review），第 2、3 轮连续无 P0/P1：
> - 第 1 轮 P1×1：许改票、已投过时，整组替换没拦也没说
> - 第 2 轮：`replaces` 只列真会被撤掉的
> - 第 3 轮：上一次投的还在提交时拒投
> 
> 都先红后绿已改；交出前、交出后到点两条路径没测 → #175。机器验收（dda9d52）：`cargo test -p atg-core -p atg-cli` 709 过 0 败，两种 clippy -D warnings 都是 0，fmt、check-no-debug、check-file-size、check-sources 过。
> 隐私扫描：密钥类命中 7 条，都是误报，大总管逐行核过（2026-11-24），准加 `--force-secrets`，只限这 7 条：
> - 6 条是 `send/vote.rs` 292、301、332 行的 `token: ConfirmToken,` 类型标注，3 条源码加 diff 里的 3 条重复；
> - 1 条是 `tdjson/interactive.rs:149` 那行 TDLib 类型名 `inlineKeyboardButtonTypeCallbackWithPassword`（035 核过的同一行，因加 `pending` 一行挪到 149）。
> 
> 属一级评审，人本人看。闸的拦截逻辑不动，交出口加 `SealedKind::Vote`。

## 第 1 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-062/vote-13b-codex-1.md`（不入库）。effort medium，41 秒（`result: P0×0 P1×1 P2×1 P3×0`）。行号指评审时的版本。
本轮 P0/P1：1 条（codex 标 P1；按威胁模型最高 P2，作者选照 P1 处理）· 停止线：未满足，从第 2 轮起重新数 · 留作 issue 的 P2：0 条

- **P1｜src/atg-core/src/send/vote.rs:305 同一投票的并发提交没有互斥**（第 1 轮 #1）
  - 核实：属实。两个调用各拿一张凭据，都在交出前复核时看到「没投」，都过；之后各自交出，后一个整组覆盖前一个，`vote_would_replace` 与 `pending` 都挡不住（交出前 TDLib 还没标「在提交」）。**定级**：codex 标的是 P1；按威胁模型（构造时序竞态最高 P2），它最高只算 P2。照 P1 当场改，是作者自己的选择：同类的 `react` 已有同样的在途保护（`InFlight`），改法小。大总管认可（2026-11-24）。**停止线照 codex 的标级数**：本轮算有 P1，从第 2 轮重新数连续两轮。
  - 处理：加同一条投票的在途槽位（键是闸的地址加消息），从复核起一直占到交出口回话；第二个报 `vote_blocked`、不交出。放开后由 TDLib 的 `is_being_chosen`（`pending`）接着挡。测试 `concurrent_vote_on_same_poll_is_refused`：先占住槽位再投 → 拒、不交出、不留痕；放开后能投；别的后端不受影响。机制是新加的，修前写不出会红的交错测试（同 #032 的 `InFlight`），如实记下。契约 actions.md 的 `vote` 行写明。
- **P2｜specs/content-tree.md:123「不进树」清单仍含 `is_being_chosen`**（第 1 轮 #2）
  - 核实：属实（codex 列在抽查「不符」里的也是这一条）。
  - 处理：清单里改成「`is_being_chosen` 进树，改名 `pending`」。

### 引用抽查

- 核实属实：4 条
  - 先 verify，再 recheck、before_submit，然后才造 `SealedKind::Vote`
  - 显示顺序映射到 `option_ids`
  - `requests::vote` 的形状
  - 真交出口调 `set_poll_answer`
- 不符：1 条，即 #2，已改
- 无法核实：1 条。CI 扫描脚本（`actions.md:190`）不在 codex 可读范围内；`specs/contracts/testing.md:72` 记它为「待写」，本块不涉及

#### 是否再审

要再审：两条改完开第 2 轮（`--previous` 带本轮原文、`--response` 带本记录）。修后验证：`cargo test -p atg-core -p atg-cli` 710 过 0 败，两种 clippy 0，fmt 与三个 check 脚本过。
