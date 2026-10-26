# 现状页 · 手下 ipc（issue #22 本机接口）

> 批次 b15 · 大总管 `telegram-accessibility-1a` · 工作目录 `atg-wt/22-ipc`（分支 `feat/22-local-ipc`）

## 手上是什么

第一刀第 9 块：占着档案的进程开 `profiles/<档案>/atg.sock`；后起的 `atg-cli` 拿不到锁就把命令转过去，输出与直跑逐字节一致。

## 做到哪

- [x] 读完 common / 交代 / 契约（config-privacy 一.2、actions 一.4.6、cli、errors）/ 路线图 M1.8 / 已有代码
- [x] 路径长度：本机 `main` 档案的套接字路径 71 字节；档案名最长 32 个字符时 99 字节，都在 macOS 上限（`sun_path` 104 字节，含结尾 0，实际可用 103）之内
- [x] 方案 A–F 报大总管，**2026-10-26 全按默认批复**，授权改契约
- [x] `src/atg-core/src/ipc.rs`：路径与上限、绑定（残留文件删、活的不抢、非套接字不碰）、0600、`getpeereid` 两头核对、分帧（行上限 1 MiB、请求 5 秒超时）
- [x] `config.rs` 加 `probe_lock`（只探不写）；`profile_busy` 说明加「稍后重试，或先开着 atg-cli serve」
- [x] `atg-cli`：输出改走出口 `Sink`（`out.rs`）；`chats` 的 Store 可常驻（`chats::Feed`）；`serve.rs` 执行方；`forward.rs` 发起方；`args.rs` 加 `serve`
- [x] 契约：`config-privacy.md` 第一节第 2 小节（谁开 + 协议 v0）、`cli.md`（`serve` 一行 + 第 64 行）、`errors.md`（`disconnected`、`profile_busy`），各写修订记录
- [x] 测试 130 个全过（atg-core 71，其中 ipc 12；atg-cli 59，其中 serve / forward 8）；新增快照 2 个（只有假数据）；clippy `-D warnings`（workspace、atg-core `tdlib-api`、atg-cli `tdlib`）、`fmt --check`、`check-no-debug` 全过
- [x] 真实账号只读冒烟（正式档案 `main`，`chats -n 100`，只看统计）：
  - `serve` 起来 103 秒（钥匙串弹框等人点）；两次转发各 100 + 1 行，退出码 0，`synced:true`，用时 0.39 / 0.04 秒
  - 停掉 `serve` 后直跑：100 + 1 行，6.03 秒；**第 2 次转发与直跑去掉时间字段后哈希一致**；第 1 次转发哈希不同，判断是两次之间账号有更新（没看原始行，未细分）
  - 停干净：套接字已删、锁空闲（flock 核过）、无进程、「运行中」标记已清
- [x] 直跑 6 秒的去向（大总管要查）：同一二进制再直跑 3 次，`elapsed_ms` 449 / 431 / 543，`first_chat_ms` 73 / 69 / 68，`synced:true`，与 #18 记的 425–485 毫秒一致。**不是本块引入的回退**：6 秒只出现在刚停掉 `serve` 后的那一次（推测是 TDLib 换进程后重连多等了一段 `updating`；那次的统计没留计时字段，无法再分）。下一步建议：用 main 分支（#18）重编、重签的二进制做「刚停掉另一个进程后直跑」的对照（会弹钥匙串，要人在）
- 坑：系统 bash 会删掉 `DYLD_LIBRARY_PATH`，脚本里改用 `ATG_TDLIB_LIB` 传进去再设回；重签 `codesign -f -s atg-dev -i atg.dev`
- [x] 自审：`atg-review` 三视角（参数要传 `{target: …}`，传字符串会被忽略，#41）+ 一个安全 / 并发子代理。**P0 0、P1 0**；P2 约 12 条已全修，P3 大多已修：
  - 请求 `v` 改 `0`（契约写明与外壳 `v` 无关）；执行方先看版本号，版本不对 / 请求坏了回错误行 + `exit` 帧
  - `serve` 里 `first_chat_ms` 从启动起就记主列表 / 归档；发起方两帧之间 90 秒无动静报 `disconnected`
  - `profile_busy` 通用说明恢复原句，按场景换话（一次性命令占着 / 常驻进程占着但命令不转 / 重复起 `serve`）
  - 探锁改共享锁，真拿锁撞上时重试 3 × 20 毫秒；执行方补查 `-n` 范围；只连上就断的探测不算请求
  - 契约三份补：发起方定义、排队、改直跑的退路、登出不转的理由、`serve` 无登录入口、`serve` 输出字段与两次 Ctrl+C、`internal.what` 新值、「能否重试」一列
  - 没修（P3，留着）：本用户客户端慢慢滴字节能占住执行方（只有本用户做得到，非安全问题）；`Listener::bind` 要求「已拿锁」只能靠约定（锁在后端里拿不出来）
- [x] 修后复测（2026-10-26）：`cargo test -p atg-core -p atg-cli` **242 个全过**（atg-core 171：lib 47 含 ipc 12 + 集成 124；atg-cli 71：单元 59 含 serve / forward 8 + `tests/cli.rs` 12）。⚠️ 之前报的「130 个」数错了，以此为准。clippy `-D warnings` 三条（workspace、atg-core `tdlib-api`、atg-cli `tdlib`）、`fmt --check`、`check-no-debug` 全过；快照 2 个未变
- 待 codex 补审（#32）

## 方案（契约没写的，等大总管 / 人定；括号里是我准备先按它做的默认）

**A. 常驻模式叫什么、怎么用**（默认：`atg-cli serve`）
- `atg-cli [--profile N | --test-dc] serve`：拿锁 → 起后端 → 开套接字 → 一直跑，Ctrl+C / SIGTERM 正常关（关后端、删套接字）
- 标准输出：起好后一行 `kind:"status"`（含 `socket` 不写路径，只写 `listening:true`），结束时一行 `result`（`reason:"interrupted"`，退出码 0，照 `watch` 的先例）；中途每处理一个请求在标准错误记一句「命令名 + 退出码 + 耗时」，不记参数
- 不登录也能起：没登录时转来的 `chats` 照常报 `auth_required`；要登录先停掉 serve，跑 `login`（秘密不过接口）

**B. 只有常驻的 `serve` 开套接字**（契约原文是「拿到锁的进程都监听」）
- 一次性的 `chats` 占着锁的那几秒不监听，后来者连不上 → `profile_busy`（holder=cli）。理由：一次性命令边跑边接别人的请求要两路共用一个事件流，复杂且收益小（它几秒就退）
- 代价：两个 AI 同时各跑一次 `chats`，后到的那个会报 `profile_busy`，要它自己重试；开着 `serve` 时没这问题

**C. `auth state` 转不转发**（规约冲突：交代说先支持「`auth` 状态」；`cli.md:64` 写「`login` / `auth *` 不转发」）
- 默认：只转 `auth state`（不带任何秘密，只读一个步骤名）；`login`、`logout` 与 `auth` 其余分步一律不转，档案被占就照旧报 `profile_busy`
- 需要改 `cli.md:64` 一句：「`login` / `auth *`（`auth state` 除外）不转发」

**D. 套接字路径超长怎么办**（契约只说「开工时核」，没说超了怎么办）
- 默认：`serve` 拒绝启动，报 `internal{what:"socket_path_too_long"}`；后来者照旧 `profile_busy`。不改用别的目录（换目录就要另想权限）

**E. 转发途中执行方没了**（连上了但没等到结尾行就断）
- 默认：`disconnected`（退出码 4，「执行方中途退出了」）。`errors.md` 的 `disconnected` 原指本进程收线程故障，这是借用；要不要另开码请定

**F. 协议 v0**（不改契约，只在 `atg-core::ipc` 模块注释里写；要不要写进 `config-privacy.md` 请定）
- 套接字一连一个请求。请求一行 JSON：`{"v":1,"origin":"cli","command":"chats","args":{…},"format":"json",…}`——参数在**发起方**按同一份定义解析好（用法错误在本地就报，和直跑一样），执行方按名字派发；不认的命令 → `not_implemented`（给 history / send 留位）
- 应答若干行：`{"t":"out","line":"…"}`（原样写标准输出）· `{"t":"err","line":"…"}`（原样写标准错误）· 最后 `{"t":"exit","code":N}`
- 输出逐字节一致的含义：结尾行 `at`、`elapsed_ms`、`first_chat_ms` 这些时间字段由执行方当场生成，其余逐字节相同（快照测试把时间字段抹平后比）

**G. 以后 GUI 当执行方**：现在「命令 → JSON 行」的渲染在 `atg-cli` 里；GUI 当执行方前（M3.1）要把它挪进 `atg-core`。本块先把派发写成可替换的处理函数，不挪

## 下一步

等名额 → 测试 → 冒烟 → 自审。

## 记一笔（留给以后）

- GUI 当执行方（M3.1）前，要把「命令 → JSON 行」的渲染从 `atg-cli`（`main.rs` 的 `render_chats` / `job_output`、`serve.rs` 的派发）挪进 `atg-core`
- `serve` 一次只处理一个请求；处理中按 Ctrl+C 要等它做完（chats 最长约 30 秒），再按一次立刻退出

## 要人定的事

A–F 已定（大总管批复）。

## 审查

待 codex 补审（#32）

## 正式回报（common 第 10 条，2026-10-26）

- **改了哪些文件**：新 `src/atg-core/src/ipc.rs`、`src/atg-cli/src/serve.rs`、`src/atg-cli/src/forward.rs`、`src/atg-cli/src/snapshots/`（2 个）；改 `src/atg-core/{Cargo.toml,lib.rs,config.rs}`、`src/atg-cli/{Cargo.toml,main.rs,out.rs,chats.rs,args.rs,session.rs}`、`Cargo.lock`；契约 `specs/contracts/{config-privacy,cli,errors}.md`；本页。`.env.local` 是指向 MAIN 的软链接（已被 git 忽略）
- **共同文件**（与 speech 手下可能冲突）：`args.rs` 只加 `Serve` 一项 + `ChatsArgs` / `Format` / `ListArg` 的 serde 派生；`main.rs` 加两个 `mod`、`serve` 一行分派，chats / 登录的渲染改走 `Sink`；`out.rs` 的 `emit` / `fail` / `text_line` / `fail_with` 签名不变（`text_line` 保留给 speech 用）；`atg-cli/Cargo.toml` 加 serde `derive` 特性与 `insta` 开发依赖
- **测试数**：242（见上）
- **关键数字**：正式档案冒烟 `chats -n 100` 转发 0.39 / 0.04 秒、直跑 0.43–0.54 秒（3 次）；第 2 次转发与直跑去掉时间字段后哈希一致；serve 停后套接字、锁、运行标记都清了
- **自审**：P0 0、P1 0，P2 已全修（见上）。待 codex 补审（#32）
- **要人定的事**：无。留给以后：GUI 当执行方前（M3.1）把渲染挪进 `atg-core`；建议用 #18 的二进制做「刚停另一个进程就直跑」的 6 秒对照（要人在，会弹框）
