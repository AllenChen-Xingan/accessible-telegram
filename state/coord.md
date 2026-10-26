# 大总管看板（coord）

> 只有大总管跟人说话。手下做完、卡住、要名额，只报给大总管。
> 大总管会话名：`telegram-accessibility-40`（2026-10-25 起；原 `-ca`）（换班后在这里改）。tmux 工作区：`aiw`。
> 做法依据：阳志平《AI 自主系统的高级玩法：建一个大总管》（2026-10-15，安仁书院）。

## Top 4（眼下最重要）

1. 系统分析 001：**取材、分析、双谱系评审已完成**，等人拍板 `docs/research/001_stack/decision.md` 的 4 条
2. 人确认 `001_stack/must-know.md` 的三条命门（命门 1 可能因拍板 3 放宽）
3. 安装 Rust（小实验要编译）
4. 初始化放行检查（冒烟：只喂 intent.md 问三句）

## Top 3（次要）

1. 开第一个 issue（人写）
   - tdlib-rs 补丁：fork 到本人账号，`local-tdlib` 编 TDLib 1.8.69 + 同步 `tl/api.tl` 与库文件名（`build.rs:93,310`）；要编译名额；做好后向上游提 PR（PR 描述人来写）
2. 三个实验 `exp001-announce` → `exp003-list-nav` → `exp002-tdjson-latency`（要 Rust + 编译名额）
3. 第一个原型：`atg-cli` 登录 + 列会话

## 手下

| 窗口 | 任务 | 交代文件 | 现状页 | 状态 |
|---|---|---|---|---|
| `gpui`（会话 `telegram-accessibility-c5`） | 001 · 命门 1、2：GPUI 无障碍接口 | `_tmp/20261023-dispatch/gpui.md` | `state/handoff/gpui.md` | 完成，已收尾（窗口已关） |
| `exp001`（会话 `telegram-accessibility-23`） | 实验：播报会不会打断朗读 | `_tmp/20261023-exp/exp001.md` | `state/handoff/exp001.md` | **完成，已收尾**；剩人工试听 |
| `exp002`（会话 `telegram-accessibility-fc`） | 实验：TDLib 往返、重绘、冷启动分段计时 | `_tmp/20261023-exp/exp002.md` | `state/handoff/exp002.md` | **完成，已收尾**（拍板 1 坐实） |
| `exp003`（会话 `telegram-accessibility-be`） | 实验：方向键跨屏读 1000 条 | `_tmp/20261023-exp/exp003.md` | `state/handoff/exp003.md` | **完成，已收尾**；剩人工试听 |
| `exp003b`（会话 `telegram-accessibility-15`） | **改向（2026-10-25）**：专门消息列表 · Tab / VO+←/→ 到达 · ↑/↓ 浏览 | `_tmp/20261025-batch/exp003b-v2.md`（原 `_tmp/20261023-exp/exp003b.md`） | `state/handoff/exp003b.md` | **完成，已收尾**（自测全过；窗口已关）；剩人工试听「第三次试用」，试完跑 `python3 summarize-live.py` 看按键是否到达应用 |
| `arch`（会话 `telegram-accessibility-8a`） | 写架构方案 `docs/plan/001_arch-stack.md` | `_tmp/20261025-batch/arch.md` | `state/handoff/arch.md` | **完成，已收尾**（三视角自审 + codex 评审 7 条全修，记录 `docs/reviews/002_arch-codex.md`；窗口已关）；第七节 10 条待人拍板 |
| `exp004`（会话 `telegram-accessibility-e7`） | 新消息提示不打断朗读（7 种模式） | `_tmp/20261023-exp/exp004.md` | `state/handoff/exp004.md` | **完成，已收尾**（自动 7/7；窗口已关）；剩人工试听 11 步 |
| `codesign`（会话 `telegram-accessibility-e2`） | 批次 b11 · 运行 `scripts/setup-dev-codesign.sh` 建自签名开发证书 `atg-dev`（密码框人亲手输） | `_tmp/20261025-b11/codesign.md` | `state/handoff/codesign.md` | **完成，已收尾**（证书 `atg-dev` 已建，有效到 2036-10-23；窗口已关） |
| `login`（会话 `telegram-accessibility-b9`） | 批次 b13 · issue #17 第一刀第 4 块：`atg-cli login` / `logout`（测试服务器 + 99966 测试号验证） | `_tmp/20261025-b13/login.md` | `state/handoff/login.md` | **完成，已收尾**（login / logout / auth；输错原因可读已验；测试服务器正确码被拒 → #35；人用正式号跑 `live/login-main.sh` 验证；窗口已关） |
| `tdlib`（会话 `telegram-accessibility-6e`） | 批次 b9 · issue #16 第一刀第 3 块：接 TDLib（收线程、关闭、监督、Backend 两份、加密 + 钥匙串、测试服务器） | `_tmp/20261025-b9/tdlib.md` | `state/handoff/tdlib.md` | **完成，已收尾**（接 TDLib；160 测试；测试服务器冒烟到 wait_phone、凭据 0 次；codex 待补审 #32；窗口已关） |
| `scanskill`（会话 `telegram-accessibility-c8`） | 批次 b10 · 按人拍板改 `codex-review`：拆 `privacy-scan.sh`、放松扫描（禁区仍硬拦）、顺手 #29 | `_tmp/20261025-b10/scanskill.md` | `state/handoff/scanskill.md` | **完成，已收尾**（新版 codex-review 已覆盖安装，旧版备份在草稿区；自测 195/195；剩余 P2 开 issue；待 codex 补审 #32；窗口已关） |
| `codexskill`（会话 `telegram-accessibility-12`） | 批次 b8 · 专门的 skill `codex-review`（Opus 子代理用 skill-creator 起草），真跑审 004 | `_tmp/20261025-b8/codexskill.md` | `state/handoff/codexskill.md` | **完成，已收尾**（skill `codex-review` 已装到 `2-aartifacts/skills/` + 软链接；codex 自审 11 轮收敛；记录 `docs/reviews/006_codex-review-skill-codex.md`；剩余 P2 → #29；窗口已关） |
| `model`（会话 `telegram-accessibility-89`） | 批次 b7 · issue #15 第一刀第 2 块：model + reducer + 投影 | `_tmp/20261025-b7/model.md` | `state/handoff/model.md` | **完成，已收尾**（model + reducer + 投影 + 缓存上限 + 唯一写入口；115 测试；codex 十三轮收敛，余 P2 → #30；窗口已关） |
| `srcmove`（会话 `telegram-accessibility-32`） | 批次 b6 · issue #25：tdlib-rs 已 subtree 进 `src/tdlib-rs`，调工作区 / TDLib 拉取编译脚本 / CI / 文档 | `_tmp/20261025-b6/srcmove.md` | `state/handoff/srcmove.md` | **完成，已收尾**（方案 B；`src/tdlib-build/build.sh`；特性 tdlib-api / tdlib；旧克隆与 161449 已删；窗口已关） |
| `skills`（会话 `telegram-accessibility-b2`，**Fable 5.1**） | 批次 b5 · 读全部对话与工具记录，找反复出现的问题 → Opus 子代理用 skill-creator 起草跨项目 skills（只出草稿，装不装人定） | `_tmp/20261025-b5/skills-miner.md` | `state/handoff/skills.md` | **完成，已收尾**（7 个 skill 草稿在 `_tmp/20261025-b5/skills/`，未安装；报告 `_tmp/20261025-b5/report.md`，待人拍板 9 条；窗口已关） |
| `skeleton`（会话 `telegram-accessibility-01`） | 批次 b4 · issue #14 第一刀第 1 块：`atg-cli` 骨架 + 禁 `{:?}` 检查 + CI | `_tmp/20261025-b4/skeleton.md` | `state/handoff/skeleton.md` | **完成，已收尾**（atg-cli 骨架 + 禁打印两层检查 + CI；三条小拍板已落；窗口已关）；转交表在现状页 |
| `patch`（会话 `telegram-accessibility-5f`） | 批次 b3 · issue #3 tdlib-rs 补丁 → TDLib 1.8.69（补丁仓 `5-App/tdlib-rs`，分支 `atg/tdlib-1.8.69`） | `_tmp/20261025-b3/patch.md` | `state/handoff/patch.md` | **完成，已收尾**（补丁 @ `4521d6d` 已推 fork；窗口已关） |
| `sync`（会话 `telegram-accessibility-45`） | 批次 b3 · 把 b2 拍板同步进契约 / 内容树 / 路线图；写 issue 草稿 | `_tmp/20261025-b3/sync.md` | `state/handoff/sync.md` | **完成，已收尾**（b2 + N1–N8 落进正文 · 9 处冲突修 · issue 草稿 19 份；窗口已关） |
| `apikey`（会话 `telegram-accessibility-a6`） | 批次 b3 · 用 Claude in Chrome 申请 api_id / api_hash 写入 `.env.local`（手机号、登录码经大总管向人要；值不许外露） | `_tmp/20261025-b3/apikey.md` | `state/handoff/apikey.md` | **完成，已收尾**（应用 atg client 已建；`.env.local` 两值齐，600，git 忽略；已登出、关标签；窗口已关） |
| `api`（会话 `telegram-accessibility-13`） | 批次 b2 · 系统分析 002：功能清单 + 路线图 | `_tmp/20261025-b2/api.md` | `state/handoff/api.md` | **完成，已收尾**（自审 + codex 003 全修；窗口已关）；待人拍板 F1–F8、S0–S3（契约 A2–A5 归 infra） |
| `tree`（会话 `telegram-accessibility-2e`） | 批次 b2 · 内容树（AST）设计 + `atg-core` 原型 | `_tmp/20261025-b2/tree.md` | `state/handoff/tree.md` | **完成，已收尾**（35 测试 / clippy / fmt 过；规约 + 原型 + 架构方案；窗口已关）；待人拍板 7 条（`specs/content-tree.md` 第十节） |
| `infra`（会话 `telegram-accessibility-8f`） | 批次 b2 · 树以外的基础契约 v0（动作 / CLI / 错误 / 配置隐私 / 测试） | `_tmp/20261025-b2/infra.md` | `state/handoff/infra.md` | **完成，已收尾**（自审 63 + codex 004 17 条全修；直发模式写入 actions.md 一·5；窗口已关）；待人拍板 16 条（汇总表在现状页） |
| `tg`（会话 `telegram-accessibility-b3`） | 001 · 命门 3：grammers vs TDLib | `_tmp/20261023-dispatch/tg.md` | `state/handoff/tg.md` | 完成，已收尾（窗口已关） |

## 规矩

- **交接写成文件**：每个手下维护 `state/handoff/<窗口名>.md`（手上是什么 · 做到哪 · 下一步）
- **写任何东西、做任何工作都要回报**（人定 2026-10-23）：当场记进现状页，重要节点再发消息
- **只有大总管提交、推送、开 PR**；手下只写自己负责的文件，不碰 git 的写操作
- **重活名额：每台机器同一时刻只许一处重编译 / 全量测试**，名额由大总管发
- **弹窗口 / 抢前台的运行**：2026-10-25 人给长期许可——**随时可跑，每次不超过 1 分钟**；超过 1 分钟的仍先经人同意（原规则：先经人同意）（人在用电脑；2026-10-23 exp004 窗口被人关了数次）。**人关窗口就让它关，绝不拦截**
- **手下可以自己派子代理**（人定 2026-10-23），子代理同样受名额与 git 规矩约束
- **会话勤轮换**：做完一个自然段落就写交接、收尾，由大总管起新的接班
- 评审分级、先便宜后贵，见 `specs/workflow.md` 第 6 节

## 名额

| 名额 | 当前持有 |
|---|---|
| 重编译 / 全量测试（1 个） | 空 |

排队（先来先得；GPUI 首次编译最重，之后共享 `_build/20261023-154000`）：

- （空）

等人许可的弹窗运行（**2026-10-25 人已同意：「run 跑」，VoiceOver 已开好**；一次只跑一个）：

1. ~~exp003b：`selftest-tab`~~ ✅ 两轮各 45 步 0 失败（2026-10-25）
2. ~~exp004：补跑~~ ✅ 7/7 通过（79.7 秒，2026-10-25）

## 发现（实验中途）

- **正式账号登录成功（2026-10-25）**：人要求「你直接帮我登录吧」，大总管用 expect 代输手机号与人给的验证码（`auth phone` → `auth code`），档案 main 已登录；号码与验证码未落盘（扫描确认）。#35 已补评论

- **人定（2026-10-25）**：登录验证改用人自己的正式账号，人在「终端」App 亲手输入手机号 / 验证码 / 二步密码（AI 不经手）；login 手下写 `src/atg-cli/live/login-main.sh`，人跑。测试服务器问题 → #35

- **2026-10-25**：PR #33 合并、#16 关；#17 `status:active`，分支 `feat/17-cli-login`

- **codesign 授权（2026-10-25）**：人要求代输登录密码；自动模式拦下「写密码进文件」后，人退出自动模式并说「在试一下」→ 大总管直接运行 `security set-key-partition-list -S apple-tool:,apple:,codesign: -s -l atg-dev`（只限 atg-dev），实测 codesign 不再弹框。密码未写入任何文件；已提醒人密码出现在对话记录里

- **codex 额度用完（2026-10-25，恢复 2026-11-01 13:13）**：人定「自审通过就合并，事后补审」→ `CLAUDE.md` 临时例外 + issue #32 补审清单；`.claude/settings.local.json` 放行 codex-review 脚本（人「直接给 claude code 加上规则，不拦截就好」）

- **开发证书（2026-10-25）**：`scripts/setup-dev-codesign.sh` 建好自签名代码签名身份 `atg-dev`（登录钥匙串，仅授权 codesign）；开发版程序用 `codesign -s atg-dev` 签名

- **2026-10-25**：PR #31 合并，#15、#28 关；#16 `status:active`，分支 `feat/16-tdlib`

- **#15 收尾（2026-10-25）**：codex 十三轮提 43、采纳 37、人定 1、转 issue 5（#28 已修、#30 留）；两条经验入 `state/memory/`（唯一写入口 + 穷举测试；检查脚本静默失败）

- **spawn-worker.sh 第二次真机（2026-10-25）**：修过的就绪正则 3 秒认出、首条消息一次提交成功，exit 0

- **人定（2026-10-25）**：每次审查都要过 codex（`CLAUDE.md` 第二节）；做专门 skill `codex-review`
- **spawn-worker.sh 真机首试（2026-10-25）**：读屏模式下提示符是 `$` + 「auto mode on」，原就绪正则认不出 → 已加；长段粘贴后首次回车常被吞 → 已改为隔 3 秒补一次回车

- **清理（2026-10-25，人「可以不用轮换 / 然后其他的该删除的就删除吧」）**：密钥不轮换。已删：`_tmp/20261025-b5/skills/`（已装到 `2-aartifacts`，留副本违反唯一编码）、`_tmp/20261025-b3/issues/`（已开成 #4–#22）、`_tmp/20261025-b5/digest/`（各项目对话摘录，隐私）、`_build/exp004-*` 零散输出（结果已在 `docs/experiments/exp004-announce-calm/out/`）。试听通过后又删 `_build/20261023-154000`（6.9 GB GPUI 实验缓存）。保留：`_build/20261025-171733`（当前 TDLib 库）、`target/`（当前编译缓存）、各批交代文件（账本引用）

- **skills 安装（2026-10-25，人定「帮我安装在 xingan chen 2. my create 里面有一个artifacts 里」）**：7 个复制到 `~/xingan-chen/B-MyCreate/2-aartifacts/skills/`，`~/.claude/skills/` 建软链接生效。**人「好我都同意」**：旧草稿 `agent-coordinator` 已删；report 第四节 9 条全按建议（登录交接取严格版、bib 自动填 author 默认关、待定格式等）；`coordinator-dispatch/scripts/spawn-worker.sh` 下次派活真机试

- **2026-10-25**：PR #27 合并、#25 关；#15 `status:active`，分支 `feat/15-model-reducer`

- **#25 人定（2026-10-25）**：ATG.md 改指本地路径 + 脚本；`LOCAL_TDLIB_PATH` 每次 `export "$(bash src/tdlib-build/build.sh path)"`；旧克隆 `5-App/tdlib-rs`（9.7 GB）与 `_build/20261025-161449` 已删

- **2026-10-25**：PR #26 合并、#14 关；#25 设 `status:active`，分支 `chore/25-src-tdlib`，`git subtree add` 搬入 `src/tdlib-rs`（`252a200`）

- **人定（2026-10-25）**：所有代码都放 `src/`（`CLAUDE.md` 第五节）；tdlib-rs 补丁搬进 `src/tdlib-rs`（git subtree 带历史，反馈上游时再推 fork）；TDLib C++ 源码**不进库**，用脚本按锁定提交拉取编译。排在 #14 之后另开 issue

- **2026-10-25**：PR #23、#24 合并，issue #3 关闭；#14 设 `status:active`，本地分支 `chore/14-skeleton`
- **2026-10-25 开 issue #4–#22**（sync 草稿，大总管代开，`by:ai`，意图只引人原话）：#4–#13 = F7 十条；#14–#22 = 第一刀第 1–9 块。均未打状态标签（活跃的只有 #3）
- **patch ①② 通过**：45 单测；新增 `tdlib_rs::execute`，建客户端前降日志级别后 stderr 0 行；生成函数不再 unwrap，崩溃信息只留类型名

- **patch 补丁验证（2026-10-25）**：build / clippy / fmt / 40 单测过；version_check 读到 1.8.69（commit ea3f69c）。补丁另修生成器递归类型（richTextButton↔InlineButton）。2 个 doctest 与补丁无关（上游本就跳过）。**给 atg-core**：TDLib 默认日志在 setLogVerbosityLevel 生效前就把整条 update 打到 stderr → 建客户端前先用同步 execute 把级别设低

- **patch（2026-10-25）**：TDLib 1.8.69 编好（203 s），`_build/20261025-161449/tdlib/lib/libtdjson.1.8.69.dylib`，依赖 homebrew openssl@3。坑：**项目路径末尾有空格**，TDLib 链接参数 `-exported_symbols_list` 没加引号被拆开 → 用不带空格的软链接编译。连测试服务器缺 `.env.local`（api_id / api_hash）→ 问人

- **2026-10-25**：b2 已提交并开 PR #2（叠在 PR #1 上）；issue #3（补丁，`status:active`）；fork `AllenChen-Xingan/tdlib-rs` 克隆在项目外 `5-App/tdlib-rs`

- 待转（infra → tree，已关）：UserStatus 加 `stopping`；撤回占位只认 `is_permanent` 且非 `from_cache`
- 待转（infra → api，已关）：`decision.md` F1「焦点经过哪条标哪条」依据不成立——`viewMessages` 把已读推到最大 id（`MessagesManager.cpp:16961`）；已读 / 已听在规则定下前默认关闭
- 待转（api 已关）：tree 已做 content-types v0.2 三条 + document `size`；`docs/research/002_features/content-types.md` 如需同步 `size`，交下一个接手的手下

- **tree 原型（2026-10-25）**：`atg-core` 30 测试全过（20 单测 + 1 覆盖 + 9 快照），clippy / fmt 过；大总管复跑一致。仓根 `Cargo.lock` 入库

- **人定（2026-10-25）**：`atg-core` 用有类型的内容树（AST）表示一切要读 / 打印的东西，各输出都是渲染器。批次 b2 分支 `docs/research-002-features`（叠在 PR #1 上）

- **exp003b-v2 自测（2026-10-25）**：绕 / 不绕输入法两轮各 45 步 0 失败（Tab 环、列表内 ↑/↓ 跨屏、离开回来选中保持、模拟 VO 落到列表后 ↓ 生效）；macOS 层 VO+←/→ 会经过 AXList「消息」。局限：进程内事件不经 VoiceOver；自测时输入法是 U.S.，双拼疑点靠人工对照。人试听：`人工试用.md`「第三次试用」17 步

- **exp004 补跑 7/7（2026-10-25）**：模式 6 标题与列表名带「2 条新消息，按 M 收听」；模式 7 选中项提示语同句。剩人试听

- **人改拍板 3（2026-10-25）**：消息列表是专门控件，Tab 或 VO+←/→ 找到，进入后 ↑/↓ 浏览；记入 `decision.md`。可写架构方案了；手下可自由派子代理

- exp003：accesskit_macos 0.26.3 不把 `position_in_set` / `size_of_set` 交给 macOS，「第几项共几项」大概率听不到；对照组试把它直接拼进 `aria_label`

- exp002：进程内调内容视图的 `accessibilityChildren` 即可激活 GPUI 无障碍树，**不需要辅助功能权限**（accesskit_macos `subclass.rs:92-99`）；用 Zed 的 `Cargo.lock` 作种子可共享编译产物。已转给 exp001、exp003

- exp001（初步）：GPUI 原生 `Live` 出口能发公告（Polite 优先级 50 / Assertive 90，改值到发通知 5–21 ms），无焦点变化；同一句重复不播，「换节点」「零宽字符」两种绕法都能补；兜底直发也通。`docs/experiments/exp001-announce/out/result.md`。**是否打断朗读仍要人用 VoiceOver 听**

- **人工试听推翻 exp001 假设 3**：礼貌式公告也会插进正在进行的朗读。「新消息不打断朗读」要另想办法

- exp003（初步）：四种模式各 1137 步在无障碍树层面全部通过，`docs/experiments/exp003-list-nav/out/`；VoiceOver 实听待人

- **人试听 exp003**：只能用 VO+←/→ 切换，单独方向键没反应（待确认快速浏览）。使用者习惯 VO 自由浏览 → 拍板 3（应用方向键驱动）可能要改，候选：接住 VoiceOver 的滚动请求（`on_a11y_action`），让 VO+→ 能走出可见区

- exp002（debug 上界）：TDLib 往返中位 0.02–0.1 ms、约 2.2 万条/秒，不是瓶颈；新消息→无障碍树更新 50 项中位 21 ms / p95 31 ms，500 项 48 / 56 ms（大头在 GPUI 渲染）；冷启动→第一帧 160 ms，→树可查 284 ms（p95 742 ms）。release 暂不跑

- exp003b 疑点：方向键可能被**双拼输入法**吞了；手下会话没有系统级发键权限（不开），用进程内 AppKit 事件验证

- **人（16:2x）**：实验窗口里**上下方向键仍然不行**；**自动朗读（exp004 窗口）感觉还可以**

- exp004（初步）：自动 5/7；暂定推荐「提示音 + 写进名称/提示语 + 按估算朗读时长说汇总 + 按 m 主动收听」。**风险：窗口被挡住时 GPUI 不重绘，新消息提示滞后**（用户在别的应用里可能收不到提示）；实测：**直发公告与提示音不受影响**，只有走 Live 节点的会滞后 → 后台提醒走直发
- 人：输入法「不确定」→ 下次试听先切 ABC 对照；弹窗测试「等我说再跑」

## 待定清单（攒着等人一次看完）

### 批次 b3 待人拍板 N1–N8（sync 2026-10-25）——✅ 已拍板，结果见 `docs/plan/005_decisions-b3.md`；下为题面存档

#### N1. 直发模式遇上本机接口：听谁的
`atg-cli --unsafe-direct-send send …` 碰上 GUI 开着（GUI 不是直发模式启动的），命令转给 GUI 执行。
- A. 只看执行方：GUI 不是直发模式，就照常弹 Touch ID；CLI 带的直发参数忽略，并在标准错误说明
- B. 看发起方：CLI 带了直发参数，GUI 就为这一条免确认
- 代价：A 让「直发」在你开着界面时失效，AI 发不出去要等你按 Touch ID；B 等于任何能连套接字的本用户程序都能借 GUI 免确认（直发参数原本「一定出现在启动命令里、看得见」的性质没了）
- 建议：A

#### N2. P2 × P4 合起来，开发版开正式账号时要不要弹框
P4 说开发版也能开正式档案；P2 说开发证书只管测试档案的钥匙。两条合起来：开发版取正式档案的钥匙时，系统钥匙串会弹框问「允许吗」，你点了才给，每次重新编译可能要再点一次。
- A. 就这样（我已按这个写进 `config-privacy.md` 第一节第 3 小节）：多一次点击，也算一道人手的关
- B. 开发证书也管正式档案的钥匙：不弹框；但 AI 用同一张证书签的任何程序都能拿到正式钥匙
- 建议：A

#### N3. 已读红线和「AI 经界面读也标已读」
`.42cog/real.md:31` 写「开发和测试时，向真实联系人……**标记已读**都是不可逆动作」，自动化测试只能在收藏夹或测试服务器。F1 定了「AI 经界面读时同样会标」，P4 又让开发版能开正式档案——AI 在开发、测试时用界面开正式档案，就会给真实联系人标已读。
- A. 不改红线：AI 的开发、测试一律只开测试档案（和「测试绝不向真实联系人发消息」同一规矩），F1 只管你本人和你在旁时
- B. 改红线：把「标记已读」从开发测试的红线里拿掉
- 建议：A（只需把这句写进 `CLAUDE.md` 第五节或 `testing.md`，不改 F1）

#### N4. C1 的选项名
拍板记录写的是「`--text` 出文字」，契约里一直是 `--format text`；`--text` 已经是 `send` / `draft set` 的正文参数（`--text T`），再当输出格式会撞。
- A. 保持 `--format text`（`cli.md` 现状，我没改）
- B. 另给一个短的全局选项，如 `--human`
- 建议：A；拍板记录里的「`--text`」可能只是简写，请确认

#### N5. F6「停手后播一次」，是谁停手
拍板原文「只在当前打开的私聊里、停手后播一次『对方正在输入』」没有主语。
- A. 对方停止输入后播一次（告诉你「刚才对方在打字」）
- B. 你自己停下操作（不在朗读、不在按键）后播一次（不打断你）
- 建议：B——A 的话，对方停了才说「正在输入」意思反了；正文暂时照原文写「停手后」，没替你选

#### N6. 直发模式管不管「写草稿」和「阅后即焚的已看」
D1 列的是「发出、编辑、删除、表情回应」。`draft_set`（按 L2 同级把关）和打开阅后即焚（按 L2）没提到。
- A. 也免（D1 的题面 B 是「一切 L2 都免确认」）
- B. 照默认规则要确认（正文现状）
- 建议：A，和题面 B 字面一致；你确认前正文按 B 写

#### N7. 「删除给所有人」在测试服务器上怎么验
TDLib 不许在收藏夹里「给所有人删」（`MessagesManager.cpp:8346`）；测试服务器上 L2 又只许碰收藏夹（`test_dc_saved_only`）。所以成功路径在测试服务器上做不成。
- A. 只验拒绝路径 + 假后端单测；成功路径留给你本人在正式账号上删一条自己发错的（M4.6 的人验）
- B. 给「删除」开个口：测试服务器上允许删自己发给另一个**自己控制的测试号**的消息（要再开一个测试号，`test_dc_saved_only` 多一条例外）
- 建议：A（不动红线）

#### N8. 撤回占位（R7）的三处细节
- 记下的发送人是原作者：群管理员删了别人的消息，会读成「林小雨撤回了一条消息」，其实是管理员删的（TDLib 的删除更新不带删除人）
- 会话项最后一条是占位时：会话项本来不读时间，读「林小雨撤回了一条消息」即可？
- 记不到发送人时读「未知发送人撤回了一条消息，19:37」（带时间）？
- 建议：第一条接受（拿不到删除人）、改读「一条消息已撤回」的兜底留给你实听再定；第二、三条按「是」

### 代码解冻后要改的快照与代码（sync 2026-10-25）

时间按夹具 + 东八区算。撤回那条在夹具里是 `1792755420` = **19:37**（拍板里的「19:36」是举例）。

| 快照文件 | 现在的句子 | 应改成 | 依据 |
|---|---|---|---|
| `snapshots__basic_messages_brief.snap` | `林小雨：图片，山顶的风景，结局是，隐藏内容，19:33` | `林小雨：图片，山顶的风景，结局是，隐藏内容，按 D 展开，19:33` | R1 |
| 同上 | `已撤回的消息` | `林小雨撤回了一条消息，19:37` | R7 |
| 同上 | `周大山：视频 1 分 5 秒，隐藏内容，19:32` | `周大山：视频 1 分 5 秒，隐藏内容，按 D 展开，19:32` | R1 |
| 同上 | `周大山：图片，隐藏内容，19:33` | `周大山：图片，隐藏内容，按 D 展开，19:33` | R1 |
| 同上 | `周大山：图片，隐藏内容，19:35` | `周大山：图片，隐藏内容，按 D 展开，19:35` | R1 |
| `snapshots__basic_chats_brief.snap` | `示例频道，天气预报：周六全天晴，最高气温二十三度，最低气温十一度，西北风三级，适合户外活动…` | `示例频道，天气预报：周六全天晴，最高气温二十三度，最低气温十一度，西北风三级，适合户外活动，请大家注意防晒补水` | R2 不截断（句末句号照现规则去掉） |
| `snapshots__entities_brief.snap` | `林小雨：🔥小雨，隐藏内容，19:30` | `林小雨：🔥小雨，隐藏内容，按 D 展开，19:30` | R1 |
| 同上 | `林小雨：隐藏内容，19:30` | `林小雨：隐藏内容，按 D 展开，19:30` | R1 |
| `snapshots__basic_messages_json.snap` | 第 247、938、1262、1513 行的 `speech.brief`；第 345 行 `tombstone` 节点、第 352 行 `"brief": "已撤回的消息"` | `brief` 同上；`tombstone` 多出 `sender`、`date` 两个字段（新增可选字段，`v` 不加一） | R1 R7 |
| `snapshots__basic_chats_json.snap` | 第 136 行 `"brief": "示例频道，天气预报：……适合户外活动…"` | 同 chats_brief 那行 | R2 |
| `snapshots__entities_json.snap` | 第 371、643 行的 `speech.brief` | 同上 | R1 |

JSON 快照里的 `brief` 带「按 D 展开」的前提是：快照测试用带详情键的上下文（像 alert 快照的 `cx()`），另加一组不带键的（像 `no_key`）验 CLI 只读「隐藏内容」。

不用改：`林小雨：跨群回复，19:34`、`周大山：天气预报…，19:30`（转发）、`林小雨：明天改到八点，19:35`（已编辑、有回应）——现在就没把回复 / 转发 / 编辑 / 回应读进名称，已合 R4、R5。提醒、连接状态的句子不受影响。

**连带的代码改动**（同样等解冻）：`speech.rs:22` `LAST_MAX_CHARS` 截断去掉；`speech.rs:133` 隐藏内容加键（`SpeechCtx` 加一个详情键，像 `listen_key`）；`speech.rs:33,68` 撤回占位读法；`tdjson.rs:124` 现在只看 `is_permanent`，要再加 `from_cache` 为假；`Tombstone` 加 `sender` / `date`；`UserStatus` 加 `stopping`。

- [x] 批次 b2 全部拍板（2026-10-25）→ `docs/plan/003_decisions-b2.md`

- [x] 架构方案第七节 10 条拍板（2026-10-25 人定，见方案第七节表）

- [x] 列表名带不带「共 N 条」：只读「消息」（2026-10-25 人定）
- [x] 人工试听 exp003b「第三次试用」：通过（2026-10-25，人「是听完了，我觉得没问题」）
- [x] 人工试听 exp004：通过（同上）

- [x] 人工试听 exp001：礼貌式**插进来播**、打断式掐断立即播 → 「不打断朗读」靠 Live 做不到，需后续实验（exp004）
- [x] **人工试听 exp003**（被 exp003b 第三次试用取代，已通过）：`docs/experiments/exp003-list-nav/人工试用.md`（第一轮 13 步 + 第二轮）
- [x] 位置怎么读：**不读**（人：听着奇怪）；A 否决
- [ ] `001_stack/must-know.md` 三条命门对不对
- [x] `001_stack/decision.md` 拍板（4 条均已定；第 3 条 2026-10-25 改定）：实验顺序（已定：列表浏览 = 应用方向键驱动；Telegram 层 = TDLib + tdlib-rs；tdlib-rs = B 自维护补丁，做好反馈上游）
- [x] 主要用哪种辅助功能：**VoiceOver**（2026-10-25 人「对就是 VoiceOver」；已写进 `.42cog/real.md`）
- [ ] 放权边界：哪些动作不用逐条确认
