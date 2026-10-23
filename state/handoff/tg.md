# 现状页 · tg

- 手上：命门 3 —— grammers vs TDLib（交代 `_tmp/20261023-dispatch/tg.md`）
- 做到哪：**完成**。结果在 `docs/research/001_stack/findings-tg.md`（汉字 792 个）
- 核查进度：6/6，两个库各答一遍，每条都附了文件:行号
- 结论：首选 TDLib + tdlib-rs（download-tdlib）；备选 grammers（Codeberg 版）
- 取材：`resources/grammers-codeberg`（正本；GitHub 版 2026-02 起已停更）· `resources/grammers`（旧 GitHub 版，大总管定：先留着不删）· `resources/td` · `resources/tdlib-rs`
- 留给大总管 / 人拍板：①要不要申请编译名额，用 local-tdlib 自己编一次固定版本（缓解 tdlib-rs 落后的问题）；②建议做一次小实验，实测 tdjson ↔ GPUI 的延迟
- 未做：没编译 TDLib，没做任何 git 写操作，没用真实账号
- 下一步：无。大总管已验收（2026-10-23），已收尾
