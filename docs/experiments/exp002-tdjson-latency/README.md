# exp002-tdjson-latency · TDLib 往返、新消息到无障碍树、冷启动：分段计时

> 2026-10-23 · 手下 exp002 · 机器：Apple M5（10 核，32 GB），macOS（Darwin 27.0），rustc 1.99.1
> 依赖：tdlib-rs 1.4.0（`download-tdlib`，预编译 TDLib **1.8.61**）· GPUI 路径依赖 `resources/zed`（`79fe6a6`）
> ⚠️ 全部数字来自 **debug 构建**：GPUI、serde_json 没有优化；只有本实验代码和 tdlib-rs 开了 `opt-level=3`，libtdjson 本身是优化过的预编译库。所以界面段的数字是**上界**，优化版只会更快。**debug 上界，release 待正式原型时测**（大总管定，2026-10-23）。

## 一、假设

1. TDLib（经 tdlib-rs）发请求到收回调的往返延迟很小，不是瓶颈
2. 「收到一条消息 → GPUI 重绘 → 无障碍树更新」这一段低于可感知阈值（按常用的 100 ms「即时」线）
3. 冷启动：分别量出进程启动到第一帧可见、到无障碍树可用要多久

## 二、做法

**复现**（在项目根目录执行，要编译名额）：`bash docs/experiments/exp002-tdjson-latency/run.sh`
跑的过程中会弹出小窗口：界面段两次，冷启动 20 次，跑完自动关。加 `PROFILE=release` 跑优化版。

不登录、不注册、不申请 api_id、不联网。只用 TDLib 的离线测试方法：`testCallEmpty`、`testSquareInt`、`testCallString`。这几个方法在 `setTdlibParameters` 之前就能调（`resources/td/td/telegram/Td.cpp:190-209`）。`getOption` 不能用：除了 `version` 之外，它在初始化前会挂起等参数（同文件 `:357-359`）。

| 程序 | 量什么 | 怎么量 |
|---|---|---|
| `src/bin/td_bench.rs` | TDLib 段 | A. 直接调 `td_send`/`td_receive`，同一线程往返 5000 次（先预热 200 次）；B. 一次发 2 万个 1 KB 请求再全部收回，算每秒条数，做 5 轮；C. 走 tdlib-rs 的真实用法（后台收线程 + async 函数）重复 A、B；D. 把一条 1237 字节的 `updateNewMessage` JSON 解析成 tdlib-rs 类型 2 万次（与 `tdlib_rs::receive` 同一路径） |
| `src/bin/ui_bench.rs` | 界面段 | 后台线程扮演 TDLib 收线程，每 120 ms 把一条「新消息」做一次真实的 `testCallString` 往返，再交给主线程，追加进一个 GPUI 列表（`Role::List` + `ListItem`，带标签和「第几项共几项」）。每条记 6 个时间点（见下表）。共 300 个样本，先预热 20 个；列表分 50 项、500 项各跑一轮 |
| `src/bin/cold_start.rs` | 冷启动 | 父进程记下 spawn 时刻，分别拉起 `td_bench --cold` 和 `ui_bench --cold` 各 20 次。子进程记下 main 开始、开窗、第一次 render、第一帧画完，以及「无障碍树里能查到列表节点」的时刻（Unix 纳秒，跨进程对齐）。`ui_bench --cold` 同时在后台起 TDLib，跟真实应用一样 |

**无障碍树怎么查（不需要「辅助功能」权限）**：GPUI 只有在辅助技术来问过之后才建树（`gpui/src/window.rs:1679-1720`）。本实验在进程内直接调窗口内容视图的 `accessibilityChildren`，AccessKit 就会激活（`accesskit_macos-0.26.3/src/subclass.rs:92-99`、`adapter.rs:223`）。然后沿 `accessibilityChildren` / `accessibilityTitle` 深度优先找目标节点（标签映射到 title：`node.rs:306-312`）。这和 VoiceOver 读到的是同一棵树，只是少了跨进程那一跳。

**界面段的时间点**：

| 点 | 时刻 |
|---|---|
| t0 | 后台线程向 TDLib 发出 `testCallString` |
| tA | 后台线程收到应答并解析完 |
| t1 | 主线程从通道拿到消息，改状态，`cx.notify()` |
| t2 | 下一帧 `render()` 开始 |
| t3 | 本帧画到最后一个元素。紧接着 GPUI 在同一次调用里发无障碍树更新，并同步发出 NSAccessibility 通知（`window.rs:3789-3813` → `gpui_macos/src/window.rs:2359-2369`） |
| t4 | 本帧 draw + present 返回（`App::defer`），此时树已更新，通知已发出 |
| t5 | 进程内无障碍查询找到这条新消息的节点 |

## 三、结论

### TDLib 段（`results/debug_td_bench.txt`）

| 测量 | 中位数 | p95 |
|---|---|---|
| 直连 tdjson 往返 `testCallEmpty` | 0.016 ms | 0.027 ms |
| 直连 tdjson 往返 `testCallString`（约 1 KB） | 0.058 ms | 0.071 ms |
| 经 tdlib-rs `test_call_empty`（含跨线程与 serde） | 0.022 ms | 0.026 ms |
| 经 tdlib-rs `test_call_string`（约 1 KB） | 0.107 ms | 0.177 ms |
| 解析一条 `updateNewMessage`（1237 字节，字符串 → Value → Update） | 0.038 ms | 0.039 ms |
| 新建客户端后的第一个请求 | 1.1 ms（单次） | — |

| 吞吐 | 每秒条数（5 轮中位） |
|---|---|
| 直连 tdjson，2 万个 1 KB 请求一起发 | 22039 |
| 经 tdlib-rs，2 万个 async 请求一起发 | 22000 |
| 单线程解析 `updateNewMessage` 的能力 | 约 26400 |

→ **假设 1 成立**。往返在 0.1 ms 量级，单线程每秒能处理 2 万条以上，比整条链路的预算（100 ms）小两到三个数量级。tdlib-rs 封装（跨线程 + serde 解成类型）在 1 KB 消息上加了约 0.05 ms。

### 界面段（`results/debug_ui_bench_50.txt`、`debug_ui_bench_500.txt`）

| 分段 | 50 项 中位 / p95 | 500 项 中位 / p95 |
|---|---|---|
| t0→tA TDLib 往返 | 0.18 / 0.24 ms | 0.10 / 0.15 ms |
| tA→t1 后台线程 → 主线程 | 0.04 / 0.06 ms | 0.02 / 0.06 ms |
| t1→t2 等下一帧 | 8.3 / 16.5 ms | 8.3 / 16.6 ms |
| t2→t3 render + 布局 + 绘制 | 10.3 / 13.9 ms | **30.9 / 31.5 ms** |
| t3→t4 无障碍树更新 + 通知 + present | 2.0 / 3.1 ms | 7.5 / 9.1 ms |
| **t0→t4 TDLib 发出 → 树已更新、通知已发** | **20.6 / 30.6 ms** | **47.8 / 55.9 ms** |
| t4→t5 进程内查询找到节点（查询本身的开销） | 3.1 / 3.9 ms | 15.4 / 16.1 ms |
| t0→t5 TDLib 发出 → 查询看得到 | 23.7 / 34.1 ms | 63.5 / 71.7 ms |

300 个样本里，每一帧画完后都能在树里查到新消息（not-found = 0）。

→ **假设 2 在 debug 构建下就成立**：50 项中位 21 ms，500 项 p95 也只有 56 ms，都在 100 ms 以内。时间几乎全花在界面层：等下一帧（0–17 ms，最大约 17.5 ms，符合 60 Hz 刷新）加上 GPUI 渲染。渲染和树更新**随渲染的项数线性增长**（50 → 500 项，渲染从 10 ms 涨到 31 ms）。TDLib 段在总时间里不到 1%。

### 冷启动（`results/debug_cold_start.txt`，各 20 次，去掉第 1 次后取 19 次）

| 分段 | 中位数 | p95 |
|---|---|---|
| 只有 TDLib：spawn → main | 7.9 ms | 9.4 ms |
| 只有 TDLib：main → 客户端第一次应答 | 0.5 ms | 0.6 ms |
| GPUI + TDLib：spawn → main | 19.6 ms | 32.8 ms |
| GPUI + TDLib：main → 第一帧画完（开窗时同步画第一帧） | 142.6 ms | 209.2 ms |
| **spawn → 第一帧画完** | **159.6 ms** | **235.6 ms** |
| 第一帧 → 无障碍树里能查到列表 | 104.0 ms | 566.6 ms |
| **spawn → 无障碍树可用** | **284.1 ms** | **741.6 ms** |
| spawn → TDLib 第一次应答（与开窗并行） | 20.4 ms | 34.1 ms |

第 1 次（磁盘缓存可能是冷的）：TDLib 单独启动 8.9 ms；GPUI 到树可用 262 ms，并不比后面慢。

→ **假设 3 量出来了**：从启动到第一帧约 0.16 s，到无障碍树能读约 0.28 s。19 次里有一次是 0.74 s（树激活那一段 567 ms），原因没有查清。TDLib 自己启动只要 20 ms，不在关键路径上。注意，这只是**未登录、空列表**时的进程级冷启动，不是「冷启动到会话列表可读」（见下节）。

### 测不了的部分

| 没测的 | 为什么 | 以后怎么补 |
|---|---|---|
| 真实收到一条 Telegram 消息的端到端延迟（网络 → TDLib → 回调） | 没有账号，也不注册测试账号；离线测试方法不走网络 | 人允许后，用测试服务器账号（`use_test_dc`）给收藏夹发消息：在发送端打时间戳，用本实验的 t1–t5 接到收消息那一端 |
| 冷启动到「会话列表可读」（含 TDLib 读本地库 `getChats` / `getChatHistory only_local`） | 要先登录，库里才有缓存 | 同上，登录一次后反复冷启动，量 spawn → 第一条会话进树 |
| 真实更新流的每秒更新数（大群刷屏） | 没有账号就没有更新 | 现在的吞吐（2.2 万条/秒）和解析能力（2.6 万条/秒）是上限参考；登录后订阅一个高流量群，量 `updateNewMessage` 的实际到达率和积压 |
| VoiceOver 真正开口的时刻 | 读屏是另一个进程，什么时候念出来不在本进程可观测范围内；本实验只量到「通知发出（t4）」和「进程内查得到（t5）」 | 由 exp001 和人工试用判断；如果要数字，可以录音加按键时间戳对齐 |
| 画面真正上屏（光子）时刻 | present 之后还要合成器上屏，大约再晚一帧 | 不需要：读屏用户不依赖画面 |
| 优化版（release）数字 | 编 GPUI release 要再占一次完整编译名额；debug 已经是上界，而且都在阈值内 | `PROFILE=release bash run.sh` |
| grammers 对照 | `gap.md` 规定只有 TDLib 段慢才需要跑对照；它不慢 | 不需要 |

另外两点：界面段跑的时候，窗口只有部分时间是前台（50 项那轮 233/300 个样本，500 项那轮 108/300），因为操作者在用别的窗口。GPUI 只在有强制重绘或下一帧回调时，才对非活动窗口限帧到 30 fps（`window.rs:1828-1838`）；本实验每次都是 `notify` 触发重绘，不会被限帧，各轮的 t1→t2 也一致（最大约 17 ms）。进程内查询要走一遍整棵树（500 项时 15 ms），这是本实验的测量开销，不算产品延迟。

## 四、改变了哪一个取舍

1. **拍板 1（TDLib + tdlib-rs）坐实，不用跑 grammers 对照。** TDLib 段在新消息链路里不到 1%（0.1–0.2 ms，总共约 21 ms），换 grammers 也省不出可感知的时间。`gap.md` 定的改选条件（「只有慢在 TDLib 那一段」）没有出现。
2. **慢在界面层，而且随渲染的项数线性涨 → 聊天列表必须虚拟化，只渲染可见的那几十项。** 这和拍板 3 选 A（方向键驱动、屏幕外的项不进树）方向一致：把 500 项全画出来进树，debug 下一帧就要 31 ms 渲染 + 7.5 ms 树更新。界面层的薄壳应该用 `uniform_list` / `list`，再加 `aria_size_of_set` 报总数，不要把整个历史放进一个 div。
3. **冷启动的大头是 GPUI 开窗加第一次建树（约 0.28 s），不是 TDLib（20 ms）。** 偶尔出现的 0.7 s 发生在树激活阶段，值得在第一个原型里用 release 构建再看一次。冷启动的预算应该花在界面层，TDLib 可以和开窗并行，不在关键路径上。
