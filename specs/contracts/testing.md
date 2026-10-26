# 契约 · 测试基座 v0

> 2026-10-25 · 手下 infra 起草 · 状态：**草稿**；末尾「待人拍板」T1、T2 **已拍板**（`docs/plan/003_decisions-b2.md`）
> 管：假后端、夹具目录约定、测试服务器与发送闸豁免的单测、无障碍树导出对比在 PR 里怎么跑
> 不管：读法快照的**期望值**（人定，`specs/style/`）；内容树的夹具形状（`tree`：`specs/content-tree.md`）
> 依据：`docs/plan/001_arch-stack.md` 第四节；第七节第 6、10 条（人已定）；`CLAUDE.md` 第五节（改界面附树对比、改读法附快照差异、测试绝不向真实联系人发消息）

## 一、定了什么

### 1. 四层测试，各在哪跑

| 层 | 跑什么 | 在哪跑 | 要不要人同意 |
|---|---|---|---|
| **单测** | 纯函数：`model` 映射、reducer、`speech`、`notify` 策略、错误映射、状态合成、发送闸 | CI + 本机 | 不要 |
| **假后端集成** | `atg-core` + 假后端；`atg-cli --fake` 的端到端（参数、外壳、退出码） | CI + 本机 | 不要 |
| **测试服务器** | 真 TDLib 连 Telegram 测试服务器：登录、列会话、发收藏夹再收到；顺带计时 | 只在本机，**不在 CI** | 要（联网 + 编译名额） |
| **树导出** | 真窗口导出无障碍树，与入库快照比 | 只在本机 | 人已给长期许可：每次 ≤ 1 分钟、人关窗即停（第七节第 6 条） |

测试服务器那层用 `#[ignore]` 标住，另要环境变量 `ATG_LIVE=1` 才跑：`ATG_LIVE=1 cargo test -p atg-core -- --ignored live_`。所有 `live_` 测试的公共前置**写死**：用 `test` 档案、档案标记是测试服务器、号码以 `99966` 开头——三条有一条不满足就直接 panic，一个请求都不发。

### 2. 假后端 `FakeBackend`

- 实现同一个 `Backend` 特征（`actions.md`），**全部动作**都有；没实现的返回 `internal` 并在测试里报出来，不许静默
- 数据来自夹具目录里的一个**场景**（第 3 小节）
- **事件时间线可编排**：场景里写好「第几毫秒推什么事件」，用**注入的假时钟**推进，不真等
- **记账**：每次调用记下来（动作名、参数、凭据的开出方式），测试可以断言「`send_text` 被调了 0 次」
- 可以注入故障：某个动作返回指定错误；收线程「崩溃」（广播故障信号）
- 不碰 tdlib-rs 的全局收线程（架构方案第一节第 3 小节末条），所以测试可以并行跑多个假后端

### 3. 夹具目录约定

只有一处：`src/atg-core/fixtures/`（`atg-cli --fake`、`atg-gui --fixture` 都读这里，不另存副本——铁律 7）。

```
src/atg-core/fixtures/
  td/                          TDLib 原样 JSON，测「解析 + 映射」
    <td类型>__<情形>.json        例：updateNewMessage__reply_to_photo.json
  scenarios/<场景名>/           一个场景 = 假后端的一整套数据
    README.md                  一句话说这个场景测什么
    me.json                    自己（定收藏夹）
    chats.jsonl                会话，一行一个，用 TDLib JSON 形状
    users.jsonl
    history/<chat_id>.jsonl    每个会话的历史，从新到旧
    timeline.jsonl             可选：{"after_ms":1200,"update":{…TDLib JSON…}}
  speech/<名>.json             读法夹具：TDLib 更新对象的 JSON 数组，按顺序喂 store（`tdjson::World`）；
                               `atg-cli speech --fixture` 与 `atg-core` 的读法快照（`tests/snapshots/`）共用
  names.txt                    唯一允许用的假人名、假会话名清单
```

- 场景名用 kebab-case，按用途起：`empty`、`basic-3-chats`、`unread-mixed`、`busy-group-flood`、`reply-forward-edit`、`media-kinds`、`unknown-types`、`offline-reconnect`
- **夹具用 TDLib 原样 JSON 形状**，这样「解析 → 映射 → 树 → 读法」一次测住（架构方案第三节）
- 读法快照（`insta`）放各 crate 默认的 `snapshots/` 目录，`cargo insta review` 审，改了就在 PR 里贴差异

**夹具隐私**（`.42cog/real.md`「隐私」）：

- 人名、会话名只许取自 `names.txt`；正文是编的
- 手机号只许 `99966` 开头的测试号；不许出现邮箱、真实链接
- `chat_id` / `user_id` / `message_id` 只许在保留区间（1–99999，超级群用 `-100` + 同区间）；`minithumbnail.data`、文件的 `remote.id` / `unique_id` 必须为空或固定占位符；`username` 也只许取自 `names.txt`；日期用固定的假日期
- `scripts/check-fixtures.py`（待写）在 CI 里扫上面每一条，命中即失败
- **绝不从真实聊天截取、导出、改名后当夹具**

### 4. 必须有的单测（CI 每次都跑）

| 对象 | 必测 |
|---|---|
| 错误映射（`errors.md`） | 表驱动：每一条映射规则至少一个正例；406 不出声、连 `td.message` 字段都不建；未登记的大写码只带数字码；`Wrong database encryption key` 不落成 `auth_required`；`FLOOD_WAIT_30` 解析出 30；L2 的错误 `retryable` 一律 false |
| 状态合成与播报 | **穷举三条线的全部组合**，每种恰好落到一个 `UserStatus`；内核 `closing` + 连接 `ready` 不得是 `online`；防抖（假时钟）；只在播过坏状态后播「已连接」 |
| **发送闸** | 见下方矩阵 |
| reducer | 架构方案第四节第 2 小节列的全部（乱序到达结果一样、两个会话同消息 id 不串、过期代次不动当前窗口……）；乱序用随机排列跑多遍 |
| 收线程监督 | 假崩溃后：等待中的请求以 `disconnected` 返回；新请求直接被拒 |
| 日志隐私（`config-privacy.md`） | 喂带「暗号」字符串的：未知更新、普通消息、**类型错位的更新**（触发 `serde` 解析错误）、会触发 panic 的路径、一次发送请求；抓日志输出，**暗号一次都不许出现**；`format!("{:?}", 消息)` 也不许出现。测试服务器那层跑完后，再扫一遍 **TDLib 原生日志** `tdlib-<档案>.log`（含一次「编辑消息」，那正是 `MessageContent.cpp:7696` 会打内容的路径）和测试手机号 |
| 只有闸能发（`actions.md` 第一节第 4 小节第 7 条） | `scripts/check-td-boundary`（待写）扫源码：`tdlib_rs` 只许出现在 `atg-core::td` 里；发送类函数只许在闸后调用 |
| 限流（`errors.md` 第一节第 5 小节） | L2 遇网络错误不重试（假后端记账里只调一次）；`flood_until` 写盘后新进程仍遵守；`account_limited` 后 L2 和登录全拒 |
| CLI 外壳（`cli.md`） | 每条命令最后恰好一行 `result` / `error`；退出码与 `error.code` 一致；标准输出每行都能解析成 JSON；`schema` 输出快照 |
| 一库一进程 | 同一档案起第二个后端 → 先经本机接口转给占着的进程（P1）；连不上才 `profile_busy` |
| L1 按入口规则 | F1、A2 人定后：GUI 进会话标到最新一条、界面开着设在线、打字发正在输入；CLI 从不调 `mark_read` / `set_online` / `typing`，`mark_listened` 只在 `play --mark-listened` 时调；`draft_set` 仍按 L2 同级把关（`actions.md` 第一节第 4 小节第 9 条）。两个入口都测 |
| 已读范围 | `mark_read(up_to)` 的说明 / 计划里写出连带标记的范围 |
| 删除事件 | `from_cache` 或非永久的删除**不**生成撤回占位；只有永久删除才生成 |
| 重发 | `can_retry` 假 → 拒；`retry_after` 未到 → `rate_limited`；`need_drop_reply` → 计划写明去掉回复；付费重发 → 拒；部分成功逐条报告 |
| 话题 | 话题群里回复沿用被回复那条的话题；话题群里不回复的新消息被拒；改话题后旧凭据作废（摘要不符） |
| 发送结果未知 | 假后端扣住成败事件 → CLI `send` 30 秒后 `send_outcome_unknown`、退出码 10、没有第二次 `sendMessage` |
| 直发模式 | 见矩阵；另测：环境变量 / 配置文件打不开直发；`direct-send.log` 每条一行且不含正文、名字；留痕写不进去就拒发 |
| 钥匙访问（本机，非 CI） | 本人 Apple Development 身份 + 标识 `atg.dev` 签的程序（`scripts/dev-sign.sh`）读正式、测试档案的钥匙 → 不弹框直接拿到，**重编一次再读仍不弹**（N2，按 007 改写）；没用这个身份签的（临时签名、自签名证书如 atg-dev、未签名）、同身份但标识不是 `atg.dev` 的 → 拿不到**由 Apple Development 版新建的**条目（`config-privacy.md` 第一节第 3 小节；临时签名 / atg-dev 版见 exp005 取证 10 ③，同身份别的标识见 `docs/experiments/exp006-keychain-migrate/` 结论 3）。迁移过来的 `test`、`main` 真条目仍认旧 atg-dev 版，不拿它们测这一条。不测「静默签名拿不到」——它拿得到，人已接受（007 A1） |
| 外部打开 | 非 `http(s)` 链接、可执行类文件一律拒；打开不经 shell |

本机跑测试的前提：`cargo test` 经 `.cargo/config.toml` 的 runner（cargo 运行测试程序前先调的包装脚本 `scripts/dev-sign-run.sh`）先用开发身份签名、再带上 TDLib 库目录。除上表「钥匙访问」一行外，单测不碰钥匙串：本机没有开发身份时可设 `ATG_DEV_SIGN=0` 跳过签名，CI 里自动跳过。「钥匙访问」这行和测试服务器层（`live_`，第 1、5 小节）要读钥匙串，**必须签名**，设了 `ATG_DEV_SIGN=0` 会弹框或读不到。

**发送闸矩阵**（第七节第 10 条：默认模式下只「测试服务器 + 收藏夹」免确认，写死在闸里；第 11 条：直发模式另起几行）：

| 服务器 | 目标 | 凭据 | 结果 |
|---|---|---|---|
| 测试 | 收藏夹 | 无 | ✅ 闸自动开 `test_exempt`，发出 |
| 测试 | 别的会话 | 无 | ❌ `send_refused(no_token)` |
| 正式 | 收藏夹 | 无 | ❌ `send_refused(no_token)` |
| 正式 | 别的会话 | 无 | ❌ `send_refused(no_token)` |
| 任意 | 任意 | 目标不符 / 内容不符 / 过期 / 用过 | ❌ 对应 `reason` |
| 正式 | 收藏夹 | 合法的 Touch ID 凭据 | ✅ 发出（假后端记账里看得到） |
| 正式 | 别的会话 | 合法的 Touch ID 凭据 | ✅ 发出 |
| 正式 | 任意（含收藏夹） | 只有终端确认 | ❌ 默认模式下终端确认不开凭据 |
| 正式 · 直发模式 | 任意 | 无 | ✅ `direct_mode`，发出并留痕 |
| 测试 · 直发模式 | 别的会话（第二个测试号私聊除外） | 无 | ❌ `test_dc_saved_only` |
| 任意 · 直发模式 | 任意（测试档案仍只许收藏夹） | 编辑 / 删除 / 回应 | ✅ `direct_mode`，留痕（D1 人定：全部免确认） |
| 任意 · 直发模式 | 任意 | `account_limited` 已记录 | ❌ |
| 测试 | 与第二个测试号的私聊（N7） | 无 | ✅ `test_exempt`（发送、给所有人删都行） |
| 测试 | 别的会话 | 任何凭据 | ❌ `send_refused(test_dc_saved_only)` |
| 测试 | 收藏夹，但登录号不是 `99966` 开头 | 无 | ❌ 不豁免 |
| 任意 | 任意 | `account_limited` 已记录 | ❌ `send_refused(account_limited)` |
| 测试 | 一个**改名叫「收藏夹」**的别的会话 | 无 | ❌（收藏夹按自己的用户 id 认） |
| 任意 | 任意 | 非交互环境请求人工凭据 | ❌ `send_refused(not_interactive)` |

### 5. 测试服务器（Telegram test DC）

- 打开方式：`setTdlibParameters use_test_dc=true`（`td_api.tl:12185`），只配 `test` 档案（`config-privacy.md` 第一节第 2 小节）
- 账号：手机号 `99966XYYYY`，X 是测试 DC 号（1–3），YYYY 随便；**验证码就是 X 连写五遍**（core.telegram.org/api/auth「Test accounts」，2026-10-25 核）。号码存 `.env.local` 的 `ATG_TEST_PHONE`。**第二个测试号**（N7 人定）存 `ATG_TEST_PHONE_2`，用单独的测试档案（如 `test2`）登录，专门用来互发、验「删除给所有人」；也按 T1 被清空就自动重注册
- 官方说明的三个坑，测试要接得住：
  1. **测试服务器会定期清空**——账号可能没了，登录会走到「注册」步骤。所以**只在测试档案上**允许测试代码调注册（`registerUser`，`td_api.tl:12253`，名字取自 `names.txt`；`names.txt` 还没建，现在写死 `Atg Test`）；正式档案上注册仍是 `unsupported`
  2. **谁都能登这些号**——测试账号里不放任何私人东西；别人可能往测试账号发消息，所以测试**只断言收藏夹**里的内容
  3. **每个号每天登录次数有限**，超了报限流——换一个 YYYY；测试代码遇到 `rate_limited` 就跳过并报告，不反复重试
- 仍然要真实的 `api_id` / `api_hash`（`.env.local`）
- **省登录额度**：所有 live 测试共用一个持久的 `test` 档案、**串行**跑，登录一次后复用；`flood.json` 里记当天登录次数，超过 3 次就跳过登录类测试；`auth resend` 一次运行最多 1 次

### 6. 无障碍树导出对比在 PR 里怎么跑

- **谁跑**：每个改 `atg-gui` 的 PR，由做这个 PR 的手下自己跑（T2 人定）
- **快照入库**：`src/atg-gui/tests/a11y/<场景名>.gpui.txt` 与 `<场景名>.macos.txt` 两层（架构方案第二节第 3 小节：VoiceOver 读的是 macOS 层，两层都要）
- **导出**：`atg-gui --fixture <场景名> --dump-tree <输出目录>`；**不带 `--fixture` 就拒绝导出**，所以只可能是夹具数据；每次导出 ≤ 1 分钟，人关窗即停
- **规范化**（写快照前做，保证同样的界面导出同样的文字）：去掉几何位置、时间戳、每次都变的数字节点号；节点用稳定 id 路径标识；**保留子节点顺序**（顺序就是读的顺序）；Live 节点另记一行（树导出里看不到 Live 字段，架构方案第二节第 2 小节末段）
- **对比**：重新导出覆盖快照文件 → `git diff` 就是对比；PR 正文贴这段差异（或写「树无变化」并贴命令输出）
- **规则检查**（机器判，`CLAUDE.md` 第三节）：`scripts/check-a11y-tree.py`（待写）扫快照：每个可操作元素都有角色和名称；列表项 id 带 `(chat_id, message_id)`；另复用 `check-fixtures.py` 的规则扫手机号样式和 `names.txt` 以外的名字；命中即失败。这个脚本只读快照文本，**能在 CI 里跑**（导出本身不能）
- CI 里能做的只有：快照文件格式合法 + 规则检查。导出必须本机真窗口（GPUI 测试平台不实现无障碍，`findings-gpui.md` 第 5 条）
- 升级 GPUI 后必须全部场景重导一遍（`.42cog/cog.md`「关键关系」）

## 二、为什么

1. **夹具一处、TDLib 原样形状**：CLI、GUI、单测读同一份数据，读法和映射一起测住；铁律 7 不留副本
2. **假时钟 + 记账**：新消息提示、防抖、乱序全是时间相关的，不注入时钟就只能真等、还会偶发失败
3. **发送闸写成矩阵**：豁免只有一格，矩阵把「别的格子都不免」钉死，改闸的人一眼看到哪格变了
4. **日志隐私用「暗号」测**：比逐条审日志可靠，任何新加的日志点一漏就红
5. **树快照入库、diff 即对比**：不用为 PR 编两遍（GPUI 编译很重），也不用另存「改前」的导出
6. **测试服务器的三个坑写进契约**：官方文档明说会清空、会限流、谁都能登——不提前接住，测试会时好时坏

## 三、正例 / 反例

| | 例子 |
|---|---|
| ✅ | `atg-cli --fake basic-3-chats chats` 在 CI 里输出 3 行 `chat` + 1 行 `result` |
| ✅ | 改了消息列表的名称 → PR 里贴 `basic-3-chats.macos.txt` 的两行差异 |
| ✅ | 测试服务器上账号被清空 → 测试走注册 → 继续往收藏夹发 |
| ❌ | 夹具里出现「王小明」而 `names.txt` 里没有他 |
| ❌ | 为了跑测试临时把豁免改成「任何服务器的收藏夹」 |
| ❌ | 测试服务器联网测试放进 CI |
| ❌ | 用真实账号导出的树当快照入库 |
| ❌ | 测试里 `sleep(3)` 等防抖 |

## 四、⚠️ 待人拍板

**T1. 测试服务器的账号** —— ✅ 人选 A
- A. 固定一个号（存 `.env.local`），被清空就自动重新注册
- B. 每次跑随机一个号：不怕别人动它，但每次都走注册，更容易撞上每日登录上限
- 建议：A

**T2. 树导出谁来跑、什么时候跑** —— ✅ 人选 A：每个 PR 自己跑
「能不能跑」人已定（第七节第 6 条：随时可跑，≤ 1 分钟）。只剩：是每个改 `atg-gui` 的 PR 由手下自己跑，还是攒到一批由一个手下统一跑？
- A. 每个 PR 自己跑：对比及时；弹窗次数多
- B. 攒批：弹窗少；PR 要等
- 建议：A（每次很短，且已有长期许可）

## 修订记录

- 2026-10-25 初稿（infra）
- 2026-10-25 按 b2 拍板同步（sync，大总管授权）：一库一进程按 P1；L1 按 F1 / A2；钥匙访问按 P2 × P4；直发矩阵编辑 / 删除 / 回应按 D1；T1、T2 结果，T2 写进第一节第 6 小节
- 2026-10-25 按 b3 拍板（`docs/plan/005_decisions-b3.md`）同步（sync）：钥匙访问按 N2；发送闸矩阵加第二个测试号例外（N7）；第一节第 5 小节加第二个测试号
- 2026-10-25 issue #17 登录（login，大总管授权，人定「全按建议，把现状写进契约」）：测试档案注册名字暂写死 `Atg Test`；登录次数计数（`flood.json`）未做，另开 issue
- 2026-10-26 issue #19 speech（大总管授权 2026-10-26）：夹具目录加 `speech/`（读法夹具，CLI `speech --fixture` 与读法快照共用）
- 2026-10-26 issue #38 按 007 人定（A1 · 乙）同步（signing，大总管授权）：第一节第 4 小节「钥匙访问」由「开发证书」改为本人 Apple Development 身份 + 标识 `atg.dev`，加「重编再读仍不弹」，写明不测静默签名；「拿不到」限定为新建条目（迁移条目仍认旧 atg-dev 版）。另：单测不碰钥匙串，但 `cargo test` 经 runner 先签名，本机没有开发身份时设 `ATG_DEV_SIGN=0` 跳过签名（CI 自动跳过）；第 3 轮自审后写明「钥匙访问」行与 `live_` 层必须签名，演练出处指向 exp006
