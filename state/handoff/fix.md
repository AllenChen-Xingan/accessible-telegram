# fix 现状页（批次 b18）

## 手上
第一批：#68（置顶 / 投票隐藏内容补「按 D 展开」）+ #70（链接部分剧透，读法整条遮）。分支 fix/68-70-speech。

## 做到哪
- 共用判定 `tree::text::link_hidden`（mask / links_marked / speech 三处都调它，规则只写一处）
- speech：`service()` 回传隐藏标记，置顶带内容的给按键提示；投票题目隐藏也算进提示；`span` / `full_span` 先判 link_hidden 整条遮；「隐藏内容，？」→「隐藏内容？」（V22，规约 message.md 第三节）
- 行数守则：speech.rs、tree/text.rs 行内测试挪到 speech/tests.rs、tree/text/tests.rs（speech.rs 900→~851，text.rs 1058→~671）；arch 门槛先合的话要 --tighten
- 夹具 hidden.json 加 3006（url 末段剧透）、3007（text_url 部分剧透）、3008（置顶 3007）；新测试 partly_hidden_links_agree_across_outputs
- 规约 specs/style/message.md 加例 M17b/M17c/M17d

- 名额跑过一次（已归还）：fmt ✓ · clippy ✓ · test -p atg-core 215 过 · check-no-debug ✗（见下）
- atg-review 一轮：P1 快照未更新（已修）；P2/P3 已修 message.md 正例与旧说明、hide 改名、投票隐藏改走 spans；content-tree.md 未同步（不在可写清单，报大总管）
- 自审修改后的小改（poll 判定走 spans、改名）还没编译过，下次领名额一并跑
- codex 020 第 1 轮在跑（_tmp/reviews/）

## 第一批状态（2026-11-13）
- 名额第二次跑：fmt ✓ · clippy ✓ · test -p atg-core 216 过 · check-no-debug ✓（含新自测；bash 3.2 / 5 都过）
- check-no-debug 按大总管定 A 改了：认 cfg(test) 声明的 tests.rs 与 tests/<主题>.rs，只认文件顶层的声明
- content-tree.md 已同步；docs/plan/README.md 加了 011 一行；出处改指 011 B18-5
- codex 020：两轮 P2×3 全部属实、全部已修，连续两轮无 P0/P1，达停止线 → docs/reviews/020_*
- PR 正文草稿：MAIN/_tmp/20261113-b18/pr-fix.md，等大总管提交并开 PR

## 下一步
等大总管验收；第二批（#61 + #63）等通知另起分支

## 要人定 / 等大总管
- docs/reviews/README.md 的 020 索引行（不在我的可写范围）
- 「隐藏内容」后问号不加逗号，等实听确认
