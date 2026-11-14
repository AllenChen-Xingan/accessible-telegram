# 评审 007 · #16 接 TDLib 补审（codex · gpt-6-astra · read-only）

> 对象：PR #33 合并提交 `b8c8401^1..b8c8401`（`src/atg-core` td / link / pump / config / error / status / keystore / secret / fake、`scripts/setup-dev-codesign.sh`）。#32 补审第 1 块。**最新一轮：修复后第 2 轮（2026-11-13）· 总评「通过」· 已达停止线（P1 #61 经 PR #91 修复；P2 → #62、#97）**。
> 累计处理：P1×3、P2×3 逐条回当前源码核实：属实 4（其中 1 条降为 P2）、已被后续 PR 修掉 2、驳回 0。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/tdlib-16-codex-1.md`（不入库）。diff 类型，effort high，耗时 117 秒。codex 行号指合并版，括号里是当前行号。
本轮 P0/P1：3 条（核实后有效 1 条）· 停止线：未满足 · 留作 issue 的 P2：#1（降级）、#4、#5 → #62

- **P1｜config.rs:159（当前 :173）`Profile::create` 遇已有档案缺标记时按调用参数补写标记**（第 1 轮 #2）
  - 核实：属实。`create` 对 `MarkerError::Missing` 一律 `write_private(MARKER, kind)`，不区分新建目录与已有库；`atg-cli/src/session.rs:47` 在 `--test-dc` 时走 `create(..., Test)`，于是正式档案丢了标记后带 `--test-dc` 运行，会被改标成测试档案并以测试服务器打开正式库。违反 `specs/contracts/config-privacy.md:33`「标记缺失或读不懂就拒绝启动」。现有测试只覆盖 `open`。
  - 处理：报大总管派修（本手下不改 src）。建议：`create` 仅在目录为新建（或 `db/` 不存在、为空）时写标记，已有内容而缺标记 → `profile_marker_missing`；补 create 路径测试。
- **P1→P2｜td.rs:205（当前 :271）`start` 先查 `run` 为空、到最后才写入，中间不持锁**（第 1 轮 #1）
  - 核实：属实，但当前所有调用方（session / serve / chats / history / watch）都是一个进程一个后端、只调一次 `start`，无并发路径。降为 P2：潜在竞态。
  - 处理：转 issue。建议进入时原子置「启动中」，失败回滚；真 / 假后端一起改。
- **P1｜setup-dev-codesign.sh:50 自签名 atg-dev 不能满足重编不弹框**（第 1 轮 #3）
  - 核实：属实于合并版；**已修**：#38（`df6882b`）改用 Apple Development 签名，脚本只做检查，atg-dev 已删（拍板 007 E）。
- **P2｜td.rs:329（当前 :628）重复 `shutdown` 会再次 `mark_clean`**（第 1 轮 #4）
  - 核实：属实。首次关闭后 `run` 仍为 `Some`、`lock` 已放；再调一次时 `is_closed` 为真、`wait_closed` 立即返回，照样删 `RUNNING`，可能抹掉随后接手的另一进程的标记。`session.rs:298/312` 等出错路径与正常路径都会调，但同一对象两次调用需特定顺序。
  - 处理：转 issue。建议只在仍持锁时清标记，关闭清理只做一次。
- **P2｜td.rs:230（当前 :296）`spawn_receiver` 失败时已建的 TDLib 客户端无人关闭**（第 1 轮 #5）
  - 核实：属实（`create_client` 后 `?` 直接返回，`run` 未写入，`shutdown` 视为未启动）；触发条件是线程创建失败，罕见。
  - 处理：转 issue。
- **P2｜setup-dev-codesign.sh:61 临时口令进子进程命令行**（第 1 轮 #6）
  - 核实：属实于合并版；**已修**：#38 删除了这段生成逻辑。

### 引用抽查

- codex 列的「核实属实」：4 条（quiet_tdlib_log、pump.rs:65 needs_refetch、resent_inputs、ChatDraftChanged 不带正文）；无「不符」。

### 是否再审

有有效 P1（标记补写）→ 修完后开第 2 轮，提示词附本记录。

### 修复后第 1、2 轮（2026-11-13，审 main 上 PR #91 合并提交 `517e70d^1..517e70d`，与 008 的 #61 部分合跑）· 总评「修改」→「通过」

原文：`_tmp/20261113-b18/codex-work/fix-61-codex-1.md`、`-2.md`（不入库）。名 `fix-61`，上一轮原文、本记录、008、修复自身的评审 021 作依据。effort high，耗时 48 / 39 秒。
两轮均无 P0/P1 · 停止线：**满足**（脚本 `stop-line:` 判定）· 留作 issue 的 P2：1 条 → #97

- #61（第 1 轮 P1，标记补写）：codex 两轮均判已修——`config.rs:189` 缺标记须过 `is_fresh`（`:381`，只认目录不存在或仅剩空 `db/`、`files/`），`:397` `write_marker` 用 `create_new` 只新建不覆盖，`:208` 按落盘标记再核种类；测试 `create_never_relabels_existing_profile_without_marker`、`create_writes_marker_only_for_fresh_profile`、`create_keeps_garbled_marker`、`unreadable_data_dir_is_not_fresh`。手下抽核属实。
- **P2｜config.rs:397 两个进程同时首次建同一档案，初始化中间态被误报**（修复后第 1 轮 #1）
  - 核实：属实。`Profile::create` 之前不持锁（档案锁在 TDLib 启动时才拿）：`create_new` 后、`write_all` 前另一进程读到空标记报 `profile_marker_unreadable`；或先判缺失、对方写完后 `is_fresh` 因已有标记失败报 `profile_marker_missing`。结果是拒绝启动（偏安全，不会标错种类），重跑即可。
  - 处理：转 issue。建议写临时文件后 `link` / `rename` 原子发布，竞争失败后重读标记再判；补可控并发测试（→ #97）。第 2 轮 codex 确认此条与处理记录一致、未升级。
- 无法核实 1：serve 等入口是否都走同一判据（不在对象内）。手下另核：产品代码打开档案只经 `session::open_profile` → `pick_profile`（`session.rs:47`、`:54`，serve 在 `serve.rs:68`），其余 `Profile::create` 都在测试里。

是否再审：**停**。剩余 P2 见 #62、#97。
