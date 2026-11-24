# 评审 068 · #169 M3.1 第二刀：场景夹具 + 加载器 + `atg-cli --fake`（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（`feat/gui` 上 PR-2；PR-1 已 squash 进 main，三点范围只含 PR-2）。第 1 轮时 HEAD = `f96e75a`。
> - 场景夹具 `src/atg-core/fixtures/scenarios/{basic-4-chats,empty}` 与夹具隐私检查 `scripts/check-fixtures.py`（接进 `arch.yml`、`GATE_FILES`）
> - 加载器 `atg_core::fake::Scenario`；私有临时目录 `config::private_temp_dir` + `TempDirGuard`，`testkit::temp_dir` 改调它（改模块边界，arch 审定）
> - `atg-cli` 的 `chats` / `history` 认 `--fake <场景>`（`fake_run.rs`，一次性测试档案，不碰钥匙串）
> - `speech/interactive.json` 投票人 id `424242` → `99001`（连同 `tests/snapshots.rs` 那条隐私断言）
> - 契约：`testing.md` 第一节第 3 小节（场景解析规则正本）、`cli.md` 的 `--fake` 一行、`errors.md` 三个新码
>
> 依据文件：`specs/contracts/testing.md`、`specs/contracts/cli.md`、`specs/architecture-rules.md`，关注点文件（不入库）。effort medium。
> **最新一轮：第 1 轮（2026-11-24）· 总评「修改」· 无 P0/P1，连续第 1 轮，待第 2 轮确认。** 留作 issue 的 P2：无（本轮 P2 都当场修了）。
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
