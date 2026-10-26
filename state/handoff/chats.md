# 交接 · 手下 chats · issue #18（第一刀第 5 块：`atg-cli chats`）

> 2026-10-25 · 交代 `_tmp/20261025-b14/chats.md` · 会话 `telegram-accessibility-87` · **终版**：代码完成，自审 P0/P1 清零（P2 已修或已改规约），正式账号只读冒烟过；codex 补审记 #32
> 大总管会话名已换成 `telegram-accessibility-64`（原 `-40`；coord.md 第 4 行待大总管改）

## 一句话

`atg-cli chats [--list main|archive] [-n N] [--unread] [--cursor C]` 接通：`loadChats` 一批批加载，顺序只看 Store；等连接线到 `ready`（最多 10 秒）再出；一行一个 `chat`（`ChatItem` + `speech.brief`），结尾 `result` 带 `synced`、`first_chat_ms`。会话项最后一条不再截断（R2）。

## 设计

- **核心** `backend.rs`：`Backend::chats_load(list, limit) -> ChatsLoad { Loaded, End }`（TDLib 404 吞掉 = `End`）· `Backend::wait_net(timeout, pred)`；`link.rs` 加 `Shared::wait_net`
- `td.rs`：`loadChats`（主列表 / 归档 / 文件夹；未知列表 → `invalid_input`）；失败日志只记机读码
- `fake.rs`：`set_chats(list, 会话们)`（一个会话 = 几条 TDLib JSON 更新）· `push_net` · `Call::ChatsLoad`；没登录回 401；**应答前用 `Feed::Barrier` 等收线程处理完这批更新**（与真实 TDLib「更新先于应答」一致）
- `speech.rs`：去掉 `LAST_MAX_CHARS` 截断（R2）；两份快照随之更新，差异与 `state/coord.md`「解冻后要改的快照」表 R2 那两行一致。R1 / R7 的快照与代码**没动**（归读法那块）
- **CLI** `chats.rs`：起后端 → 等授权线离开「等参数」（不是 `ready` → `auth_required`）→ 循环 `chats_load` 直到凑够 `limit+1`（多一个判断有没有下一页）或到底（防死循环：连 5 批没新会话就停，按「还有更多」交出）。**整段加载最多 10 秒**，到点照出已有的（`load_timed_out:true`、`more:true`），不再多等补发 → `wait_net(ready, 10 秒)` → 再喂一遍事件 → 投影。游标 `<order>:<会话 id>`
- `first_chat_ms`：进程启动到 Store 里这个列表第一次有会话（每批喂完检查）
- 文字模式：每行一句 `chat_brief`；统计（条数、冷启动耗时、没同步完、还有更多）写标准错误

## 文件

`src/atg-core/src/{backend.rs, fake.rs, td.rs, link.rs, speech.rs}` · `src/atg-core/tests/snapshots/snapshots__basic_chats_{brief,json}.snap` · `src/atg-cli/src/{chats.rs(新), main.rs, args.rs, session.rs}` · `src/atg-cli/tests/cli.rs`

## 验证（第一次名额，2026-10-25）

fmt ✓ · clippy -D warnings 三套 ✓ · check-no-debug ✓ · `cargo test --workspace` 221 过 0 败。新增：atg-cli `chats` 单测 12（排序、补发途中挪位、各字段与有效静音、不截断、空列表、2500 个会话分页与游标全覆盖、只列未读、归档、断网 `synced:false`、没登录、加载出错、游标解析）+ 二进制测试 2（参数错误、`--fake` 未做）

## 自审（/atg-review，3 视角）→ 已改

| 级 | 发现 | 处理 |
|---|---|---|
| P1 | 加载循环 `block_on(chats_load)` 无超时：断网且本地不够一页时永远卡住 | 整段加载 10 秒上限；`load_timed_out`；假后端 `hang_chats_load_after` + 单测 `load_that_never_answers_stops_at_the_deadline` |
| P2 | 「补发」没解释（--help、注释） | 写明：服务器把离线期间漏掉的更新补送完，TDLib 报 `ready` |
| P2 | README 没写 chats | 已补 |
| P2 | 全局选项帮助缺前提（--fake / --redact / --wait / --tz） | 标「还没做」并说清含义 |
| P2 | 契约要 `sync_done`，实现用连接线 ready | 注释写明是暂代；规约冲突报大总管 |
| P2 | `result.data` 新字段未登记 | 报大总管（我不改 specs） |
| P3 | fake 记账先于本地检查 | 改成拦下的不记 |
| P3 | 游标、order 没解释；Global 注释过时；帮助里写死 10 秒 | 已改 |
| P3 | session / chats 两份后端起停流程 | 未改，建议下一块统一 |
| P3 | `store/mod.rs`、`src/README.md` 术语 | 不是本块改动，未改 |

修完：clippy 默认 ✓ · test 222 过（⚠️ 这一次没先申请名额，已向大总管说明）。第三次名额：tdlib 两套 clippy ✓ · check-no-debug ✓ · fmt ✓。

规约（大总管授权）：`specs/contracts/cli.md` 写明 `sync_done` 暂行定义（连接线到 `ready`）与加载上限；登记 `chats` 的 `result.data` 字段 `list` / `synced` / `load_timed_out` / `first_chat_ms` 与游标形状；加修订记录。

## 正式账号只读冒烟（档案 main，2026-10-25）

只报统计：输出管道进 scratchpad 的统计脚本，原始行、会话名、正文都没出现在任何地方。只调 `loadChats`，不打开会话、不标已读、不发、不改设置。

- 第一次卡在钥匙串弹框（读 `atg.tdlib-db-key` / `main`），kill。人定 A：人点「始终允许」后再跑；根治另开 **#38**
- 在线（`-n 100`，跑 3 次）：100 行，字段齐，id 无重复，置顶不在非置顶之后（本列表 0 个置顶）；种类 私聊 69 · 群 16 · 超级群 12 · 频道 3；有未读 52 · 有提及 1 · 静音 6（其中继承范围设置的会话共 93 个）· 有最后一条 99；`synced:true`；**冷启动到第一条会话 67–84 毫秒**，整条命令 425–485 毫秒
- **断网**（`sandbox-exec` 只拦本进程的网络）：`-n 100` → 100 行照出，第一条会话 77 毫秒，`synced:false`，整条 10.1 秒（等补发到点）；`-n 1000` → 本地库只有 113 个，再要就转去问服务器、无应答，10 秒加载上限到点照出 113 行，`synced:false`，整条 10.0 秒
- **R14 结论**：会话列表能先从本地库出，`loadChats` 先交本地已缓存的会话（约 75 毫秒）；本地不够时才去问服务器，断网时那次请求一直不应答——所以加载必须有上限（本块已加）
- 读法 `brief` 长度：平均 131 字，最短 10，最长 2377（不截断，R2）；没有以「…」结尾的
- ⚠️ **发现**：12 个私聊的名字是空的（都有最后一条），读出来是「未命名会话」。没看内容，推测是已注销的账号（TDLib 给空标题）。要人定怎么读（见下）
- api_id / api_hash 在我写的所有文件里出现 0 次；`~/Library/Logs/atg/tdlib-main.log` 0 字节（600）

## 规约缺口 / 要人定

1. ~~新字段登记~~、~~`sync_done` 暂行定义~~：已写进 `cli.md`（大总管授权）
2. ✅ **已定**（人，2026-10-25）：会话行的读法放在 `speech.brief`（`json::value` 现状；`chat_brief` 句子），契约表里另有 `chat_brief` 这个键名——建议就用 `brief`，契约注明「会话行的 brief 即会话项读法」
3. ✅ **已定**：`-n` 默认 100（已写进 `cli.md`）
4. `sync_done` 事件本身仍没做（归 `watch`），做的时候按 `cli.md` 的暂行定义
6. ✅ **已定**：读「已注销的账号」，先核 TDLib 是否 `userTypeDeleted`，归 #20 读法定标（本块不改代码）。原题面——**空名字的私聊怎么读**：A. 照现在「未命名会话」 B. 私聊空名读「已注销的账号」（要先核 TDLib 对注销账号是不是给空标题、`user.type` 是否 `userTypeDeleted`）C. 用对方用户的名字兜底。建议 B（先核再改，归读法定标那块）
5. ✅ **已定**：`--fake` 场景目录以后做（已写进 `cli.md`）；仍没做（要假时钟、时间线，`testing.md`），`chats --fake` 报 `not_implemented`
