# 评审 068 · #169 M3.1 第二刀：场景夹具 + 加载器 + `atg-cli --fake`（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（`feat/gui` 上 PR-2；PR-1 已 squash 进 main，三点范围只含 PR-2）。第 1 轮时 HEAD = `f96e75a`。
> - 场景夹具 `src/atg-core/fixtures/scenarios/{basic-4-chats,empty}` 与夹具隐私检查 `scripts/check-fixtures.py`（接进 `arch.yml`、`GATE_FILES`）
> - 加载器 `atg_core::fake::Scenario`；私有临时目录 `config::private_temp_dir` + `TempDirGuard`，`testkit::temp_dir` 改调它（改模块边界，arch 审定）
> - `atg-cli` 的 `chats` / `history` 认 `--fake <场景>`（`fake_run.rs`，一次性测试档案，不碰钥匙串）
> - `speech/interactive.json` 投票人 id `424242` → `99001`（连同 `tests/snapshots.rs` 那条隐私断言）
> - 契约：`testing.md` 第一节第 3 小节（场景解析规则正本）、`cli.md` 的 `--fake` 一行、`errors.md` 三个新码
>
> 依据文件：`specs/contracts/testing.md`、`specs/contracts/cli.md`、`specs/architecture-rules.md`，关注点文件（不入库）。effort medium。
> **最新一轮：第 2 轮（2026-11-24）· 总评「修改」（只有 P2×2，已当场修）· 第 2、3 轮连续无 P0/P1 → 满足停止线；第 3 轮没再改代码 → 收敛。** 留作 issue 的 P2：1 条（第 3 轮 #1，`history/` 下文件属性读不了被当成「不是文件」跳过，issue 待开）。
> 之前：**多视角自审 4 轮**（`atg-review`，范围同上）——第 2 轮有 P1（`cli.md` 没跟上 `--fake` 的现状），第 3、4 轮无 P0/P1，停。衍生 issue：#183（CI 不跑 release 测试）。另：全套测试 8 遍里偶发红过 1 遍、没抓到名字，循环 20 遍复现不出；顺带把「跑完不留临时目录」的核对收窄到只看本子进程（同机别的会话同时跑测试会撞）。

## 第 1 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-068/068-scenario-fake-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×4 P3×0`。行号指评审时的版本。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2｜`scripts/check-fixtures.py:109` `user`、`message` 对象自己的 `id` 被套用会话 id 规则**（第 1 轮 #1）
  - 核实：属实。传给 `id_ok` 的键是 `"id"`，落到「会话 id 可为负、可 `-100` 前缀」那一支，负的用户 id、消息 id 也放行
  - 处理：对象自己的 `id` 按对象种类换成 `chat_id` / `user_id` / `message_id` 的规则；补自测 `test_negative_user_or_message_id_is_red`。改坏验证：撤掉映射 → 新测试红
- **P2｜`scripts/check-fixtures.py:112` `minithumbnail.data` 的占位被拒**（第 1 轮 #2）
  - 核实：属实。规约与脚本头都写「空串或 `fixture-` 占位」，实现只认空串
  - 处理：`data` 也认 `fixture-` 开头的占位；补自测 `test_placeholder_minithumbnail_is_green`
- **P2｜`scripts/check-fixtures.py:150` 子目录读不了会被悄悄跳过**（第 1 轮 #3）
  - 核实：属实。`os.walk` 默认吞掉遍历错误；只要别处扫到 JSON，「一份都没扫到」的保护就不起作用，结果仍报 0 处命中
  - 处理：`os.walk(onerror=…)` 把遍历错误记成 `[read]` 命中；补自测 `test_unreadable_subdir_is_red_even_if_others_scan`（root 下跳过：权限拦不住 root）。改坏验证：撤掉 `onerror` → 新测试红
- **P2｜`specs/contracts/cli.md:64` 「别的命令带 `--fake` 一律报 `not_implemented`」不成立**（第 1 轮 #4）
  - 核实：属实，但只差 `version` 一个。逐个查了各命令入口：连后端的命令都判了 `--fake`（`session.rs` 的各入口、L2 命令共用的 `l2_run`、`forward.rs`、`send_file.rs`、`serve.rs`），所以没有命令会带着 `--fake` 去连真实后端；`version` 不连后端、不看它；`speech` 报 `usage`
  - 处理：`cli.md` 写明 `version` 不看它、参数本身写错先报 `usage`、`--fake` 不会拿去连真实后端

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：4 条（规范文件名、发布构建裸名字、`FakeRun` 字段丢弃顺序、投票人哨兵断言）
- 无法核实：「照常拿 `atg.lock`」（`chats::run` / `history::run` 不在补读清单）——本机核过：假后端 `start` 里 `profile.lock(…)`（`src/atg-core/src/fake.rs`），一次性档案照常拿锁

### 修复后的验证

只动了 Python 脚本、它的自测和 `cli.md`，不用编译：`scripts/tests` 全部自测 OK（`test_check_fixtures` 15 → 18 条）；`check-fixtures.py` 在全部夹具上 0 处命中。

### 是否再审

本轮无 P0/P1，只连续 1 轮 → 再审一轮确认。

## 第 2 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-068/068-scenario-fake-codex-2.md`（不入库）。范围同上，HEAD = `ed31497`（第 1 轮修复）。`result: P0×0 P1×0 P2×2 P3×0`。
本轮 P0/P1：0 · 停止线：本轮与上一轮都无 P0/P1 → **满足** · 留作 issue 的 P2：无（当场修了）

### 上一轮处理的验证结果

codex 逐条核了第 1 轮四项，都已落实：id 规则按对象种类、`fixture-` 缩略图占位、`os.walk` 的错误回调、`cli.md` 写明 `version` / `speech` 的例外。另核实假后端 `start` 会 `profile.lock(…)`（第 1 轮「无法核实」的那条）。

- **P2｜`src/atg-cli/src/args.rs:37` 第 1 轮 #4 的修法没同步到帮助文字**（第 2 轮 #1）
  - 核实：属实。`--fake` 的帮助文字和 `fake_run.rs` 文件头仍写「别的命令报 not_implemented」
  - 处理：两处都改成指向 `cli.md` 的 `--fake` 一行（正本），不再各自维护一份命令清单
- **P2｜`scripts/check-fixtures.py:38` 保留域名带查询、片段时误报**（第 2 轮 #2）
  - 核实：属实，而且反过来还有一个漏报：旧正则把 `?`、`#` 之后也算进主机名，所以 `https://example.invalid?q=1` 被误报；`https://example.com.evil.net?q=example.invalid` 的「主机名」以 `.invalid` 结尾，反倒被放过
  - 处理：用 `urllib.parse.urlsplit` 取主机名；补自测两条（保留域名带查询 / 片段 / 端口放行；真域名带 `?q=example.invalid` 仍红）。拿提交里的旧版脚本跑这两条，都红

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：8 条

### 修复后的验证

`scripts/tests` 全部 OK（`test_check_fixtures` 18 → 20 条）；`check-fixtures.py` 在全部夹具上 0 处命中。`args.rs` 只改了帮助文字（doc 注释）：编译、clippy 过，`atg-cli` 测试 270 过 0 败，`--help` 里显示的是新文字。

### 是否再审

第 1、2 轮连续无 P0/P1，已满足停止线；但第 2 轮之后又改了代码（`check-fixtures.py` 的取主机名、帮助文字），最后这份要有人审过（大总管定）→ 第 3 轮确认：无 P0/P1、且不再改代码，才开 PR。

## 第 3 轮（2026-11-24）· 总评「修改」

原文：`_tmp/reviews-068/068-scenario-fake-codex-3.md`（不入库）。范围同上，HEAD = `db3615d`（第 2 轮修复）。`result: P0×0 P1×0 P2×1 P3×0`。
本轮 P0/P1：0 · 停止线：本轮与上一轮都无 P0/P1 → **满足** · 留作 issue 的 P2：1 条（本轮 #1）

### 上一轮处理的验证结果

codex 核了第 2 轮两项（帮助文字指向 `cli.md`、`urlsplit` 取主机名，含伪装查询的漏报），都已落实；第 1 轮四项仍在。

- **P2｜`src/atg-core/src/fake/scenario.rs:118` 文件属性读不了时被当成「不是文件」跳过**（第 3 轮 #1）
  - 核实：属实。`Path::is_file()` 把读属性失败也变成 `false`；`history/` 有读权限、没有进入权限时能列出文件名、查不了属性，那些文件被跳过，场景装成空历史，不报 `fixture_bad`（规约 `testing.md` 第一节第 3 小节第 6 条「读不了」）
  - 处理：**不在本 PR 修，开 issue**。这种权限组合要专门造才会出现；本 PR 已满足停止线，再改代码又要多审一轮。修法照 codex：改用返回 `Result` 的 `metadata()`，失败映射为 `fixture_bad`，确认是目录才跳过；补一条非 root 才跑的权限测试

### 引用抽查

- codex 列的「不符」：无
- codex 列的「核实属实」：7 条

### 是否再审

第 2、3 轮连续无 P0/P1，第 3 轮之后不再改代码 → 停，收敛。三轮共提 P2×7：属实 7，已修 6，留作 issue 1。
