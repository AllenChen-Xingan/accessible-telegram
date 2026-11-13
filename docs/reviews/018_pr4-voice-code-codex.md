# 评审 018 · #4 下载与播放的代码（atg-audio、download / play）（codex · gpt-6-astra · read-only）

> 对象：本 PR 新加或主要改动的源文件——`src/atg-audio/`（除二进制夹具）、`src/atg-cli/src/{play,download,args,session}.rs`、`src/atg-cli/src/voice/mod.rs`、`src/atg-core/src/{fake/files,tdjson/file,tree/media,speech/playback,backend,td}.rs`；依据 `docs/experiments/exp008-local-stt/design.md`、`specs/contracts/{cli,actions,errors}.md`。**最新一轮：第 3 轮（2026-11-13）· 总评「修改」· 已达停止线（第 2、3 轮连续无 P0/P1）**。
> 为什么不用 diff 类型：评审脚本对 diff 里的二进制文件（两份 `.ogg` 夹具，合成的编造句子）一律拒发，且不收路径过滤；改用 doc 类型逐文件审，纯挪动的测试文件（`td/tests.rs` 等）不在对象里。
> 累计处理：P1×2、P2×6 逐条核实，属实 8、驳回 0；剩 1 条 P2（测试缺口）开 issue。

## 第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-media/reviews/pr4-code-codex-1.md`（不入库）。effort medium。
本轮 P0/P1：2 条 · 停止线：未满足

- **P1｜voice/mod.rs:155 准备期间 Ctrl+C 仍会标已听**（第 1 轮 #1）
  - 核实：属实（退出标志只在 `drive` 里看）
  - 处理：`prepare_play` 收取消标志，下载前、解码前、开出口前、**标已听前**都看；取消 → `interrupted`、不标、不出声、出口释放。测试 `cancel_during_prepare_never_marks`（一开始取消；开出口时取消）
- **P1｜queue.rs:324 暂停失败被当作成功**（第 1 轮 #2）
  - 核实：属实（`AudioQueuePause` 返回码被丢掉）
  - 处理：`Output::pause` 返回 `Result`，失败 → `OutputFailed`、释放出口；测试补暂停失败
- **P2｜play.rs:51 准备阶段出口打不开的错误缺 `marked_listened`**（第 1 轮 #3）
  - 核实：属实
  - 处理：`play::fail` 把准备阶段的 `audio_output_failed` 按同一形状出（`marked_listened:false`）；测试断言只有一行结尾
- **P2｜voice/mod.rs:399 顺序测试太弱**（第 1 轮 #4）
  - 核实：属实（假出口不记录、不能注入失败）
  - 处理：换成记录日志的假出口 `Rec`（打开 / 恢复 / 释放、可让打开失败、可在打开时触发取消）：出口打开失败 → 不标（证明「出口好了才标」）；标失败 → 出口释放、没出声；`play` 补「resume 失败 → 只有一行 error、注明已标」的端到端测试

### 引用抽查

- 「不符」1 条（即 #4）；其余见原文

### 是否再审

有 P1 → 改完开第 2 轮。

## 第 2 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-media/reviews/pr4-code-codex-2.md`（不入库）。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1

上一轮处理的验证：#1、#2 codex 认可；#3、#4 的测试仍不够（下面 #1、#3）。

- **P2｜voice/mod.rs:163 取消期间某步失败，报的是普通错误不是 interrupted**（第 2 轮 #1）
  - 核实：属实（先 `?` 传播错误，再看取消）
  - 处理：`after_step` 每步先看取消再传播错误（含找消息那一步）；测试 `cancel_wins_over_a_failing_step`
- **P2｜play.rs:190 空行 `continue` 绕过放完与失效检测**（第 2 轮 #2）
  - 核实：属实
  - 处理：空行只跳过解析，每轮照样 `tick`；测试 `blank_lines_do_not_block_finishing`（持续送空行仍能放完）
- **P2｜play.rs:543 端到端测试没走标已听、没验释放**（第 2 轮 #3）
  - 核实：属实（直接构造播放器、硬编码 `finish` 参数）
  - 处理：`voice` 里补两条真正贯通的测试：`prepare_play`（标已听）→ `drive` → `finish`，分别注入「恢复失败」与「播放中出口失效」，断言调了 `mark_listened`、出口 `stop`、结尾只有一行 error 且 `marked_listened:true`

### 是否再审

连续第 1 轮无 P0/P1 → 改完再审一轮确认。

## 第 3 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-media/reviews/pr4-code-codex-3.md`（不入库）。挪出去的测试文件 `play/tests.rs`、`voice/tests.rs` 作为依据一并给读（评审对象不变，保持同一评审身份）。
本轮 P0/P1：0 条 · 停止线：**满足**（第 2、3 轮连续无 P0/P1）

上一轮处理的验证：第 2 轮 #1–#3 codex 认可（取消优先、空行、贯通测试）。

- **P2｜voice/mod.rs:163 等标已听应答时取消、应答又失败，报普通错误不是 interrupted**（第 3 轮 #1）
  - 核实：属实（标记那一步没走 `after_step`）
  - 处理：代码已改（标记结果也经 `after_step`，先看取消）。**测试未补**：要在「标记请求进行中」那一刻立取消标志，假后端没有这个钩子；开 issue 留后处理（待大总管开号）

### 是否再审

停止线已满足，停。剩余 P2：1 条（上面那条的测试），开 issue。
