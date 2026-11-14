# 评审 021 · #61 档案缺标记不再按调用方补写种类（codex · gpt-6-astra · read-only）

> 对象：`git diff`（worktree `atg-wt/fix`，分支 `fix/61-profile-marker`，基于 main 68ac7a0），依据 `specs/contracts/config-privacy.md`、`specs/contracts/errors.md`。**最新一轮：第 2 轮（2026-11-13）· 总评「通过」· 已达停止线（连续两轮无 P0/P1）**。
> 累计处理：P0×0、P1×0、P2×3、P3×0 逐条回源码核实，属实 3、驳回 0、无法核实 0；3 条都已修，没有留作 issue 的。

开审前的多视角自审（atg-review）：P0/P1 无；P2×5 全部属实已修（「新档案」收紧到整个档案目录、契约登记两个 `what` 并写明新档案定义（大总管授权）、标记只新建不覆盖并按落盘标记复核种类）；P3 里「留着数据却缺标记另开一个 `what`」不采纳——两种情况都名副其实是「缺标记」，另开码要多一个契约值，大总管认可。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/reviews/profile-marker-61-codex-1.md`（不入库）。effort medium，耗时 31 秒。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜src/atg-core/src/config.rs:179 缺标记的旧档案被拒前仍改目录权限**（第 1 轮 #1）
  - 核实：属实。`create` 先对 `profiles/` 和档案目录各跑一次 `ensure_private_dir`（含 chmod 0700），再读标记；被拒的旧档案权限已被改，chmod 失败还会先报 `profile_dir_io` 盖住 `profile_marker_missing`
  - 处理：先读标记、判新旧，只有要写标记的新档案才建目录；`is_fresh` 认「目录还不存在」。session 层测试断言被拒后目录仍是 0750
- **P2｜src/atg-core/src/config.rs:198 种类不符的拒绝落在有副作用的 `open` 之后**（第 1 轮 #2）
  - 核实：属实。自审时把首次读标记的种类检查合进了末尾复核，`open` 会先建 `db/`、`files/`；`db` 是普通文件时报 `profile_dir_io`，盖住 `profile_kind_mismatch`
  - 处理：恢复读到标记时立刻核种类，末尾复核保留（只为并发新建）。补测 `test_dc_on_main_profile_is_kind_mismatch_before_touching_disk`
- **P2｜src/atg-cli/src/session.rs:55 收窄后的 CLI 分支没有测试**（第 1 轮 #3）
  - 核实：属实。新测试都直接调 `Profile`，撤销 session.rs 的改动没有测试会红
  - 处理：选档案规则拆成 `pick_profile(root, test_dc, name)`（`open_profile` 只补默认根目录），新文件 `session/tests.rs` 5 条：首次 main、别的名字须已存在、带数据缺标记两种模式都拒且不改权限、读不懂标记无副作用、`--test-dc` 撞正式档案先报种类不符
  - 变异验证：去掉 `create` 读到标记时的种类检查 → `test_dc_on_main_profile_is_kind_mismatch_before_touching_disk` 红。单独撤销 session.rs 的收窄（改回 `Err(_) if main`）→ 不红：#1 修后 `create` 先读后动盘，读不懂的标记进 `create` 也原样报 `profile_marker_unreadable`、无副作用，收窄后行为与不收窄一致，只是多一层保险（别的错误不再经过 `create`）。不为它造一条只测实现细节的测试

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：5 条
- 无法核实：codex 未编译、未跑测试（作者修后实跑：config 15 过、session 5 过，clippy、fmt、check-no-debug 过）

### 是否再审

本轮无 P0/P1，只连续 1 轮 → 修后再审一轮确认。

## 第 2 轮（2026-11-13）· 总评「通过」

原文：`_tmp/reviews/profile-marker-61-codex-2.md`（不入库）。effort medium，耗时 25 秒。新测试文件 `session/tests.rs` 未跟踪（手下不做 git 写操作），作为依据文件随附、`--allow-untracked`。
本轮 P0/P1：0 条 · 停止线：本轮与上一轮都无 P0/P1 → **满足** · 留作 issue 的 P2：无

上一轮处理的验证：第 1 轮 #1、#2、#3 codex 核实已修；session.rs 收窄撤销不变红的解释，codex 静态推演认可，不再报。

本轮无发现。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条
- 无法核实：TDLib 实际传参是否就是 `db/`、`files/`（`td.rs` 不在 codex 获准阅读范围）——作者核过：`src/atg-core/src/td.rs:317-318` 取 `profile.db_dir()`、`profile.files_dir()`

### 是否再审

连续两轮无 P0/P1 → 停。
