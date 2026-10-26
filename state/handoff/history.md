# 现状页 · 手下 history（issue #21 `atg-cli history` / `links`）

工作目录 `atg-wt/21-history` · 分支 `feat/21-history`（大总管已快进到 df6882b）· 大总管 `telegram-accessibility-1a`

## 手上
#21 第一刀第 8 块：`atg-cli history`（F2 落点、翻页、循环拉够、brief / --detail / --full）+ `atg-cli links` + 接进 `serve` 转发 + 假后端单测与快照。

## 进度
- 2026-10-26 09:35 报到；11:12 收到「直接开工」
- 2026-10-26 读完：issue #21、路线图 M1.6、001 拆法、003 F1/F2、005 N3、007、cli.md、actions.md、config-privacy.md、content-tree 第九节、style message/service

## 规约对照发现（已报 / 待报大总管）
1. 标已读：契约、路线图、actions.md 一致——CLI history / show 永不标已读，**无冲突**
2. history 参数形状：cli.md 是 `history CHAT_ID [-n N] [--from MSG_ID] [--local]`、「从新到旧、从最新开始」；交代与路线图要 F2 落点（第一条未读）+ 往上 / 往下翻 + `--detail` / `--full`——契约里没有往下翻、没有 detail / full，要改契约（待报）
3. 「详情」怎么说：style 稿只定了哪些东西进详情（R3–R5），**没有详情的具体说法**；我起草，快照为正本，待人实听

## 大总管批复（2026-10-26，授权改契约，修订记录写「大总管授权 2026-10-26，属技术细节」）
- H1 同意建议；H4 同意（`hidden` 为真时 text / url 也遮住，`--reveal` 才给）；H5 同意（只 force=false）
- H2 改：`--full` 出全文，隐藏内容仍读「隐藏内容」；另加 `--reveal`（= 人按了 D）才读原文。我的落法：`speech.full` = 正文原样（保留换行、末尾标点，不接发送人与钟点；附件照 brief 的说法 + 说明文字全文）
- H3 详情句子是读法、要人定：快照先照草稿，标「待人实听」，列进「要人定的事」

## 契约改动草案（原稿，见上方批复）
**H1 history 的参数与顺序**（cli.md 现为「从最新起、从新到旧、`--from` 往旧翻」，没有 F2 落点、往新翻、详情、全文）
- 建议：`history CHAT_ID [-n N] [--before MSG_ID | --after MSG_ID] [--detail] [--full] [--local]`
  - 都不给：按 F2 落点——有未读就从第一条未读（`last_read_inbox` 之后第一条别人发的）起往新取 N 条；没有未读取最新 N 条
  - `--before X`：比 X 旧的 N 条（不含 X，= 原 `--from`，`--from` 留作别名）；`--after X`：比 X 新的 N 条（不含 X）
  - 输出**从旧到新**（阅读顺序；落点在第一行）。⚠️ 与契约「从新到旧」相反
  - `result.data`：`count` · `more` / `next_cursor`（往旧，游标 = 本页最旧 id，同契约原义）· 新增 `more_newer` / `newer_cursor`（本页最新 id）· `first_unread`（落点那条 id，无未读 `null`）· `landing`（`first_unread` / `latest` / `before` / `after`）
- 备选：保持从新到旧，落点放最后一行——读的一方要倒着读，不建议
**H2 `--full` 是什么**：R2 定了 brief 不截断，「全文」已无截断可展开。建议：`--full` 给 `speech.full`＝隐藏内容（剧透、18+ 遮挡）照读出原文的那一句；不加时隐藏内容只读「隐藏内容」（R1）
**H3 `--detail` 的说法**：style 稿只定了哪些进详情（R3–R5），没有句子。我起草放 `speech.detail`，快照为正本、待人实听：「日期 10月26日；回复周大山：〈引用〉；转发自示例频道，原发于 10月25日 19:30；已编辑于 19:40；回应 3 个👍（含我）、1 个❤️；话题 〈名字〉；已读」——有哪项说哪项，逗号 / 分号分段
**H4 `links` 的输出行**：外壳 `kind` 表里没有链接这种。建议新增 `kind:"link"`，`data`：`{index, text, url, hidden}`（`hidden`：链接在剧透里）+ `result{count}`；`links CHAT_ID MSG_ID` 照契约
**H5 `saved`**：会话 id 处写 `saved` 用自己的收藏夹（契约已有），实现用 `my_id` + `createPrivateChat(force=false)`（L0，只取不建）

## 已做（2026-10-26 下午）
- atg-core：`Backend` 加 `chat_get` / `history` / `message_get`（真假两份；只读，不调 openChat / viewMessages）· `speech` 加 `detail` / `full` 与 `SpeechCtx.reveal` · `json::value_with` · `text::links_marked`（标剧透）· `Time::local_ymd`
- atg-cli：新 `history.rs`（F2 落点、`--before/--from/--after`、循环补满、从旧到新、`--detail/--full/--reveal/--local`、`links`）· 参数 · `serve` 转发 history / links · `forward` 文档
- 测试：假后端单测 12 条（落点、翻页连续不重、首次短应答、收藏夹、未登录、找不到、链接遮住 / 展开）+ 转发与直跑一致 + 快照（history JSON / 文字、links、core 的 detail / full / reveal）+ 剧透不漏穷举
- 全过：`cargo test --workspace` 277 条（另 `--features tdlib` 也全过）· clippy 两种特性 `-D warnings` 0 · fmt · `check-no-debug.sh`
- 观察：`serve::tests::serve_replaces_stale_socket_and_refuses_busy_profile` 跑了 4 次失败 1 次（「关掉后锁放了」断言），像是旧测试的时序抖动，不是这次改动引入；待查
- 契约已改（大总管授权）：`specs/contracts/cli.md`（history / links 行、`kind:"link"`、history 的 result 字段、修订记录）· `actions.md`（history 动作加 offset、修订记录）
- 收藏夹只读冒烟（2026-10-26，main 档案，只出统计，未弹钥匙串框）：
  - `history saved -n 20 --detail --full`：退出 0，**0 条**，landing latest，more / more_newer 都 false，synced true，result 齐，执行方耗时 847 ms（整次 1.8 s）
  - `history saved --local`：退出 0，0 条，470 ms
  - 0 条的原因：收藏夹是空的——主列表 114 个会话、归档 103 个里都没有收藏夹（Telegram 不列空收藏夹），`chat_id` 解析正确（= 自己的 id）
  - 所以翻页（--before / --after）、links 在真实账号上**没测到**；逻辑由假后端单测覆盖。要真测：人往收藏夹里放几条（手机上转发 / 自己发），或等测试服务器（#35）
- 真实大群只读冒烟（2026-10-26，人允许读一个人指定的大群，只出统计，未弹框）：落点 20 条 first_unread 且在第一行、字段齐、860 ms；--before 两页各 20 条，不重、依次更旧，与一次 -n 40 逐条一致；--after 正好拿回中间那页、再接回落点页；links 抽 3 条：1 / 1 / 9 个，与树里数的一致，hidden 0；这批没有剧透（真实数据上「不漏」未测到）
- 自审第 1 轮（atg-review）：P0 0 · P1 2（config-privacy 转发清单没改；JSON 的 data 树带剧透原文 → 已报大总管待定 A/B）· P2 7 · P3 10
- 大总管定 P1(2) 选 A（2026-10-26）：不加 --reveal 时 data 树里的剧透也遮住。做法：新 `tree::mask`，`json::value` 按 `cx.reveal` 调，所以 history、chats 会话项、speech --fixture 一起生效；chats、speech 加 `--reveal`；`chat_brief_with(c, reveal)`
- 自审修复（P1(1) + P2 + 部分 P3）：config-privacy 转发清单与 tz 规则 · actions 的 chat_get / message_links / history 含不含起点 · cli.md 外壳 speech 加 full、从没读过的理由、first_unread 为 null 的两种情况、next_cursor 交给 --before、history 也等补发、--full 多行、修订记录挪到末尾 · 去掉 WithTz 外包，tz 进 HistoryArgs · 参数检查收进 `history::query` · 注释里的 H1–H5 引用改成「大总管授权」
- 自审第 2 轮：P0 0 · P1 1 · P2 9 · P3 7。已修：P1 翻页停住（TDLib 连着几次只回起点）不再当成到头——`more` 照为真、照给游标，退避重试，`result.data.partial` 标「没拉全」，假后端加 `stall_history` + 测试 · `tree::mask` 改成穷举 match · `links_marked` 与遮挡同一条规则 · 新夹具 `fixtures/speech/hidden.json`（投票题目、回复引用、剧透里的链接、置顶内容、会话项最后一条）+ 按 JSON 形状穷举的不漏测试 · errors.md 登记 `me_unknown`、`td_message_reserialize_failed` · `history` / `links` 遇 `--redact` 报 not_implemented · 契约：18+ 说明文字照出、partial、chats / speech 的 --reveal、tz 为何只带 history · 去掉 H2 / H5 / 选 A 这类查不到出处的编号。未修（P3）：serve 里 chats 的 -n 范围检查没收进 chats_query
- 第 2 轮修复复测时查了 TDLib 1.8.69 源码（`OrderedMessage.cpp` `get_history`、`MessagesManager.cpp` `get_dialog_history`）：`offset` 为 0 **不含** `from` 那条；内存里没有更早的时 TDLib 自己去本地库 / 服务器拉，拉完还空才回空 → 空应答 = 真到头，只回起点 = 更早的还在后台拉。据此：假后端改成 TDLib 的切法；往新翻请求量按 TDLib「起点算进 offset」修正；actions.md 我上一轮写错的「含 from」改正
- 复测：`cargo test --workspace` 283 条（`--features tdlib` 也 283）· clippy 两种特性强制重查 0 · fmt · check-no-debug；新快照 `snapshots__hidden_everywhere`（hidden.json 的 5 段剧透与 1 个隐藏地址在 JSON 里 0 处出现，--reveal 读法照出）；旧快照只有上一轮核过的 6 处 spoiler 变空
- 自审第 3 轮：P0 0 · P1 0（连续第 1 轮）· P2 6 · P3 13。已修 P2：chats 遇 --redact 报 not_implemented · 停住的说法与重试次数 / 间隔 · links 行补隐藏范围 / 不分页 / --redact · actions 的 message_links 标为组合、history 登记 invalid_input · content-tree 第七节与文首 · errors.md 写清 history 里丢一条。顺手修的 P3：chats --reveal 含 18+ · Global 注释 · 「存下来的树」说清只在内存。没修的 P3：同一条遮挡规则在 cli.md 几行里各说一遍（已加指针，没删）、content-tree 修订记录原有的日期次序
- 自审第 4 轮（只审第 3 轮后的改动）：P0 0 · P1 1 · P2 4 · P3 10。P1：停住且空页时代码给 more_newer:false，与第 3 轮写的契约「照为真」矛盾 → 改契约（空页没游标可给，more 为假，只看 partial），测试改严。P2：chats / speech 的 --reveal 说法指向 content-tree 第八节（不再说 18+ 在 data 里也遮）· links result 字段对齐 · td_message_reserialize_failed 在 history 里整条报错不再丢一条 · （重复的一条同上）。P3 修了：partial / newer 注释、测试改名 redact_refused_until_done、errors / content-tree 修订记录、link 表 hidden 范围、--redact 帮助、message_links 说法、「data 的遮挡」措辞。未修 P3：「读法上下文」「18+」「R1」未进 content-tree 术语表
- 自审第 5 轮（只审第 4 轮后的改动）：P0 0 · P1 0（第 4 轮有 P1，这是连续第 1 轮）· P2 2（文件头、测试注释没随第 4 轮改）· P3 9。已修 P2 与便宜的 P3（td.rs 注释、--reveal 帮助统一并指向第八节、chats 行只留指针、往旧也可能停住、actions 修订记录）；只动注释与文档。第 6 轮已起
- 为什么「18+ 在 data 里不遮」（对 H2）：内容树本来就**不带媒体本身**——图片、视频节点只有 `spoiler` 标记和时长，没有画面、文件、地址（`specs/content-tree.md` 第三节第 3 小节），所以 data 里没有可遮的 18+ 内容；18+ 标记落在 `content.sensitive` 上照出，读法据此读「图片，隐藏内容」。说明文字（caption）不算被遮的部分：读法（R1 现行实现、例 M14「图片，隐藏内容，山顶的风景」）本来就照读说明文字，data 与读法保持一致。若人认为 18+ 消息的说明文字也该遮，是一道新的读法题（R1 的范围），要人定，我没自己改
- 给大总管：H1–H5 批复只记在本现状页，正本决定记录（docs/plan/）我没权限写，请收进去
- 抖动的旧测试：`serve::tests::serve_replaces_stale_socket_and_refuses_busy_profile`，断言「关掉后锁放了」（`src/atg-cli/src/serve.rs` 该测试末尾 `p.probe_lock() == Ok(())`）。复现：`for i in $(seq 20); do cargo test -p atg-cli serve_replaces_stale_socket 2>&1 | grep -E "test result"; done`；本机 4 次里 1 次失败（与其他 serve 测试并行跑时）。疑是 `serve` 返回后档案锁释放与断言之间的时序
- 自审第 6 轮（只审第 5 轮后的改动）：P0 0 · P1 0（与第 5 轮连续两轮无 P0/P1，达停止线）· P2 1 已修：落点补拉那次翻到了最新，却仍报 `partial: true`——改成补拉的结论说了算（`stalled = s2`）；假后端加 `cap_history` / `stall_history_after`，新测试 `landing_refill_that_reaches_the_end_is_not_partial`；顺带文档：只有停住的那一侧 more 为真、「起点」「进内存」的说法、cli.md 修订记录、links 帮助指向第八节
- 终测（2026-10-26，领名额后）：`cargo test --workspace` **284** 条全过 · `--features tdlib` 也 284 全过 · clippy 默认与 tdlib 两种特性强制重查 `-D warnings` 0 · fmt · `check-no-debug.sh` 过。名额已还。正式回报已发大总管，等验收

## 下一步（旧）
先写不受 H1–H5 影响的：Backend 加 `chat_get` / `history` / `message_get`（真假两份）→ store 投影 → CLI 翻页循环 → 读法 detail / full → links → serve 转发 → 单测快照

## 要人定的事
1. 详情（`--detail`）的句子（H3，待人实听）：题面 = 详情里各项怎么说、什么顺序；选项 = 现草稿 / 人改；代价 = 改句子只动 speech 与快照；建议 = 先听草稿再改

## 待 codex 补审（#32）
- 本块按临时例外只过多视角自审（6 轮，第 5、6 轮无 P0/P1），codex 恢复后补审
