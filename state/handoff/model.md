# 交接 · 手下 model · issue #15（第一刀第 2 块：model + reducer + 内容树投影）

> 2026-10-25 · 交代 `_tmp/20261025-b7/model.md` · 状态：**终版——做完，codex 十三轮收敛，可收尾**（未提交；git 写操作归大总管。注意：`store.rs → store/mod.rs` 的改名被我误用 `git mv` 暂存了，大总管已知、提交时一并处理）

## 一句话

`atg-core` 有了 model、reducer（`store`）和投影（`project`）；原型夹具改走「夹具 → 事件 → reducer → 投影」；拍板 006 的 12 条已落地（代码 + 规约）；115 个测试、clippy（默认 + `tdlib-api`）、fmt、`check-no-debug.sh`（含自测）全过。codex 共十三轮、提 43 条：37 条已修（每条带回归测试）、1 条转人定（已由 006 定并实现）、5 条转 issue（#28 的 2 条已在增量中修掉；第十三轮 3 条脚本 P2 见 issue）。前七轮第五至七轮、增量第十二至十三轮各连续无 P0/P1，收敛。

## 改了哪些文件（`src/atg-core/` 为主；另有大总管授权的规约与脚本，见文件表末与「拍板 006 落地」）

| 文件 | 改了什么 |
|---|---|
| `src/model.rs` | 新写：`User` `Chat` `Message`（正文用 `Body = Content \| Service(Action)`，另存 `ephemeral`、`restriction`、`sensitive`）、`SenderId` `OriginId` `ReplyRef` `ChatPosition` `ChatList` `NotifyScope`、事件 `CoreEvent`；`Message::enforce`（受限换节点、不留原文）、`effective`（有临时内容就用它）、`mark_opened` |
| `src/store/mod.rs`（原 `src/store.rs`） | 新写：`Store` + `Input`（`Update{seq,event}` / `OpenChat` / `Refetch` / `HistoryLoaded`）+ `reduce` 纯函数；查询 `chat_list` `next_unread_chat` `stable_key` `ticket` 等 |
| `src/store/table.rs` | 新建：内容的唯一写入口（消息表、孤儿补丁、受限登记、换 id 链；落盘前统一清理） |
| `scripts/check-no-debug.sh` | （大总管授权）扫描 d 改为按 Rust 词法逐字符（字符串、原始字符串、字符字面量、行注释、嵌套块注释）；测试模块只认内联 `mod X {`、逐字符跳到花括号配平，同行闭合后照扫；`mod X;` 声明不跳；`LC_ALL=C`；awk 失败即脚本失败；每次运行先自测（11 必报 + 4 必不报 + 多文件） |
| `src/project.rs` | 新建：`message` `window` `chat_list` `chat_item`——名字、已读、有效静音现取 |
| `src/tdjson.rs` | 重写：TDLib JSON → `CoreEvent`；`World` 改成「事件喂 `Store`，`docs()` 时才投影」，对外接口不变 |
| `src/tree/mod.rs` | `Tombstone` 加可选 `sender` `date`；`UserStatus` 加 `Stopping` |
| `src/speech.rs` | **只加了编译必需的一支** `Stopping => "正在退出"`（说法交 #19），其余没动 |
| `src/redact.rs` | 新建：含文字的 30 种类型（含表情回应）的 `Debug` 只打印 `类型 { <redacted> }`（本 crate 单元测试里仍是完整 Debug） |
| `src/tree/text.rs` | 只改 `EntityKind`、`LinkRef` 的派生（脱敏） |
| `src/lib.rs` | 挂上 `project`、`redact`，模块说明更新 |
| `tests/reducer.rs` | 新建：77 个假事件单测 |
| `tests/privacy.rs` | 新建：暗号测试——每种事件每个文字字段（含表情回应、限制原因）塞暗号，事件、输入、整个 Store、投影的 `{:?}` 里零出现 |
| `src/store/mod.rs` 末尾 `#[cfg(test)] mod tests` | 库内单测：穷举 15 条写入路径的受限原文测试（完整 Debug dump 里不得有暗号）+ 对照组 |
| `tests/fixtures/basic.json` | 第 60 行 1006 的副本与第 16 行对齐（原来少编辑时间和回应） |
| `tests/snapshots/*` | 两个改动，见下 |

## 合并规则怎么落的

- **版本**：两样分开存。整条快照的版本 `(序号, 请求编号)`：随更新到达的整条记 `(seq, 0)`，应答记 `(since_seq, request)`，同一序号下后发的请求赢（`ticket` 每张编号加一，所以是 `&mut self`）；另记每个字段最后被更新的序号。应答到达：快照比 store 旧 → 整条不收；否则以应答为底，请求之后被更新过的字段取 store 的
- **已听**：只改原内容，临时内容不动（同 TDLib `MessagesManager.cpp:9300-9312`）
- **编辑时间**：0 一律存成「没有」（整条消息、字段更新、孤儿补丁三个入口一致）
- **提及数**：`updateChatUnreadMentionCount` 与 `updateMessageMentionRead` 都改它
- **代次**：`OpenChat` / `Refetch` 加一；旧代次的应答只进缓存、不进窗口
- **孤儿补丁**：更新先于消息到达时按字段（各带序号）记下，消息随应答进来时只补比请求新的字段；最多 4096 条。挤掉过就记水位：早于水位的请求里没见过的消息不收，窗口标「要重拉」（`needs_refetch()`）
- **撤回**：只认 `is_permanent && !from_cache`，且消息不是「待发送 / 发送失败」。墓碑记撤回前的 `sender` `date`；没见过原消息的墓碑待判定：后到的应答说它没发出去 → 拿掉占位，否则补记这两项（原文不留）。墓碑不会被重复更新、应答、换 id 复活
- **换 id**：`SendSucceeded` / `SendFailed` 都换；窗口原位、会话最后一条跟着换；`stable_key` 沿「新 id → 上一个 id」的链走到最初的 id（换 id、重发都记链，晚到的关联也传得下去）；旧 id 记进 `retired`，晚到的应答、整条消息、字段更新里的旧 id 都不会复活；回复对象、置顶对象投影时按 `current_id` 换成新 id
- **重发**（TDLib 实际做法，已核 `resources/td/td/telegram/MessagesManager.cpp:25221-25223`）：先对失败那条发一次永久删除，再来一条新的待发送消息 → 删除不当撤回；新旧对应由 `Input::Resent{old,new}`（`resendMessages` 的应答）给，新消息沿用旧的稳定键。不凭相邻事件猜
- **重复到达**：`seq` 不比已见过的大就丢（所以序号在一个 store 的生命里只许递增）；随更新到达的整条消息 = 那个序号的完整状态，整条替换
- **受限**：限制原因非空 → 原内容、临时内容（含系统消息）整块换成受限节点
- **只随「最后一条」到达的消息**（`td_api.tl:11312`）：会话正开着就进窗口
- **会话位置**：`updateChatPosition` 换单个列表；`updateChatLastMessage`、`updateChatDraftMessage` 整体替换；置顶按调用方给的列表看
- **会话排序**：按 (order, 会话 id) 从大到小，order 为 0 不在列表里

## 快照改了哪些（`v` 不加一）

1. `snapshots__basic_messages_json.snap`：撤回那条的 `tombstone` 多出 `sender`（林小雨）和 `date`（`2026-10-23T11:37:00Z` = 东八区 19:37）——coord 清单里的那项
2. `snapshots__basic_chats_json.snap`：**清单外，大总管已确认接受**。林小雨会话项的最后一条（1006）`tail` 从 `{}` 变成带 `edited` + 3 个回应。原因：夹具里 `updateChatLastMessage` 带的那份 1006 少了编辑时间和回应，和前面 `updateNewMessage` 的那份不一致；现在 store 只存一份，会话项投影的是维护过的那份。夹具没改
3. 所有 `brief` 句子一个字没变

## 测试

atg-core 共 115 个：库内单测 27（内容树 25 + store 穷举 2）· coverage 1 · 暗号 1 · reducer 77 · 快照 9。涉及隐私的测试都做过变异检验（去掉清理 / 换回派生 Debug，测试即失败）。覆盖乱序（字段更新 6 种排列、两份应答倒序、撤回与应答先后、换 id 与应答先后）、重复（同序号、整条消息、会话、换 id）、晚到旧应答、撤回 vs 清缓存 / 非永久 / 取消 / 重发、换 id、有效内容、受限内容、孤儿补丁挤出、夹具换 id。

## 审查记录

- 全部记录：`docs/reviews/005_model-reducer-codex.md`（十三轮逐条：发现 · 核实 · 处理 · 测试名）；`docs/reviews/README.md` 一行汇总
- 最初：两路对抗自审（老手挑刺 · 核对员与规矩）+ codex 跨谱系一轮（`codex exec -s read-only`），发现的都已修并补了回归测试：
  - 自审：换 id 后晚到应答带回临时 id（P1）· 墓碑发送人取决于先后 · 重发被当撤回 · 夹具丢换 id 的消息 · 孤儿被挤后永远不收 · 换 id 时旧 id 没见过不进窗口 · `SendFailed` 缺能否重发 · `stopping` 没测试
  - codex：整条消息重复到达不记版本 · 同序号旧代次覆盖新的 · 孤儿补丁共用一个序号 · 重复换 id 复活已撤回的目标
- codex 记录（大总管授权写）：`docs/reviews/005_model-reducer-codex.md`，`docs/reviews/README.md` 加了一行。第一轮（medium）提出 5 采纳 4；第二轮（high）提出 10 采纳 10；第三轮提出 4 采纳 4；第四轮提出 3 采纳 3；第五轮提出 2 采纳 2（无 P0/P1）；第六轮提出 3 采纳 3（无 P0/P1）；第七轮提出 2 P2 转 #28（收敛）

## 交给 #19（读法）的清单

- `speech.rs` 里 `Stopping` 的说法「正在退出」是占位（规约：这一态不播报，但名称要一句）
- 撤回占位读法 R7：数据已齐（`Tombstone.sender` / `date`），`speech.rs:33,68` 还是「已撤回的消息」
- coord「代码解冻后要改的快照与代码」里其余各项（R1「按 D 展开」、R2 去掉 `LAST_MAX_CHARS` 截断）原样未动

## 交给 #16（接 TDLib）的

- tdlib-rs → `CoreEvent` 的映射照 `tdjson.rs::event` 写；`updateChatLastMessage` 的 `positions` 一律给 `Some`（夹具缺字段才用 `None`）
- `SendFailed.td_error_code` 是 TDLib 原码，换成 `errors.md` 机读码归 `map_td_error`
- 序号：一个 store 的生命里只许递增；重建 TDLib 客户端 / 重新登录时连 store 一起新建
- 合成三线状态时可能出 `Stopping`，store 原样存
- `resendMessages` 的应答逐条喂 `Input::Resent{old, new}`（TDLib 对不能重发的那条返回 null，跳过）
- 每处理完一批事件看一眼 `Store::needs_refetch()`，为真就发 `Input::Refetch` 并重拉当前会话历史
- `updateChatDraftMessage` 映射成 `ChatDraftChanged`（只要位置）

## 没做 / 留着的

- `json.rs` 外壳与 `atg-cli/src/out.rs` 重复（skeleton 转交的 P1）：要改 cli 那边，不在我范围，没动
- ~~缓存不设上限~~：已按 006 甲1 设上限（见「拍板 006 落地」）
- 工作区里 `.42cog/real.md`、两份 exp 试用记录、`docs/plan/003_decisions-b2.md`、`state/coord.md`、`state/handoff/skills.md`、`CLAUDE.md` 有改动，不是我的，开 PR 时别混进 #15

## 拍板 006 落地（`docs/plan/006_decisions-b7.md`）

**代码**（atg-core 105 个测试全过：树 25 · coverage 1 · 暗号 1 · reducer 69 · 快照 9；clippy 默认 + `tdlib-api`、fmt、check-no-debug 全过；快照未变）：

- **甲1 缓存上限**：`StoreConfig { keep_per_chat }`，默认 50（理由写在 `store.rs` 的 `Default` 上：一屏十几条、首拉一屏多，50 够切回去有一屏可读；500 会话 × 50 条约 25–50 MB），`Store::with_config` 可改。非当前会话超出就从最旧的删；最后一条、墓碑、换 id 链不删；当前会话不限，切走时再删。删过东西的会话记一个水位（**按会话记**，上限 = 会话数；孤儿补丁被挤掉也记这个），早于水位的请求里 store 没有的消息不收、当前窗口标「要重拉」。测试 `cache_cap_keeps_recent_last_and_tombstones`、`pruned_message_not_restored_by_old_history`
- **#28 之一**（淘汰屏障被新补丁绕过）：水位判定不再看「有没有补丁」，新补丁解不开；补丁留给重拉。测试 `evicted_then_new_partial_patch_still_blocks_old_history`（两种先后正文一致）
- **#28 之二**（64 跳上限）：`stable_key` 走到真正的根，用走过的集合判环。测试 `long_resend_chain_keeps_first_key`（40 次重发、80 条边）
- **甲3 撤回占位保留**：关窗口时把窗口里的撤回占位按会话记下，重拉、切走再回来都带回窗口（`Window.carried`）；投影只显示不早于已加载的最旧消息的那些，免得孤零零挂在顶上；历史应答里有它就转成正常窗口成员。测试 `tombstones_survive_refetch_and_reopen`
- **乙11 引用清空**：投影时被回复那条在 store 里且受限，`quote` 不出。测试 `quote_cleared_when_reply_target_restricted`
- 新 API：`StoreConfig`、`Store::with_config`、`Window.carried`、`Window.loaded_from`、`Store::is_restricted`

**规约**（大总管授权，各加修订记录）：

- `specs/contracts/actions.md` 第一节第 7 小节：写明 `CoreEvent` 即事件表（乙6）；补 `message_ephemeral_changed`、`chat_draft_changed`、`chat_mention_count_changed`；`send_succeeded` / `send_failed` 带整条消息；`messages_deleted` 补未发出的不是撤回、没见过的待应答判定（乙7）；`message_edited` 补编辑时间来源与 0（乙8）；`message_content_opened` 只改原内容；`chat_last_message_changed` 补新消息可只随它到达；末尾补「喂给 store 的应答」（历史、`Resent`）
- `specs/contracts/config-privacy.md` 验收判据第一条：类型级脱敏 + 暗号测试（甲2）
- `specs/content-tree.md`：第二节投影函数改成实际四个（乙9）；第八节补加新取值不加一（乙10）；第三节第 2 小节补被回复那条受限则引用不出、回复对象跟随换 id（乙11）；乙12 记一笔
- `docs/plan/001_arch-stack.md` 第一节第 3 小节第 6 条：编辑时间来源（乙8）

**增量评审第八轮**（3 条，全修）：受限状态单独记（`restricted` / `Store::is_restricted`），回复引用存的时候就清、受限消息后到也回扫；删旧屏障改为 `(序号, 编号)`；窗口加 `loaded`，空历史时带来的占位也显示；`retired` 查找去掉 16 步上限。atg-core 110 个测试全过

**增量评审第九轮**（2 条，全修）：限制登记从消息表写入拆出（`note_message`，墓碑、拒收、换 id 各路径都过）；带来的撤回占位在应答到达时一次性转正（不再按投影时的窗口活算），去掉 `Window.loaded`。atg-core 112 个测试全过

**增量评审第十轮**（2 条，结构性修法）：store 拆成 `src/store/mod.rs` + `src/store/table.rs`——消息表、孤儿补丁、受限登记、换 id 链只有 `table` 能碰，写内容只经 `put_live` / `modify_live` / `add_orphan`，落盘前统一清理；新登记或换到受限消息时全量重清；库内穷举写入路径测试（15 条路径 + 对照组 + 变异检验）。撤回占位改由 `Window::loaded_from`（只由应答推进的加载边界）判定。atg-core 115 个测试全过

**增量评审第十一轮**（2 条，全修）：整个 `Table` 类型级脱敏（限制原因也是内容），暗号测试加限制原因；`check-no-debug.sh` 测试模块只跳到花括号配平、剥块注释，自测扩到 8 条。两处都做了变异检验

**增量评审第十二轮**（无 P0/P1；2 条 P2 都在脚本，全修）：扫描改按 Rust 词法逐字符；同行闭合后照扫；当轮自己抓到并修掉两处——macOS awk 多字节报错（改 `LC_ALL=C`）、`xargs LC_ALL=C` 让实扫静默失败（改直接调 awk，awk 失败即脚本失败，并用注入副本验证）

**#28**：两条都已在本次修掉，可随 #15 的 PR 关（`Closes #28` 由大总管决定加不加）

## 要人定的事（终版）—— ✅ 已全部拍板（2026-10-25，`docs/plan/006_decisions-b7.md`），题面保留作依据

### 甲、行为取舍

1. **缓存上限**（codex 第一轮 P2，消息表、版本表、换 id 链只增不减）
   - A. 先不设上限，接 TDLib 后用真实账号量内存再定 ← **建议**：现在只有夹具，定数字没依据
   - B. 现在就按会话设上限：非当前会话只留最近 N 条，墓碑与换 id 标记另留（淘汰时会动到 #28 的水位逻辑，一起做更稳）
2. **脱敏做法**（与 `config-privacy.md:89` 不一致）
   - A. 保持类型级脱敏：含文字的 30 种类型 `{:?}` 只打 `类型 { <redacted> }`，JSON 形状不动，暗号测试守着 ← **建议**：已满足验收判据「`{:?}` 整个消息也漏不出内容」，改动小
   - B. 改成规约写的字段级包装、打印 `<redacted len=N>`：多了长度信息，但每个文字字段换类型，`tree` / `tdjson` / `speech` 大面积改，JSON 还要加透明序列化
   - 选 A 的话，规约那句改成「类型级脱敏 + 暗号测试」
3. **重新打开 / 重拉同一会话时，窗口里的撤回占位留不留**
   - A. 清掉（现做法）：窗口重建，服务器历史里已没有撤回的那条，占位消失
   - B. 保留原位占位 ← **建议**：拍板 7 定过「选中项被撤回原位留占位、焦点不动」，重拉后占位突然没了，读屏会跳；代价约十几行 + 测试
4. **清缓存 / 非永久删除时，当前窗口里那条拿不拿掉**
   - A. 拿掉（现做法，不是撤回、不留占位） ← **建议**先这样，GUI 阶段开着 VoiceOver 实测
   - B. 窗口里保留旧内容，直到重拉
5. **缓存淘汰触发「要重拉」的处理**（第六轮新加 `needs_refetch()`）
   - A. 交给 #16 的接线层：每批事件后查一次，为真就 `Refetch` 并重拉 ← **建议**
   - B. store 自己产出一个「请重拉」事件给 notify / GUI（更显式，但 store 要多一种输出）

### 乙、规约补丁（实现已按左边做，规约要不要跟着改；规约我没动）

6. **`actions.md` 第一节第 7 小节事件表缺项**：代码有、表里没有的——临时内容变化、内容已打开、提及数（含 `updateMessageMentionRead`）、会话通知、范围通知、草稿位置（`ChatDraftChanged`）、用户、用户状态；`send_succeeded` 表里只写 id，代码带整条消息；另有不是事件的应答输入 `Resent{old,new}`
   - A. 补进表里，`CoreEvent` 算对外事件 ← **建议**（表自称唯一登记处）
   - B. 声明 `CoreEvent` 是 store 内部输入，对外事件表另成一套
7. **撤回判定补一句**：「待发送 / 发送失败的消息被删不是撤回；没见过原消息的删除待应答判定」（TDLib 重发、取消都会发永久删除，`MessagesManager.cpp:25221-25223`）
   - A. 补进 `actions.md` 事件 `messages_deleted` ← **建议**
   - B. 不补，只留在代码注释里
8. **编辑时间来源**（`001_arch-stack.md` 第一节第 3 小节第 6 条「只认 `updateMessageEdited`」）：整条消息（新消息、历史）自带的 `edit_date` 也会写进来；0 一律当「没有」
   - A. 规约改成「字段更新只认 `updateMessageEdited`；整条消息自带的照收；0 = 没有」 ← **建议**
   - B. 维持原句（与实现不符）
9. **投影函数形状**（`content-tree.md` 第二节写一个 `project(&model,&store)->Doc`）：实现是 `message` / `window` / `chat_list` / `chat_item(store, id, list)` 四个
   - A. 规约照实现改 ← **建议**
   - B. 实现再包一个总入口去对齐规约（没有调用方需要）
10. **版本规则没写「给已有字段加新取值」**（`content-tree.md` 第八节）：`status` 多了 `stopping`
    - A. 补一句：给枚举值字段加新取值不加一，使用方遇到不认识的值按「未知」处理、读 `speech.brief` 兜底 ← **建议**（与「新增节点种类不加一」同理）
    - B. 算破坏性变更、`v` 加一
11. **回复里的引用可能带受限原文**：被回复那条受限时，回复消息的 `quote` 里仍是原文
    - A. 投影时若被回复那条在 store 里且受限，`quote` 清空 ← **建议**（与「受限不留原文」一致；改动小，可并进 #28 或 #16）
    - B. 不清：引用是回复者自己附上的，算回复者的内容
12. **受限规则覆盖系统消息**：第二轮已按规约改成系统消息也换受限节点，规约无需改（记一笔，已不冲突）

### 丙、交给别的 issue 的（不用人定，列出备查）

- #19 读法：`Stopping` 的说法「正在退出」是占位；撤回占位 R7 读法（数据已齐）；coord 里 R1「按 D 展开」、R2 去截断
- #16 接 TDLib：见上「交给 #16」一节，含 `Resent`、`ChatDraftChanged`、`updateMessageMentionRead`、`needs_refetch()`、序号只增
- `json.rs` 外壳与 `atg-cli/src/out.rs` 重复（skeleton 转交的 P1），要动 cli，不在本块

## 终版小结（给大总管 / 下一棒）

- **可以开 PR**：`Closes #15`；#28 两条已修，可一并 `Closes #28`（大总管定）。第十三轮 3 条脚本 P2 见 issue
- **暂存区**：只有我误 `git mv` 留下的 `store.rs → store/mod.rs` 改名，内容无害，提交时一并处理
- **本块之外动过的**（都经大总管授权）：`specs/contracts/actions.md` · `specs/contracts/config-privacy.md` · `specs/content-tree.md` · `docs/plan/001_arch-stack.md`（各加修订记录）· `scripts/check-no-debug.sh` · `docs/reviews/005_model-reducer-codex.md` · `docs/reviews/README.md` 一行
- **交给 #19**：`Stopping` 说法「正在退出」是占位；撤回占位 R7 读法（数据已齐）；R1「按 D 展开」、R2 去截断
- **交给 #16**：映射照 `tdjson.rs::event`；`Resent`、`ChatDraftChanged`、`updateMessageMentionRead`；每批事件后查 `needs_refetch()`；序号在一个 store 生命里只增；`SendFailed.td_error_code` 换机读码
- **经验**（可沉淀，未写进 `state/memory/`，由大总管定）：① 受限 / 隐私这类「每个入口都要守」的规则，逐入口补会连续漏，要收拢成唯一写入口 + 穷举路径测试 + 变异检验；② 检查脚本里的 `|| true` 会把扫描器自己的失败吞成「没问题」，扫描器要「自己失败就失败」并有自测

