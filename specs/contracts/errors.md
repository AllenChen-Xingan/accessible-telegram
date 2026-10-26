# 契约 · 错误与状态模型 v0

> 2026-10-25 · 手下 infra 起草 · 状态：**草稿**；末尾「待人拍板」E1、E2 **已拍板**（`docs/plan/003_decisions-b2.md`）
> 管：`atg-core` 对外报的错误种类、连接 / 授权 / 内核三条状态线、它们怎么变成用户听得懂的状态（T10、T01「出错时读出原因」）
> 不管：读法的最终措辞（归 `specs/style/` 读法规约，人定）；CLI 的退出码与 JSON 形状（归 `cli.md`，本文只给对照）
> 依据：`docs/plan/001_arch-stack.md` 第一节第 4 小节（收线程监督）、第四节第 1 小节（发送闸）；`resources/td/td/generate/scheme/td_api.tl`

## 一、定了什么

### 1. 错误只有一个类型：`CoreError`

`atg-core` 所有对外函数失败时都返回 `CoreError`，**不向外泄露 tdlib-rs 的错误类型**（架构方案第一节第 1 小节）。它是一个枚举，每种带一个**稳定的机读码**（snake_case，CLI 原样输出）：

| 机读码 `code` | 什么时候 | 带的字段 | 能不能自动重试 |
|---|---|---|---|
| `auth_required` | 还没登录就调了要登录的动作；或会话被吊销（TDLib 401） | — | 否，要人重新登录 |
| `auth_phone_invalid` | 手机号格式不对（`PHONE_NUMBER_INVALID`） | — | 否 |
| `auth_code_invalid` | 验证码不对（`PHONE_CODE_INVALID`） | — | 否，让人重输 |
| `auth_code_expired` | 验证码过期（`PHONE_CODE_EXPIRED`） | — | 否，提示「重发验证码」 |
| `auth_password_invalid` | 二步验证密码不对（`PASSWORD_HASH_INVALID`） | — | 否 |
| `auth_phone_not_for_profile` | 号码与档案不配：测试档案收到非 `99966` 开头的号，或正式档案收到 `99966` 开头的测试号（`config-privacy.md` 第一节第 2 小节）。本地检查拦下，不交给 TDLib，所以不带 `td` 附件；退出码同 `auth_*`（3） | — | 否，换号或换档案（测试加 `--test-dc`） |
| `auth_wrong_state` | 当前授权步骤不收这个输入（如等密码时送了验证码） | `expected`：当前的 `AuthStep` 值 | 否 |
| `rate_limited` | 限流：TDLib 429 `Too Many Requests: retry after N`，服务器 `FLOOD_WAIT_N`，群慢速模式 `SLOWMODE_WAIT_N` | `retry_after_s` | 是，**等够秒数**才许再发（L2 除外，见第 5 小节） |
| `account_limited` | 账号被限制的信号：`PEER_FLOOD`（发给陌生人太多）、`PHONE_NUMBER_FLOOD`（登录太多）、码 420 的其余情况 | — | 否；收到后**停掉全部 L2 动作和登录**，直到人处理 |
| `not_found` | 会话 / 消息 / 用户不存在或已删（TDLib 404，或 400 `*_NOT_FOUND` / `*_ID_INVALID`） | `what` | 否 |
| `forbidden` | 没权限（TDLib 403，`CHAT_WRITE_FORBIDDEN`、被拉黑、频道只读等） | — | 否 |
| `invalid_input` | 其他参数错误（TDLib 400 的其余情况、本地校验失败） | — | 否 |
| `network` | 请求超时、网络不可用时请求失败 | — | 是，连接恢复后 |
| `disconnected` | 收线程已故障（架构方案第一节第 4 小节）；之后一切请求都直接回这个。**另**：经本机接口转发时，连上了执行方却没等到结尾就断了（执行方中途退出），或 90 秒没回话，发起方也报它，说明换成「替你执行的进程中途退出了」或「90 秒没回话」，退出码同为 4（`config-privacy.md` 第一节第 2 小节） | — | 否（不自动重试）：收线程故障要重启内核；转发中途断开时调用方可以再跑一次 |
| `profile_busy` | 同一个本地库已被另一个进程占用，且这条命令没法经本机接口转过去（见 `config-privacy.md`「一库一进程」「本机接口」）。说明按场景换：占着的是一次性命令 →「稍后重试，或先开着 atg-cli serve」；占着的是常驻执行方而命令不转（登录、登出）→「请先停掉它再跑」；又起一个 `serve` →「已经有常驻进程在服务」 | `holder`：能查到就写是 CLI 还是 GUI | 否（不自动重试；要不要稍后再跑由调用方按说明定） |
| `db_key_invalid` | 本地库的加密钥匙不对或取不到（TDLib 401 `Wrong database encryption key`，`TdDb.cpp:184`；或钥匙串拒绝） | `source`：`keychain_denied` / `wrong_key` | 否，见 `config-privacy.md`「钥匙丢了怎么办」 |
| `send_refused` | 发送闸拒绝：没凭据、凭据与目标或内容不符、凭据过期或已用过、非交互环境 | `reason`（见下） | 否 |
| `send_outcome_unknown` | L2 请求已交给 TDLib，但在时限内既没等到成功也没等到失败（如 CLI `send` 30 秒超时） | `chat_id` · 临时 `message_id` | **否**。不许重发；用 `status` 查这条待发消息的下落，或 `pending_cancel` 撤销 |
| `cancelled` | 人在确认步骤按了取消（只指这个；Ctrl+C 见下方「CLI 自有码」） | — | 否 |
| `internal` | TDLib 406，或本项目自己的 bug | `what`：写死的说明（如 `not_started`、`profile_kind_mismatch`、`td_406`；本机接口的：`socket_path_too_long` · `socket_path_occupied`（同名的不是套接字）· `socket_in_use`（残留的居然有人在听）· `ipc_io` · `ipc_version_mismatch` · `ipc_bad_request`），不放运行时的值 | 否 |
| `unknown` | 认不出来的 TDLib 错误 | `td_code` | 否 |

`send_refused.reason` 的取值：`no_token` · `target_mismatch` · `content_mismatch` · `token_expired` · `token_reused` · `not_interactive` · `auth_failed`（Touch ID / 系统密码没过）· `test_dc_saved_only`（测试服务器上目标不是收藏夹）· `account_limited` · `l1_disabled`（这一类 L1 副作用被整类关掉；F1、A2 人定后第一阶段不会出现，留作以后的开关，`actions.md` 第一节第 4 小节第 9 条）。`not_interactive` 指弹不出 Touch ID / 系统密码框（如经 SSH 运行）。

**CLI 自有码**（不是 `CoreError`，只出现在 CLI 的 `error` 行，`cli.md`）：`usage`（参数不对）· `interrupted`（Ctrl+C）· `not_implemented`（子命令还没实现，带 `command`；退出码 1）。

**`error.data` 的字段**（CLI 原样输出；GUI 读 `message_zh`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `code` | 字符串 | 上面两张表里的码 |
| `message_zh` | 字符串 | 第 5 小节 |
| `retryable` | 布尔 | 上表最后一列为「是」的为 true；**但凡是 L2 动作的错误一律为 false**（L2 不自动重试，`actions.md` 第一节第 2 小节），AI 不得据此重发 |
| 附加字段 | — | 各码自带的（`retry_after_s`、`reason`、`what`、`expected`、`holder`、`source`、`td_code`），平铺 |
| `td` | 对象，可选 | 见下 |

每个 `CoreError` 还可以带一个 `td` 附件：`{ code: i32, message?: String }`。`message` **只在原文是第一节第 2 小节点名登记过的那几个码**（`PHONE_NUMBER_INVALID`、`PHONE_CODE_INVALID`、`PHONE_CODE_EXPIRED`、`PASSWORD_HASH_INVALID`、`PEER_FLOOD`、`PHONE_NUMBER_FLOOD`、`CHAT_WRITE_FORBIDDEN`）时才带；限流只带解析出的秒数（`retry_after_s`），不带原文；其余一律只带数字码。**码 406 在生成附件之前就截断，连 `message` 字段都不建**。理由见第二节第 3 条。

### 2. TDLib 错误 → `CoreError` 的映射规则

按顺序匹配，先中先出：

0. 先认两条**本地**错误（它们的码会和别的撞）：消息是 `Wrong database encryption key` → `db_key_invalid`（它的码是 401，不能落到第 4 条）；消息含 `already in use`（锁文件被占，`tdutils/.../FileFd.cpp:421,503`）→ `profile_busy`。正常情况下这两种在起 TDLib 之前就被本项目自己的检查拦下（`config-privacy.md`），这里是兜底
1. 码 **406** → `internal`，且**绝不显示、绝不读出**（`td_api.tl:16` 原文要求「406 的消息不得以任何方式处理或展示」），只记日志
2. 消息是 `PEER_FLOOD` / `PHONE_NUMBER_FLOOD` → `account_limited`
3. 码 **429**，或消息以 `FLOOD_WAIT_` / `SLOWMODE_WAIT_` 开头 → `rate_limited`，从消息里解析秒数；解析不出按 60 秒；码 **420** 的其余情况 → `account_limited`
4. 码 **401** → `auth_required`
5. 码 **403** → `forbidden`
6. 码 **404**，或消息以 `_NOT_FOUND` / `_ID_INVALID` 结尾 → `not_found`
7. 码 **400** 且消息是第一节第 1 小节表里点名的那几个 → 对应的 `auth_*`；另 `EMAIL_CODE_INVALID`（邮箱验证码错）→ `auth_code_invalid`（`actions.md` `auth_submit_email_code`「同验证码」）。它的原文**不**登记，附件只带数字码
8. 码 **400** 其余 → `invalid_input`
9. 码 **500**，或请求超时 → `network`（例外：内核正在关闭时 TDLib 也回 500，如 `Request aborted`，此时归 `disconnected`；候选，未核）
10. 其余 → `unknown`

映射是一个**纯函数**（`fn map_td_error(code, message) -> CoreError`），有表驱动单测（`testing.md` 第一节第 4 小节）。

### 3. 三条状态线，合成一个「用户状态」

| 线 | 取值 | 来源 |
|---|---|---|
| **内核** `CoreState` | `starting` · `running` · `faulted` · `closing` · `closed` | 本项目自己：收线程监督（架构方案第一节第 4 小节） |
| **授权** `AuthStep` | `wait_parameters` · `wait_phone` · `wait_code` · `wait_email` · `wait_email_code` · `wait_password` · `wait_registration` · `wait_other_device` · `ready` · `logging_out` · `closing` · `closed` · `unsupported` | `updateAuthorizationState`（`td_api.tl:11193`；各状态 `:206-263`） |
| **连接** `NetState` | `waiting_for_network` · `connecting_to_proxy` · `connecting` · `updating` · `ready` · `unknown{td_type}`（TDLib 升级后新增的状态；合成时当 `connecting`，宁可保守不说「已连接」） | `updateConnectionState`（`td_api.tl:11818`；各状态 `:10657-10669`） |

`unsupported` 收容本项目第一阶段不支持的授权步骤（如 `authorizationStateWaitPremiumPurchase`，`td_api.tl:217`），不崩，告诉人「这一步本应用暂不支持，请先在官方客户端完成」。

**登录走到不支持的一步时 CLI 怎么报**：`login` / `auth *` 遇到 `unsupported`、`wait_other_device`（扫码确认）、或**正式档案**上的 `wait_registration`，以 `auth_wrong_state` 结尾，`expected` 写那一步，`message_zh` 换成上面那句，退出码 3。注意 `auth_wrong_state.expected` 一律是**现在所在**的那一步，不是调用方本想交的那一步。

**合成规则**（一个纯函数 `fn user_status(core, auth, net) -> UserStatus`，归本契约；`UserStatus` 这个数据类型定义在 `atg-core::tree`，JSON 形如 `{"type":"status","status":"offline"}`，见 `specs/content-tree.md`），优先级从高到低：

| `UserStatus` | 条件（按行从上往下，先中先出；三条线的**每一种组合都落到恰好一行**） | 级别 |
|---|---|---|
| `fault` | 内核 `faulted` | 必说 |
| `stopping` | 内核 `closing` / `closed`，或授权 `closing` / `closed` | 不说（是人自己关的） |
| `connecting` | 内核 `starting`，或授权 `wait_parameters` | 防抖后说 |
| `unsupported_step` | 授权 `unsupported` | 必说 |
| `signed_out` | 授权 `wait_phone` / `logging_out` | 必说 |
| `signing_in` | 授权 `wait_code` / `wait_email` / `wait_email_code` / `wait_password` / `wait_registration` / `wait_other_device` | 由登录界面自己读，不另播 |
| `offline` | 连接 `waiting_for_network` | 必说 |
| `connecting` | 连接 `connecting` / `connecting_to_proxy` / `unknown` | 防抖后说 |
| `syncing` | 连接 `updating` | 防抖后说 |
| `online` | **只有**内核 `running` 且授权 `ready` 且连接 `ready` | 只在恢复时说「已连接」 |

措辞不在这里写：说法正本是读法快照（`src/atg-core/tests/snapshots/`，人定标后搬 `specs/style/`），规则见 `specs/content-tree.md` 第九节。`stopping` 是本次新增值，已告知 `tree`。表驱动单测要穷举三条线的全部组合（`testing.md` 第一节第 4 小节）。

### 4. 什么时候播报状态变化（T10）

- **防抖**：`connecting` / `syncing` 要持续 **2 秒**以上才播；短暂闪断不出声（E1 人定 2 秒）
- **成对**：只有播过「断开 / 连接中」，恢复时才播「已连接」；没播过坏状态就不播恢复
- **合并**：2 秒内连续变化只播最后一个
- **不抢焦点、不打断**：走播报器（架构方案第二节第 2 小节），级别同新消息提示的低优先级；`fault` / `offline` 用较高优先级
- **常驻**：当前用户状态同时写进窗口标题或状态栏的无障碍名称，用户随时能查（CLI 用 `atg-cli status`）
- **初始同步结束**：`updating → ready` 这一跳**候选**作 `notify` 的「初始同步结束」信号（架构方案 R11），未核
- 播报策略本身是 `atg-core` 里的纯函数，时钟可注入，可单测；真正发声在 `atg-gui`
- **读法只吃 `UserStatus`**，不直接读 `NetState`（否则会绕开内核与授权两条线）；内容树的 `Status` 节点承载的就是 `UserStatus`

### 5. 错误怎么告诉人

- 每个 `CoreError` 有一句中文说明（`message_zh`）。CLI 原样打印，GUI 读出；措辞正本同上（读法快照 → `specs/style/`），本文只定码
- 说明只说**发生了什么 + 人下一步能做什么**，不带 TDLib 原文、不带内容
- **限流怎么处理**（`.42cog/real.md`：必须遵守 FLOOD_WAIT）：
  - L0 动作：要等的秒数 ≤ 5 秒，内核自己等完再发，不打扰人；> 5 秒，立刻返回 `rate_limited` 带 `retry_after_s`，由调用方决定（CLI 不阻塞，加 `--wait` 才等）
  - L2 动作：**从不排队、从不自动重试**（`actions.md` 第一节第 2 小节），直接返回 `rate_limited`
  - 「到什么时候之前不许再发」**写进档案目录**（`flood_until`，按动作类别记），新进程启动也先读它——否则 CLI 每跑一次都是新进程，AI 循环调用就无视了等待
  - `account_limited` 同样写进档案目录，清除要人确认

## 二、为什么

1. **一个错误类型、稳定机读码**：AI 靠 `code` 分支，人靠 `message_zh` 听懂；tdlib-rs 类型不渗出去（`findings-tg.md`「三个风险」第 3 条）
2. **按码分大类、只点名少数消息**：TDLib 官方说错误码和消息「随时可能变」（`td_api.tl:16-17`），点名越多越脆。登录那几个是 T01「读出原因」的硬需求，才点名
3. **TDLib 错误原文只放行登记过的码**：「全是大写字母和下划线」只是格式，不是脱敏——符合格式的敏感值照样会漏（codex 评审 004）；错误消息偶尔会回显输入（如用户名）
4. **三条线分开存、合成时才排优先级**：三者来源不同、变化节奏不同；合成是纯函数，好测，CLI 和 GUI 读同一个结果
5. **防抖 + 成对**：断网 / 重连会抖，每抖一次说一句等于噪声（真难题 1：别打断正在朗读的内容）
6. **FLOOD_WAIT 必须等够**：`.42cog/real.md`「底线红线」：不遵守会被封号

## 三、正例 / 反例

| | 例子 |
|---|---|
| ✅ | 输错验证码 → `{"code":"auth_code_invalid","message_zh":"验证码不对，请重新输入"}`，授权步骤仍是 `wait_code` |
| ✅ | 断网 1 秒又恢复 → 什么都不说；断网 10 秒 → 说「网络已断开，正在等待网络」，恢复后说「已连接」 |
| ✅ | 收到 406 → 日志记 `td_error code=406`，界面和 CLI 都不出声（CLI 只在该请求本身失败时输出 `internal`） |
| ❌ | 把 TDLib 原文 `PHONE_CODE_INVALID` 直接读给人 |
| ❌ | 收到 `FLOOD_WAIT_30` 后立刻重试 |
| ❌ | 收线程死了，请求一直挂着不返回（必须以 `disconnected` 返回） |
| ❌ | 每次连接状态变化都播一句（「正在连接」「正在同步」「已连接」连着念三遍） |
| ❌ | 错误说明里带对方名字或消息内容 |

## 四、⚠️ 待人拍板

**E1. 状态变化的防抖时长** —— ✅ 人选 A：2 秒
- A. 2 秒：闪断不吵；真断网晚 2 秒知道
- B. 0 秒：立刻知道；抖动时会连念
- C. 5 秒：最安静；断网后可能已经发了一条才知道
- 建议：A，原型里人试听后再调

**E2. 「已连接」要不要说** —— ✅ 人选 A
- A. 只在播过「断开」之后说
- B. 从不说，只写进标题：最安静，但人不知道什么时候恢复的
- 建议：A

## 修订记录

- 2026-10-25 初稿（infra）
- 2026-10-25 按 b2 拍板同步（sync，大总管授权）：`profile_busy` 按 P1；`l1_disabled` 含义按 F1 / A2；E1、E2 结果
- 2026-10-25 issue #14 骨架（skeleton，大总管授权，人选建议）：CLI 自有码加 `not_implemented`
- 2026-10-25 issue #17 登录（login，大总管授权，人定「全按建议，把现状写进契约」）：加 `auth_phone_not_for_profile`（退出码 3）；`internal` 带 `what`；`EMAIL_CODE_INVALID` → `auth_code_invalid`（原文不登记）；不支持的授权步骤按 `auth_wrong_state` + 专门说明报；写明 `expected` 是当前步骤
- 2026-10-26 issue #22 本机接口（ipc，大总管授权 2026-10-26，属技术细节；P1 人定原意不变）：`disconnected` 加「转发中途执行方退出」一种，退出码 4；`profile_busy` 说明里提示稍后重试或常开 `serve`
- 2026-10-26 issue #22 自审后（ipc，大总管授权 2026-10-26，属技术细节；P1 人定原意不变）：`profile_busy` 按场景换说明（通用说明恢复原句）；「能不能重试」一列统一成「否（不自动重试）」再说调用方能做什么；`internal.what` 登记本机接口的几个值
