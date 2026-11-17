# 评审 046 · #108 testkit 特性只许在 dev 依赖里开的机器检查（codex · gpt-6-astra · read-only · 门槛类）

> 对象：`scripts/check-sources.py` 的 `testkit` 一条 · `scripts/tests/test_sources.py` · `specs/architecture-rules.md` 第 10 条（contract 模板，effort high）。重点写在 `_tmp/20261114-108-codex/focus.md`（不入库）。
> **状态：第 3 轮（2026-11-16）· 总评「通过」· 连续第 2 轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> **合入顺序**：本 PR 必须在 R3a（#110，加 `testkit` 特性）之后合。本分支开出时 main 上还没有这个特性（2026-11-16 已核：拿本检查对合了新 main 的 #110 分支跑，testkit 这条 ✓），所以在本分支上对真实仓库跑，这条现在报「解析不出」：这是故意的锚定，不让正本缺失时空转变绿。#110 合并、把新 main 合进来以后就绿。
> 之前：多视角自审 1 轮（atg-review），P0/P1 零，P2×6 已改：
> - 根 `Cargo.toml` 的 `resolver` 不是 "2" / "3" 就报红。resolver 1 会把 dev 依赖开的特性合并进正式构建，整条检查的前提就不成立了。
> - 正本锚定：atg-core 的 `[features]` 里没有 `testkit` 就报「解析不出」。
> - 特性串里改名依赖的本地名（`core/testkit`）也认。
> - 守则那句写准范围。
>
> P3 已改：写明 tomllib 要 Python ≥ 3.11；补 target 下 dev 依赖的绿例、无关依赖同名特性的绿例。变异检验：去掉 resolver 判断、去掉本地名收集、去掉正本锚定、去掉 build-dependencies 与 target 表、去掉 workspace.dependencies，每一种都会让对应的测试变红。

## 第 1 轮（2026-11-14）· 总评「修改」

原文：`_tmp/20261114-108-codex/g108-codex-1.md`（不入库）。effort high（`result: P0×0 P1×1 P2×0 P3×0`）。
本轮 P0/P1：1 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜scripts/check-sources.py:362：`workspace = true` 继承时没还原包的身份，改了名的依赖能漏检**（第 1 轮 #1）
  - 核实：属实。根上集中声明 `core = { package = "atg-core", path = "src/atg-core" }`，成员写 `core = { workspace = true, features = ["testkit"] }`，这是集中管理依赖的正常写法，cargo 会打开 atg-core/testkit。检查器只看成员这张表里的 `package` 或键名，认成了 `core`，结果放行。同一个缺口还漏掉成员特性里的 `core/testkit`、`default = ["dep:core", "core?/testkit"]`。另外 resolver 2 也挡不住：这是正式依赖自己主动打开的特性。
  - 处理：**改结构**。新增 `_pkg(name, spec, ws_deps)`，作为认「这条依赖到底指向哪个包」的唯一入口：`package = …` 取它；`workspace = true` 的先查根 `workspace.dependencies` 里同名那条（根上也可能改名）；都没有才用键名。依赖表检查和本地名收集都改用它；根清单排第一个读，先拿到 `workspace.dependencies`。补 3 条自测：继承改名后直接开 testkit → 红；继承改名后经特性转发（强的与 `?/` 弱的） → 红；继承无关的包、特性恰好叫 testkit → 不误报。变异检验：去掉继承那一支，前两条变红。

### 引用抽查

- 「不符」0 条。「核实属实」10 条：resolver 与成员规则；010 里对 testkit 的约定；正式表与 dev 表分开收集；特性串正则依赖本地名（即本轮那条）；正本锚定与 resolver 判断；失败转退出码 1；自测数量；无关依赖绿例；CI 跑完整检查；门槛文件清单。
- 无法核实 3 条：atg-core 的真清单与 `testkit.rs`（不在本轮可读范围；它们在 #110 里，合并顺序见头部）；发送闸与档案隔离的运行时（与本 PR 无关）；`scripts/tests/_common.py`（本 PR 没改）。

### 是否再审

再审一轮：本轮有 P1，已改。

## 第 2 轮（2026-11-14）· 总评「通过」

原文：`_tmp/20261114-108-codex/g108-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 确认上一轮那条已改到位（`_pkg` 统一还原继承来的包身份；3 条新自测）。未发现新问题。

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 3 轮（2026-11-16）· 总评「通过」

原文：`_tmp/20261114-108-codex/g108-codex-3.md`（不入库）。模型 gpt-6-astra，sandbox read-only，effort high。`result: P0×0 P1×0 P2×0 P3×0`。
本轮之前：本分支已提交（b363b30），并合进了 Touch ID 删除之后的 main（1364105）。重点说明照此更新。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无

- 没有发现新问题。codex 用 21 个纯内存场景跑了检查函数，结果都符合预期（继承改名、强弱转发、`dep:`、平台构建依赖、合法的 dev 用法、无关包同名特性、缺正本、非法 TOML）
- **抽查「不符」1 条**：第 1 轮的处理记录说继承转发的自测覆盖强、弱两种，实际只测了弱的 `core?/testkit`（codex 用内存补查，确认检查器能拦住强转发）
  - 核实：属实，是记录写过了头，检查器本身没问题
  - 处理：补自测 `test_strong_feature_via_inherited_renamed_dependency_is_red`（继承改名后写 `tdlib = ["core/testkit"]` → 红），记录与测试这才对上。变异检验：去掉 `_pkg` 的继承分支，这条变红。自测 84 条全过
- 无法核实 3 条：atg-core 真清单与 `testkit.rs`（#110 里，合并后已用本检查跑过，✓）；发送闸运行时（与本 PR 无关）；`.42cog/intent.md`、`scripts/tests/_common.py`（本 PR 没改）

### 是否再审

不再审：连续两轮无 P0/P1，按停止线收敛。本轮只补了一条测试，没改检查器。
