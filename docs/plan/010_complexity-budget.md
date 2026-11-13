# 010 · 压复杂度体检：一棵语法树、一个正本、组件只做一次（#60）

> 2026-11-13 · 手下 arch（批次 b18；本线分两段：**第一段**体检 + 门槛，不改 `src/`；**第二段**按第五节清单重构）· 状态：**草稿，第一段**（只读 `src/`，体检基于 main `9d2e459`；#52 分支 `feat/52-send` 另看了一眼）
> 标准：issue #60 人原话（原样在 issue 里，不在此转述）· skill `complexity-budget`、`single-source-truth`
> 门槛已落地：`scripts/check-file-size.py`（行数 + 棘轮——只能往一个方向拧：存量超标的只许变小，变小后基线跟着下调，再也长不回去）· `scripts/check-sources.py`（正本比对四条 + 工作区成员一条）· 自测 `scripts/tests/`（按检查分文件：`test_file_size.py`、`test_sources.py`、`test_review_target.py`）（每条检查在改坏的副本上必须红；条数以文件为准）· CI 新工作流 `.github/workflows/arch.yml`（自测、行数、比对）与 `arch-guard.yml`（防 PR 放宽门槛，跑 main 上那份）
> 给手下的守则：`specs/architecture-rules.md`（一页）

## 〇、结论先说

> 读前几个词：**手下** = 一个干活的 AI 会话，各管一条工作线（arch 线 = 本线；send2 线 = #52 发送；media 线 = #4/#59 语音；fix 线 = 读法修补）；**大总管** = 派活、收报告、跟人说话的那个会话，「大总管定」是在人授权的技术范围内定的；**批次 b18** = 这一轮派活的编号。**正本** = 一件事唯一说了算的那份定义（唯一真相源），别处要么从它生成，要么由机器跟它比对。**内容树**（`specs/content-tree.md`）= 本项目给「一切要读出来、打印出来的东西」定的一种有类型的数据形状（`tree::Doc`，像编译器里的语法树，但装的是消息、提醒、状态）；它只放数据，句子由读法模块 `speech` 从树生成，JSON 由 `json` 模块从树生成。**档案** = 一个 Telegram 账号在本机的数据目录，同一时刻只许一个进程占着（TDLib 的库不能两个进程同开）。**TDLib** = Telegram 官方的客户端库，本项目从它拿数据。

1. **最大的乘法在 CLI 的命令接线**：每条命令要在 `main.rs`（`run_X`）、`session.rs`（起真实后端的 `X`）、`serve.rs`（`execute` 的一支）各写一遍，「起后端 → 干活 → 关后端」写了 4 遍，「`saved` → 自己的 id」main 上写了 2 遍（`history`、`watch`），#52 又加第 3 遍。命令 × 跑法是乘法——三种跑法：**直跑**（这个 `atg-cli` 进程自己起 TDLib、自己干活）；**转发**（档案已被另一个常驻进程占着，这个进程只把命令经本机套接字转过去，`forward.rs`）；**执行方**（那个常驻进程——`atg-cli serve`，以后是 GUI——收到转来的命令替它执行，`serve.rs`）。要收成「每条命令一行登记 + 一条执行路径」（第四节第一行、第五节 R5）。
2. **树有了，但不是一切都走树**：消息、会话项走树（好）；但 `watch` 的「新消息，」「对方正在输入」「断网期间收到……」、`history` 的「以下为未读」、`links` 的整条输出（含它自己做的遮挡）、每条命令文字模式的统计句，都是 CLI 里现拼的中文，绕开了 `tree` 和 `speech`。树里早有 `Doc::Alert`，正式路径上**从没建过**（第三节）。
3. **超 600 行的文件 14 个**，其中 3 个（`serve.rs`、`login.rs`、`chats.rs`）超线的主因是行内测试（测试占一半以上）：把测试挪出去，原文件就降到线下，零行为变化。但**测试文件本身也守 600 行**：`serve` 的测试 763 行、`login` 685 行要按主题再分成两份（`foo/tests/<主题>.rs`），只有 `chats`（468 行）挪成一个 `tests.rs` 就行。其余 11 个：有行内测试、挪出后仍超线的 7 个（`notify` 629、`history` 706、`watch` 873、`text` 662、`store/mod` 967、`speech` 822、`td` 645）；没有行内测试的 4 个（`tdjson` 842、`fake` 825、`tree/mod` 604，以及本身就是测试文件的 `tests/reducer.rs` 2222）——都得按职责拆（第一节）。
4. **契约表与代码现在是一致的**——四条正本比对第一次跑就全绿；问题是以前靠人记着同步，现在由 CI 盯（第二节）。`arch` 和防门槛被放宽的 `arch-guard` 定为合并必过项（人定 2026-11-13 B18-6，出处：大总管账本 `state/coord.md`（MAIN 工作区，2026-11-13 待定清单第 369 行 B18-6，本分支开出时还没有；批次收尾时进 `docs/plan/` 的拍板记录）；`scripts/github-ruleset.json` 在本 PR 里改，合并后由大总管在 GitHub 上应用；**应用之前它们红了只是提示，拦不住合并**），应用后红了不能合。

## 一、行数：超 600 行的文件与拆法

基线 `scripts/file-size-baseline.json` 按 main 记 14 个，只许变小。#52（发送闸 + `send`，由 send2 线在做）新增的三个文件会超线，门槛一进 CI 就红。两条路：A）#52 先合，门槛后进，三个文件按合并后的现状进基线、第二段最先拆；B）#52 合并前先拆。大总管定 A（2026-11-13，不拖 send2）。

**和别的线谁先合**：fix 线（`fix/68-70-speech`）会让 `speech.rs`、`tree/text.rs` 变小。它若在门槛**之后**合，它的 PR 要跑 `--tighten`（否则「变小没收紧」会红）；它若在门槛**之前**合，本 PR 按下面 merge 最新 main 后重记基线时自然记成新数，不用另做。

**按 A 的操作顺序**：#52 合并 → `git fetch` → 本 PR 分支 `git merge origin/main`（分支已推过远端，不能 rebase，rebase 后要强推，项目禁止；合并时是 squash，分支里的合并提交不进 main）→ `python3 scripts/check-file-size.py --write-baseline --rev origin/main` 在**本 PR 里**重记基线（合并和重记用同一个 `origin/main`）→ 再合本 PR。main 上那时还没有门槛脚本和 `arch-guard` 工作流，`arch-guard` 不会运行（它读的是 main 上的文件）（脚本那时打印的「要带标签」提示不适用），**本 PR 不需要** `baseline:loosen` 标签。门槛进库**以后**任何 PR 再重记或放宽基线、改门槛文件（门槛脚本、自测、`arch` / `arch-guard` 工作流，任何改动），才要人或大总管加这个标签（见守则第 1 条）。

**拆文件时别改模块根的名字**：`foo.rs` 拆成 `foo.rs` + `foo/<子模块>.rs`（Rust 2018 起的写法），不要改名成 `foo/mod.rs`——基线里记的是 `foo.rs` 这个路径，改了名，新路径会被当成「新超标」、旧路径当成「已删除没收紧」，要标签才过。

**怎么挪行内测试**（Rust）：`foo.rs` 里的 `mod tests { … }` 换成一行 `#[cfg(test)] mod tests;`，测试原样搬到 `foo/tests.rs`；`foo.rs` 不用改名成 `foo/mod.rs`。测试超过 600 行时，`foo/tests.rs` 只留 `mod <主题>;` 几行，测试按主题放 `foo/tests/<主题>.rs`。

| 文件 | 总行 | 其中测试 | 拆法（按职责，不切 part1/part2） |
|---|---|---|---|
| `atg-core/tests/reducer.rs` | 2222 | 全是 | 已按主题分节（排序 · 墓碑 · 按字段合并 · 版本 · 重复 · 换 id · 有效内容 · 投影 · 拍板 006（`docs/plan/006_decisions-b7.md`）· 第八至十轮）：每节一个文件，`tests/reducer/` 下共用「造假数据」一节（并入 R3 testkit） |
| `atg-cli/src/watch.rs` | 1343 | 470 | `watch.rs` 留作模块根（Watcher 与直跑循环）· `watch/hub.rs`（订阅者 Sub/Hub，执行方用）· `watch/tests.rs`；第 412–873 行要按函数分开处理，**不是整段挪个文件**：约 412–627 行「事件 → 数据」随 R6 改成事件节点 + `json` 渲染后删掉；约 630 行状态行（`status_line`）随 R6 改用状态节点（S9）；约 651–693 行目标解析（`resolve`）随 R4 收进 `target.rs`；约 694–865 行起停与订阅循环（`run`、`watch_loop`、取论坛话题设置）随 R5 成为登记表里的订阅类命令；约 866 行起关会话（`close_chat`）随 R5 归进订阅的清理；读法句子去 `speech` |
| `atg-cli/src/serve.rs` | 1328 | 763 | 测试挪出即 565；R5 之后 `execute` 那一大段消失 |
| `atg-cli/src/history.rs` | 1264 | 558 | `links` 那段（603–706）独立成 `links.rs`；起后端那段（138–225）并入 R5；测试挪出 |
| `atg-core/src/store/mod.rs` | 1263 | 296 | `store/patch.rs`（Patch、FieldSeqs、按字段合并）· 按输入分 `store/chats.rs`、`store/messages.rs`；`mod.rs` 只留 Store、Input、查询 |
| `atg-core/src/notify.rs` | 1247 | 618 | 测试挪出即 629；再把 `effective`（三层静音的取值，194–246）拆成 `notify/mute.rs` |
| `atg-cli/src/login.rs` | 1077 | 685 | 测试挪出即 392 |
| `atg-core/src/tree/text.rs` | 1058 | 396 | fix 线分支上已把测试挪到 `tree/text/tests.rs`（未合）（剩约 671；比按 main 算的 662 多，是 fix 线同时改了别的地方）。还要做的：建树（`build`，240–500）留下；查询（`links`、`plain_text`、`contains_spoiler`，503–663）成 `tree/query.rs` |
| `atg-core/src/speech.rs` | 900 | 78 | fix 线分支上已把测试挪出（未合）（剩 847；按 main 算是 822，差的是 fix 线同时改的读法代码）。还要做的：`speech.rs` 留作模块根（brief / detail / full 入口）· `speech/service.rs`（系统消息，482–556）· `speech/content.rs`（正文与附件，557–）· 常量表一处 |
| `atg-cli/src/chats.rs` | 847 | 468 | 测试挪出即 379 |
| `atg-core/src/tdjson.rs` | 842 | 0 | 它是 TDLib JSON → model 的**唯一映射**（正式路径和夹具共用，好）。按对象拆：`tdjson/event.rs` · `chat.rs` · `message.rs` · `text.rs`（实体）；夹具专用的 `World` 单放 `tdjson/world.rs` |
| `atg-core/src/fake.rs` | 825 | 0 | 按后端能力拆：`fake/auth.rs` · `fake/read.rs`（会话、历史）· `fake.rs` 留作模块根（共用状态、开关） |
| `atg-core/src/td.rs` | 773 | 128 | 同上：`td/auth.rs` · `td/read.rs` · `td.rs` 留作模块根（起停、收线程） |
| `atg-core/src/tree/mod.rs` | 604 | 0 | 消息节点（Message/Head/Content/Attachment/Span）与其余顶层节点（ChatItem/Alert/Status）分两个文件 |

#52 合并后另有：`atg-core/src/send.rs` 1490 · `gate.rs` 1145（send2 第 9 轮后已 1176；基线按合并那一刻的现状记）· `atg-cli/src/send.rs` 870（新超标）；`serve.rs`、`fake.rs`、`td.rs`、`tdjson.rs`、`chats.rs` 还会变大（实测 #52 分支：1431 · 1159 · 968 · 862 · 853）。

## 二、正本盘点

「比对」= 已有或要加的机器检查；「生成」= 第二处由正本导出、不手写。✅ = 本 PR 已接进 CI（`.github/workflows/arch.yml`）。

| # | 什么事实 | 正本 | 第二处（及更多） | 做法 |
|---|---|---|---|---|
| S1 | 错误机读码 | `atg-core/src/error.rs` `CoreError::code` + `atg-cli/src/out.rs` `Code::as_str` | `specs/contracts/errors.md` 码表 + 「CLI 自有码」 | 比对 ✅ `error-codes` |
| S2 | 退出码 | `out.rs` `core_exit` + `Code::exit` | `specs/contracts/cli.md` 第一节第 4 小节 | 现在比对 ✅ `exit-codes`；以后**生成**（R11，#81）（`out.rs` 里 `exit_codes_match_contract` 手抄了一份数，留着当单测，正本仍是 match） |
| S3 | 子命令清单 | `atg-cli/src/args.rs` `enum Command` | `cli.md` 子命令表；**还有**：`serve.rs` `execute` 认哪些命令、`args.rs` 里 `Serve` 的注释「（chats、history、links、auth state）」、`config-privacy.md`「能转的」清单 | 前两处比对 ✅ `subcommands`；「能不能转发」R5 后由命令登记表的一列生成，注释删掉 |
| S4 | 外壳版本 `v`（CLI 每行 JSON 外面那层 `{"v","kind","at","data"}` 的版本）/ 本机接口协议版本 | `tree::SCHEMA_V` · `ipc::PROTOCOL_V` | `cli.md`、`content-tree.md` 的外壳例子；`config-privacy.md` 的请求例子 | 比对 ✅ `versions` |
| S5 | 树的形状（节点、字段） | `atg-core/src/tree/` 的类型 | `content-tree.md` 节点清单；JSON 消费方 | **生成**：`schemars` 从 Rust 类型导出 JSON Schema 文件进库；**金样测试**（每次测试重新导出，和库里那份逐字比，不同就红，提示跑哪条命令更新）；版本号按 `content-tree.md` 第八节的兼容规则判：和合并目标上那份比，**结构上的破坏性改动**（改字段名、删字段、改类型、改判别值）没升 `SCHEMA_V` 就红；**改含义**（名字、类型都不变，只是算法或单位变了）Schema 看不出来，靠 `speech` / JSON 快照测试和 PR 审查时人看版本号兜底；新增可选字段、新节点、新取值**不用**升版本，重新生成即可。这份树的 Schema 是 `schema` 子命令输出的一部分（第五节 R9）。要加依赖，**Q1 人定 A**（第六节） |
| S6 | 参数范围 `-n` | `args.rs` clap `range(1..=100_000)` / `range(1..=1000)` | `serve.rs` 里再写一遍 chats 的范围；`history::query` 再写一遍 history 的 | 收成 `args.rs` 里两个常量，clap 和 `query` 都引用（R4） |
| S7 | 会话写法与用法错误句「会话写数字 id，或 saved（收藏夹）」 | — | `history::query`、`links_query`、`watch::query`、#52 `send.rs` 各一份 | 一个 `ChatRef::parse_arg` 带这句（R4） |
| S8 | 隐藏内容的说法与遮挡规则 | `content-tree.md` 第八节 → `tree::mask` + `speech::HIDDEN` | `history.rs` `HIDDEN_LINK`（同一个「隐藏内容」）且 `render_links` 自己判断要不要遮 | links 走树、走 `mask`（R6） |
| S9 | 状态行字段（core/auth/net/user_status/pending_sends/direct_send…） | 应是 `Doc::Status` | `serve.rs` `status_line` 与 `watch.rs` 各手拼一份 `json!` | 扩 `tree::Status`，两处都渲染它（R6） |
| S10 | 「现在」与时区 | — | `watch::now_ms`、`link::now_ms`、`atg-cli/speech.rs::now`（秒）、`out::now_rfc3339_ms`；系统时区 `local_offset_at` 在 CLI 里，GUI 也要 | 收进 `atg-core/src/tree/time.rs`（R8） |
| S11 | 测试用的 TDLib JSON 造法 | — | 手拼 `json!({"@type": …})` 散在 9 个文件约 90 处；`temp_root`/`profile` 5 份 | 数据与后端夹具（TDLib JSON 造法、临时档案）收进一个 `atg-core::testkit`（R3）。CLI 的输出替身（`Cap`）**不进 core**：`history`、`serve` 的两份收标准输出 / 标准错误（`Sink`），可在 `atg-cli` 的测试里合成一份；`watch` 那份实现 `LineOut` 且带「到条件就停」，不等价，单留 |
| S12 | 超时与轮询间隔 | 各自 | `ME_TIMEOUT` 两份（history、watch）同为 5 秒；`chats::SYNC_TIMEOUT` 被 `history` 转引（好） | **由 fix 线做**（#78）：命令层期限全收进 `atg-cli/src/deadlines.rs`，每个一行注释（等什么、为什么是这个数、出处），两份 `ME_TIMEOUT` 合一；内核机制类（`SHUTDOWN_TIMEOUT`、ipc 两个）留原处 |
| S13 | 互斥锁取值助手 `lock<T>` | — | `fake.rs`、`link.rs`、`td.rs` 各一份 | 一份 `pub(crate)`（R8） |
| S14 | 一行要不要提示 | `notify::Verdict.alert`（结构化） | `forward.rs:217–240` `count_line` 文字模式下拿中文前缀「新消息，」「对方正在输入」「断网期间收到」**重判**这一行要不要提示——读法句子成了协议，改一个字计数就错（media 线自审发现，已核实） | 执行方转发的每一帧事件带结构化的 `alert` 标记（文字模式也带，不只是那行字），发起方按标记计数，不再解析句子；这样与 `cli.md` 第一节第 5 小节例外④「Ctrl+C 时发起方按收到的行在本地写结尾」一致；随 R5、R6 |
| S15 | 「哪些命令能转发」的文字说明 | 同 S3 | `serve.rs:10`、`forward.rs:10` 的模块注释和 `args.rs:127` `Serve` 的文档注释（即 `--help` 那条，D4）三处都写「chats、history、links、auth state」，**漏了已经能转的 `watch`**（media 线自审发现） | 注释不再列清单，只指向登记表（守则第 6 条）；随 R5 |
| S16 | `src/` 下哪些目录是 crate | 根 `Cargo.toml` `members = ["src/atg-*"]` | `src/` 下的目录名（以 `atg-` 开头却没有 `Cargo.toml` 会让所有 cargo 命令失败） | 比对 ✅ `workspace`（守则第 10 条） |

`find-duplicates.py`（skill 自带）另报 7 组同名类型（`Query`×3、`Cap`×3、`Feed`、`Shared`、`Head`、`Origin`、`Message`）：都是各模块自己的东西、同名不同事；其中 `Cap` 是 CLI 测试里的输出替身，`history` 与 `serve` 那两份可在 `atg-cli` 测试里合一，`watch` 那份接口不同、单留（S11）。其余不动。

### 文档与注释的过期（别的线自审顺带发现，归在这里统一处理）

根子是同一个：**注释和 README 里抄了一份代码里的清单**（做了哪些命令、能转哪些、状态有几条线），代码一变它就过期。处理一律是「删掉清单，改成指向正本」，不是再抄一遍新的。

| # | 在哪 | 是什么 | 来源 |
|---|---|---|---|
| D1 | `src/atg-cli/README.md:5–21` | 「已做」只列到 login/auth/logout、chats、speech；history、links、serve、watch 都已做了 | media 自审 |
| D2 | `src/atg-cli/src/main.rs:3–6`、`src/atg-cli/src/args.rs:3` 文件头 | 「已做」清单同样过期（`main.rs` 漏 `watch`；`args.rs` 漏 `watch`、`serve`） | media 自审 |
| D3 | `src/atg-core/src/lib.rs` | 把 `notify` 标成占位，实际已做（#53） | media 自审 |
| D4 | `args.rs:127` `--help`（`Serve` 那条）| 能转发的范围漏了 `watch`——与 S15 是同一处，随 S15 一起改，这里只为文档清单完整再列一次 | media 自审 |
| D5 | `notify.rs` 等处的 F6、N5 这类拍板代号 | 没写出处（哪份 `docs/plan/00N_decisions-*.md`）；Unigram 出现处没说明是什么 | media、send2 自审 |
| D6 | `link.rs` | 状态线一处写「两条」、一处写「三条」（契约是三条：内核 / 授权 / 连接，`errors.md`） | send2 自审 |
| D7 | `speech` 子命令的 `--help` | 用了没解释的术语 | send2 自审 |
| D8 | `src/atg-cli/src/args.rs:27–29`（`Global` 的注释）、`src/atg-cli/README.md:21` | 写 `--profile` 只管登录组与 chats/history/links、`--tz` 只有 speech/history 用、`--redact` 只在 chats/history/links 拦；实际 `watch` 也读 `--tz`、也拦 `--redact`，`serve` 也用 `--profile` | arch 自审 |
| D9 | `src/README.md:16–43` | 「常用命令」里的路径都要在仓库根执行，文件却放在 `src/` 里，没写这个前提 | arch 自审 |

D1–D9 随第二段相关 PR 一起改；不单开大扫除。守则第 6 条防再犯。

另：media 线自审还报了一条 **P1 正确性问题**（不是复杂度问题，不归本清单，大总管已知）：`serve` 单线程、给每个挂着的 `watch` 阻塞写（写超时 60 秒），一个读端停住就卡住整个执行方。R5 改接线时顺带要看，但修法要单开 issue。

## 三、一棵树

### 1. 现在绕开树的

| 输出 | 现在在哪拼 | 该挂到哪 |
|---|---|---|
| 新消息提醒（「新消息，」+ 读法）、正在输入、断网汇总、「同步完成，之后的是新消息」（issue #74） | `atg-cli/src/watch.rs` 常量与 `render_inner`（拼进 `speech.brief` 的那段约在第 207 行起） | 已有的 `Doc::Alert` 扩成「这条为什么提醒 / 汇总几条几个会话 / 正在输入」，数据从 `notify::Judged` 来；句子全进 `speech` |
| 链接列表与链接的遮挡 | `history.rs` `render_links` | `Doc::Link`（序号、文字 spans、地址、在不在剧透里），遮挡走 `tree::mask`，读法「链接 N，……」进 `speech` |
| 每条命令的结尾统计（JSON `result` 的数据 + 文字模式的那句话） | `main.rs` `render_chats`、`history.rs` `render`、`watch.rs` `result_line`，#52 的 send/resend 又各一份 | `Doc::Summary`（每条命令一个变体，只放数据——数字、布尔、游标字符串如 `next_cursor`，**原有字段一个不少**，不放句子）；JSON 照出 `data`，文字由 `speech` 说 |
| 状态行 | `serve.rs`、`watch.rs` | 扩 `Doc::Status`（S9） |
| 「以下为未读」分隔 | `history.rs` `UNREAD_BELOW` | `content-tree.md` 第一节已预定的 `divider{kind: unread}` 节点，由投影插进消息序列；句子归 `speech` |
| `watch` 的其余事件：消息删除、发送成功 / 失败、已读位置、会话位置、草稿、通知设置……（`watch.rs` `event_data`，约第 495–587 行，每种手拼一份 `json!`） | `atg-cli/src/watch.rs` | 事件节点（每种 `CoreEvent` 一个变体，只放数据），由投影从事件 + Store 建，JSON 统一走 `json` 模块；`watch.rs` 只剩订阅与转发。现有每种事件行的字段与含义逐项对照、一个不少（快照兜底） |
| `auth` 的每一步、`version` 的那一行 | `main.rs` `emit_step`（约第 141 行）、`version`（约第 388 行）手拼 `json!` | 登录步骤节点（`content-tree.md` 第一节「以后加」已列）与版本信息节点；句子归 `speech` |
| 发送前的 `plan` 行（#52） | `atg-cli/src/send.rs` | `Doc::Plan`（R6 时一起收） |

规矩不变：**树只放数据**；句子只在 `speech`；JSON 只在 `json`；渲染器不写 `_ =>`，加节点编译器逼每个渲染器补上。

### 2. 以后的东西怎么挂（给 media 和后续线）

- **文件**：所有带文件的附件（图片、视频、语音、圆视频、音频、文件）共用一个 `FileRef { id, size, local: 未下载 | 下载中{已得} | 已下载 }`，挂在附件节点上。`download`、`open`、`play` 都只认 `FileRef`，不各自拿 TDLib 的 file 对象。下载进度行 = `Doc::Download`。
- **语音与转写**：`Attachment::Voice` 加 `transcript: Option<Transcript>`，`Transcript { spans, source: 电报服务端 | 本机, state: 进行中 | 完成 | 失败 }`。本机转写和 Telegram 的转写是同一个节点，只是 `source` 不同；读法只认节点，不问来源。
- **播放**：播放状态是会话里的临时状态，不进消息树；但 `play` 打印的每一行（`kind:"playback"`）是一个 `Doc::Playback`，读法同样由 `speech` 出。
- **投票**：`Attachment::Poll` 现只有题目；加 `options: Vec<PollOption { spans, votes, chosen }>`、`closed`、`multiple`、`quiz`。
- **消息下方的按钮**：`Content` 旁加 `buttons: Vec<Vec<Button>>`（行 × 列），`Button { spans, kind }`，`kind` 是开放集合（`Url` · `Callback` · …… · `Unknown { td_type }`，兜底变体照第一条硬规矩）。按了之后做什么是动作（`actions.md`），不在树里。

## 四、复用：做了不止一次的，收成哪一个

| 能力 | 现在几份 | 收成 | 边界 |
|---|---|---|---|
| 命令接线（解析参数 → 查询 → 干活 → 输出） | 3 处 × 每条命令（`main.rs` run_X · `session.rs` X · `serve.rs` 一支）；`tz` 塞进转发参数 2 份 | `atg-cli/src/cmd.rs`：一张登记表，每条命令一行（名字、能不能转发、要不要带 `tz`、`fn(&Ctx, 参数, sink) -> 退出码`）。直跑 = 起后端 + 同一个函数；执行方 = 已起的后端 + 同一个函数 | 只在 `atg-cli`；不做 trait 层级，是一张表 + 函数指针 |
| 起后端 → 干活 → 关后端 | 4 份（`chats::run_with`、`history::with_backend`、`session::drive`、`watch::run` 开头），#52 第 5 份 | `session::with_backend` 一份 | 只在 `atg-cli` |
| 真实后端 / 假后端 / 没编 TDLib 三岔 | `session.rs` 4 份几乎一样的函数 | 一个 `session::real(g, command, f)` | 同上 |
| 目标解析（`saved` → 自己的 id、`getChat` 后确认在） | 3 份（`history::resolve`+`load_chat`、`watch::resolve`、#52 `send::resolve`） | `atg-core/src/target.rs`：GUI 也要「收藏夹」 | core 只管把 `ChatRef` 变成确实存在的 `ChatId`；参数怎么写（`saved`）归 CLI |
| 挂错地方的共享助手 | `speech_ctx`（读法上下文）、`ChatRef` 住在 `history.rs`，`watch`、`serve` 绕道 `crate::history::` 去取；`links` 整条命令也在 `history.rs` | `speech_ctx` 去 `atg-cli/src/speech.rs`（时区解析已在那）；`ChatRef` 随上一行；`links` 独立成 `links.rs`（第一节） | 按命令命名的文件只装那条命令 |
| 输出一行（树 → JSON 行补 `at` / 文字一行） | 3 份（chats、history、watch）+ #52 2 份 | `out::emit_doc(sink, format, doc, cx, layers)` | `out.rs` |
| 测试造数据 | 见 S11 | `atg-core/src/testkit.rs`，`#[cfg(any(test, feature = "testkit"))]`，atg-cli 的 dev-dependency 开这个特性 | 只给测试用，正式构建不编 |

**组合动作放哪（定论，2026-11-13 答 media）**：「转写一条」这类跨 core 与 audio 的组合动作，以及 R5 的命令登记表、`serve` 的执行逻辑，都放 `atg-cli`，但写成**库代码**——不碰 clap、不直接写标准输出，吃后端和参数，吐树节点 / 结果。GUI 动工时给 `atg-cli` 加一个 lib 目标（`src/lib.rs`，二进制只剩参数解析和输出），GUI 直接链接它。理由：
- GUI 本身就是执行方（`config-privacy.md`：档案被 GUI 占着时，CLI 把命令转给 GUI 执行），它不能反过来经本机接口去调 CLI，那是绕圈
- 不另建编排 crate：它和 CLI 的命令层会是同一批代码，建了就是两份；「CLI 是唯一能力来源」字面成立——GUI 用的就是 CLI 的那份库
- 依赖方向：`atg-gui → atg-cli(lib) → atg-core、atg-audio`；`atg-core` 与 `atg-audio` 互不依赖

**新模块的边界**（media 线要建 `atg-audio` 时按这个看）：

- `atg-audio` 是独立 crate（F8 人定「新建的 atg-audio」，`docs/plan/003_decisions-b2.md` 第 19 行）：输入是**本地文件路径**和控制命令（pause/resume/seek/speed/stop），输出是播放状态流。它**不认识** Telegram、Store、树、CLI；解码（Opus）在这里，播放和转写共用这一份解码。
- `atg-core` 管：拿到 `FileRef`、下载、把转写结果写回树（`Transcript`）、`mark_listened` 的把关。
- `atg-cli` 的 `play` / `transcribe` 只是胶水：core 下载 → audio 播放 / 转写 → `Doc::Playback` / `Transcript` → `speech`。
- 转写引擎若要拉大依赖（模型、C++ 库），单成一个 crate（如 `atg-stt`），同样只吃本地音频、吐文字，不认识 Telegram。

## 五、重构清单（第二段按这个顺序，一项一个小 PR）

收益：删掉的重复 / 乘法变加法的程度。风险：改动面、行为变化的可能。冲突：和在途线（send2 = #52，media = #4/#59）撞不撞文件。每项一轮只碰 ≤4 个实体。

| 序 | 项 | 收益 | 风险 | 冲突 | 什么时候 |
|---|---|---|---|---|---|
| R1 | 拆 #52 新增的三个超线文件（core `send.rs`、`gate.rs`，cli `send.rs`） | 中：三个新超标清掉 | 低（按职责挪，不改逻辑） | 与 send2 撞 | #52 合并后**最先**（大总管定） |
| R2 | 行内测试挪出（chats、login、serve、notify、history、watch、store、td；`speech`、`tree/text` 的已由 fix 线 `fix/68-70-speech` 挪走，不重复做）；超 600 行的测试按主题再分 | 中：chats、login、serve 3 个文件守线（login、serve 的测试各分两份），其余明显变小 | 极低（零行为变化） | 挪哪个文件就和改它的线撞：等那条线合并 | R1 后；按文件逐个，避开 media 在改的 |
| R3 | `atg-core::testkit`（S11：只收数据与后端夹具；CLI 输出替身留在 `atg-cli` 测试） | 中：删掉 9 个文件里重复的造数据 | 低（只动测试） | 低 | R2 同批或之后 |
| R4 | 目标解析与参数范围收一（S6、S7、S12；`target.rs`） | 中：3→1，#52 的第 4 份一并收 | 低 | 动 history/watch/send：#52 后 | R3 后 |
| R5 | 命令登记表 + 一条执行路径（`cmd.rs`；删 `session.rs` 四个三岔函数、`serve.rs` 各支、`main.rs` run_X）。登记表里命令分两种：**一次性**（函数 → 结果与退出码）与**订阅**（`watch` 这类：返回订阅句柄，定义取消、清理、结尾帧；保留 `cli.md` 第一节第 5 小节六条例外与 `config-privacy.md`「流式订阅」的全部行为）。先写清这两种的接口与验收（并发订阅、读方断开、发起方 Ctrl+C、上限 8 个），再删原分支 | **高**：命令 × 跑法从乘法变加法，以后加命令只加一行 + 一个函数 | 中（改接线；`serve` 转发快照测试 `forwarded_*_matches_direct` 兜底） | 和所有加命令的线撞：#52 后、media 加 `play`/`download` 前最好 | R4 后，尽早 |
| R6 | 绕开树的输出收回树（第三节第 1 小节**表里每一行**：Alert、`watch` 的其余事件、Link、Summary、Status、Plan、未读分隔 `divider`、登录步骤、版本；逐种输出先列出现有字段、映射到节点类型，字段一个不少；含 #74，PR 写 Closes #74） | **高**：「一切都是树」真正成立，GUI 直接复用 | 中（文字模式输出的字要逐字不变：快照兜底；句子本身不改措辞） | 动 watch/history/send 渲染 | R5 后 |
| R7 | 拆 core 大文件（store、speech、tdjson、notify、text、fake、td、tree/mod、reducer 测试） | 中：行数基线清零 | 低—中（挪动为主） | media 会改 tree/tdjson/fake/td：逐个排 | 穿插，每次一个文件 |
| R8 | 小零件归一（S10 时间与时区进 core、S13 `lock`） | 低 | 低 | 低 | 随手 |
| R9 | 树的 JSON Schema 生成 + 金样 + 按兼容规则判版本（S5；Q1 人定 A，`schemars`）。输出格式和内存类型不同的（如 `Time`：内存是整数，JSON 是 RFC3339 字符串；以及其他自写序列化的类型）要**手写 Schema 映射**，不能只靠派生；并用真实渲染结果（`speech` / JSON 快照与夹具产出的每一行）对生成的 Schema 做校验，确认说明书与实际输出一致。**只交付树这一块**；`schema` 子命令按 `cli.md` 第一节第 6 小节还要外壳、各命令参数、`result.data`、事件、错误码、对外等级、退出码表、单命令筛选——其中退出码与错误码随 R11（#81），其余见 #82，不在 R9 宣称完成 | 高（结构上的破坏性改动必升版本） | 低 | 动 `tree/` 的类型（加 derive）：media 也在改树，等它合并后做 | R6 之后（树的节点那时才齐） |
| R10 | 禁止无期限等待：根目录 `clippy.toml` 的 `disallowed-methods` 禁 `atg_core::link::block_on`，只许经 `link::block_on_until`（有截止时间、响应 Ctrl+C）；编译期拦，不另写扫描脚本 | 中：卡死的一类 bug 从源头堵住 | 低 | 无：**由 fix 线在 #78 里一并做**（24 处全换掉 + 加 `clippy.toml`，零例外，大总管定 2026-11-13），arch 线不再做；`clippy.toml` 已列进 `arch-guard` 管的门槛文件 | 随 #78 |
| R11 | 退出码表、错误码表**由代码生成**（#81）：Rust 测试从**两套**正本渲染 `cli.md` / `errors.md` 里带 BEGIN/END 标记的生成区，逐字比对，不同就红并提示更新命令：内核码（`CoreError::code`、`core_exit`）与 CLI 自有码（`Code::as_str`、`Code::exit`：`usage`、`interrupted`、`not_implemented`，以及借用的 `disconnected`），外加「成功 = 0、错误码不占 0」；`error.code` 逐个列出不用通配；含义列放生成区外。**先证明覆盖不缩水**（现有 `exit-codes`、`error-codes` 的每条负例在生成方案下都还会红），再删 `check-sources.py` 的这两条 | 高：一整类「解析表格语义」的问题消失（019 门槛第 4、5、7 轮都出在这里） | 低 | 动 `out.rs`、`error.rs` 的测试与两份契约：#52 后 | R5 前后均可 |

R5、R6 做完，`check-file-size` 的基线预计还剩 store、notify、tdjson 几个大头，R7 收尾。

## 六、拍板（人定）

**Q1 · 内容树的「形状说明书」要不要由程序自动生成**
- 题面：CLI 每行 JSON 里装的是内容树。树长什么样（有哪些节点、每个节点有哪些字段）现在只写在 Rust 代码和 `content-tree.md` 的文字里，没有一份机器能读的说明书。后果：以后改了树的某个字段，忘了把版本号 `v`（`tree::SCHEMA_V`）加一，**没有任何东西会报警**；读 JSON 的 AI 和脚本会悄悄读错。`cli.md` 早就留了 `schema` 子命令（吐出这份说明书），一直没做
- 选项：A）引入一个现成的 Rust 库 `schemars`，从代码自动生成说明书（JSON Schema 格式）存进仓库；每次测试重新生成一遍和存的那份比，改了字段没升版本号就报警（按 `content-tree.md` 第八节的兼容规则：改名、删字段、改类型这类结构上的破坏性改动会自动报警，新增可选字段、新节点不用升版本；只改含义而结构不变的它看不出来，靠快照测试和审查兜底。codex 019 文档第 1、2 轮补的细化，不改人定的选项）；这份说明书会成为 `schema` 子命令输出的一部分（子命令的其余部分——参数、事件、错误码等——另做，#81、#82）。B）不引库，写一条测试只核对「节点种类的名单」：加了、删了节点会报警，**改了字段不会**。C）先不做
- 代价：A 多依赖一个库（纯 Rust、很成熟、不牵涉系统库，只在编译时用），每个树类型上加一行标注；少数输出格式和内存不同的类型（如时间：内存是整数、输出是日期字符串）要手写一小段映射，并用真实输出校验说明书（codex 019 文档第 3 轮补的细化，不改人定的选项）；B 改字段照旧没人报警；C 版本号继续靠人记
- 建议：**A**
- 默认（人不回时）：B——不加依赖，先把能比的比上
- **人定：A**（2026-11-13，大总管转达）。落到第五节 R9：引入 `schemars`，从树的类型生成说明书进库，测试重新生成、和合并目标那份比，结构上的破坏性改动（改名、删字段、改类型）没升 `SCHEMA_V` 就红（只改含义、结构不变的，Schema 看不出来，靠快照测试与 PR 审查兜底），新增可选字段与新节点不用升（`content-tree.md` 第八节）；这份说明书是 `schema` 子命令输出的一部分

## 修订记录

- 2026-11-13 · 手下 arch：初稿（第一段体检）
