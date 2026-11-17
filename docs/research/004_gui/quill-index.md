# Quill（上游 GPUI Telegram 客户端）资料索引

> 2026-11-14 · 手下 gcomp（子代理读、主会话抽查）· 按 CLAUDE.md 铁律 8：厚材料只带索引，细节回原件。原件在 MAIN `resources/quill-20261114/`（只读、不进库）。摘要见 `findings-components.md` 第九节。
> 术语：**parity slice** = 对标官方客户端补齐某项功能的一块工作；**tdesktop** = Telegram 官方桌面客户端（C++/Qt）；**display link** = macOS 按屏幕刷新率叫醒应用画帧的定时器；**cdhash** = 签名后程序内容的哈希；**ad-hoc 签名** = 不带证书身份的本机签名；**公证** = 交苹果扫描后盖章；**XProtect** = macOS 自带的恶意软件扫描；**Xvfb / lavapipe** = Linux 上无显示器的虚拟屏和软件渲染。表中「行」列是 `DECISIONS.md` 的行号，不是数量。

> 对象：`resources/quill-20261114/`（浅克隆 d4a18b7，2026-11-14，MIT）。下文路径都相对这个根目录。
> 只做索引：一行一条，自己的话；要细节回原文。相关度：高 / 中 / 低 / 无（对我们 = VoiceOver 优先、键盘优先、macOS、GPUI 薄壳）。

## 一、DECISIONS.md（6987 行）

前 75 行是项目定调；76–222 行是第一阶段的聊天核心；223 行以后基本是一块一块补齐功能（"parity slice"）。

| 行 | 节（缩写） | 主题 | 相关度 |
|---|---|---|---|
| 1–14 | Product | 定位、只做一个账号、不做 App Store、凭据怎么读 | 中：范围怎么划 |
| 15–25 | UI family | 工具链固定版本；用 Kit 的 `MessageScroller` 做聊天列表；回车发送规则读输入法标记区（IME marked range）；VoiceOver 留到上 Mac 再做 | **高**：列表选型、IME 下的回车 |
| 26–35 | TDLib | 不用 tdlib-rs，直接绑 tdjson C 接口；一个接收线程 + 单一 reducer；schema 逐字节校验 | **高**：和我们选的 tdlib-rs 正好相反 |
| 36–47 | Phase 1 | 数据库密钥放钥匙串；登录界面是授权状态的纯函数；未读以服务端为准；openChat/viewMessages | **高**：未读与已读的语义 |
| 48–53 | Linux DB key | Linux 上密钥落文件（0600） | 低 |
| 54–60 | Licenses | 各依赖的许可 | 低 |
| 61–67 | Measured prototype notes | MessageScroller 混合行高、前插、重测；Kit 的回车事件不带"正在组字"标记 | **高** |
| 68–75 | Blockers | 接通真实 TDLib 的 smoke；VoiceOver 和真 IME 要在 Mac 上验 | 中 |
| 76–138 | 全局搜索、会话内搜索、回复、编辑删除、转发、表情回应、置顶、静音归档、"正在输入" | 第一阶段消息功能 | 中：CLI 功能清单可参考 |
| 139–201 | 贴纸、语音、GIF、视频、圆视频、音频、相册 | 媒体 | 低 |
| 202–208 | Composer chat drafts | 草稿去抖动（1 s / 5 s）、只做私聊、`updateChatDraftMessage` 的坑 | **中高**：输入框草稿 |
| 209–222 | 链接预览、图片和文件接收 | 下载优先级、文件路径沙箱 | 中 |
| 223–241 | Roadmap Phase 2+ | 功能补齐顺序；VoiceOver 和 IME 一直挂着没做 | 中 |
| 242–1329 | Phase 2.1–7.3：广告消息、频道、机器人、富文本、投票、位置、骰子、查看器、话题、联系人、文件夹 | 功能补齐 | 低（688 富文本实体、1181 文件夹算中） |
| 1330–1383 | 聊天列表头像和频道标题栏 | 界面 | 低 |
| **1384–1442** | **Phase 8.1 Desktop notifications** | `decide_notify` 是纯函数：什么样的新消息该弹通知 | **高**：通知规则 |
| 1443–2216 | Phase 9.x 动态（Stories） | 无关 | 无 |
| 2217–2387 | 话题发帖、查看器里播视频 | | 低 |
| **2388–2503** | **通知声音和通知设置界面** | 声音、作用范围 | 中 |
| 2504–2615 | Phase A 核心聊天差距盘点和慢速模式 | 盘点方法 | 中 |
| 2616–3038 | B1–B4 私密聊天、自毁消息；C1 通话信令 | | 无/低 |
| 3039–3083 | Phase E RTL / 双向文本 | 文字方向 | 低 |
| 3084–4481 | Phase C 通话全家桶、D2/D3 频道管理 | | 无 |
| 4482–4522 | 私密聊天补齐 | | 无 |
| **4523–4549** | **M1 输入框格式 + 消息操作** | 发送选项、实体 | 中 |
| 4550–4882 | G1/G2 群组、M2、MED1–4 媒体下载 | | 低 |
| **4883–4974** | **CL1–CL3 聊天列表** | 行菜单、置顶排序、多选、标记已读 | 中：键盘操作清单 |
| 4975–5025 | 扫码登录、两步验证 | | 低 |
| 5026–5141 | 机器人键盘、通话 | | 低 |
| **5142–5157** | **Dev-profile iteration** | 只留行号调试信息，增量编译快约 35% | **中高**：构建速度 |
| 5158–5391 | 机器人、投票、会话、预览 | | 低 |
| 5392–5422 | 设置 → 外观 | 主题 | 低 |
| 5423–6489 | 账号、支付、通话、贴纸和表情后端、社区等几十个功能块 | 功能补齐 | 无/低 |
| **6490–6507** | **A11 改做托盘图标显示未读数** | macOS 托盘 | 中 |
| 6508–6898 | 机器人、表情、导出等 | | 低 |
| **6899–6928** | **platform-autostart** | 开机启动 | 低中 |
| **6929–6942** | **核实系统桌面通知** | 早期 macOS 走 osascript（后来改成 UNUserNotificationCenter，见 codex-notify-native） | **高**：通知走过的弯路 |
| 6943–6987 | 消息里的自定义表情、输入框里的动画表情建议 | | 低 |

## 二、docs/decisions/*.md（共 766 份；下面挑出和我们有关的）

| 文件 | 一句话要点 | 相关度 |
|---|---|---|
| a11y-cached-views.md | GPUI 的缓存视图被复用（replay）时，它那一块无障碍树原本会丢；补丁让复用时一并带上。附 CPU 实测 | **高** |
| accessibility-and-dock-menu.md | 消息列表的屏幕阅读器焦点模型（照 tdesktop：上下键逐条、PageUp/PageDown、Home/End、Space）、每条消息读什么、新消息不播报；Dock 菜单 | **高** |
| codex-screen-reader-content.md | 消息正文原来不在无障碍树里，补上文本、链接、剧透按钮；`macos-accessibility-smoke.sh` 用 AXPress 和 AXValue 实测 | **高** |
| codex-parity-account-export-accessibility.md | 全量盘点控件名称：输入框、对话框标题、可点击行都要有角色和名字；VoiceOver 的实际朗读仍未核 | **高** |
| focus-return-affordances.md | 自绘浮层关掉后焦点回到原处：每帧跟踪一次，不用每层各写一遍 | **高** |
| parity-platform-high-contrast.md | 高对比主题做成第三种主题选项 | 中 |
| agent-access.md（第 375–760 行） | 用强制生成的无障碍树做界面驱动和测试：快照、注入输入、等下一帧 | **高**（测试） |
| codex-ingest-budget.md | 主线程每个 tick 处理 TDLib 更新不超过 8 ms | **高** |
| codex-idle-cpu.md | 重绘按紧急程度分四级；空闲时让 display link 停下 | **高** |
| codex-idle-redraw.md / idle-inactive-redraw.md / sweep-idle-* | 空闲时的重绘和定时器 | 中 |
| main-thread-stack.md | 主线程栈溢出：大结构体按值放在 App 里，改成 Box，再用测试卡住大小 | 中 |
| tdlib-close-off-ui-thread.md | TDLib 的 close 不能在 UI 线程上同步等；照 Telegram X 的做法异步等 Closed | **高** |
| tdlib-stale-library-abort.md | 旧的 tdjson 库导致中止 | 中 |
| ui-thread-decodes-spell.md / sweep-r3-ui-blocking.md | 拼写引擎、解码图片、写设置、子进程都挪出 UI 线程 | **高** |
| launch-window-first.md / launch-time-macos-appkit.md / launch-time-early-connect.md / launch-time-fonts.md / launch-warm-waits.md / launch-first-frame-text.md / launch-cold-floor.md / launch-writing-tools-hold.md / launch-first-load-check.md | macOS 冷启动和热启动逐项剖析（AppKit、Writing Tools、XProtect 扫描、提前连 TDLib、字体列表） | **高** |
| codex-perf.md / codex-perf-pass.md / codex-perf-round3.md / render-path-work.md / sweep-perf-update-scans.md | 渲染路径：每帧不克隆、拆成缓存切片、动画单独一层 | **高** |
| codex-history-rows-cache.md / sweep-history-row-cache.md | 历史消息行按"修订号键"缓存，内容和外框分开 | **高** |
| codex-history-window.md | 历史只保留一段连续窗口，打开时从第一条未读处加载 | **高** |
| unread-open-landing.md | 打开聊天落在未读分隔条上（照 tdesktop）；落点修正、视口保持 | **高** |
| sweep-scroll-restore.md | 按聊天恢复滚动位置的功能被撤掉，规则改为"有未读落未读，没有就落底部" | **高** |
| history-loading-reliability.md / history-preload.md / scroll-load-everywhere.md | 加载历史的可靠性（200 个随机种子的模型测试）、提前三屏预加载 | **高** |
| codex-scroll-text-cache.md / paragraph-cache-widths.md / shaped-line-size.md | 文本排版缓存；taffy 一帧会量好几次 | 中 |
| codex-go-to-message.md / codex-jump-buttons.md / codex-jump-highlight.md | 跳到某条消息、高亮 | 中 |
| parity-platform-shortcuts-reference.md | 一张 `shortcut_rows()` 表同时生成键位绑定和快捷键参考页 | **高** |
| parity-platform-custom-keybindings.md | 16 个动作可改键；冲突怎么处理；系统和窗口级的键不许改 | **高** |
| native-macos-shortcuts.md | 每个快捷键同时绑 cmd 和 ctrl 是坑：macOS 上 ctrl 要留给 Cocoa 文本键 | **高** |
| codex-shortcut-pack.md / codex-chat-nav-keys.md / codex-platform-shortcuts-data.md | 照 tdesktop 的快捷键集合（Cmd+1..8、Alt+上下、Ctrl+Tab） | **高** |
| sweep-chat-list-keyboard.md | 聊天列表的键盘导航（照 tdesktop 的 selectSkip 语义） | **高** |
| sweep-r3-menu-focus*.md / sweep-r3-submenu-keys.md / context menu | 菜单的键盘可达性（上下键循环、Enter/Space 执行、左右开关子菜单） | **高** |
| codex-esc-overlays.md | Esc 关闭顺序收成一张表（最上层先关） | **高** |
| dismiss-layer.md | 点外部关闭浮层时的命中测试和遮挡 | 中 |
| codex-composer-input.md | 输入框里的格式标记、@提及标签（照 tdesktop 的 TextWithTags） | 中 |
| codex-composer-keys.md | 全局 action 默认吃掉按键 → 输入框里方向键失灵；处理不了时要 `cx.propagate()` | **高**（坑） |
| codex-rtl-composer.md | gpui-base 输入引擎打了补丁 | 低中 |
| long-text.md | 一行 100 KB 时每次按键 260 ms → 只增量重排 | 中 |
| sweep-per-chat-composer.md | 发送选项挂在全局 → 串到别的聊天；改成按聊天存 | **高** |
| cloud-draft-sync.md / codex-drafts-everywhere.md | 草稿同步 | 中 |
| flaky-ui-tests.md | 对话框入场动画走墙钟时间，测试时钟管不到 → 测试里打开"减少动态效果" | **高** |
| codex-ui-tests-green.md | UI 测试从没进过 CI，三条一直是红的；全局 atomics 在测试里改成线程局部 | **高** |
| ci-launch-smoke.md | CI 在 Xvfb 和 lavapipe 下真开窗，截图后检查是不是一张空图 | **高** |
| sweep-test-env-cache-race.md / codex-flaky-core-test.md / sweep-*-flake.md | 并行测试里的全局缓存竞争 | 中 |
| entity-leaks.md | 关掉的界面是否还留着 Entity 和订阅：在 test-support 下加钩子统计 | **高** |
| codex-notify-native.md | macOS 用 UNUserNotificationCenter 做内联回复和头像；在 GPUI 平台 crate 里开了一条旁路 | **高** |
| codex-notify-os.md / notification-preview.md / notification-click-repaint.md | 勿扰模式、Dock 弹跳；点通知后窗口不再重绘（display link 的坑） | **高** |
| macos-install-without-notarization.md | 不公证怎么装：curl 加去掉 quarantine 属性 | **高** |
| macos-release-signing.md | ad-hoc 签名每版 cdhash 都变 → 每次更新钥匙串都弹密码；换成固定的 team 身份 | **高** |
| macos-developer-id.md | 换到 Developer ID 时，designated requirement 要分两步过渡 | 中 |
| macos-bundle-update.md | 原地更新（ditto 解包、签名校验、renamex_np 交换） | 中 |
| launch-first-load-check.md | 公证过的包，每次冷启动 XProtect 都要重新扫一遍（每 MB 7–9 ms） | **高** |
| gpui-pre-0-3-12.md / gpui-kit-0-9-1.md / gpui-kit-0-9-0.md / gpui-kit-0-8-0.md / gpui-kit-0-7-3.md / codex-gpui-kit-0.7.1.md | 每次升级的重新 vendor 流程和补丁冲突 | **高** |
| gpui-fork-panics.md | 把 fork 进来的平台 crate 里的 panic 全改成可恢复 | **高** |
| no-crash-audit.md / panic-lints.md / panic-sites-data.md | 全仓禁 unwrap、expect、panic 的 lint | 中 |
| codex-refactor-guardrails.md / docs/contributing/structure.md | 单文件 1000 行上限，存量用棘轮只减不增 | 中 |
| account-scoped-ui-state.md / sweep-window-slot-scope.md | 界面状态按账号、按窗口分开存 | 中 |
| covered-main-window.md / idle-drawables.md | 被自家窗口挡住时仍在画；可绘制对象的内存 | 低中 |
| codex-power-saving.md / thermal-power-saving.md | 省电模式 | 低 |
| crash-log.md / stability-hub-crash-log.md | 崩溃日志 | 低中 |

## 三、其他值得知道的顶层文档

| 文件 | 一句话 |
|---|---|
| README.md（173 行） | 总介绍和安装 |
| STATUS.md（1219 行） | 对标 Telegram 的功能清单，带 `parity:` 锚点；1172–1173、1218 行是无障碍那几条（VoiceOver 未勾） |
| ROADMAP.md（196 行） | 第 2 阶段以后的功能补齐路线 |
| PRODUCT.md（64 行） | 只讲官网页面，和应用本身无关 |
| AGENTS.md（40 行） | 硬规则：不许有隐式状态机、不许崩、文件大小上限、测试必须确定、合并前跑哪些检查脚本 |
| CLAUDE.md（1 行） | 指向 AGENTS.md |
| docs/build.md | 工具链、gpui-kit、TDLib 的固定版本，以及怎么构建 |
| docs/native-bundle.md | 让 tdjson 不依赖 Homebrew rpath 的打包实验 |
| docs/dependency-updates.md | 每周巡检依赖（自写脚本、不用 Dependabot）；gpui 家族禁止自动升级 |
| docs/quality-audit.md | 2026-10-02 的质量审计 |
| docs/contributing/{state-machines,panic-safety,structure}.md | 状态机、不崩、目录结构三份守则 |
| third_party/*/QUILL-CHANGES.md（共 2482 行） | 每个 vendor 进来的 crate 各记一份改动，gpui-pre 那份 600 行最关键 |
| scripts/ | `macos-accessibility-smoke.sh`、`macos-voiceover-smoke.sh`、`launch-bench.py`、`launch-profile.py`、`ci-launch-smoke.sh`、`check-*.py` 守卫脚本 |

## 四、源码入口（`findings-components.md` 引过的）

| 路径（相对 `resources/quill-20261114/`） | 是什么 | 相关度 |
|---|---|---|
| `src/main.rs` | 入口：命令行开关 → 开窗（约 :78、:330-351） | 中 |
| `src/ui/app_slice.rs` | 把大视图拆成小「切片」实体、`Entity::cached` 复用 | **高** |
| `src/ui/notifications/mod.rs`、`src/ui/background_accounts/mod.rs` | 主线程按 8 ms / 4 ms 时间预算取 TDLib 更新 | **高** |
| `src/state/redraw.rs` | 更新打分决定重绘时机（四级） | 中 |
| `src/telegram/hub.rs` | 全进程唯一收线程、按账号分发 | 中 |
| `src/ui/history_a11y.rs` | 消息列表的读屏焦点模型（active descendant、落点、检测读屏在跑） | **高** |
| `src/message_a11y.rs` | 每条消息的读法（照官方桌面客户端移植） | **高**（对照我们的 `speech`） |
| `src/composer/send.rs` | 回车是否发送的纯函数（组字不发） | **高** |
| `src/ui/keybindings/{mod,shortcut_rows}.rs` | 快捷键表、改键、按键追踪开关 | **高** |
| `tests/replay_*.rs`、`tests/replay_common.rs` | 回放 JSON 喂 reducer 的测试 | 中 |
| `third_party/gpui-pre/QUILL-CHANGES.md` | 对 GPUI 的全部补丁说明（强制建树、缓存视图带树等） | **高** |
| `third_party/gpui-pre-macos/src/system_notifications.rs` | 通知：查 bundle id、授权、工作线程 | 中 |
| `scripts/macos-accessibility-smoke.sh`、`macos-voiceover-smoke.sh` | 真窗口 AX 冒烟 / 读 VoiceOver 最后一句 | **高** |
