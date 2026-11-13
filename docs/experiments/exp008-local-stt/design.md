# 设计 · #4 听语音 + #59 语音转文字（共用一套音频底座）

> 2026-11-13 · 手下 media · 依据 exp007（解码）、exp008（转写）
> 状态：**已定，可写代码**。大总管 2026-11-13 定第三节各条；arch 2026-11-13 定模块边界、文件 id、错误码、辅助程序（正本 `atg-wt/arch/state/handoff/arch.md`「给 media 的意见」）与转写编排的落层（arch 分支 `docs/plan/010_complexity-budget.md` 第四节）
> 约束：arch 的十条守则 `specs/architecture-rules.md`（arch 分支；源自 #60 人定）
>
> 代号出处：F3 / F4 / F8 = `docs/research/002_features/decision.md`（拍板 `docs/plan/003_decisions-b2.md`）；R1 = `specs/content-tree.md` 第九节；M2.5 / M2.6 / M3.7 / M3.8 = `docs/plan/002_roadmap.md`；T07 = `specs/tasks.md`；「`v` 不加一」= 树 JSON 的版本号不变（`specs/content-tree.md`，新增可选字段不算破坏）；「名额」= 全机一个的编译 / 重活许可

## 一、模块怎么分

```
atg-gui（以后）──链接──▶ atg-cli 的 lib 目标
atg-cli
   ├─ main / download.rs / play.rs / transcribe.rs   外壳：clap 参数、JSON 行、stdin 控制
   └─ voice/                                           库代码：组合动作（下 → 解 → 放 / 转 → 写回 store）
         吃 Backend 和参数，吐树节点或结果；不碰 clap、不写 stdout
   │                       │
   ▼                       ▼
atg-core                  atg-audio（新 crate，F8 = A）
 只管 Telegram 与内容树      只认路径和 PCM，不认识消息、会话
 file_download（新）        ├─ decode.rs  路径 → Pcm{rate, samples: Vec<f32>}  AudioToolbox ExtAudioFile，C FFI（exp007）
 mark_listened（已占名）      ├─ player.rs  播放状态机（纯逻辑，可单测）+ 声音出口 trait；控制命令只在 player::Cmd 定义一次
 transcripts 表（新）        ├─ queue.rs   声音出口的 AudioQueue 实现（C FFI；保持音高的变速 0.5–2 倍）
 Doc::Playback（新节点）      └─ stt.rs     &Pcm → Recognition；起辅助程序，PCM 经管道喂（不落盘）
                                  └─ stt-helper/main.swift + build.sh（Swift，exp008 探针的正式版）
core 与 audio 互不依赖
```

- ⚠️ `queue.rs` 的倍速与跳转**未实测**：写到它时先用 ≤ 1 分钟的合成音验（放声音前报大总管）
- **解码只做一份**：`decode` 出的同一份 48 kHz 单声道 f32 PCM，`player` 拿来放，`stt` 拿来喂转写（分析器自带转 16 kHz）
- **「CLI 是唯一能力来源」怎么守**（守则第 5 条）：能力代码（下载、解码、播放状态机、转写、组合动作）都在库里，先由 `atg-cli` 做通、有测试；GUI 链接同一份库代码（`atg-cli` 的 lib 目标与 `atg-audio`），只做键位、焦点、展示——GUI 按键直接给 `player::Cmd`，不经文本协议，但命令集合是同一个枚举，没有第二份
- **档案被占时**（arch 定 2026-11-13）：v0 `download`、`play`（含 `--mark-listened`）、`transcribe` 都**不经本机接口转发**，档案被常驻执行方占着就报 `profile_busy`（说明照现有「占着的是常驻执行方」那句）。以后**只转发「下载」这一步**（执行方下好、回本地路径；`--mark-listened` 同样只转发那一个 L1 动作），解码、播放、转写永远在 CLI 本进程做——所以播放**不需要**经接口的双向控制通道，暂停多久都与接口超时无关（codex 017 第 2 轮 #2）。这是 `cli.md`「界面开着时自动转发」那条规则的例外，**已写进 `cli.md`**（随 PR1，2026-11-13）
- **下载复用**：`download` 用现有「起后端 → 取 → 关」那一份（`history::with_backend`），不另写（守则第 4 条）
- 每个新文件 < 600 行；按守则第 8 条拆成三个 PR，每个 ≤ 4 个实体：
  1. **下载**：`FileRef` · `file_download` 动作 · `download` 子命令
  2. **播放**：`atg-audio`（decode / player / queue）· `Doc::Playback` · `play` 子命令 · `mark_listened`
  3. **转写**：`Transcript` 节点与 transcripts 表 · `stt` 与辅助程序 · `transcribe` 子命令 · `stt_unavailable` 错误码

## 二、数据怎么走（内容树）

1. **语音节点带文件**（arch 定 A）：所有带文件的附件共用 `FileRef`（类型放 `tree/mod.rs`，先只给 `Voice` 用）。`#[serde(skip)]` 只跳 `id`（只在本次运行里有效的传输句柄）；`FileRef` 没有别的可输出字段时整体不输出 → 快照不动、`v` 不加一。以后「下没下完」这类给人看的状态照常输出
   - 为什么不放 `model::Message` 旁边：内容和它的文件拆两处，TDLib 整块替换内容时映射要写两处（违背唯一写入口）
2. **下载完才解码**：exp007 测出截断文件会被静默解出半截、不报错。所以 `file_download` 只在 TDLib 报下载完成（`file.local.is_downloading_completed`）后才把本地路径交出去；没完成就是错误，不交半截给解码
3. **转写结果挂哪**：转写结果**不是**消息本身的内容（TDLib 不知道它；`Content` 规定「只取决于消息本身」，只由 TDLib 的三种更新改：内容替换、临时内容变化、已听 / 已看标记——`specs/content-tree.md` 第二节），所以**不进存下来的 `Content`**：
   - store 新加一张表 `transcripts`，键用 store 的稳定键（`stable_key` / `current_id`）→ `Transcript { segments, lang, source }`；`source` 先只有 `local`，以后接 Telegram 的 `speech_recognition_result` 也落同一个节点
   - 写入只经 reducer：组合动作产出一个新事件，喂进 store；**删和迁也由 reducer 做**（发送成功 / 失败换 id、撤回、内容替换、临时内容出现或消失时）——这和上面否掉的「旁字段」不同：那里是 TDLib 映射要写两处，这里是派生数据，写和删都在 reducer 一处
   - **迟到的结果不许复活旧转写**（codex 017 第 1 轮 #1、第 5 轮 #1）：**从 store 里读出这条语音、拿到它的 `FileRef` 的那一刻**就取凭据（不是转写开始时）——凭据贯穿下载、解码、转写、提交，下载或解码期间内容被换掉也逃不过检查。取凭据时（稳定键 + 当时最后一条更新的序号，同 `HistoryTicket` 的做法）；结果事件带着凭据回来，reducer 只在「消息还在、没被撤回、这条消息的有效内容在凭据序号之后没变过」时才写，否则丢弃。同一条并发重转：后到的覆盖先到的，但都要过这道检查
   - **先打开会话才收得到更新**（第 8 轮 #1）：超级群、频道只有打开的会话 TDLib 才推实时更新（`actions.md:84`）。所以 `transcribe` 在取这条消息**之前**调 `chat_open`（L0，绝不标已读），提交之后关；用一个守卫对象保证**所有退出路径**（成功、作废、错误、取消）都释放。
     - **会话开着是共享的**（第 9 轮 #1）：同一进程里 GUI 正在看、`watch` 订阅者、几项转写可能同时要同一个会话开着。统一按 `config-privacy.md` 第一节第 2 小节给订阅者定的**引用计数**（「租约」）：第一个要的人真调 `chat_open`，最后一个放手才真调 `chat_close`；转写拿的是一份租约，放手只减一。**这套计数只写一处**：现在执行方给 `watch` 订阅者的计数在 `serve.rs`（约第 370 行），转写那块把它挪成 `atg-cli` 库里的一个小模块共用（挪之前报 arch，守则第 4 条），GUI 以后也用它，不另写
     - CLI 一次性的 `transcribe` 是这个进程里唯一的使用者，租约退化成「开一次、关一次」
     - 测试：假后端「没打开就不推更新」，断言没开时校验抓不到、开了能抓到；「转写中 GUI 切走会话（放掉自己那份）仍收得到」「两项转写先结束一项不关」「转写结束但 GUI 仍开着不关」；每条退出路径都释放租约
   - **更新的消费顺序**（第 3 轮 #2）：转写期间更新照常排进事件流（收线程不停），但没人喂进 store。所以**提交结果之前，先把事件流里已收到的更新全部按序喂进 store（`feed.drain`），再提交结果事件**——凭据检查看到的就是截至提交那一刻收到的全部更新。后端在提交之后才关（CLI：组合动作在 `with_backend` 的闭包里做完提交）；GUI 常驻同理，提交与喂更新走同一个消费循环。提交之后才到的撤回 / 替换，由 reducer 按「删和迁」那条清掉转写，结果一样
   - **提交的结果决定输出**（第 4 轮 #2）：提交是 store 的一个输入，reducer 回「接受（带当前身份）」或「作废（原因：消息没了 / 内容变了）」。`transcribe` 只在「接受」时输出成功节点，节点里的 id 用 reducer 回的**当前**身份（换过 id 的就是新 id）；作废时不出成功节点：消息没了 → `not_found{what:"message"}`，内容变了 → `internal{what:"transcript_stale"}`（再跑一次即可）。`no_speech` 同样要过这道检查（只是接受后不写表）
   - 测试：撤回、内容替换、换 id **经假后端的真实事件流**进队列（不是直接调 reducer），分别落在「下载中」「转写中」两段时间里，断言 CLI 的输出（作废的错误码、换 id 后节点用新 id）；外加两次重转乱序回来
   - **失败不覆盖成功**：只有成功结果会写进表；失败只回给调用方，不动表里已有的那条
   - 投影 `Doc` 时把它填到语音节点上（JSON 新增可选字段 `voice.transcript`，没有就不输出 → `v` 不加一），和「名字投影时填」同一条规矩
   - **只在内存里，不落盘**（大总管定）：转写文字等同消息内容，落盘就是一份新的明文副本；重转一次约一秒
   - **脱敏登记**（codex 017 #2，`specs/contracts/config-privacy.md` 第三节验收判据）：`Transcript`、结果事件、`atg-audio` 的 `Recognition` 都是含内容的类型——不派生完整 `Debug`，登记进 `redact.rs`（`atg-audio` 里同样手写只打印 `类型 { <redacted> }` 的 `Debug`）；暗号测试 `tests/privacy.rs` 补上：结果事件、Store、投影的 `{:?}` 里零出现，辅助程序的畸形输出（含暗号的坏 JSON）解析失败时错误里也零出现
4. **读法**（`speech.rs`，句子是 AI 草稿，**待人实听**）：
   - `brief` **不变**：「林小雨：语音 12 秒，19:34」（`specs/style/message.md:95`）。「已转文字」是状态，按人定原则（`specs/content-tree.md` 第九节：名称只放谁发的、说了什么、几点）进 `detail`，不进 `brief`
   - `full`：转写全文（人按全文键逐句读）；没转过读「还没转文字」，要不要带按键提示（如「按 T 转」，键由调用方给，同 R1）留给 GUI 那一块定
   - 补标点只在这里做（第三节第 2 条）
5. **播放状态**（arch 定）：`play` 每次状态变化的那一行是树节点 `Doc::Playback { state, position_s, duration_s, speed }`，JSON 由 `json` 出，句子只由 `speech` 出，`play.rs` 里不拼中文

## 三、已定的事

### 1. 语种策略：B（大总管定 2026-11-13）

同一分析器挂 zh-CN、en-US 两个转写器，取平均置信度高的那个；另加 `--lang zh|en` 让人或 AI 手动指定（指定了就单跑）。

**怎么选**（codex 017 第 1 轮 #4、第 2 轮 #3）：每个语种的结果只有三种——**有字**（跑成功、文字非空；没有置信度按 0 算，仍是有字）、**空**（跑成功、没有文字）、**败**（出错）。单跑（`--lang`）就是只有一个语种的同一张表。

| 各语种的结果 | 返回 | 写不写转写表 |
|---|---|---|
| 至少一个有字 | 成功：有字的里面取平均置信度最高的；相等取 zh（排在前面） | 写（覆盖同一条已有的） |
| 没有有字、至少一个空（其余败也算） | 成功，本次结果是「没有听出内容」 | **不写**（不覆盖已有的成功结果） |
| 全败 | 错误：任何一个败因是模型缺失 / 不可用 → `stt_unavailable`；否则 `internal{what:"stt_helper_failed"}` | 不写 |

**辅助程序按语种报结果**（第 5 轮 #3）：探针那样「任一语种出错就整个进程退出 2」会把单边失败变成全败，正式版改成：
- 每个语种的收集任务**各自**捕获错误，互不连累；每个语种出一行 `{"kind":"locale","locale":…,"status":"text"|"empty"|"failed","conf":…,"segments":[…],"error":<码>}`（`failed` 时只带码，不带系统说明文字）
- 全部语种报完后出一行 `{"kind":"done","count":N}`，退出码 0
- **完整性判据**：读到 `done`、`count` 等于请求的语种数、每个请求的语种恰好一行 → 协议完整，按上表选；缺行、多行、没有 `done`、非零退出 → 这是**进程失败**（`internal{what:"stt_helper_failed"}`），不是某个语种失败，不进上表
- 测试：假辅助程序在协议层给出「zh 有字 / en failed」「zh failed / en empty」「少一行」「没有 done」等，断言选择与错误码

测试：这张表逐行做成表驱动单测（单跑、双跑各一组）。

**`transcribe` 输出的是「本次结果」，不是表里的那份**（第 3 轮 #3）：本次结果是一个树节点 `Doc::Transcription { id, outcome }`，`outcome` = `text(Transcript)` 或 `no_speech`，JSON 由 `json` 出、句子由 `speech` 出（「没有听出内容」只在这里出现）。表里存的那份只在投影消息（`history` 等）时填进 `voice.transcript`。所以「重转为空」时本次输出 `no_speech`，而**同一个常驻 store**（GUI、以后的执行方）里投影出的消息仍带上一次的转写——两者分开。**CLI 的边界**（第 4 轮 #3）：每条 CLI 命令各起各的 store、用完即丢，表只在内存，所以 `transcribe` 的结果对之后的另一条 `history` 命令**不可见**（v0 就是这样，不为跨命令共享设计写回路径；要共享得经常驻执行方，等转发做了再说）。快照：首次就空、同一 store 里有旧转写时重转为空、正常有字

| 案 | 做法 | 代价 | 实测 |
|---|---|---|---|
| A 只开 zh-CN | 一个转写器 | 最快（40 s → 0.54 s）、最省内存；**纯英语语音会错得多**（「Pease bring the cortel report…」） | 中、混说对；英错 |
| **B 双跑取高（采用）** | 两个转写器同时跑 | 耗时约翻倍（40 s → 1.10 s；一分钟语音**推算**约 1.6 s）；两个模型同时进内存；置信度在不同模型间不严格可比，接近时可能判反（未测到） | 四个夹具全对，差距大 |
| C 先 zh，置信度 < 0.8 再跑 en | 两段式 | 中文语音与 A 一样快；英语语音最慢（两次）；多一个阈值要调 | **推算**：四个夹具都会判对 |

### 2. 长段标点丢失：只在读法里补（大总管定）

40 s 长段后半一个标点都没有。分析器按片段（`Result`）陆续给结果，探针把片段直接拼在一起，片段之间没有标点。
- **v0**：只做一件事——片段之间若前一段末尾没有标点，补「，」
- 「静音长就补句号」**先不做**（codex 017 #6）：探针没采集片段时间范围，也没验证片段边界是否对应停顿。要做先用含已知停顿的夹具记下片段边界验证；对不上就改用 PCM 静音检测，或只补逗号
- **不改字**：`Transcript` 里存原始片段，补标点只在 `speech` 里做（规则可单测）
- 不行再看 `DictationTranscriber`（听写用，有自动标点），代价是要另测准确度

### 3. 下载：v0 按需下（大总管定）

`play` / `transcribe` 发现没下就先下，`download` 命令手动下。「收到即下」留待定（`specs/contracts/config-privacy.md` 第一节：第一阶段不自动下载，理由是下载文件是明文）。roadmap M2.5 那一句已改（修订记录「按 exp007/exp008 大总管定 2026-11-13」）。

### 4. 错误码（arch 定）

- `stt_unavailable` **单开**（人要去系统设置装语言模型，GUI 要说清楚）：`errors.md` 码表、`cli.md` 退出码表、`out.rs core_exit` 三处一起改（`scripts/check-sources.py` 比对）
- 解码、出声失败进 `internal{what}`：`audio_decode_failed` · `audio_output_failed`（真碰到「没有声音设备」再升格）
- `atg-audio` 有自己的小错误枚举；映射到 `CoreError` / `Fail` **只写一处**（先放 `atg-cli`）

### 5. 播放控制的语义（codex 017 #5）

- **拒绝不是结束**（codex 017 第 2 轮 #1）：按 `cli.md` 第一节，`error` 行表示命令结束，所以播放中拒绝一条控制命令**不出 `error`**，出一行非终结的 `kind:"control_rejected"`（`code:"invalid_input"` + 说明，说明属参数用法报错，守则第 3 条允许直接写），照常继续播、继续读标准输入；整条命令最后仍只出一次 `result` 或 `error`
- `speed X`：X 必须是有限数且在 0.5–2 之间；越界、非数、`NaN` / `inf` → 拒绝这一条（`control_rejected`），不截断
- `seek ±N`：N 必须是有限数；结果位置**截到** [0, 时长]；截到时长 = 直接播完：出 `result{reason:"finished"}`、释放声音出口、结束命令。**不论当时是播着还是暂停**（这条优先于下面「暂停中只改位置」）
- 跳转时清掉已排队的旧音频（声音出口按新位置重开），不会先放完旧的几百毫秒
- 暂停中跳转（没跳到末尾）、改倍速：只改位置 / 倍速，仍是暂停
- 读不懂的命令行：出一行 `control_rejected`，继续播；标准输入关了（EOF）：**播着** = 不再收命令、播完为止，不算 `stop`；**暂停着** = 再没人能恢复，按 `stop` 处理（出 `result{reason:"stopped"}`、释放声音出口）——否则「`pause` 后关管道」会让进程和声音资源一直挂着（第 6 轮 #1）。测试：播着时 EOF 播完退出、暂停时 EOF 立即停并释放、EOF 之后再暂停不可能（没有命令了）
- `stop`：立刻停、出 `result{reason:"stopped"}`

### 5b. `play --mark-listened` 的顺序与失败（第 7 轮 #1）

默认**不标**已听（`cli.md`、`actions.md` A1）。加了 `--mark-listened` 时按这个顺序，前一步失败后一步都不做：
1. 下载（下完才交路径）→ 解码 → **打开声音出口但先停着**（`start` 带 `paused`）——确保真放得出来
2. 标已听（`mark_listened`，L1）
   - **阅后即焚（v0）：拒绝**（第 11 轮 #1）。它按 L2 要先确认（`actions.md:94`：一标就开始不可逆倒计时），确认还没做，所以 v0 在**第 1 步之前**（找到消息、知道它是阅后即焚时）就报 `not_implemented`，不下载、不调 `mark_listened`；不加 `--mark-listened` 照常能听。测试：拒绝路径不下载、不标；不加标记照常准备
   - 以后（确认做了）：确认在第 1 步之后、标之前；确认被取消 → 不标、不放，`error(cancelled)`，释放出口
   - 标记失败 → 不放，按标记的错误报错，释放出口（不说「已标」）
3. 标成功后才开始出声（`resume`）
- 第 1 步任何失败（下载、解码、出口）→ 根本不会去标
- **标了之后出声仍可能失败**（第 8 轮 #2）：`resume` 失败、播放中出口失效 → 释放出口，报 `internal{what:"audio_output_failed"}`。**已标的已听无法撤回**（TDLib 没有「取消已听」，阅后即焚的倒计时也已开始），错误说明里写明「已标为已听」，不假装没标。第 1 步的「停着打开」只能挡住大部分出口问题，挡不住全部
- 测试：默认不调 `mark_listened`；取消不标不放；标记失败不放、错误码是标记的；出口准备失败不标；**标后 `resume` 失败、播放中出口失效**：释放出口、报 `audio_output_failed`、说明里注明已标

### 6. 辅助程序的生命周期（codex 017 第 1 轮 #3、第 2 轮 #4）

- 找不到辅助程序、启动失败、版本不对 → `stt_unavailable`（说明里提示跑 `build.sh`）
- **三个管道各有去处**：标准输入由一条写线程写 PCM、写完关闭；标准输出由主线程读；**标准错误接 `null`**（丢掉，不接管道——既不会因为没人读而堵住辅助程序，也不会把可能带内容的诊断带进本进程）
- **监督**：另起一条看门线程，持有子进程的杀手柄；期限一到（握手 5 秒内没读到版本行，或总期限 10 秒 + 音频时长）就 `kill`。主线程阻塞在读标准输出上，子进程被杀后读到 EOF 返回，据「看门线程是否已动手」判成 `stt_unavailable`（握手超时）或 `internal{what:"stt_timeout"}`
- **统一收尾**：用一个守卫对象包住子进程、写线程、看门线程；**任何**退出路径（成功、版本不对、坏协议、超时、调用方取消 / Ctrl+C、本进程 panic 展开）都由它的 `Drop` 做同一套：关标准输入 → `kill`（已退出则无害）→ `wait` 回收 → 通知并 `join` 看门线程 → `join` 写线程（子进程死了，写线程的 `write` 会报管道断开而返回）
- 非零退出、输出不是合法协议行 → `internal{what:"stt_helper_failed"}`；错误里只带码与长度，不带输出原文
- **取消**（第 3 轮 #1）：Ctrl+C 不会自己跑 Rust 析构，所以不靠它。CLI 已有的信号处理只立一个退出标志（`atg-cli/src/signals.rs`，第二次 Ctrl+C 才立刻退出）；**看门线程除了看期限，每 50 毫秒也看这个标志**（GUI 传自己的取消标志进来，同一个参数）。标志立起 → 看门线程 `kill` 子进程 → 主线程的读拿到 EOF 返回 → 判成 `interrupted` → 正常返回、守卫在栈上按序析构收尾 → 出结尾行、退出码 130。第二次 Ctrl+C 立刻 `_exit`，本进程来不及收尾，所以**辅助程序自己兜底**（第 4 轮 #1；标准输入 EOF 是「输入写完了」的正常信号，不能拿来判父进程死活）：① 辅助程序起一条线程每 200 毫秒看 `getppid()`，父进程没了（被系统收养、父 pid 变了）就立刻退出；② 启动参数带总期限（与看门线程同一个数），到点自己退出。两条都写进辅助程序的规矩，测试：真辅助程序的 `#[ignore]` 测试里，输入写完、分析进行中杀掉父进程（用一个中间进程模拟），断言辅助程序在 1 秒内消失
- 测试全部用假辅助程序：不出版本行、版本不对、睡过期限、**一直不关标准输出也不退出**、半路退出、输出坏 JSON、输出含暗号、**卡死时立取消标志**；每个都断言子进程已被回收（`wait` 拿到状态）、本进程线程数回到原样

### 7. 辅助程序的构建、定位与隐私（arch 定 + 本设计补）

- 源码 `src/atg-audio/stt-helper/main.swift` + 同目录 `build.sh`，编到 `_build/<时间戳>/`（照 `src/tdlib-build` 先例），**不用 build.rs**、不建 `src/atg-stt/`（守则第 10 条）
- 运行时找：环境变量 `ATG_STT_HELPER` > 可执行文件旁边 > `_build` 最新一份；辅助程序启动先报协议版本，对不上 → `stt_unavailable` 并提示跑 `build.sh`
- 授权：exp008 在终端和 `.app`（经 LaunchServices 拉起）两种方式下都没弹窗、不需要 Info.plist 用途说明；Apple Development 签名版未测，写代码时顺带验
- **隐私**：辅助程序的标准输出只有协议行（版本、结果）；**标准错误与错误信息只带码、长度、语种、耗时，绝不带转写文字**；Rust 端不把辅助程序的标准错误原样转出。测试：假辅助程序在标准错误写一句暗号，断言 CLI 的标准错误、错误 JSON 里都没有它

### 8. 契约改动（随对应 PR 改，修订记录照写）

**已改**（PR1，2026-11-13）：
- `actions.md` `file_download`：参数 `FileRef`、同步等下完、只在下完时交路径、`download_incomplete`
- `cli.md` `download` 一行（只认语音、不标已听、v0 无进度行）；「界面开着时」的例外（`download` / `play` / `transcribe` v0 不转发，以后只转发下载与 `mark_listened`）
- `content-tree.md` 语音附件的 `file`；`inventory.md` 第 163 行（解码）、第 165 行（转写）

**已改**（PR2，2026-11-13）：`cli.md` `play` 一行（`playback` 节点、非终结 `control_rejected`、EOF 与跳到末尾、`--mark-listened` 顺序与阅后即焚 v0 拒绝、`--redact`）；`download` 一行补 `--redact`；`content-tree.md` 顶层加 `playback`

**待改**：
- PR3：`actions.md:125` `speech_to_text` 改成本机（#59 取代 F4，L0；「不外发」以 Apple 端侧说明为据，**断网未实测**，待人验证）；`cli.md` 加 `transcribe CHAT_ID MSG_ID [--lang zh|en]`（一行 `kind:"transcription"` + `result`，`--redact` 报 `not_implemented`）；`errors.md` 码表与 `cli.md` 退出码表加 `stt_unavailable`；`content-tree.md` 加 `Transcript`、`Doc::Transcription`、`voice.transcript`

`--redact` 的口径（第 5 轮 #2、第 6 轮 #2、第 8 轮 #3）：`transcribe` 带转写原文；`download` 带本机路径（含用户名，`cli.md` 打码规则不放行文件名与路径）；`play` 每行带 `speech` 读法与说明（`cli.md` 要求打码时也替换）——打码还没做，三条都报 `not_implemented`。测试：三条命令加 `--redact` 两种输出格式都只有一行 `not_implemented`

## 四、测试计划

- 解码：`atg-audio` 单测用入库的小夹具（`say` 合成编造句，Opus 24 kbps，几 KB；生成脚本随附），断言采样率、帧数、非静音；截断文件解出部分不崩
- 下载：假后端测「没下完不交路径」
- 播放：`player` 状态机全用假声音出口单测（暂停 / 继续 / 前后跳越界 / 倍速上下限 / 播完 / stop），CLI 控制命令解析单测（含坏输入）
- 转写管线：`stt` 用假辅助程序（一个打印固定 JSON 的脚本）测子进程协议、版本不对、超时、错误码、标准错误不外泄（第三节第 6 条）；选语种规则（第三节第 1 条）；迟到结果（第二节第 3 条）；真模型的端到端测试标 `#[ignore]`，领名额手动跑
- 读法：快照测试（有 / 无转写、补标点规则、`Doc::Playback`）
- 真账号：只读人放进收藏夹的语音，跑前报大总管，只报统计
- 放声音给人听：先报大总管，由人来听
- 待人验证：断网时转写仍可用
