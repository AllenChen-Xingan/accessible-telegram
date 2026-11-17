# 评审 034 · #12 发文件 / 图片（`send-file`，直发；闸只加留痕字段）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`384da95...HEAD`（三点；分支从 main 384da95 起）。重点写在 `_tmp/reviews-034/focus.md`（不入库）。
> **最新一轮：第 1 轮（2026-11-17）· 总评「修改」· 连续第 1 轮无 P0/P1**。
> 之前：多视角自审 2 轮（atg-review），两轮都无 P0/P1：第 1 轮 P2×4 P3×11（硬链接、许可文件夹外先报不许、本机检查先于查会话、失败能重发留快照……）、第 2 轮 P2×4 P3×9（CLI 入口先查会话、文案与契约），都已改；`resend` 带文件不按进度等 → #158。机器验收：`cargo test -p atg-core -p atg-cli` 679 过 0 败，clippy -D warnings 两种（不带 tdlib 全工作区、`--features atg-cli/tdlib --all-targets`）0。
> 隐私扫描：密钥类命中 9 条，全是 ConfirmToken 类型标注，大总管逐条核过（7 处源码行，另 2 条是 diff 里同两行的重复），准加 `--force-secrets`。
> 闸的改动只有留痕字段（`Gated::log_file` 默认 `None`、`DirectLogLine.file{kind,size}`），拦截逻辑不动；属一级评审，人本人看。

## 第 1 轮（2026-11-17）· 总评「修改」

原文：`_tmp/reviews-034/send-file-12-codex-1.md`（不入库）。effort medium，78 秒（`result: P0×0 P1×0 P2×3 P3×0`）。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮 · 留作 issue 的 P2：0 条

- **P2｜src/atg-core/src/send/file/local.rs:118 判许可之后把中间文件夹换成链接，仍能越界（构造时序）**（第 1 轮 #1）
  - 核实：属实。`path_ok` 判的是 canonical 字符串，之后 `metadata` 与 `open` 都按路径走，中间一级被换成指向外面的链接时两者都跟过去，dev/ino 一致；`O_NOFOLLOW` 只管最后一级。
  - 处理：打开后按句柄取它此刻的路径（macOS `F_GETPATH`），再过一次 `path_ok`，取不到按不许；许可判断由此绑在真正读的那个文件上。竞态构造不出确定性的测试（威胁模型：构造时序最高 P2），未补红测试；既有的符号链接、硬链接、许可文件夹外的测试照过。契约 actions.md 写明。
- **P2｜src/atg-core/src/send/file.rs:200 改扩展名就绕过防重复（构造绕过）**（第 1 轮 #2）
  - 核实：属实。意图摘要装着整份请求（只清了路径），`.png` 改成 `.bin` 后按图片发变按文件发，请求形状不同、指纹不同。
  - 处理：意图摘要只装目标会话、话题、回复、说明文字与文件 SHA-256（`send/file` 自己的摘要内容，没动 gate）。先红后绿：`edges::renaming_the_extension_does_not_escape_the_guard`。契约第 5 小节写明。
- **P2｜src/atg-core/src/send/file.rs:410 进度查询重复用掉已花的时间，可能越过 60 秒 / 10 分钟、过期的进度还能清零计时**（第 1 轮 #3）
  - 核实：属实。事件等待用掉 `slice` 后，查询又拿一份 `slice`；回来后不看是否已过截止就更新 `moved`。
  - 处理：改成绝对截止时刻（总上限、没进度上限取小），事件等待与查询共用剩下的时间；查询回来已过截止的这次进度不算。补测试 `edges::upload_wait_stops_at_the_cap`（一直有进度时到总上限就收）；假后端的进度查询是即时应答，越界的那一小段在修前也测不红，如实记下。

### 引用抽查

- codex 列的「不符」：1 条（actions.md「换个名字再发也拦」没盖到改扩展名）——即 #2，已改
- codex 列的「核实属实」：4 条（`inputMessagePhoto` / `inputMessageDocument` 嵌套形状、失败能重发留快照、旧留痕行照常整理）
- 无法核实：0 条

### 是否再审

要再审：三条改完开第 2 轮（`--previous` 带本轮原文）。
