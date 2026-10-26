# 读法夹具

每个 `.json` 是一串 TDLib 更新对象（JSON 数组），按顺序喂进 store，再投影成内容树、渲染成读法。
`atg-cli speech --fixture <文件>` 与 `atg-core` 的读法快照（`tests/snapshots/`）共用这些文件。人名、会话名全是编的，只许取自上一层的 `names.txt`（`tests/coverage.rs` 会查）。

| 文件 | 覆盖什么 | 怎么听 |
|---|---|---|
| `basic.json` | 各种内容（文字、图片、语音、文件……）、各种系统消息、连接状态、三个会话的列表 | `--level brief` 听消息；`--level chat_brief` 听会话列表 |
| `entities.json` | 正文里的格式与实体：粗体、链接、提及、剧透、代码、引用、坏偏移 | `--level brief` |
| `cases.json` | 路线图 M1.5 列的难读情形：连续同一发送人、重名、频道身份 / 匿名管理员发言、名字没到、连续表情、隐藏内容、撤回占位；系统消息里的「我」、频道里的系统消息、置顶带内容 | `--level brief` |
| `hidden.json` | 隐藏内容出现在各处：正文、投票题目、回复引用的那段、剧透里的文字链接、置顶带的内容、会话项最后一条（#21：不加 `--reveal` 时 `data` 也遮住） | `--level brief`；加 `--reveal` 听原文 |
| `chat_items.json` | 会话项：`specs/style/chat-item.md` 的例 C1–C15（已注销、收藏夹、撤回、隐藏内容……） | `--level chat_brief` |
