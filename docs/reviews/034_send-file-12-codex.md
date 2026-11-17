# 评审 034 · #12 发文件 / 图片（`send-file`，直发；闸只加留痕字段）（codex · gpt-6-astra · read-only · diff 类）

> 对象：`384da95...HEAD`（三点；分支从 main 384da95 起）。重点写在 `_tmp/reviews-034/focus.md`（不入库）。
> **最新一轮：第 3 轮（确认轮，2026-11-17）· 总评「修改」· 无 P0/P1**。第 1、2 轮连续无 P0/P1，已满足停止线；第 2 轮后又改了行为、且已合 main（9434d39，含 #156 #157），大总管定再跑第 3 轮确认（一级评审），也无 P0/P1。剩余 P2×2 交大总管开 issue。
> 之前：多视角自审 2 轮（atg-review），两轮都无 P0/P1：第 1 轮 P2×4 P3×11（硬链接、许可文件夹外先报不许、本机检查先于查会话、失败能重发留快照……）、第 2 轮 P2×4 P3×9（CLI 入口先查会话、文案与契约），都已改；`resend` 带文件不按进度等 → #158。机器验收（合 main 后）：`cargo test -p atg-core -p atg-cli` 680 过 0 败，clippy -D warnings 两种（不带 tdlib 全工作区、`--features atg-cli/tdlib --all-targets`）0。
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

## 第 2 轮（2026-11-17）· 总评「修改」

原文：`_tmp/reviews-034/send-file-12-codex-2.md`（不入库）。effort medium，91 秒（`result: P0×0 P1×0 P2×2 P3×0`）。行号指评审时的版本。
本轮 P0/P1：0 条 · 停止线：满足（第 1、2 轮连续无 P0/P1）· 留作 issue 的 P2：0 条（两条都在本块改了）

- **P2｜src/atg-core/src/send/file.rs:175 按文件发仍开着 TDLib 的自动类型识别**（第 2 轮 #1）
  - 核实：属实。`disable_content_type_detection:false` 时 TDLib 会按内容把文件改成视频、音频等（`api.tl:6425`），与「其余按文件发」不符；改了类型后进度解析也认不出。
  - 处理：设为 `true`。先红后绿：`edges::document_request_turns_off_type_detection`。契约写明。
- **P2｜src/atg-core/src/send/file.rs:417 一次进度查询占满剩下的时间，期间到的成败事件会被判成结果未知**（第 2 轮 #2）
  - 核实：属实。查询拿走全部剩余预算，回到循环顶部已到点就直接报未知，不再看流里排着的结果。
  - 处理：单次查询最多等一个轮询间隔（真机 2 秒）；判未知之前再看一次流里已到的事件，有成败就按成败报。假后端的进度查询是即时应答，这条修前测不红，未补红测试（如实记下）；既有的等待测试照过。契约写明。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：6 条（上一轮三条修复各自到位、两种请求嵌套形状、`disable_content_type_detection` 的含义、旧留痕行照常整理）
- 无法核实：0 条

### 是否再审

满足停止线；但第 2 轮后又改了行为、合了 main，大总管定跑第 3 轮确认，没有 P0/P1 就开 PR。

## 第 3 轮（确认轮，2026-11-17）· 总评「修改」

**合 main 后改用三点范围（`origin/main...HEAD`，分叉点 fe8d1ec），避免审到 #156、#157。** 脚本把名字和审查范围绑在一起，换了范围要换名字：本轮名 `send-file-12-3dot`，它自己的停止线计数从第 1 轮起；本轮是第 1、2 轮之后的确认轮（大总管认可）。隐私扫描同样 9 条 `token: ConfirmToken,`（`send/file.rs` 两行挪到 303、348），大总管重核放行。
原文：`_tmp/reviews-034/send-file-12-3dot-codex-1.md`（不入库）。effort medium，92 秒（`result: P0×0 P1×0 P2×2 P3×0`）。
本轮 P0/P1：0 条 · 停止线：已满足（第 1、2 轮）· 留作 issue 的 P2：2 条

- **P2｜src/atg-core/src/send/file/local.rs:96 悬空符号链接仍能探出许可文件夹外的文件在不在（构造绕过）**（第 3 轮 #1）
  - 核实：属实。`Downloads/x` 链向外面：目标在 → canonicalize 成功 → `file_not_allowed`；目标不在 → 走「看所在文件夹」那支 → `not_a_file`。要先在许可文件夹里建链接，属构造绕过。
  - 处理：留作 issue（大总管开）。修法：解析失败时若路径本身是符号链接，一律 `file_not_allowed`；补目标在 / 不在的成对测试。
- **P2｜src/atg-core/src/send/file.rs:400 10 分钟总上限没算交出时等应答的那段**（第 3 轮 #2）
  - 核实：属实。`wait_upload` 在交出应答回来后才开始计 10 分钟，交出最多 30 秒，最坏约 10 分 30 秒。
  - 处理：留作 issue（大总管开）。修法：在交出时记截止时刻，等结果用剩下的；契约同步。

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条（前两轮修复各自到位、请求嵌套形状、闸只加留痕字段）
- 无法核实：0 条

### 是否再审

不再审：确认轮无 P0/P1，开 PR；两条 P2 开 issue。
