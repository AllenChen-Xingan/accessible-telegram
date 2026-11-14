# 评审 013 · skill `codex-review` scanskill 改动补审（codex · gpt-6-astra · read-only）

> 对象：skill `codex-review`（仓外 `2-aartifacts/skills/codex-review/`）批次 b10 的 scanskill 改动（拆 `privacy-scan.sh`、放松扫描、`--force-secrets`、#29）。#32 补审第 7 块。
> **最新一轮**：增量第 2 轮（2026-11-13）· 总评「通过」· **已达停止线**（增量两轮无 P0/P1；第 9–11 轮按威胁模型重分级亦无 P0/P1）。

## 一、第 1–11 轮（批次 b10，2026-10-25）按人定威胁模型重新分级

原文：`_tmp/20261025-b10/codexrounds/scanskill-codex-1..11.md`（不入库）。当时第 9、10、11 轮各有 P1（1、2、1 条），codex 额度用完后按临时例外以多视角自审收尾，#32 记「未到停止线」。

第 9–11 轮的 P1 全部属于同一类「换种排版就漏扫」：`api_id` / `api_hash` 跨行书写、超长类型限定（`std::option::Option<std::primitive::i32>`）、中间夹注释、`if` 表达式包裹值、比较豁免吞掉同句赋值。人定（2026-10-25，`SKILL.md`「威胁模型」）：「扫描器只防常见写法；为绕过而构造的排版（跨行拆开、注释隔开、超长类型限定、条件表达式包裹等）最高算 P2，开 issue 留着，不进停止线」。按此重新分级，**第 9–11 轮无 P0/P1**，scanskill 当时的版本已满足停止线；这些条目已在 #34。（大总管确认此结论，2026-11-13。）

## 二、第 11 轮之后的增量

第 11 轮之后已安装版本的实质改动只有「纯文本闸」：`scripts/lib/text-gate.sh`（2026-11-05，perl 严格校验整份字节为无 NUL 的 UTF-8，替换会在 macOS 27 上误报的 `iconv` 复查）、`scripts/privacy-scan.sh` 的接线（加载失败一律不放行）与测试 `scripts/tests/test-privacy-scan.sh`；其余为 `SKILL.md`、`_output.md` 补的威胁模型文字。只审这一段。

### 增量第 1 轮（2026-11-13）· 总评「修改」

原文：`_tmp/20261113-b18/codex-work/scanskill-gate-codex-1.md`（不入库）。contract 类型，effort high，`--cwd` 指向 skill 目录（只读）。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：#1–#3

- **P2｜text-gate.sh:33 Netpbm 例外分支只看前 4096 字节**（第 1 轮 #1）
  - 核实：属实；需刻意构造（无扩展名 P6、头部前塞超 4096 字节注释、像素全为合法 UTF-8），按威胁模型为「构造绕过」，且内容照样过隐私扫描。处理：转 issue。
- **P2｜test-privacy-scan.sh:39 关键故障路径缺回归**（第 1 轮 #2）
  - 核实：属实（输入为符号链接指向禁区 / 黑名单扩展名、无扩展名二进制、缺 `file` / `perl`、库加载报错未单独测）。处理：转 issue。
- **P2｜text-gate.sh:38 校验器不可用、读不全被报成「不支持的编码」**（第 1 轮 #3）
  - 核实：属实（`ps_bytes_utf8` 一律回 1，`ps_is_plain_text` 统一转 2）；仍是硬拦，只是原因说错。处理：转 issue。

引用抽查：核实属实 8、不符 1（即 #3 的注释）、无法核实 1（`codex-review.sh` 不在本轮对象内）。

### 增量第 2 轮（2026-11-13）· 总评「通过」

原文：`_tmp/20261113-b18/codex-work/scanskill-gate-codex-2.md`（不入库）。带第 1 轮原文与处理记录。effort high。
本轮 P0/P1：0，无新增条目 · 停止线：**满足**（增量第 1、2 轮均无 P0/P1）· 留作 issue 的 P2：增量第 1 轮 #1–#3 → 记为 #34 评论（不单开）

是否再审：**停**。scanskill 整体结论：第 1–11 轮按人定威胁模型重新分级后已收敛（剩余在 #34），第 11 轮后的增量两轮无 P0/P1。
