# 评审 064 · #169 M3.1 第一刀：`atg-gui-spec`（界面的表 + 纯函数投影）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`385c5c9..HEAD`（`feat/gui` 上 PR-1 的提交；`385c5c9` 是合进 main `27038c9` 的那次合并，之前 b19、b20 的规约提交已由 040 审过，不在范围）。第 1 轮时 HEAD = `e24e5dc`。
> - 新 crate `src/atg-gui-spec`：稳定标识 `aid`、无障碍树契约表 `a11y::TABLE`（M3.1 的 8 类元素）、文案表 `TextId`、「`Doc` + 界面状态 → 无障碍树」投影与 GPUI 层格式渲染、界面自描述 `gui-schema`（`GUI_SCHEMA_V` + 内容哈希，金样比对）
> - `scripts/schema-stamp.py` 与自测；`arch.yml` 单独一步盖章检查；`GATE_FILES` 加 `schema-stamp.py`、新自测 `scripts/tests/test_gate_list.py`
> - `specs/gui/a11y-tree.md`：第三节「机读契约」段由契约表生成（金样逐字比）；第四节 GPUI 层格式加 `roledesc`
>
> 依据文件：`docs/plan/012_gui-ssot.md`、`specs/gui/a11y-tree.md`、`specs/architecture-rules.md`，关注点文件（不入库）。effort medium。
> **最新一轮：第 1 轮（2026-11-24）· 总评「修改」· 待第 2 轮。** 留作 issue 的 P2：无（本轮 P2 全部当场修了）。
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
