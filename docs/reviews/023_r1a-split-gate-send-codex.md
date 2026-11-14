# 评审 023 · R1a 拆 `gate.rs`、`send.rs`（#60 第二段）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`src/atg-core/src/gate.rs` + `gate/{check,display,tests}.rs`，`src/atg-core/src/send.rs` + `send/{text,outcome,resending,tests}.rs` + `send/tests/{text,resending}.rs`；口径是只挪不改，公开名字原样导出。重点写在 `_tmp/20261113-r1a-codex/focus.md`（不入库）。
> **状态：第 1 轮（2026-11-14）· 总评「修改」· P1 一条，驳回（范围给错造成的假象）。等合入新 main 后用三点范围另起第 2 轮。**
> 之前：多视角自审 1 轮（atg-review），P0/P1 零。P2×7 都是摆放问题：发文字和重发共用的助手归到模块根；模块改名 `wait` → `outcome`、`resend` → `resending`，免得 intra-doc 链接撞名；模块根补上文件分工；`Outcome` 放回 gate 根。已按纯挪动改完。机器验收：行多重集比对只多出 mod、pub use、use、注释和 5 处测试用的 `pub(super)`；公开名字 gate 33、send 32，前后一致；`#[test]` 前后都是 41 条；fmt、no-debug、clippy 两种、`test --workspace` 全绿。
> 隐私扫描：密钥类命中 15 条，全是参数类型标注 `token: ConfirmToken`（源文件 5 处，加上 diff 里删、增各 5 处）。人在大总管会话里确认是误报（2026-11-14），放行 `--force-secrets`。

## 第 1 轮（2026-11-14）· 总评「修改」

原文：`_tmp/20261113-r1a-codex/r1a-codex-1.md`（不入库）。effort medium，耗时 51 秒（`result: P0×0 P1×1 P2×0 P3×0`）。
本轮 P0/P1：1 条（驳回）· 停止线：未满足 · 留作 issue 的 P2：无

- **P1｜src/atg-core/src/config.rs:187：纯拆分混入了 #61 档案保护的回退**（第 1 轮 #1）
  - 核实：**驳回**。这一条是我交的范围造成的假象。我用的是两点范围 `origin/main`（main 直接和工作区比）；开跑之前 main 刚合进 #91（#61 档案标记）和 #96，于是 main 上的新改动在 diff 里显示成「被本分支删掉」。实查：`git diff $(git merge-base HEAD origin/main) HEAD --name-only` 只有 gate、send 那 11 个文件，`config.rs`、`session.rs` 和契约一行没动。
  - 处理：代码不改。下一轮改用三点范围 `origin/main...HEAD`（只看本分支相对分叉点的改动），开跑前先由大总管把新 main 合进来、复编。改了范围，身份要换新 name，停止线从头算。
  - 沉淀：codex diff 评审的范围**一律用三点** `<目标>...HEAD`，不用两点 `<目标>`——目标分支在评审期间可能前进。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：6 条，都是关于拆分本身的：`Gate`、`visible`、`hidden_count` 在根上导出；`#[must_use]` 跟着 `Submitted`；三个 `pub(super)` 字段只给测试用；11 个文件都不超过 600 行；codex 自己按 patch 做了增删行比对，没发现逻辑行丢失、重复或被改。
- 无法核实：编译、测试、rustdoc 链接（只读沙箱）。由本地 fmt、clippy 两种、`test --workspace` 实跑补上。

### 是否再审

要再审：本轮的 P1 被驳回，按协议开下一轮，让 codex 看到驳回理由，同时用正确的范围再审一遍。
