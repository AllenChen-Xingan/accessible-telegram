# 评审 064 · #169 M3.1 第一刀：`atg-gui-spec`（界面的表 + 纯函数投影）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`385c5c9..HEAD`（`feat/gui` 上 PR-1 的提交；`385c5c9` 是合进 main `27038c9` 的那次合并，之前 b19、b20 的规约提交已由 040 审过，不在范围）。第 1 轮时 HEAD = `e24e5dc`。
> - 新 crate `src/atg-gui-spec`：稳定标识 `aid`、无障碍树契约表 `a11y::TABLE`（M3.1 的 8 类元素）、文案表 `TextId`、「`Doc` + 界面状态 → 无障碍树」投影与 GPUI 层格式渲染、界面自描述 `gui-schema`（`GUI_SCHEMA_V` + 内容哈希，金样比对）
> - `scripts/schema-stamp.py` 与自测；`arch.yml` 单独一步盖章检查；`GATE_FILES` 加 `schema-stamp.py`、新自测 `scripts/tests/test_gate_list.py`
> - `specs/gui/a11y-tree.md`：第三节「机读契约」段由契约表生成（金样逐字比）；第四节 GPUI 层格式加 `roledesc`
>
> 依据文件：`docs/plan/012_gui-ssot.md`、`specs/gui/a11y-tree.md`、`specs/architecture-rules.md`，关注点文件（不入库）。effort medium。
> **最新一轮：第 3 轮（2026-11-24）· 总评「修改」（只有 P2×1，已当场修）· 第 2、3 轮连续无 P0/P1 → 满足停止线，收敛。** 留作 issue 的 P2：无（三轮的 P2 都当场修了）。
> 之前：**多视角自审 5 轮**（`atg-review`，范围同上）——第 1–3 轮各有 P1（v0 表重复列与「新消息提醒」丢失、`schema-stamp.py` 不在 `GATE_FILES`、投影不按契约表的标识种类 / 名称来源走），都修了；第 4、5 轮无 P0/P1，停。衍生 issue：#171（第二节标识表移交）· #172（`speech` 拼「文案 + 新消息提醒」的接口）· #174（`ALL` 数组的编译期保证）。

## 第 1 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-064/064-gui-spec-pr1-codex-1.md`（不入库）。`result: P0×0 P1×1 P2×3 P3×0`，耗时 54 秒。行号指评审时的版本。
本轮 P0/P1：1 · 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜`src/atg-gui-spec/src/project.rs:162` 契约表的 `operable` 一列不约束投影**（第 1 轮 #1）
  - 核实：属实。`list()` 给项加 `selected`、给容器加 `focused` / `active=` 时不看契约表；把 `chat_item` 改成 `operable: false` 照样被选中，金样、md 段跟着改了，测试却不红——表说一套、树做一套
  - 处理：`A11yNode` 加 `element` 字段，记住自己是契约表哪一行；`list()` 只给 `operable` 的容器加 `focused` / `active=`、只让 `operable` 的项被选中、被指向；`tests/project.rs` 的 `check_rules` 逐节点断言「带 focused / selected / active= 的，表里必须能操作」。改坏验证：`chat_item` 改成不可操作 → `basic_open_private_chat` 快照红；恢复即绿
- **P2｜`src/atg-gui-spec/tests/project.rs:88` 兜底逻辑没有反例**（第 1 轮 #2）
  - 核实：属实（正常夹具不走「种类不符」「数据不配」「锚点不在列表」这几条路）
  - 处理：`project.rs` 模块内加 5 条单测：种类不符不出标识；读法来源给错数据没有名称、文案来源不看数据；锚点不在列表里不悬空；项没标识不被选中；不可操作的容器不聚焦、不指向
- **P2｜`src/atg-gui-spec/tests/project.rs:227` 顺序测试只比长度和编号**（第 1 轮 #3）
  - 核实：属实
  - 处理：输入里混进不是消息的 `Doc`（连接状态、不认识的更新），逐项比对投出的消息标识序列与输入消息的身份序列
- **P2｜`src/atg-gui-spec/src/bin/gui-schema.rs:24` 写错误全被吞掉、退出 0**（第 1 轮 #4）
  - 核实：属实（重定向生成金样时磁盘满会留半截文件却报成功）
  - 处理：只忽略 `BrokenPipe`；别的写错误（含 `flush`）在标准错误报一句并以 1 退出

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：5 条
- 无法核实：`scripts/check-no-debug.sh`（不在补读清单）——本机已跑，过

### 修复后的验证

fmt 过；`clippy -p atg-gui-spec --all-targets -D warnings` 0 条；`cargo test -p atg-gui-spec` 26 过 0 败（单测 13 → 18）；金样哈希不变（`b427a473774cc7b5`）；`scripts/tests`、`check-no-debug`、`check-file-size` 过。

### 是否再审

有 P1 → 修完开第 2 轮，提示词附本记录。

## 第 2 轮（2026-11-24）· 总评「通过」

原文：`_tmp/reviews-064/064-gui-spec-pr1-codex-2.md`（不入库）。范围同上，HEAD = `2eb3bec`（第 1 轮修复）。`result: P0×0 P1×0 P2×0 P3×0`，耗时 46 秒。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

### 上一轮处理的验证结果

codex 逐条核了第 1 轮四项的修法，都已落实：容器的 `focused` / `active=` 受自身 `operable` 约束、项的 `selected` 与被指向资格受该项 `operable` 约束；5 条反例单测覆盖种类不符、数据不配、锚点缺失、项无标识、不可操作容器；顺序测试逐项比对标识序列；`gui-schema` 只忽略 `BrokenPipe`。记录、索引、交接页互不矛盾。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条（另独立复算金样哈希 `b427a473774cc7b5`，与常量和金样一致）
- 无法核实：`scripts/check-no-debug.sh`（不在补读清单）——本机每轮都跑，过
- codex 顺带指出：`test_gate_list.py` 只保证「一个调用都没解析到」时红，局部漏解析没有完整性保证；当前 `arch.yml` 的三个脚本调用都解析到、都在 `GATE_FILES` 里。它没把这条列为问题，属检查器只防无心之失的范围

### 是否再审

本轮无 P0/P1，只连续 1 轮 → 再审一轮确认。

## 第 3 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-064/064-gui-spec-pr1-codex-3.md`（不入库）。范围同上，HEAD = `2eb3bec`。`result: P0×0 P1×0 P2×1 P3×0`，耗时 36 秒。
本轮 P0/P1：0 · 停止线：本轮与上一轮都无 P0/P1 → **满足** · 留作 issue 的 P2：无（当场修了）

- **P2｜`scripts/tests/test_schema_stamp.py:52` 「缺版本」的自测被哈希不配掩盖**（第 3 轮 #1）
  - 核实：属实。测试删掉 `v` 后没重新盖章，哈希先对不上就红了；把检查器里的缺版本判断删掉，测试照样过
  - 处理：拆成两条——删 `v` 后**重新盖章**再查，断言退出 1、报「缺 v 字段」、不报哈希没更新；缺哈希单独一条。改坏验证：拿掉检查器里的缺版本判断 → 新测试红；恢复即绿。纯 Python，无须编译

### 引用抽查

- codex 列的「不符」：`state/handoff/gui.md` 的下一步还写着「codex 第 2 轮」——属实，已改
- codex 列的「核实属实」：6 条（再次独立复算金样哈希一致）
- 无法核实：`scripts/check-no-debug.sh`——本机每轮都跑，过

### 是否再审

第 2、3 轮连续无 P0/P1 → 停。三轮共提 P1×1、P2×4，全部属实、全部已修；没有留作 issue 的。
