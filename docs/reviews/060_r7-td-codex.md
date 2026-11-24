# 评审 060 · #60 R7 第九刀：拆 core `td.rs`（只挪不改）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`。分叉点是 36d2cea；本分支从 f5ca5b2 起，中途合过一次 main（c41e790），#162 带进来的 fake 拆分不在范围内。包括检查点 c89a35a、7bf60cb，合并时解的冲突在基线（解成空）、010、交接页。
> - `td.rs` 802 → 36：内联的 `mod real { … }` 原样挪成 `td/real.rs`，缩进退 4 格
> - 再拆 `td/real/start.rs`、`login.rs`、`read.rs`：根上 16 个 `Backend` 方法一行转给 `do_*`，签名不变、函数体照原样；`td/real.rs` 539 行
> - 内部助手留在 `td/real.rs`，可见性不变；`td/real/submit.rs` 没动
> - rustfmt 合回一行的有 7 处，属于格式变、内容不变；`td/real.rs` 删了 4 处不再用的导入（`config` 的两条文档链接随之改写成 `crate::config::…`）
>
> 依据文件：拆前原文件、`--color-moved` 中没被识别为挪动的行（忽略缩进变化）、合行清单、新旧几个 td 文件、010。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：
> - **多视角自审 1 轮**（c89a35a）：P0/P1 零；P2×1，P3×11 改了 8 条（7bf60cb），另 3 条不改，理由见交接页
> - **全套**（c41e790，合 main 后）：fmt、两种 clippy（`-D warnings`，tdlib-api 那种才编到 `real`）、`cargo test --workspace`（713 过、0 败、5 忽略）、check-no-debug、check-file-size（超线 0，基线已空）、check-sources 全过
> - **只读真机冒烟**（正式档案，拆前 f5ca5b2 对本分支）：
>   - `history saved`、`links saved <id>`（走 `message_get`）：一致，只有时间戳不同
>   - `chats`：按「拆后 → 拆前 → 拆后」跑三次，按 `chat_id` 配对比较，完全一致
>   - 只比键名，输出比完即删

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7td-codex/r7td-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现。codex 核实了这几点：
  - 16 个转发都调到对应的 `do_*`，参数原样、顺序一致
  - 对照原文件，函数体只有清单里列的格式差异
  - `do_start` 启动顺序、失败处理、资源交接不变
  - `TdBackend`、`TdConfig`、`tdlib_version` 仍从原路径导出；特性条件仍套住整个 `real`
  - 三个已有子模块仍解析到 `td/real/` 下；`submit.rs` 的 `super::{TdBackend, requests}` 仍能取到，没被修改
  - 文档链接已改写；可见性没变；010 的行数和「基线已清空」与实际一致
- 无法核实 1 条：测试和冒烟结果只有文字记录，codex 只读、不跑，以 CI 和记录为准
- 另记：第一次起本轮时脚本报 `--name 'r7td' already belongs to another object`。原因是之前 dry-run 用旧范围 `f5ca5b2...HEAD` 建过身份文件，当时没有真跑过任何一轮，所以删掉身份文件后按新范围重跑

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7td-codex/r7td-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮，脚本的 stop-line 行同此） · 留作 issue 的 P2：无

- 无发现。codex 重新逐个比对了函数体：只有允许的空白调整，以及合行时删掉的尾逗号，没有漏挪、多挪或逻辑改动。上一轮的其余结论也复核过，并核实了三个新子模块的行数（108、80、139）
- 无法核实 1 条：同第 1 轮

### 是否再审

不再审，按停止线收敛。
