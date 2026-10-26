# 003 · 发消息 / 收消息的开源先例（Rust 为主）

> 2026-10-26 · 手下 `prior`（批次 b17）· 状态：**多视角自审进行中**（各轮 P0 均为 0，P1 依次 4·3·3·1·1·1·1·2·1·1·1·1，均已核实并修，见修订记录；按「什么时候算够」应连续两轮无 P0/P1；12 轮后由大总管定停，未达停止线，余项见第八节），待大总管验收；codex 额度恢复后补审（issue #32）
> 项目：给 macOS 上靠 **VoiceOver（读屏）** 用 Telegram 的人做的无障碍客户端，用 Rust 写，经 tdlib-rs（TDLib 的 Rust 绑定，我们维护一份补丁版）接 Telegram 官方库 TDLib；代码分 `atg-core`（内核）、`atg-cli`（命令行，先做通）、`atg-gui`（界面，后做）——所以先找 Rust、TDLib 系的先例。所以本文反复看重「焦点别跳」「读屏能听出状态」——使用者看不见气泡、引用框和图标
> 服务：#52（M2.1 发送闸 + M2.2 `atg-cli send`）、#53（M2.3 `atg-cli watch`）
> 人的意图（2026-10-26 原话）：「在做之前，可以在启动一个系统原型分析，用克隆或者其他方法，看一下类似的发消息或者之类的功能，是否在开源仓库已经有人用rust 实现过 / 然后他们怎么做的，是否有什么我们可以参考」
> 版权：克隆与 `resources/` 只读懂，本文只写思路与出处（路径:行号），不贴代码（`CLAUDE.md` 铁律 3）。克隆在仓库里被 git 忽略的 `_tmp/20261026-b17/clones/`，逐仓笔记在 `_tmp/20261026-b17/notes/`（一次性，不入库）

## 这次要决定的

#52、#53 开工前：哪些做法**直接采用**、哪些**改着用**、哪些**明确不要**；契约要不要因此改；有没有要人拍板的事。

## 两份文件，按这个顺序读

| 文件 | 管什么 |
|---|---|
| `README.md`（本文） | 结论、取舍、术语、对照、逐个项目的出处——本期结论都在这里 |
| `decision.md` | 只装要人拍板的卡（B17-2 已拍板选 A；B17-1 已撤回）；不同于 `docs/research/001_stack/`、`002_features/`，本期的取舍不在它里面 |

## 什么时候算够

派活交代（`_tmp/20261026-b17/prior.md`，一次性）里的完成标准，原文大意：至少 3 个项目真读过代码（实际 8 个 + TDLib 本体）· 每条借鉴有出处 · 多视角自审连续两轮无 P0/P1。超出这些（编译跑先例、实测 TDLib 行为）不在本期，列进第七节「待实测」交给 #52 / #53

## 一、一页结论

> 术语不熟先查第二节（临时 id、凭据、直发模式、reducer……都在那里）。B17-N 是本批（第 17 批派活）待人拍板的编号。

**一句话**：Rust 先例（tgt、Paper Plane、tdlib-rs 示例、grammers）在「发」和「收」两头都很薄——没有一个处理了发送失败更新、话题、按规则重发；我们的契约已经比它们严。真正值得拿的是 **TDLib 自己的机关**（自定发送编号、不清草稿、正在输入自动过期、内置通知判定）和 **Unigram 的「原地换 id」**，外加一串别人踩过的坑。最要紧的新发现是一个坑：**TDLib 会把不合规的回复悄悄去掉、照常当普通消息发出**（发6）。契约与路线图建议改 8 处、其余条目各有去处（见本节末）；**人已拍板 1 件**（`decision.md` B17-2，2026-10-26）：直发模式下同一内容被提交两次——拦下。

### #52 发送闸 + `atg-cli send`

**直接采用**
- **发1** `sendMessage` 用 1.8.69 的 `topic_id: MessageTopic` + `reply_to: inputMessageReplyToMessage`（文档）；**话题取被回复那条的**（Unigram 做法同我们的契约）【TDLib】【Unigram】
- **发2** `inputMessageText.clear_draft = false`（源码）：传 true 会顺手清掉人在手机上的云端草稿（tgt、Paper Plane 都传 true）【TDLib】【tgt】【PaperPlane】
- **发3** **send 的结尾条件是三种**：`send_succeeded` / `send_failed` / **临时 id 出现在 `messages_deleted` 里**——TDLib 注明有些待发消息会被不可恢复地删掉，这时不来失败（文档）【TDLib】。⚠️ 第三种**不等于没发出**（源码）：发送途中待发那条被删（用户删、会话变得不可访问……），服务器的成功应答照样会回来，TDLib 这时才去服务器上删掉它，会话不可访问时干脆放弃删除、消息留在对方那边（`td/telegram/MessagesManager.cpp:27848-27851`、`:8463-8473`）。所以第三种按「**结果未知**」报（退出码 10，不许重发），不按失败报；这时 `pending` 里已没有这条，说明里要写明「待发消息已被删除，是否已送达未知」。同理，**删除待发消息不等于撤销发送**：取消只停本地的网络请求（`td/telegram/MessagesManager.cpp:21330-21345`），请求已发出就照样送达，之后才在服务器上撤回或放弃（见上）——契约里的 `pending_cancel` 只能算「尽力撤销」。不加这一条，碰上时 `send` 会白等 30 秒才报同样的「结果未知」
- **发4** **结果可能比应答先到**：tdlib-rs 的应答和更新走两条路，没有先后保证（我们的补丁没改）【tdlib-rs】；Unigram 专门处理「正式消息先到」【Unigram】。做法：CLI 循环把收到的成功 / 失败按 `(chat_id, 临时 id)` **先存表**，应答回来再对
- **发5** **换 id 原地改**：同一条消息对象换身份、索引重键、界面容器复用，VoiceOver 焦点不丢【Unigram】——与我们的稳定键同一思路，`send_succeeded` 也照此

**改着用**
- **发6** ⚠️ **回复会被 TDLib 悄悄改掉**（源码）：被回复的消息找不到或不能回复时，TDLib 不报错，把这条当**普通消息**发出（在话题里时改成回复话题开头那条）；与目标话题不同也直接去掉回复（`td/telegram/MessagesManager.cpp:21144-21170` 及其后的跨话题分支）【TDLib】。`only_preview`（TDLib 走完会话、权限、内容等校验，返回一条假消息、不发）能把这种改写提前暴露出来【TDLib】。防线三道：① prepare 调一次 `only_preview`，回复对象和话题与请求逐项比对，不一致就拒、在计划里写明原因；② **send 真正交出前紧挨着再调一次 `only_preview` 比对**，不一致 `send_refused`——prepare 到 send 之间最长隔 60 秒、还要等人确认，被回复那条可能被删、被挪话题；③ `send_succeeded` 到了再核一遍，不一致在结尾行标明（事后兜底）。凭据摘要仍按我们交给 TDLib 的请求算，不按预览消息算：预览是 TDLib 改写后的结果（回复可能已被去掉），闸核对的是「人确认的那份请求」和「真正交出的那份请求」是否同一份，拿预览算就核不到交出的东西。②也只是把时间差缩到最小，消不掉；**要实测**预览对回复 / 话题群的返回
- **发7** **`options.sending_id`**（文档）：我们自定一个数，TDLib 原样放回待发消息的状态里【TDLib】。为什么临时 id 不够：「待发那条」的新消息更新可能比带临时 id 的应答先到（第二节第 3 小节第 1 步），那时我们还不知道哪个临时 id 是自己这次的（同一进程可能同时有别的发送，比如界面开着时转发来的 CLI 命令）；有了 `sending_id`，这条更新一到就能认出是我们这次发的。与发4 叠加：发7 认领早到的「待发」新消息，发4 处理早到的成功 / 失败
- **发8** **待发消息会在下次启动时自动续发**（源码）：只有开着消息库才落盘续发【TDLib】；tgt 因此给命令行发送关掉了消息库【tgt】。我们不关库（契约要求列出 / 撤销待发消息），但要把「CLI 没等到结果就退出 → 下次启动续发」写成 M2.1 的实测用例
- **发9** **单个请求要有超时**：`atg-core` 的 `link::guard`（`src/atg-core/src/link.rs:328`）只在收线程故障时放行等待者，单个请求没有超时；tdlib-rs 也没有（其 #22 请求永不返回）【tdlib-rs】。`send` 等结果设 30 秒上限（`cli.md` 已有，`actions.md` 还没写，见建议改契约第 3 条），晚到的应答 / 结果照样喂 reducer，不丢
- **发10** 发送中 / 失败**必须进读法**：Unigram 的消息读屏名里这两态是空分支，读屏用户听不出哪条没发出去【Unigram】→ 内容树 / 读法补这两态（GUI 之前先在 CLI 的 JSON 里有）

**不采用**
- **发11** 自动重试（grammers 默认 I/O 错误重发一次）【grammers】——L2 不自动重试照旧
- **发12** 拿 Ack 当成功、超时也当成功（tgt）【tgt】
- **发13** 吞掉发送错误（tgt、nchat、Paper Plane 只记日志）【tgt】【PaperPlane】【nchat】
- **发14** 换 id = 删旧行 + 插新行（Paper Plane、tg）【PaperPlane】【tg】——界面对象重建，焦点会跳
- **发15** 关掉消息库来躲续发（tgt）【tgt】——待发消息会变得不可见

**待人定**
- **发16** 照搬 nchat 输入框的「同文 5 秒冷却」（PR #562）【nchat】——那是界面层补丁。但它点出的问题我们也有：一次性凭据只防同一张凭据被重放，不防同一意图提交两次；直发模式下闸每次自动开新凭据，双击、界面重复触发、AI 在「结果未知」后重跑，都会发出第二条。人定拦下（`decision.md` B17-2 选 A，2026-10-26），机制由 #52 设计、过评审后再改契约

### #53 `atg-cli watch` + notify v0

**直接采用**
- **收1** **有效通知设置按「话题 → 会话 → 三档范围」逐层取**：论坛群的每个话题有自己的一份通知设置（`td_api.tl:4433` `forumTopic.notification_settings`，经 `updateForumTopic` `:11484` 推来），设成「跟随默认」时回落到所属会话（`:3795` 注释「或所属论坛会话」）；会话再「跟随默认」时取私聊 / 群 / 频道对应那一档【Unigram】。读屏用户在热闹的论坛群里静音某个话题是常见用法，漏掉话题这一层就会被他关掉的话题打断。只看会话自己的静音秒数是 nchat 开着的 bug #591、tg 的 TODO【nchat】【tg】。同一套取法还管两项（文档 + 源码）：**提及通知**（`disable_mention_notifications`：没关时，静音会话里 @ 你的消息照样提示）和**置顶通知**（`disable_pinned_message_notifications`）【TDLib】——这是用户自己在官方客户端里设、同步到云端的，照读即可，不替用户定（`td_api.tl:3809-3811`，`td/telegram/MessagesManager.cpp:16525-16533,25964-25966`）。我们的 store 目前只存会话的静音两项（`src/atg-core/src/model.rs:212-219`），要补提及、置顶两项，并收话题的通知设置（话题静音存绝对到期时间，见收2）。TDLib 内置判定只看会话一层（`td/telegram/MessagesManager.cpp:16499-16508`），所以收8 的对照组查不出话题层的差异，话题层要单独写用例（第七节）
- **收2** **会话与范围的静音到期不用自己倒计时**（源码）：TDLib 到期会自己推设置更新【TDLib】。⚠️ **话题层例外**：话题的通知设置更新路径里明写「TODO 更新解除静音定时」（`td/telegram/ForumTopicManager.cpp:796`），到期不会再推——store 收到话题设置时要把「还剩几秒」按收到时刻换成**绝对到期时间**存下，判定时和当前时间比。会话层同理只在要显示「静音到几点」时换算
- **收3** **自己发的用 `is_outgoing` 判**，不用「发送者 = 我」（tg 这么判，以频道身份发言时漏掉）【tg】；收藏夹不提示（源码：TDLib 也这么判）【TDLib】
- **收4** **别处已读的不提示**（源码）：消息 id 不大于会话「已读到」位置就不提（TDLib 的判据）【TDLib】；store 已收 `chat_read_inbox` 事件
- **收5** **收到的「正在输入」不用自己设过期**（源码）：TDLib 5.5 秒没续就自己推「取消」，对方发出消息也立即取消【TDLib】；键用 `(chat_id, 话题, 发送者)`，Paper Plane / nchat / tg 都漏了话题或发送者【PaperPlane】【nchat】【tg】

**改着用**
- **收6** **积压护栏**：`sync_done` 前不提示（契约已有；Unigram 在连接状态为「更新中」时也整批丢【Unigram】），外加「超过 1 小时的旧消息不提示」（Unigram）【Unigram】——它也拿消息时间比本机时钟，但留了 1 小时余量，常见的几秒到几分钟时钟偏差判不错；收12 的 nchat 是拿消息时间和「刚连上那一刻」比，余量为零，偏几秒就会误判。TDLib 自己「登录之前的消息不提」那条挡不住积压（第六节）
- **收7** **短窗合并**：Unigram 7 秒内的后续通知只进列表不弹【Unigram】；tgt 刷新 16 ms 节流 + 尾部补发【tgt】→ notify v0 用假时钟测「连发 N 条只播一次 + 计数」
- **收8** **TDLib 内置通知接口作对照**（源码）：打开后 TDLib 已算好静音（含范围）、自己发的、别处已读、@ 穿透静音（Paper Plane、Unigram 全靠它）【TDLib】【PaperPlane】【Unigram】。v0 不拿它当主判据：CLI 永不设「在线」（`actions.md` 的 `set_online`），而本端不在线、手机在线时 TDLib 会把通知**推迟 30 秒**（推迟值由服务器下发，客户端改不了）；它也不管「当前打开的会话」。建议写成测试里的对照组，比两者分歧。其中「@ 提及穿透静音」取决于用户的提及通知设置，见收1
- **收9** **当前会话里来新消息**（UnigramPlus，盲人用户自己写的 NVDA 插件）【UnigramPlus】：它是自动读全文；我们已拍板的是「提示音 + 写进标题与列表名 + 读完当前这条再说汇总 + 按 m 键收听」（001 拍板 3，见第二节词表；M4.3），不照搬自动读全文。可借两条：**只对对方发的出声**；**切会话 / 回到前台先记基线、不补报积压**。CLI 的 `watch` 只输出判定，播报在 M4.3
- **收10** 积压监控：grammers 用有界缓冲、满了丢，结果漏消息【grammers】；我们用无界通道不丢，但要看着积压——收线程最新发出的 `seq` 减去 reducer 已处理到的 `seq`，超过某个数（原型里量）就记一条警告

**不采用**
- **收11** 轮询界面 + 按界面文字关键词判收发（UnigramPlus）【UnigramPlus】
- **收12** 用「消息时间 > 本机连接时刻」判积压（nchat）——时钟偏差会误判【nchat】
- **收13** 多线程共享表 + 锁（nchat 大锁、Unigram、tgt 持锁跨 await）【tgt】【nchat】【Unigram】——收线程和界面互相等锁，处理顺序难保证、容易卡；我们的 reducer 单线程、无锁，按 `seq` 顺序处理
- **收14** 有界缓冲满了悄悄丢（grammers 默认 100 条，#378 / #383 漏消息）【grammers】
- **收15** 新消息一到就强制标已读（tgt）【tgt】——违反契约「CLI 永不标已读」
- **收16** 消息内容拼进 shell 命令发通知（nchat #443、tg #280）【nchat】【tg】——别人发来的文字可能被当成命令在本机执行（「注入」，nchat #443 真出过）；GUI 用 macOS 的无障碍公告接口，文字只当数据

### 每条的去处

- **改契约 / 路线图**：下面 8 条
- **#52 按条目直接实施，不改契约**：发1、发4、发5
- **#53 按条目直接实施，不改契约**：收3
- **#53 改着用、不改契约**：收10（积压警告随 `watch` 一起做，阈值待实测）
- 收2 并入契约建议第 6 条（话题静音存绝对到期时间）
- **#52 写成 M2.1 实测用例**：发8
- **不采用的照不做**：发11–发15、收11–收16
- **写成测试**：收7（notify v0 假时钟测合并）、收8（TDLib 通知接口作对照组）
- **M4.3 播报器**：收9
- **待实测**（第七节）：发3、发6、发8、收1、收2、收8、收10
- **已拍板**（`decision.md`）：发16 → B17-2 选 A，落点 #52

### 建议改契约与路线图（报大总管定，本文不改）

1. `specs/contracts/cli.md`「send」行与 `actions.md` 第一节第 7 小节：结尾条件加「临时 id 出现在 `messages_deleted` 里」→ 立即输出 `error(send_outcome_unknown)`，原因写「待发消息已被删除，是否已送达未知」，退出码 10，**不许重发**（发3）
2. `actions.md` 第一节第 3 小节「发送」：`send_text` 写明 `clear_draft=false`（发2）、带 `sending_id`（发7）；`send_text_prepare` 与 `send_text` 交出前各比对一次 TDLib 预览的回复对象与话题，不一致拒发，`send_succeeded` 后复核（发6）
3. `actions.md` 第一节第 3 小节「发送」：写明 `send_text` 交出后等结果有 30 秒上限，晚到的应答与结果照常进 store（发9）
4. `specs/content-tree.md` 与读法规约：消息节点带发送状态（待发 / 失败），读法要读出来（发10）
5. `actions.md` 事件表 `chat_action` 行：「正在输入」以 `(chat_id, 话题, 发送者)` 为键，过期靠 TDLib 推的取消（收5）
6. `actions.md` 事件表 `chat_notification_changed` / `scope_notification_changed` 两行：除静音外加「提及通知」「置顶通知」两项，并新增话题通知设置的事件（来自 `updateForumTopic`）；有效值都按「话题 → 会话 → 私聊 / 群 / 频道三档」取，话题静音按绝对到期时间判（收1、收2）；`docs/plan/002_roadmap.md` M2.3 的 notify v0 加「别处已读不提示」「超过 1 小时不提示」「提及通知没关时 @ 你的消息穿透静音」（收1、收4、收6）
7. `decision.md` B17-2 已选 A（拦下）：#52 设计机制、过评审后再改契约，落点见该卡（发16）
8. `actions.md`「待发出的消息」段与 `errors.md` 的 `send_outcome_unknown` 行：`pending_cancel` 改称「尽力撤销：请求已发出时可能已送达、随后撤回，会话不可访问时连撤回都没有」，不再写成结果未知时的补救（发3）

大总管采纳 2026-10-26，落点 #52 / #53（第 1、2、3、7、8 条归 #52，第 5、6 条归 #53；第 4 条的读法与第 6 条里「超过 1 小时不提示」这类行为先按本文建议做默认，做完再交人定）。

## 二、术语与记号

### 1. 记号

- 【TDLib】【tdlib-rs】【tgt】【PaperPlane】【grammers】【nchat】【tg】【Unigram】【UnigramPlus】：出处项目，即第四节的九个项目（顺序固定，全文一致）
- **发1**…**发16**：第一节 #52 的条目；**收1**…**收16**：#53 的条目。1–5 直接采用、6–10 改着用、11 起不采用（发16 例外：待人定）
- 出处三分（本目录体例，`docs/research/README.md`）：**官方明载**（`td_api.tl`——TDLib 的接口定义文件，每个函数 / 更新都带官方注释——标「文档」）· **本机逆向所得**（本目录体例的叫法，这里指读公开源码，没有反汇编；读 TDLib 1.8.69 与各项目源码，版本见第四节表；第一节条目里出自 TDLib 源码的标「源码」，其余未标的是读各项目代码所得；来自 issue 讨论的标 issue 号）· **已被推翻的传言**（见第六节；推断、待实测与局限在第七节）
- 按出处分：**官方明载**——发1、发3、发7、收1（提及 / 置顶 / 话题设置）与本节第 3 小节；**本机逆向所得·TDLib 源码**——发2、发3（后半）、发6、发8、收2、收3（收藏夹不提示那句）、收4、收5、收8；**本机逆向所得·各项目代码与 issue**——其余条目与第四节；**已被推翻**——第六节
- TDLib 路径一律相对 `resources/td/`

### 2. 应答、更新、Ack——三个不同的东西

- **应答**：TDLib 对**这一次请求**的直接回复（如 `sendMessage` 立刻回的那条消息）。tdlib-rs 用请求编号（`@extra`）把它送回发请求的那个任务
- **更新**：TDLib **主动推**的事件（新消息、发送成功 / 失败、删除……），不对应某次请求，从收线程的 `receive()` 出来
- **Ack**（`updateMessageSendAcknowledged`）：是一条**更新**，只说「服务器收到了你的请求」，**不是发送成功**
- 应答和更新走两条路，谁先被我们处理到**没有保证**（【tdlib-rs】）

### 3. TDLib 发一条消息要经过哪几步（文档：`td/generate/scheme/td_api.tl:3455-3465,11202-11214`）

1. 调 `sendMessage`，TDLib **立刻**给出应答：一条带**临时 id**、状态「待发」的消息；同时推一条「新消息」更新（这条更新可能比应答先到我们手里）
2. （可选）推 Ack
3. 最终三选一：`updateMessageSendSucceeded`（带正式 id 和整条消息，`old_message_id` 是临时 id）· `updateMessageSendFailed`（也换一个新 id，状态变「失败」，附能否重发等字段）· 待发那条被删掉（`updateDeleteMessages`，不来失败；接口注释没写何时发生，源码查到的情况与后果见发3）
4. **换 id**：界面 / store 里的那条消息，从临时 id 换成正式 id

我们把 TDLib 的更新映射成自己的事件（`specs/contracts/actions.md` 第一节第 7 小节）：成功 → `send_succeeded`、失败 → `send_failed`、`updateDeleteMessages` → `messages_deleted`。

### 4. 我们自己的概念

| 词 | 一句话 | 出处 |
|---|---|---|
| 第一阶段 | 本项目第一批要交的版本（MVP），有些功能先拒、以后再做 | `docs/research/002_features/decision.md` |
| 契约 | `atg-core` 对外动作、CLI 输出的约定 | `specs/contracts/actions.md`、`cli.md`、`errors.md` |
| L0 / L1 / L2 | 对外等级：L0 只读或只影响本机 · L1 别人看得到痕迹但不发内容（已读、正在输入）· L2 对外不可逆（发送、删除），一律过发送闸、**不自动重试** | `actions.md` 第一节第 2 小节 |
| 发送闸 · 凭据 · 摘要 | L2 只能凭一次性凭据发；凭据绑目标 + 我们交给 TDLib 那份请求的 SHA-256 摘要，60 秒、只能用一次 | `actions.md` 第一节第 4 小节 |
| 直发模式 · 测试豁免 | 以 `--unsafe-direct-send` 启动的这次运行，一切 L2 免确认，闸自动开凭据——人定下它是为了让 AI 驱动 CLI 时能不经确认直接发（人已知 AI 也能自己加这个参数） · 测试服务器上发往收藏夹（及第二个自己的测试号），闸同样自动开凭据 | `actions.md` 第一节第 4、5 小节；`cli.md` 选项表 |
| 档案 · 执行方 · 直跑 | 档案 = 一个登录账号在本机的目录（配置、日志、TDLib 数据）· 执行方 = 正占着这个档案、连着 TDLib 的进程（界面或常驻的 `atg-cli serve`），后起的 `atg-cli` 把命令转给它 · 直跑 = 没有执行方时 `atg-cli` 自己占着档案跑完就退 | `config-privacy.md` 第一节第 2 小节；`cli.md`「界面开着时」 |
| `pending` · `direct-send.log` | 列出待发消息（能撤销）的命令 · 直发模式每发一条记一行（只记 id、类型、结果，不记内容） | `cli.md` 命令表；`actions.md` 第一节第 5 小节 |
| prepare | 发送前「只查不发」那一步（`send_text_prepare`），产出给人确认的计划 | `actions.md` 第一节第 3 小节 |
| 结尾行 | `send` 等命令最后输出的那一行 JSON（`result` 或 `error`），调用方据此判断成败 | `cli.md` 命令表 `send` 行 |
| 结果未知 | `send` 30 秒没等到成败 → `error(send_outcome_unknown)`，**不许重发**：消息可能其实已发出，重发对方就收到两条 | `cli.md` 命令表 `send` 行 |
| `send_refused` · `resend` · 退出码 | 闸拒发的错误 · 重发失败消息的命令 · CLI 结束时返回给调用方的数字（0 成功，8 被拒，10 结果未知……） | `errors.md`；`cli.md` 第一节第 4 小节 |
| 收线程 · 无界通道 · reducer · store | TDLib 一个收线程 → 不限长度的队列（无界通道：再多也不丢、不堵收线程；有界的满了要么堵、要么丢）→ 消费方在自己线程上把事件**按顺序**交给 reducer，由它更新唯一一份状态（store）；reducer 不持锁、不等任何请求。每个事件带递增序号 `seq` | `docs/plan/001_arch-stack.md` 第一节第 3 小节；`src/atg-core/src/link.rs`、`pump.rs`、`store/` |
| 稳定键 | store 里一条消息的身份不随换 id 变，焦点按它锚定；「重键」= 索引从旧 id 改挂到新 id。重发时 store 收一条「旧 id → 新 id」的输入（代码里叫 `Input::Resent`） | `actions.md` 第一节第 7 小节末段 |
| 积压 · `sync_done` | 启动 / 重连时 TDLib 一下子补发进来的一批旧更新 · 补发结束的信号（暂定：连接状态到 ready） | `cli.md`「什么时候算数据齐了」 |
| notify v0 | 新消息提不提示的第一版规则 | `docs/plan/002_roadmap.md` M2.3 |
| 内容树 · 读法 · 读屏名 | 一切要读出来或打印出来的东西（消息、状态……）的统一数据结构，读法、CLI 的 JSON、界面都由它生成 · 消息怎么读出来（纯函数）· 界面元素交给读屏的名字 | `specs/content-tree.md`；架构第一节第 2 小节 |
| 拍板编号 | 001 拍板 3：新消息提示 = 提示音 + 写进标题与列表名 + 估算读完再说汇总（按你正在听的那条的朗读时长估算，读完后再说一句「几个会话来了几条」）+ 按 m 键主动收听 · F1：进会话标已读 · A2：在线与正在输入同官方客户端 · F6 / N5：收到「正在输入」怎么播 | `docs/plan/001_arch-stack.md` 第七节；`docs/plan/003_decisions-b2.md`、`docs/plan/005_decisions-b3.md` |
| `atg-cli watch` | 常驻运行、订阅 TDLib 更新，每来一个事件输出一行 JSON（带 notify v0 的「提不提示」判定），Ctrl+C 结束；给 AI 和脚本看「来了什么」，真正出声由以后的界面播报器做 | `cli.md` 命令表 `watch` 行、第一节第 5 小节 |
| 里程碑 | M2.1 发送闸 · M2.2 `send` · M2.3 `watch`（本批 CLI）· M4 GUI 能回（M4.1 输入框、M4.3 新消息提示） | `docs/plan/002_roadmap.md` 第五、七节 |

### 5. Telegram / TDLib / Rust 的几个词

- **收藏夹**：Telegram 里自己跟自己的会话（Saved Messages），发进去只有自己看得到；测试豁免只许发这里
- **基线**：切到一个会话或回到前台时，先记下「当前最后一条」，此前的不播，只播之后新来的
- **话题 · 论坛群 · 话题群**：超级群开了「论坛」后，消息分在一个个话题里；本文的「论坛群」「话题群」指的都是这种群。回复别人时要落在同一个话题
- **焦点**：VoiceOver 当前停在、正在读的那个界面元素。界面对象被删掉重建时，焦点会被拽走，用户读到一半就丢了位置，得重新找
- **临时 id**：TDLib 发出之前先在本机给消息编的号，服务器确认后换成正式号（第 3 小节）
- **以频道身份发言**：群里可以用自己管理的频道名义发消息，这时消息的发送者是那个频道，不是「我」的用户 id
- **NVDA · Orca**：Windows 上 / Linux 上的读屏软件（相当于 macOS 的 VoiceOver）
- **持锁跨 await**：拿着一把锁去等异步结果，别的线程这段时间都进不来，容易卡死
- **范围（scope）**：私聊 / 群 / 频道三档默认通知设置，会话设「跟随默认」时用它。TDLib 里频道和大群都是「超级群」，按它是频道还是群归到频道档或群档
- **消息库**：TDLib 启动参数 `use_message_database`，把消息存进本地数据库；开着时待发消息也落盘，下次启动续发
- **MTProto 与 TDLib**：MTProto 是 Telegram 的底层协议；TDLib 是官方在它上面封好的库（替你管缓存、补漏、去重）。grammers 直接说 MTProto，它踩的坑多是 TDLib 已替我们挡掉的
- **FLOOD_WAIT**：服务器限流，要等若干秒再发
- **tokio / async 执行器**：Rust 的异步任务调度器；在它的任务里调会阻塞的函数，会卡住别的任务
- **`random_id`**：MTProto 发消息时客户端带的随机数，服务器按它去重
- **尾部补发 · 假时钟**：节流时最后一次变化一定补发一次 · 测试里可手动拨的时钟，用来验证「几秒内只播一次」

## 三、对照表

只列六个项目：TDLib 是被比的底座，tdlib-rs 只是把 TDLib 接到 Rust 的绑定、没有自己的收发逻辑，UnigramPlus 是插件，三者不在列里；grammers 虽是库，但自带完整的发送与收更新逻辑，所以入表。

| 关注点 | tgt | Paper Plane | grammers | nchat | tg | Unigram | **我们（契约 / 架构）** |
|---|---|---|---|---|---|---|---|
| 处理发送失败 | 否 | 否 | —（同步请求） | 否 | 否 | 是（只换状态） | 是，`send_failed` 事件 + 结尾行 |
| 换 id 方式 | 不换 | 删旧插新 | — | 当新消息插 | 加新删旧 | **原地改 + 重键** | 稳定键（同 Unigram） |
| 发送结果等到哪 | Ack / 10 秒当成功 | 不等 | 应答 | 只看应答 | 只看应答 | 不等 | 成功 / 失败，30 秒「结果未知」；**建议加「临时 id 被删 → 结果未知」** |
| 重发 | 无 | 无 | 自动重发 I/O | 无 | 无 | 手动，不查能否重发 | 手动 `resend`，查全部失败字段 |
| 话题 | 无 | 无 | 无 | 无 | 无 | 取被回复那条 | 取被回复那条；话题群里不回复任何一条的新消息第一阶段拒发（`actions.md` `send_text_prepare` 行）；**建议核预览** |
| `clear_draft` | true | true | 可选 | — | — | — | **建议 false** |
| 收更新线程 | tokio 任务里阻塞 | 系统线程 + 无界 | 单一所有者的状态机 | 系统线程 + 大锁 | 库线程直接改、无锁 | 系统线程 + 锁 | 系统线程 + 无界 + 单线程 reducer |
| 提醒判据 | 无 | TDLib 通知接口 | 无 | 自算（漏范围） | 自算（漏范围、积压） | TDLib 通知接口 + 4 道自加过滤（见【Unigram】） | 自算 v0；**建议 TDLib 接口作对照** |
| 有效通知设置的取法 | 否 | 仅显示用 | — | 否（#591） | 否 | 会话 → 三档 | 话题 → 会话 → 三档（收1） |
| 正在输入的键 | — | 发送者 | — | 发送者 | 会话 | 会话 + 话题 + 发送者 | 会话 + 话题 + 发送者 |
| 新消息给读屏 | — | — | — | — | — | **不读（#1363）** | 提示音 + 汇总 + 按 m 收听（001 拍板 3，M4.3） |

## 四、九个项目

| 记号 | 项目 · 上游地址 | 语言 · Telegram 层 | 许可证 | 读的版本 | 怎么读 |
|---|---|---|---|---|---|
| 【TDLib】 | TDLib 本体 · github.com/tdlib/td | C++ | BSL-1.0 | 1.8.69 `ea3f69c`（`resources/td`） | prior 自读，核每一条「TDLib 会自己做」 |
| 【tdlib-rs】 | tdlib-rs（上游 + 我们的补丁版）· github.com/FedericoBruzzone/tdlib-rs | Rust | MIT 或 Apache-2.0 | `3bfef94`（v1.4.0）；补丁版 `src/tdlib-rs` | 子代理读代码 + gh 查 issue |
| 【tgt】 | tgt · github.com/FedericoBruzzone/tgt | Rust · tdlib-rs | MIT 或 Apache-2.0 | `ffa4856` | 同上 |
| 【PaperPlane】 | Paper Plane（原 Telegrand）· github.com/paper-plane-developers/paper-plane | Rust + GTK4 · `tdlib` 库 | GPL-3.0 | `ab48a3e`（2024-06，已不活跃） | 同上 |
| 【grammers】 | grammers · codeberg.org/Lonami/grammers（GitHub 有旧镜像 Lonami/grammers） | Rust · 直接说 MTProto | MIT 或 Apache-2.0 | `fa7692e`（读的是仓里 `resources/grammers`） | 同上 |
| 【nchat】 | nchat · github.com/d99kris/nchat | C++ · TDLib | 源码 MIT（二进制 GPL / AGPL） | `e424aea` | 同上 |
| 【tg】 | tg · github.com/paul-nameless/tg | Python · TDLib | Unlicense | `b0eee6f` | 同上 |
| 【Unigram】 | Unigram · github.com/UnigramDev/Unigram | C# · TDLib（Windows，读屏口碑最好的第三方客户端） | GPL-3.0 | `75bc8e8` | 同上 |
| 【UnigramPlus】 | UnigramPlus · github.com/Kostya-Gladkiy/UnigramPlus | Python · NVDA 插件，给 Unigram 补读屏功能 | GPL-2.0 | `1436882` | prior 自读 |

另搜过读屏专用客户端：AppleVis 上有 iOS 的 Accessible Gram（SwiftUI + TDLib），仓库私有，读不到代码。
路径：【tdlib-rs】相对 `resources/tdlib-rs/`；【grammers】相对 `resources/grammers/`；其余相对各自克隆根目录。克隆清掉后，按上表地址 + 提交号重新克隆即可复核。下面各条默认是读代码所得，来自 issue / PR 讨论的标了号。

### 【TDLib】TDLib 1.8.69——这些不用我们自己做（以及一个坑）
- **发送**（文档）：`sendMessage(chat_id, topic_id, reply_to, options, reply_markup, content)`（`td/generate/scheme/td_api.tl:13143`）；`sending_id`、`only_preview`（`:6531,6532`）；待发 / 失败态字段，「`retry_after` 变化不推更新」（`:3455-3465`）；「有些待发消息会被删、不来失败」（`:11210`）
- **发送**（源码）：`only_preview` 走完会话、权限、内容等校验后返回假消息（`td/telegram/MessagesManager.cpp:21664-21738`）；**回复对象不可回复 / 找不到 / 跨话题时返回空回复、照常发送**（`:21144-21170` 及其后跨话题两处 `Ignore reply ... from a different topic`）；`clear_draft=false` 时不清云端草稿（`:28476-28509`）；待发消息只在开消息库时落盘续发（`:21923-21926`）
- **内置通知判定**（源码，默认关：`td/telegram/NotificationManager.h:139`，设 `notification_group_count_max` > 0 才开）：自己发的 / 收藏夹不提（`MessagesManager.cpp:9319-9331`）；已读的不提（`:26684-26685`）；有效静音含范围（`:16499-16508`）；@ 提及走单独一组、静音也提（`:26659` 起）；本设备登录账号之前的消息不提（`:26633`）；推迟规则（`td/telegram/NotificationManager.cpp:841-875`：本端不在线而别的设备在线 → 推迟 30 秒；两端都在线 → 1.5 秒），推迟值是服务器下发的只读选项（`td/telegram/OptionManager.cpp:516-518`）
- **静音到期自动推更新**（话题层没有，`td/telegram/ForumTopicManager.cpp:796` TODO）：会话（`MessagesManager.cpp:6065-6094`）、范围（`td/telegram/NotificationSettingsManager.cpp:782`）
- **正在输入自动过期**：5.5 秒（`td/telegram/DialogActionManager.h:46`，`.cpp:457-483`）；对方发出消息即取消（`MessagesManager.cpp:5528-5537`）

### 【tdlib-rs】tdlib-rs（Rust）
- 应答带 `@extra`（请求编号）进一次性信道，更新从 `receive()` 返回——**两条路，没有先后保证**（`tdlib-rs/src/lib.rs:35-59`）；`send_request` 没有超时（上游 `lib.rs:68`；补丁版 `src/tdlib-rs/tdlib-rs/src/lib.rs:137-145` 未改）
- 示例只有登录 + `get_me`，没有发消息（`tdlib-rs/examples/get_me.rs`）
- issue：#25 在 async 任务里阻塞调 `receive()` 卡死执行器；**#26 在收更新的同一个循环里等请求应答 → 自己等自己**；#22（开着）请求永不返回；#31 回复结构随版本变
- 对我们：收线程是专用系统线程、reducer 不等应答，架构已避开 #25 / #26（`docs/plan/001_arch-stack.md` 第一节第 3 小节）；剩下「应答 / 结果谁先到」和「单个请求无超时」，见发4、发9

### 【tgt】tgt（Rust 终端客户端）——反面清单
- **发**：话题位与发送选项写死空（`src/tg/tg_backend.rs:404`）；`clear_draft: true`（`:400`）；发送成功更新只打日志（`:844-846`），失败没分支；界面发送结果 `let _ =` 丢掉（`src/run.rs:463`）；命令行发送拿 Ack 当成功，全局单槽、10 秒超时也当成功（`src/run.rs:1027-1045`）
- **续发坑**：注释说开着消息库时命令行发送会把删掉的消息再发出去，于是带参数就关库（`src/main.rs:91-94`）——真因见第七节「推断」
- **收**：在 tokio 任务里阻塞 `receive(2.0)`（`src/tg/tg_backend.rs:824-1382`）；收线程直接改一堆互斥锁表；「稍后重放」队列每轮新建，暂存的更新悄悄丢（`:838`）；打开的会话里一来新消息就强制已读（`:1258`）；列表刷新 16 ms 节流 + 尾部补发（`:832-835,1344-1356`）；没有任何提醒逻辑

### 【PaperPlane】Paper Plane（Rust + GTK）
- **收**：一条系统线程循环 `receive()`，经无界通道回 GTK 主线程（`src/model/client_manager.rs:268-283`），但每条更新先另起一个异步任务再进通道，顺序无显式保证（`:278-280`）；状态全在主线程，不加锁
- **提醒**：全交给 TDLib 通知接口（`src/model/client.rs:120-133` 设 5；`src/ui/window.rs:129-212` 展示与撤回），没处理启动时的现存通知全集
- **发**：结果丢掉、话题恒为 0、`clear_draft: true`（`src/ui/session/content/chat_action_bar.rs:450-547`）；换 id = 删旧插新（`src/model/chat.rs:272-283`）；失败不处理且发送状态不可变，失败的永远显示「发送中」（`src/model/message.rs:70`）
- **正在输入**：每次输入变化发、5 秒冷却、从不发取消（`chat_action_bar.rs:647-672`，PR #98：每键一发吃 FLOOD_WAIT）；接收按发送者存、不分话题（`src/model/chat_action_list.rs:102-125`）
- issue / PR：#368 讨论「本机发的消息重启才出现」；#611 切会话草稿丢（推测是异步任务里再读「当前会话」）；#478 Orca（Linux 读屏）报控件无标签

### 【grammers】grammers（Rust，直接说 MTProto）
- **发**：同步请求，应答里就有正式 id，按 `random_id` 对（`grammers-client/src/client/messages.rs:24-91`）；对不上时拿唯一一条凑数（`:76-86`）；重试把同一份请求体原样再发、靠服务端按 `random_id` 去重，默认遇 I/O 错误自动重发一次（`grammers-client/src/client/net.rs:124-165`，`retry_policy.rs:62-82`）
- **收**：一个只归调用方所有的更新状态机，不加锁（`grammers-client/src/client/updates.rs:102-222`）；缺口先等 0.5 秒、15 分钟无更新主动补（`grammers-session/src/message_box/defs.rs:36,49`）；处理后缓冲默认 100 条、多了丢（`grammers-client/src/client/client.rs:97-104`，`updates.rs:246-276`）→ issue #378 / #383 漏消息
- **正在输入**：约 5 秒过期，库每 4 秒续（`grammers-client/src/peer/action.rs:19`，issue #95）
- 对我们：TDLib 已替我们补漏与去重，只要**我们自己不重调 `sendMessage`** 就不会重复——印证「L2 不自动重试」

### 【nchat】nchat（C++）
- **发**：无话题；待发消息在新消息更新里直接丢、界面不显示（`lib/tgchat/src/tgchat.cpp:1943-1945`）；成功当新消息插入，不读旧 id（`:1962-1981`）；失败不处理（`:2359-2362`）；点发送就清输入框，失败时原文丢（PR #562 开着）
- **正在输入（发）**：持续打字每 2.5 秒续、停手 3 秒发取消、换会话先给旧会话发取消（`src/uimodel.cpp:468-545`）——M4.1 发「正在输入」时可直接参考这组数
- **提醒**（`src/uimodel.cpp:2584-2650`）：自己发的不提；归档的不提；@ 提及穿透静音（issue #11）；积压靠「消息时间 > 授权完成时的本机时间」压（`:2606-2607`）；静音只看会话自己的秒数、忽略范围（`lib/tgchat/src/tgchat.cpp:2311-2314`，**issue #591 开着**）；通知命令把正文拼进 shell（issue #443）
- **正在输入（收）**：只认用户发送者，按会话存「谁在输入」的集合（键 = 会话 + 发送者），不看话题、无本地超时（`lib/tgchat/src/tgchat.cpp:1983-2014`，`src/uimodel.cpp:2032-2049`）
- **线程**：TDLib 线程直接拿界面大锁（`src/uimodel.cpp:4862`）

### 【tg】tg（Python）
- **发**：只填会话和内容、同步等应答，失败只写日志（`tg/tdlib.py:79-98`，`tg/models.py:624-630`）；回复用 TDLib 旧字段，新版下已失效（`tg/tdlib.py:118-131`）；换 id = 先按新 id 加、再按旧 id 删（`tg/update_handlers.py:210-221`）；发送失败没有处理器；存消息不去重（`tg/models.py:533-540`）
- **提醒**（`tg/controllers.py:840-865`）：只看会话自己的静音秒数（`:851`，TODO 在 `:850`）；「自己发的」用发送者 id 比（`:855`）；不排除积压；不排除当前会话（issue #170 开着）
- **通知命令**：issue #280 注入，修法是拆参数不经 shell（`tg/utils.py:172-189`）
- **正在输入（收）**：按会话存一份（键 = 会话），不分发送者、无超时（`tg/update_handlers.py:298-305`）
- **版本漂移**：收正在输入听的是旧更新名，新版下收不到（`tg/update_handlers.py:298-305`）→ 我们遇到不认识的更新只记类型名（已有 `unknown_update`），快照测试固定 TDLib 版本

### 【Unigram】Unigram（C#）——最成熟，读屏却有大洞
- **收**：一条专用线程循环 `Receive(300)`，应答按请求号回调（`Telegram/Td/Client.cs:222-247`）；更新在 TDLib 线程上同步改共享缓存（`Telegram/Services/ClientService.cs:3590` 起），多处锁 + 并发字典，再投到界面线程
- **提醒**：开 TDLib 通知接口（`notification_group_count_max=25`，`ClientService.cs:545`），自己只叠几道：连接为「更新中」时整组丢（`Telegram/Services/NotificationsService.cs:490`）；超过 1 小时不展示（`:526`）；窗口在前台且正开着该会话 / 话题时不弹（`:607-648`）；7 秒内的后续通知不弹窗（`:724-736`）。另有一套「有效静音」：跟随默认时按私聊 / 群 / 频道取范围（`Telegram/Services/Settings/NotificationsSettings.cs:47-55,103-127`）
- **读屏**：新消息**不朗读**（issue #1363，2020 年开至今）；正在输入的实时区域（读屏自动读出变化的区域）播报被注释掉（`Telegram/Views/ChatView.xaml.cs:6146-6147`）；消息读屏名里「发送中 / 失败」是空分支（`Telegram/Controls/Messages/MessageBubble.xaml.cs:471-476`）；连接状态用「立即打断」级的实时区域播（`Telegram/Views/MainPage.xaml.cs:840-853`）
- **发**：话题取被回复那条的（`Telegram/ViewModels/Dialogs/MessageComposerHeader.cs:17-41`、`Telegram/ViewModels/ComposeViewModel.cs:984-988`）；换 id **原地改同一对象 + 重键**（`Telegram/ViewModels/DialogViewModel.Handle.cs:1285-1359,1497-1570`，`Telegram/ViewModels/Dialogs/MessageCollection.cs:193-228`）；正式消息先到则删重复那条（`DialogViewModel.Handle.cs:1531-1537`）；重发纯手动，只看付费星数，不查能否重发 / 多久后可重发（`Telegram/ViewModels/DialogViewModel.Messages.cs:1098-1113`）；从通知快速回复漏带话题（TODO，`NotificationsService.cs:803-812`）
- **正在输入**：发送 4 秒节流（`Telegram/Common/Chats/OutputChatActionManager.cs:24-52`）；接收按「会话 → 发送者」存，话题另存一份（键 = 会话 + 话题 + 发送者），无本地计时器，靠 TDLib 推取消（`Telegram/Services/ClientService.cs:3823-3850`）
- issue：#1334 进大群首次加载出现重复消息（以「不再复现」关闭）

### 【UnigramPlus】UnigramPlus（NVDA 插件）——盲人用户自己补的新消息朗读
- Alt+L 开关，默认关（`addon/appModules/unigram.py:1415-1418`）；每 0.3 秒看当前会话最后一项（`:235-290`）
- 只在「最后一项变了 且 还是同一个会话 且 是收到的（界面文字含 Received at，`addon/appModules/data.py:9`）」时自动读全文；切会话 / 回前台先记基线、不补播（`unigram.py:245-247,285-290`）；播报排进 NVDA 事件队列，不打断当前朗读（`:266`）
- NVDA+Ctrl+1…0 读倒数第 N 条，**不挪焦点**（`unigram.py:320,1007-1033`）
- 意义：读屏口碑最好的第三方客户端都不做，用户只能自己补——当前会话里对方的新消息要让读屏用户知道，是刚需。我们的做法按 001 拍板 3（收9），按协议事件判，不靠界面文字

## 五、待人定

见同目录 `decision.md`（B17-2 已拍板选 A；B17-1 已撤回），本文不重复题面与建议。

## 六、已被推翻的传言

1. 「收到正在输入要自己加本地超时」（逐仓笔记里多处这么建议）——TDLib 自己 5.5 秒推取消、对方发消息即取消（`td/telegram/DialogActionManager.h:46`、`.cpp:457-483`，`MessagesManager.cpp:5528-5537`；收5）
2. 初稿里「TDLib 不提示本次登录之前的消息，可当积压护栏」——那个时间是本设备第一次登进账号的时刻（`td/telegram/AuthManager.cpp:1505` 只设一次，`MessagesManager.cpp:10523` 每次启动读回），挡不住积压（收6）
3. 初稿里「`only_preview` 跑完全部校验，含回复」——回复不合规时它不报错、悄悄去掉（`MessagesManager.cpp:21144-21170`；发6）
4. 初稿里「一次性凭据已防住重复发送」——直发模式与测试豁免下闸每次自动开新凭据，挡不住（发16，`decision.md` B17-2）

## 七、推断、待实测与局限

- **推断**：tgt 注释说「删掉的消息被命令行发送重新发出」，我们推断真因是「上次没发完的待发消息下次启动续发」（与发8 对得上）
- **待实测**（交 #52 / #53 开工时列进验收）：① `only_preview` 对回复 / 话题群的返回（发6）；② CLI 没等到结果就退出、下次启动是否续发（发8，含上面的推断）；③ 发送途中删掉待发消息：是否仍送达、TDLib 是否在服务器上补删（发3）；同理核契约里「`pending_cancel` 撤销待发」能否真的拦住送达；④ 积压警告的阈值（收10，原型里量）；⑤ TDLib 内置通知接口与 notify v0 的分歧（收8）；⑥ 话题单独静音 / 单独取消静音、话题限时静音到期后，notify v0 的判定（收1、收2，对照组测不出）
- **局限**：克隆都是浅的（20–212 个提交），旧 bug 主要靠 issue；gh 搜索中途限流，「重复发送 / 卡死」类关键词在 Unigram、grammers 未搜全；只读不编不跑，`only_preview` 对回复 / 话题群的返回、CLI 中途退出后的续发，都要 M2.1 在测试服务器上实测

## 八、开工时要核（自审没到停止线，余项交给 #52 / #53）

大总管定（2026-10-26）：自审跑满 12 轮仍每轮有 1 个 P1 就停。原因：后几轮的 P1 都是审查往旁边深挖出的新事实（提及通知是用户设置、话题有自己一层通知设置、话题静音到期不推更新、删除待发消息拦不住送达），每条都核实属实、已修，但说明这块还有没挖到的地方。#52 / #53 开工时请先核：

- **#52**：① 发3 / 契约建议第 8 条——「待发消息被删」与 `pending_cancel` 的真实效果，在测试服务器上实测（第七节②③）；② 发6 的两次预览比对，在话题群、被回复消息已删两种情形各跑一次；③ 第 12 轮最后一次修改（契约建议第 8 条、`pending_cancel` 措辞）没再经审查；④ B17-2 人已选 A（拦下），机制在 #52 设计、过评审
- **#53**：① 收1 / 收2 的「话题 → 会话 → 三档」取法与话题静音绝对到期时间，补单测（第七节⑥）；② TDLib 内置通知接口对照组（收8）只看会话层，测不出话题层；③ 第 12 轮余下的 P2 / P3 未修：第一节篇幅偏长、出处与第四节重复，词表里 `link::guard` 等少数词未解释

## 修订记录

| 日期 | 改了什么 | 为什么 |
|---|---|---|
| 2026-10-26 | 初稿 | b17 交代 |
| 2026-10-26 | 自审第 1 轮：补术语与发送步骤、统一项目记号与顺序、补 tg 发送出处、去掉「登录前消息」护栏 | `authorization_date` 核实为首次登录时刻 |
| 2026-10-26 | 自审第 2 轮：术语提到最前、条目改编号列表、`only_preview` 改为「须比对回复与话题」、新消息提示对齐 001 拍板 3 | 核实 `MessagesManager.cpp:21144-21170` 回复会被悄悄去掉 |
| 2026-10-26 | 自审第 3 轮：补「应答 / 更新 / Ack」与项目背景、上游地址、发6 加「交出前再比对」、契约建议补到 6 条 | 防 prepare 与 send 之间的时间差；出处可复核 |
| 2026-10-26 | 自审第 4 轮：结论提到最前、出处记号改用项目名（免与评审分级 P0–P3 撞名）、发16 改为待定 B17-2、契约建议加第 7 条、补结尾行 / 内容树 / 注入等解释 | 一次性凭据在直发模式下挡不住重复提交 |
| 2026-10-26 | 自审第 5 轮：待人定移到 `decision.md`，B17-2 改为落盘记录 + 按状态拦截；补直发模式 / 测试豁免词条、出处三分索引、对照表「正在输入的键」出处；推翻与待实测分节；契约建议第 6 条落点拆分 | 30 秒窗口挡不住「结果未知」后的重跑，且跨进程看不到 |
| 2026-10-26 | 自审第 6 轮：发3「临时 id 被删」改按「结果未知」报；补「每条的去处」「两份文件」「什么时候算够」；decision.md 补词解释 | 核实被删的待发消息可能已送达（`MessagesManager.cpp:27848-27851`） |
| 2026-10-26 | 自审第 7 轮：B17-2 选项 A 改为「10 分钟内同目标同指纹一律拦、`--again` 放行、到期作废」；词条集中回 README；B17-1 挂回收8；「新发先拒」补出处 | 原方案「结果未知」只进不出，会把同一句话永远拦死 |
| 2026-10-26 | 自审第 8 轮：B17-2 退回原则层（机制留给 #52 设计评审）、默认改为「人不答就不做」、测试豁免不拦；补项目背景（Rust、atg-core / cli / gui）、拍板 3 的白话；第五节只留指针；decision.md 加状态行 | 防重复的机制三轮被推翻，不该在调研里定、更不该带默认值进契约 |
| 2026-10-26 | 自审第 9 轮：撤回 B17-1——提及通知是用户自己的云端设置，收1 扩为「静音 / 提及 / 置顶三项按会话 → 三档取」；补收藏夹、基线、第一阶段词条；删词表里已不用的 HMAC；第二节改名「术语与记号」 | 核实 `disable_mention_notifications`（`td_api.tl:3809-3811`） |
| 2026-10-26 | 自审第 10 轮：收1 加话题层（话题 → 会话 → 三档），契约建议第 6 条与待实测同步；补 watch、直发模式用途、发6 摘要理由；出处索引补收1 | 核实 `forumTopic.notification_settings`（`td_api.tl:4433`） |
| 2026-10-26 | 自审第 11 轮：收2 限定会话与范围，话题静音改存绝对到期时间；收6 补「为什么不怕时钟偏差」；去处索引补收1、收2 | 核实 `ForumTopicManager.cpp:796` 话题到期不推更新 |
| 2026-10-26 | 自审第 12 轮（按大总管定的最后一轮）：发3 补「删除待发消息不等于撤销」，契约建议加第 8 条（`pending_cancel` 改称尽力撤销）；去处索引理顺收2、收10；补焦点、论坛群、临时 id 词条；对照表 Unigram 正在输入的键改正；加第八节「开工时要核」 | 核实 `MessagesManager.cpp:21330-21345` 取消只停本地请求 |
| 2026-10-26 | 验收后：记 B17-2 人选 A、8 条契约建议大总管采纳及落点 | 大总管验收回执 |
