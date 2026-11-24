# 评审 040 · GUI 唯一真相源与规约（012、specs/gui、decision.md、013；b20 随 Touch ID 删除改为直接发送）（codex · gpt-6-astra · read-only）

> 对象：`specs/gui/`（全部）、`docs/plan/012_gui-ssot.md`、`docs/research/004_gui/decision.md`、`docs/plan/013_decisions-b19-gui.md`。依据 `specs/contracts/actions.md`、`errors.md`、`cli.md`、`docs/plan/002_roadmap.md`，以及对象里引到的 core 源码（`src/atg-core/src/send.rs`、`send/outcome.rs`、`send/resending.rs` 等）。**最新一轮：第 5 轮（2026-11-17）· 总评「修改」（仅 P2）· 满足停止线（第 4、5 轮连续无 P0/P1），不再审；剩余 P2 已直接改掉，无 issue**。
> 之前：atg-review 自审 11 轮（b20），第 10、11 轮连续无 P0/P1；自审 P1 全在 L2 发送流程，已收成 `specs/gui/focus.md` 第六节。
> 已知不在本线修：#115、#77、#118、#119、#120（`docs/plan/012_gui-ssot.md` 第八节第 8–12 条）。
> 累计处理：P1×6、P2×8 逐条回源码核实，属实 14、驳回 0。

## 第 5 轮（2026-11-17）· 总评「修改」（仅 P2）· 停

原文：`_tmp/reviews-040/040-gui-ssot-codex-5.md`（不入库）。effort medium，91 秒。`--force-secrets` 同前授权。
上一轮验证：#1、#2 已落实。
本轮 P0/P1：0 条 · 停止线：**满足**（第 4、5 轮连续无 P0/P1）· 留作 issue 的 P2：无（两条已直接改）

- **P2｜调试日志落点与契约冲突、引用不存在的 debug 级**（第 5 轮 #1；抽查「不符」同一件）
  - 核实：属实（`specs/contracts/config-privacy.md` 日志在 `~/Library/Logs/atg/`；`src/atg-core/src/logline.rs` 只有 `info` / `warn`）
  - 处理：focus 第三节改为写 `~/Library/Logs/atg/atg-gui.log`、引隐私契约；debug 级入口登记 012 第八节第 13 条，定之前用 `info`
- **P2｜卡 G2 没说清普通 CI 怎么排除 GUI**（第 5 轮 #2）
  - 核实：属实（只改成员清单不改变 `--workspace` 的选包）
  - 处理：G2 写定「留在工作区；普通任务 `--workspace --exclude atg-gui`，GUI 任务 `-p atg-gui`；用 `cargo metadata` 核选包」；012 第八节第 4 条同步

### 引用抽查（第 5 轮）

- 不符：1 条（即 #1，已改）；核实属实：9 条；无法核实：`keymap.md` 引 GPUI 上游源码（仓外，同第 3 轮说明）

### 是否再审

连续两轮无 P0/P1 → 停。

## 第 4 轮（2026-11-17）· 总评「修改」（仅 P2）

原文：`_tmp/reviews-040/040-gui-ssot-codex-4.md`（不入库）。effort medium，96 秒。`--force-secrets` 同前授权。
上一轮验证：#1–#3 全部落实。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无（两条已改）

- **P2｜换 id 缺「结果先于交出应答」的顺序**（第 4 轮 #1）
  - 核实：属实（`src/atg-core/src/send.rs` 文件头「谁先到没有保证」；`send/resending.rs` 先喂旧 id → 临时 id 再喂成功）
  - 处理：focus 第六节第 5 条加顺序约束（界面按「先交出、再成败」收，早到的由 core 暂存）；012 第八节第 11 条（#119）加同一要求；A28 加早到夹具
- **P2｜A30「重发整次未知才说结果未知」与收尾段逐条判冲突**（第 4 轮 #2；抽查「不符」同一件）
  - 核实：属实（`send/resending.rs` 收尾逐条 `Waited`）
  - 处理：A30 改为「交出段整次未知；收尾段逐条判」，加一成一未知夹具

### 是否再审

本轮无 P0/P1，只连续 1 轮 → 再审一轮确认。

## 第 3 轮（2026-11-16）· 总评「修改」

原文：`_tmp/reviews-040/040-gui-ssot-codex-3.md`（不入库）。effort medium，108 秒。`--force-secrets` 同第 2 轮授权。
上一轮验证：#2、#3 已落实；#1 漏 `last_focus`，本轮 #1 接着报。
本轮 P0/P1：1 条 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜换 id 迁移漏 `last_focus`**（第 3 轮 #1）
  - 核实：属实（012 UiState `last_focus` 存元素标识；`focus.md` `window_reactivated` 用它）
  - 处理：迁移正本改为「所有持有该消息 `MsgId` 的字段，含写在元素标识里的，实现上统一遍历」，清单加 `last_focus`；A28 加「切走 → 换 id → 切回」
- **P2｜成功复核没区分文字发送与重发**（第 3 轮 #2；抽查「不符」同一件）
  - 核实：属实（`send/text.rs` `check_kept(plan: &TextPlan, …)`；重发计划可含「会去掉回复关系」）
  - 处理：只文字发送复核；重发不复核，012 第八节第 11 条登记「重发复核待定」
- **P2｜删除、重发的计划读法漏目标会话**（第 3 轮 #3）
  - 核实：属实（`actions.md` 第一节第 4 小节要给人看目标；在途期间可切会话）
  - 处理：`announce.md` `send_plan` 删除、重发句带「〈种类 · 名字〉」（从提交计划带出，不念编号）；A29 同步

### 引用抽查（第 3 轮）

- 不符：1 条（即 #2，已改）；核实属实：6 条；无法核实：`keymap.md` 引 GPUI `App::build_action` / `KeyBinding::load`（上游源码在仓外 MAIN `resources/`，b19 ggpui 已核，见 `docs/research/004_gui/findings-components.md` 第四节）

### 是否再审

有 P1 → 改完开第 4 轮。

## 第 2 轮（2026-11-16）· 总评「修改」

原文：`_tmp/reviews-040/040-gui-ssot-codex-2.md`（不入库）。effort medium。带第 1 轮原文与本记录；`--force-secrets`（命中 3 处均为参数声明 `ConfirmToken` 类型标注及本记录提到它的句子，大总管按 b20 共同规矩第 15 条准）。
上一轮验证：#2、#4 已落实；#1、#3 部分落实，本轮 #1、#3 接着报。
本轮 P0/P1：2 条 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜`specs/gui/focus.md` 第六节第 5 条：换 id 迁移只覆盖选中锚点**（第 2 轮 #1）
  - 核实：属实（`docs/plan/012_gui-ssot.md` UiState 的 `return_to`、`links.msg`、`reading`、`search.anchor`、`revealed`、回复目标都持有 `MsgId`）
  - 处理：012 第三节第 2 小节加「换 id 迁移」正本——列全持有 `MsgId` 的字段，新增字段必须加进来，单测逐字段造一次换 id；focus 第六节第 5 条改为引它；A28 加阅读视图开着时换 id
- **P1｜成功分支漏了发送后回复 / 话题复核**（第 2 轮 #2）
  - 核实：属实（`actions.md` `send_text` 行「`send_succeeded` 到了再核一遍，不一致在结尾行标明（`reply_kept:false`）」；core `check_kept`）
  - 处理：收尾段「成功」行加复核（普通与晚到都核），不一致说「已发送，但回复关系 / 话题被改了」（`announce.md` `send_ok` 改写态）；A28 加判据
- **P2｜删除、重发返回时「仍选中原来那条」会把选中拉回**（第 2 轮 #3）
  - 核实：属实（第 3 条允许在途时浏览）
  - 处理：第 3 条加「一切异步结局都不移动焦点与选中，只在当前引用正指着那条时随它换 id 或变占位」；交出段表三格改「选中在哪就在哪」；A28 加删除 / 重发在途时移动选中

### 引用抽查（第 2 轮）

- 不符：无；核实属实：7 条（含第 1 轮无法核实的 `012:185`，本轮读到 `send/text.rs` 已核实）

### 是否再审

有 P1 → 改完开第 3 轮。

## 第 1 轮（2026-11-16）· 总评「修改」

原文：`_tmp/reviews-040/040-gui-ssot-codex-1.md`（不入库）。effort medium，耗时 113 秒。行号指评审时的版本。
本轮 P0/P1：3 条 · 停止线：未满足 · 留作 issue 的 P2：无（#4 已改）
隐私扫描：`src/atg-core/src/send/text.rs:273,298` 报「密钥赋值」，实为参数声明 `token: ConfirmToken,`。本轮未用 `--force-secrets`，以 `--exclude` 排除该文件（codex 抽查里「无法核实 012:185」即因此）；大总管逐条核实后按 b20 共同规矩第 15 条回「准」，第 2 轮起加 `--force-secrets`、放回该文件。

- **P1｜`specs/gui/focus.md` 第六节第 5 条：失败换 id 没有锚点迁移规则**（第 1 轮 #1）
  - 核实：属实（`actions.md` 事件 `send_failed`「TDLib 也给它新 id」；`src/atg-core/src/send/outcome.rs` `Waited::Failed` 注释同；规约只写了成功换 id）
  - 处理：第 5 条加「换 id 一律原地改」——成功、失败、重发交出都让选中锚点、`unknown_pending`、`send_again` 记的那条跟着换，稳定键不变，重发键作用在换后的那条；规则表 `send_failed` 行同步；清单 A28 加失败 / 晚到失败后选中与重发的判据
- **P1｜`specs/gui/announce.md` 交出段几个来源只在前台出声**（第 1 轮 #2）
  - 核实：属实（`send_plan`、`send_refused`、`delete_pending`、`resend_skipped` 被挡 / 后台格都是「—」，而交出段在后台跑，人按下就能切走）
  - 处理：被挡一律直发；后台——`send_refused`、`delete_pending`、`resend_skipped` 走系统通知，`send_plan` 不说（结果另由 `send_ok` / `send_failed` 报）；`send_ok` 被挡直发、后台不说（明写）；A28 加「回车后马上切走，拒发原因走系统通知」
- **P1｜`specs/gui/focus.md` 第六节第 4 条：返回时无条件清回复目标、留在输入框，与在途期间可离开输入框冲突**（第 1 轮 #3）
  - 核实：属实（第 3 条只锁文字与发送键；人可 Esc 回列表按 `r` 设新回复目标；「焦点留在输入框」若人已离开即违反规矩 1）
  - 处理：第 3 条加**提交快照**（会话、草稿、回复目标），结局只按快照处理；发送在途时该会话回复目标锁住（`reply` 被忽略）；「交出了」改为清快照里那个会话的输入框、**焦点不动**（已离开不拉回）；A28 加交错判据
- **P2｜`specs/gui/focus.md` 第六节第 3 条：在途防重复只吃键，按钮 / 菜单 / AX Press 绕过**（第 1 轮 #4）
  - 核实：属实（`a11y-tree.md` 发送按钮只在输入为空时禁用；`keymap.md` 第六节菜单栏每个能力动作一项）
  - 处理：第 3 条改为「挡在动作入口」——按键、按钮、菜单走同一动作，入口先查 `in_flight`；发送按钮与菜单项在途时禁用（`a11y-tree.md` 发送按钮行同步）；A28 加按键与按钮交错

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条
- 无法核实：`012:185` 对 `send/text.rs` 的断言（该文件本轮被排除；第 2 轮放回）

### 是否再审

有 P1 → 改完开第 2 轮，带本记录与第 1 轮原文。
