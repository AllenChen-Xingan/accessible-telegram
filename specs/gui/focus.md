# 焦点规则 v0

> 2026-11-14 · AI 起草 · 状态：**草稿 v0**（`docs/plan/001_arch-stack.md` 第二节第 4 小节「规则先列成表交人看」的那张表；三份调研已并进，2026-11-14）。⏳ = 要实测或要人定
> 机读正本（移交后）：焦点规则表 `focus`。`FocusManager` 照表查，不另写判断；本表移交后由它生成（`docs/plan/012_gui-ssot.md` 第二节第 10 行）

## 一、七条总规矩

1. **焦点只在人按键时移动**（人点系统通知、切回应用也算人的操作；外部无障碍动作见第三节）。 新消息、编辑、撤回、加载更早的历史、连接状态变化、发送结果到达，**一律不动焦点**（001 第二节第 4 小节；exp004 七种模式实测新消息都没挪选中项）
2. **焦点按 id 锚定，不按下标**（001 第一节第 3 小节）。选中项的 `MsgId` / `ChatId` 不变，每帧按 id 重算下标
3. **每个触发事件恰好一条规则**（单测查）。规则 = 触发 → 焦点落到哪个区、哪个元素 → 说不说一句（说的话来自 `speech`，走 `announce.md`）
4. **焦点落到的元素必须正在被渲染**（GPUI 只给渲染了的节点报焦点，`docs/research/001_stack/findings-gpui.md` 第 2 条）：落到列表里某一项时，先把它滚进可见区再选中（「先滚再选」，exp003）
5. **获焦不能有副作用**：VO 光标路过会把键盘焦点拉过去（「键盘焦点跟随 VO 光标」，`findings-a11y.md` 第三节第 6 小节），所以选中、获焦本身不标已读、不播放、不打开；进会话标已读只发生在人按回车打开时（003 F1；人 2026-11-14 再确认「可以直接吧已读标到最新」，B19-2，`docs/plan/013_decisions-b19-gui.md`）
6. **焦点改动推到下一帧再生效**：GPUI 的焦点监听在一帧末尾跑，这时 `cx.notify()` 不再排下一帧（Quill 踩过，`findings-components.md` 第五节）。exp003 的「滚动和选中同一帧生效」要在正式代码里复核
7. **重新打开一个会话一律落在 003 F2 的落点**，不恢复上次的滚动位置或选中项（按会话恢复滚动会把人带离未读，Quill 踩过，`findings-components.md` 第五节）。`UiState.msgs_anchor` 只在「会话一直开着、焦点离开列表再回来」时保留（exp003b-v2）

## 二、规则表

「区」= `UiState.region`，取值照 `keymap.md` 第二节 `view` 一行（去掉 `global`）；「落在」= 那个区里具体哪个元素（标识写法见 `a11y-tree.md` 第二节）。

| 触发（`trigger`） | 落在 | 说一句 | 出处 / 状态 |
|---|---|---|---|
| `app_started_signed_in` | 会话列表，选中上次退出时选中的会话；没有记录就第一项 | 不说 | 草案 |
| `app_started_signed_out` | 登录页的第一个空框 | 不说（框名就是提示） | 草案（T01） |
| `chat_opened`（会话列表按回车） | 消息列表 `messages:<chat>`，选中 003 F2 落点：第一条未读（它的名称前加「未读：」，未读分隔不进 ↑↓ 顺序、不可停，B19-7）；没有未读就最新一条。同时已读标到最新一条（003 F1、B19-2） | 不说（列表名「消息」+ 选中项会被 VoiceOver 读） | 003 F2；003 F1；B19-2；B19-7 |
| `chat_opened_from_notification`（点系统通知） | 同上，但选中通知对应的那条 | 不说 | 003 F2；M4.4 |
| `search_hit_opened`（结果列表里按回车） | 搜索关掉（`return_to` 里搜索那一层弹掉、不回去）。消息命中：那条所在会话的消息列表，选中那条（不落第一条未读，清单 F17）；会话命中：同 `chat_opened` | 不说 | 003 F2；M4.5 |
| `back_to_chats`（消息列表按 Esc） | 会话列表，选中刚才那个会话（它若已因排序变化挪了位置，跟着 id 走） | 不说 | 草案 |
| `first_unread_jumped`（`cmd-j`） | 当前会话消息列表，选中第一条未读；没有未读就最新一条 | 不说 | 003 F2 |
| `next_unread_jumped` | 会话列表或消息列表里被跳到的会话 / 它的落点 | 到底时说「没有更多未读」，焦点不动 | 003 A3；T09 |
| `composer_entered`（`i` / `r`） | 输入框 `composer:<chat>`；`r` 时回复目标 = 当时选中的那条 | 不说（输入框名称带「回复 X」） | 草案（T04） |
| `composer_left`（输入框按 Esc） | 消息列表，选中进输入框之前选中的那条 | 不说；草稿保留 | 草案 |
| `send_submitted`（输入框按回车，或有效时的 Cmd+回车 `send_again`；L2，不弹框，#113） | 第六节第 3、4 条 | 第六节第 4 条 | 002 M4.2；T04；第六节 |
| `send_refused_now`（交出段「当场没成」；名字里的 refused 泛指当场没成，不只错误码 `send_refused`） | 第六节第 4 条 | 第六节第 4 条 | 第六节 |
| `send_unknown_now`（交出段「交出时就未知」） | 第六节第 4 条 | 第六节第 4 条 | 第六节 |
| `delete_submitted` · `resend_submitted`（动作 `delete_for_all` / `resend`，键见 `keymap.md`；L2，不弹框） | 第六节第 2、3、4、6、7 条 | 第六节 | 第六节；`actions.md` `delete` · `resend` 行；002 M4.6（其「过确认」随 #113 不适用，待 #115 改） |
| `confirm_opened`（确认框：**只有打开链接 / 附件**，L1 外部，契约要求当场确认——`specs/contracts/actions.md` 第一节第 2 小节「L1 外部」行与 `open_link` / `open_file` 行：「打开前把完整网址（域名单独读出）给人看并确认」） | 确认框（`AlertDialog`）打开即读正文；键盘焦点落在**取消**（最不具破坏性的按钮）；Tab 在框内循环；压入打开前的元素 | 不说（VoiceOver 读正文） | APG alertdialog（`findings-a11y.md` 第二节第 1 小节）；种类与正文见 `a11y-tree.md` 第三节「确认框」 |
| `confirmed`（确认框按了确认） | 回打开前的元素（应用随即失去前台，切回来走 `window_reactivated`） | 不说 | M3.5、M3.7 |
| `error_dialog_opened`（只在要人处理的错误时弹，如 `account_limited`；一般错误只播报不弹框，`announce.md` `action_error`） | 错误框正文；焦点在唯一的「好」按钮 | 不说（读正文） | `errors.md` 第一节第 5 小节；清单 A10 |
| `dialog_closed`（Esc，或按了取消） | 回打开对话框之前的那个元素，草稿保留；那个元素没了就去逻辑上的下一处 | 不说 | APG dialog（`findings-a11y.md` 第二节第 1 小节） |
| `send_failed`（收尾段的明确失败、未知、晚到失败） | 第六节第 5 条（焦点不动；失败换 id 时锚点跟着换） | 第六节第 5 条 | 规矩 1；第六节；`actions.md` 事件 `send_failed` |
| `message_sent`（`send_succeeded`，含晚到成功） | 不动；若人正选中这条待发消息，锚点跟着换 id（`actions.md` 事件 `send_succeeded`：原地改、稳定键不变） | 第六节第 5 条 | 调研 003 发5；第六节 |
| `selected_message_deleted`（被撤回） | 不动：原位留占位，仍选中它 | 不说（VoiceOver 下次读到占位） | 001 拍板 7 |
| `selected_message_edited` | 不动 | 不说 | 001 拍板 7 |
| `selected_message_removed`（清缓存 / 非永久删除，不是撤回；不含待发消息的临时 id 出现在 `messages_deleted` 里——那是第六节第 5 条的「未知」） | 选中下一条更新的；没有就上一条 | 不说 | 006 甲4「拿掉；开着 VoiceOver 实测再看」——草案 |
| `selected_chat_removed`（退群、会话消失） | 会话列表里它的下一项；没有就上一项 | 说一句会话名 + 「已不在列表里」（`announce.md` 来源 `chat_removed`；树节点待加） | 草案 |
| `older_history_loaded` | 不动（选中项 id 不变，下标重算） | 不说 | 001 第一节第 3 小节；B19-12 |
| `detail_read`（消息列表按 D） | 不动 | 说详情（`announce.md` 来源 `detail`）；有隐藏内容时同时展开，再按 D 收起、不说话 | B19-6 |
| `new_message_arrived` | 不动 | 按 `announce.md` | 规矩 1；T05 |
| `reading_view_opened`（`enter`） | 阅读视图 `reading:<chat>:<msg>`，光标在开头 | 不说 | M3.5 |
| `reading_view_closed`（Esc） | 消息列表，选中原来那条 | 不说 | M3.5「Esc 回原处、焦点不动」 |
| `login_step_advanced` | 下一步的输入框 | 不说（框名） | T01 |
| `login_failed` | 留在出错的框，文字保留；框的提示语 = 出错原因 | 说出错原因（`announce.md` 来源 `action_error`） | T01「出错时读出原因」 |
| `connection_lost` | 不动 | 按 `announce.md` 来源 `status_bad`（防抖 2 秒，003 E1） | `errors.md` 第一节第 4 小节 |
| `connection_restored` | 不动 | 按 `announce.md` 来源 `status_ok`（只在播过坏状态后，003 E2） | 同上 |
| `window_reactivated`（切回本应用） | 切走那一刻焦点在的那个元素（`UiState.last_focus`；它若没了，就去它所在区的选中项） | 不说 | 草案 |
| `vo_cursor_landed_on_list`（VO+←/→ 落到列表） | 列表持键盘焦点，选中项保持；**不打开、不标已读**（规矩 5） | 不说 | exp003b-v2（要 VoiceOver「键盘焦点跟随 VO 光标」开着，001 R4） |
| `search_opened`（`cmd-f` / `cmd-k`） | 搜索框 `search`；压入打开前的元素 | 不说（框名「搜索消息」/「搜索会话」） | 清单 K14 |
| `search_results_entered`（搜索框里 ↓） | 搜索结果列表 `search-results`，选中第一项（列表持焦点 + active descendant，同 `NavList`） | 不说（VoiceOver 读选中项） | 清单 K14、S28；卡 G17 |
| `search_input_returned`（结果列表里 Esc 或打字） | 搜索框，光标在末尾；打的那个字符接着输进去 | 不说 | 卡 G17 |
| `search_closed`（Esc） | 回打开搜索之前的元素（`UiState.return_to`） | 不说 | 清单 K14 |
| `help_opened` | 帮助页第一项（使用前提：关快速导航，B19-3） | 不说 | 草案 |
| `help_closed` | 回打开帮助页之前的元素 | 不说 | 草案 |
| `links_opened`（按 `l`） | 链接列表 `links:<chat>:<msg>` 第一项；这条没有链接就不开、焦点不动 | 不说；没有链接时说「这条没有链接」（`announce.md` 来源 `nothing_here`） | 草案 |
| `attachment_missing`（按 `o`，这条没有附件） | 不动 | 说「这条没有附件」（来源 `nothing_here`） | 草案 |
| `links_closed`（Esc） | 回打开链接列表之前的元素 | 不说 | 草案 |

说一句的那几行，公告要**错开焦点移动**：焦点刚移动的那一刻发的公告常被吞（`findings-a11y.md` 第五节第 1 小节），由播报器延后再发（不丢；常数 `FOCUS_SETTLE_MS` 只在 `announce.md` 第四节一处；清单 A6）。

## 三、外部无障碍动作（VoiceOver、语音控制、agent 发来的 Focus / Click 等）

- **策略**：外部 Focus 暂按 GPUI 默认——照常移键盘焦点（`Z:gpui/src/window.rs:7558-7567`）；外部 Click 照常按下。这是规矩 1「焦点只在人按键时移动」的唯一例外：外部 Focus 也是人（或人的辅助技术）发起的。曾见焦点漂移，原因未定、不再追；真碰到再问人（卡 G16）
- **调试日志**（和发送留痕 `direct-send.log` 无关）：`atg-gui` 的调试日志对每个外部无障碍动作记一行：**时间、动作名（Focus / Blur / Click / ScrollIntoView……）、目标的稳定标识**（`a11y-tree.md` 第二节，如 `msg:101:1001`）。**不记名称、值、任何内容**——日志只记形状不记内容（`specs/contracts/config-privacy.md` 第一节第 5 小节）。正本位置就是这一节。写法同 core 的 `src/atg-core/src/logline.rs`：它只收写死的名字和数字，所以标识拆成「种类（静态字符串，如 `msg`）+ 各段数字」记，不拼字符串进日志。写进本项目日志 `~/Library/Logs/atg/atg-gui.log`（落点正本 `specs/contracts/config-privacy.md` 第一节「日志」行，不另建）；`logline` 现在只有 `info` / `warn`，要一个开发构建默认开、发布构建默认关的 debug 级入口，记 `docs/plan/012_gui-ssot.md` 第八节第 13 条，定之前用 `info`
- **用途**：焦点再漂时，拿这份记录和树导出对照，判断是外部动作拉走的还是本应用自己移的

## 四、Tab 顺序

已测的一圈（exp003b-v2）：会话标题 → 搜索 → 消息列表 → 输入框 → 发送 → 回到会话标题（「会话标题」是实验里一个读当前会话名的按钮，正式界面里当前会话名已在消息列表的区块名里，所以去掉）。正式界面加了会话列表之后：**会话列表 → 消息列表 → 输入框 → 发送**，草案 ⏳。搜索不是常驻的一站：`cmd-f` / `cmd-k` 打开、Esc 关（`search_opened` / `search_closed`），开着时在 Tab 一圈里插在当前位置之后。列表本身是一站，列表里的项不各占一站（001 第二节第 1 小节）。

## 五、正例与反例

| | 例 | 为什么 |
|---|---|---|
| ✅ | 人停在第 30 条，大群一秒来 20 条，选中仍是第 30 条，标题变成「……，20 条新消息，按 M 收听」 | 规矩 1；新消息数写进名称（001 拍板 3 ⑥） |
| ❌ | 新消息来了，选中跳到最新一条 | 抢焦点（T05 判据） |
| ❌ | 往上加载了 50 条更早的，选中还是「第 30 个下标」，实际指到了另一条 | 按下标锚定（规矩 2） |
| ✅ | 关掉打开链接的确认框（Esc），焦点回到链接列表里刚才那项 | 回到触发处 |
| ❌ | 输入框按回车后弹出「确定发送吗」框 | 发送一律直接发送，不弹框（#113） |
| ❌ | 关掉确认框后焦点落到窗口（VoiceOver 读「无障碍 Telegram，窗口」） | 焦点丢了，人要重新找 |
| ❌ | 选中一条屏幕外的项，但没先滚过去 | GPUI 不给没渲染的节点报焦点，退回整窗（`docs/research/001_stack/findings-gpui.md` 第 2 条） |

## 六、L2 发送流程（发送、重发、删除；规则表各行只引本节）

> 自审 b20 第 3–10 轮收成这一节。步骤名用契约名，括号里是 core 名（对照见 `src/atg-core/src/send.rs` 文件头）；两段式入口、晚到结果的上报是 issue #119（`docs/plan/012_gui-ssot.md` 第八节第 11 条）。

1. **两段**：一次按键，界面在后台任务里不等人地接连做——**交出段** = 准备（`send_text_prepare`（core `prepare_text`）/ `resend_prepare`（`prepare_resend`）/ `delete_prepare`）→ 闸开凭据 → 交出（`send_text` / `resend` / `delete`；`send_text` 内部再验一次凭据，并紧挨交出前再调一次预览、把回复对象与话题和请求逐项比对，不一致按闸拒 `send_refused` 报）；**收尾段**（发送、重发才有）= 等成败（`wait`）→ 闸记结果、写发送留痕（`finish`）。界面不自己计时、不写闸记录。
2. **时限**：准备是只读查询，有自己的查询期限（现在只有 CLI 定了这个数，core 要给界面一个，记 `docs/plan/012_gui-ssot.md` 第八节第 11 条），超时按 `network` 报，算交出段的「返回错误」。**发送、重发的交出与等结果共用一个时限**：从 `send_text` / `resend` 开始算（契约 `send_text` 行「交出本身也算在这 30 秒里」；core `send_and_wait` 里 `wait` 只拿剩下的时间），到点报 `send_outcome_unknown`。数由 core 定，界面规约不抄。**删除**的交出（等 `Ack`）契约与 core 都还没有时限，在途态可能不结束——记第八节第 11 条，定之前界面按发送同一个时限结束在途态，结局见第 4 条表「删除没回应」一行（AI 定，可推翻）。
3. **在途态**（`UiState.in_flight`）：按下到交出段有结局。**挡在动作入口，不只挡键**：按键、发送按钮的 AX Press / 点击、菜单栏项都走同一个动作，动作入口先查 `in_flight`，为真就忽略；同时发送按钮与对应菜单项设禁用（`a11y-tree.md` 发送按钮行）。**发送**在途时那个会话的输入框只读、回复目标锁住（`reply` 在该会话被忽略；回车、Cmd+回车都吃掉，优先于 `send_again` 是否有效）；人可以照常离开输入框、去别处浏览。删除、重发不锁输入框。按下那一刻记下**提交快照**（会话、草稿、回复目标、所指消息），结局只按快照处理，不碰之后的状态；**一切异步结局都不移动焦点与选中**（规矩 1），只在当前引用正指着那条消息时随它换 id 或变占位。
4. **交出段的结局**（恰好一种；删除只有「交出了」「当场没成」「删除没回应」三格）：

| 结局 | 怎么认 | 焦点 · 草稿 | 说一句 |
|---|---|---|---|
| 交出了 | 发送：拿到待发消息（临时 id 不为 0）；重发：`resend` 整体返回、且至少一条拿到临时 id（没拿到的那几条按第 7 条）；删除：`delete` 返回 `Ack` | 发送：清空快照里那个会话的输入框、清掉回复目标，**焦点不动**（人还在输入框就留在那里；已经离开不拉回，规矩 1）；删除、重发：选中不动（返回时选中在哪就在哪，不拉回原来那条） | 读一遍计划（`announce.md` `send_plan`，边念边发，卡 G18） |
| 当场没成 | 任一步返回错误，`send_outcome_unknown` 除外；重发整体返回但**每一条**都是 `None`（全部不能重发）也算这一格，这时只逐条播 `resend_skipped`、不读计划——准备阶段的错误（含 `network`）、闸拒（`send_refused`）、TDLib 对交出请求回的错误应答（如 `forbidden`）。core 把交出途中的断线改报成 `send_outcome_unknown`，不在这一格 | 不动（选中在哪就在哪）；发送时快照里的草稿与回复目标原样保留、输入框恢复可写 | 说原因（`announce.md` `send_refused`；`send_again` 是否有效见 `keymap.md`） |
| 删除没回应 | 只有删除：到上面第 2 条的时限还没有 `Ack`。不当「当场没成」：请求可能已在 TDLib 里，稍后真的删掉 | 不动（选中在哪就在哪） | 「删除还没有回应，可能稍后生效」（`announce.md` `delete_pending`） |
| 交出时就未知 | 只有发送、重发：整次调用报 `send_outcome_unknown` 且临时 id 为 0（到时限没拿到应答，或交出途中断线；`errors.md` `send_outcome_unknown`）。重发是一个 `resendMessages` 请求，未知只会整次发生；单条没有临时 id 不是未知，是第 7 条。删除没有这一格 | 不动；发送时草稿保留（不丢字），`send_again` 无效 | 「结果未知，可能已经发出，先别重发」（`announce.md` `send_failed` 未知态） |

5. **收尾段的结局**（发送、重发，逐条；焦点一律不动。**换 id 一律原地改**：成功（正式 id）、失败（TDLib 也给失败那条新 id，`actions.md` 事件 `send_failed`）、重发交出（旧 id → 新临时 id）都让**界面状态里所有指着这条消息的引用**跟着换（清单与规则的正本在 `docs/plan/012_gui-ssot.md` 第三节第 2 小节「换 id 迁移」），元素稳定键不变，提示的重发键作用在换后的那条上。**顺序**：成败事件可能比交出的应答先到（`src/atg-core/src/send.rs` 文件头「谁先到没有保证」）；界面只从 core 的统一分发里按「先交出（旧 id → 临时 id），再成败（临时 id → 正式 / 新 id）」的顺序收，早到的成败由 core 暂存到交出应答之后再交出（同 `send_and_wait` 的做法；要求记 012 第八节第 11 条）——人正选中的那条待发消息若因 `pending_deleted` 从列表拿掉，原位留一条「结果未知」占位、仍选中它，同撤回占位的做法，001 拍板 7）：

| 结局 | 怎么认 | 说一句 |
|---|---|---|
| 成功 | 事件 `send_succeeded`。**文字发送**到了再按提交时的计划核一遍回复关系与话题（契约 `send_text` 行「`send_succeeded` 到了再核一遍」；core `check_kept(&TextPlan, …)`，CLI 记 `reply_kept:false`），普通成功与晚到成功都核。**重发**不复核：core 没有对 `ResendPlan` 的复核，且重发计划里本就可能写明「会去掉回复关系」，要加须另定逐条比较规则（记 `docs/plan/012_gui-ssot.md` 第八节第 11 条） | 一致：「已发送」（`announce.md` `send_ok`）；不一致：仍算发出，说「已发送，但回复关系 / 话题被改了」（`send_ok` 的改写态） |
| 明确失败 | 事件 `send_failed` | 原因；契约 `can_resend` 为真时提示重发键，为假不提示（`announce.md` `send_failed`） |
| 未知 | core 报 `send_outcome_unknown`，其 `why` 字段（`errors.md`）为：`timeout` 到时限没有成败；`disconnected` 等成败的途中断线（只是 `why` 的一个取值，不是 `errors.md` 里另有的独立错误码 `disconnected`）；`pending_deleted` 临时 id 出现在 `messages_deleted` 里而没有成功事件——TDLib 这时不再推失败事件，消息可能已经送达 | 「结果未知，可能已经发出，先别重发」，不提示重发键 |
| 晚到 | 收尾段已按「未知」播过之后，同一个临时 id 又来了 `send_succeeded` / `send_failed`（契约：晚到的照常进 store，GUI 一直开着，一定会碰到）。按临时 id 认（`send_succeeded` 带着原临时 id，`actions.md` 事件表）；播过「未知」的临时 id 记在 `UiState.unknown_pending`。交出段「交出时就未知」那种没有临时 id，对不上号：那条若后来出现，按普通新消息 / 已发送处理，不念晚到 | 晚到成功：「刚才结果未知的那条已发出」（`announce.md` `send_ok`）；晚到失败：同「明确失败」（`announce.md` `send_failed`，按 `can_resend` 提示重发键）。档位见 `announce.md` 路由表。焦点都不动。闸记录要随晚到结果改，否则晚到失败后同一句仍被重复提交拦截按「未知」拦 24 小时（`actions.md` 第一节第 5 小节「何时拦」；issue #119） |

6. **删除**没有收尾段：`messages_deleted` 到了不另播（选中那条原位变成撤回占位，001 拍板 7）；契约对删除没有结果时限，迟迟不到也不另说。
7. **重发**：TDLib 对某条说不能重发（core `ResendOutcome.result` 为 `None`，CLI 记 `not_resent`）——确定没发出，没有 `CoreError`——说「〈那条的节选〉不能重发」（带数据，走 `speech`；`announce.md` 来源 `resend_skipped`），不并成整体失败。
