# 评审 039 · #59 3c `transcribe` 命令（codex · gpt-6-astra · read-only）

> 对象：`git diff origin/main`（`feat/59-transcribe`，main 已合进 HEAD）：`atg-cli` 的 `transcribe.rs`、`voice/transcribe.rs`、`lease.rs`（`Guard`）、`session.rs`、`args.rs`、`out.rs`；`atg-audio` 的 `stt/locate.rs`；`atg-core` 的 `Doc::Transcription`、`SttUnavailable`；契约 `cli.md`、`errors.md`、`content-tree.md`，`design.md`。**最新一轮：第 1 轮（2026-11-16）· 总评「修改」（仅 P2）· 待第 2 轮**。
> 之前：atg-review 自审一轮（wf_2a8176f3-c56，三视角），P0/P1 无；P2×5 全改（`ATG_STT_HELPER` 只认绝对路径、提示修法、`what` 码三处比对测试、`transcript_stale` 注明可重试）；P3 改 6 条、留 5 条（随 PR 开 issue）。
> 累计处理：P2×2 逐条核实，属实 2、驳回 0。

## 第 1 轮（2026-11-16）· 总评「修改」（仅 P2）

原文：`_tmp/reviews-039/diff-origin-main-codex-1.md`（不入库）。effort medium，90 秒。
本轮 P0/P1：0 条 · 停止线：连续第 1 轮无 P0/P1

- **P2｜撤回 / 换文件 / 换 id 的测试靠时序落在「转写中」，且没覆盖途中改 18+**（第 1 轮 #1）
  - 核实：属实。`push_after_download` 只等看到下载请求就推，靠假辅助程序 `ok-zh-slow` 睡 1 秒，不保证落在下载完之后、最后一次 drain 之前；design 第二节第 3 条要的「下载中」「转写中」两段也没分开
  - 处理：「转写中」改成闸门：每条测试生成一份带闸门的假辅助程序（收完音频留 `started`、等 `go` 才出结果），测试线程推更新 → `FakeBackend::settle`（确认进了事件流）→ 放行；三条测试改用它，删掉 `ok-zh-slow.sh`。新增「转写中被标为 18+」测试：节点 `sensitive`、照写表，不加 `--reveal` 时 JSON 与读法都不带字、`outcome.type` 为 `hidden`。「下载中」那段没单独测：假后端的下载立即返回，要加下载闸门得改 `fake.rs`（已在超标基线上），留作 P2 issue
- **P2｜租约测试只数开关次数，没验证「开着才收得到更新」，也没测守卫与别的使用者共存**（第 1 轮 #2）
  - 核实：属实。`FakeBackend::chat_open` 只记账、`push` 不看开没开；没有两个守卫、守卫 + 订阅者、panic 展开的测试
  - 处理：新增 `guard_shares_the_table_and_only_last_release_closes`（真实 `Guard` + `lease::open` 的订阅者共用一张表）：两项转写先结束一项不关、订阅者开着时转写结束不关、panic 展开也放回且最后一份才关。「假后端没打开就不推更新」要改 `fake.rs` 的投递（超标基线文件、影响全部假后端测试），留作 P2 issue
- 引用抽查：「属实」5 条；「无法核实」1 条（`transcript_verdict` 内部，不在补读清单；3b 已由 codex 025 审过）

### 是否再审

再审一轮（连续第 1 轮）。
