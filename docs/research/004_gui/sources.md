# 004_gui · 来源登记

> 四人共写，各只追加自己那一节，不动别人的。

## ggpui

| # | 来源 | 类型 | 版本 / 日期 | 在哪 | 用来答 |
|---|---|---|---|---|---|
| G1 | Zed 主干（gpui、gpui_macos、settings_ui） | 他人 · 源码 | `15d136b` 2026-11-14 | MAIN `resources/zed-20261114/`（部分克隆，只读，191 MB） | 1–8 |
| G2 | Zed 老版本 | 他人 · 源码 | `79fe6a6` 2026-10-23 | MAIN `resources/zed/` | 1（对比） |
| G3 | accesskit / accesskit_consumer / accesskit_macos | 他人 · 源码 | 0.24.1 / 0.38.0 / 0.26.3（Zed 锁定版本） | 本机 `~/.cargo/registry/src/index.crates.io-*/` | 2、3 |
| G4 | accesskit_macos CHANGELOG | 他人 · 文档 | 0.27.2（2026-10-25） | github.com/AccessKit/accesskit `adapters/macos/CHANGELOG.md` | 3、8 |
| G5 | Zed PR #65740（list 自动滚动时无障碍节点回滚，未合） | 他人 · PR | 2026-10-17 开 | github.com/zed-industries/zed/pull/65740 | 4、6 |
| G6 | Zed 开放 / 已合 PR 列表（关键词 accessibility、a11y、accesskit） | 他人 · PR 列表 | 2026-11-14 查 | `gh pr list -R zed-industries/zed` | 1、8 |
| G7 | 本项目旧核查与实验 | 自有 | 2026-10-23~25 | `docs/research/001_stack/findings-gpui.md`、`docs/experiments/exp001/003/004` | 全部 |
| G8 | Zed issues #66690（缓存视图丢节点）、#66691（应用级焦点报成窗口）、#66730（被裁元素报可见） | 他人 · issue | 2026-11-13 开 | github.com/zed-industries/zed/issues/66690 等 | 4、6、8 |
| G9 | AccessKit 提交 `e6ae72f`（#782，Live 改按名称触发）、`cc49415`（#756） | 他人 · 源码 | 进 accesskit_macos 0.27.0 | github.com/AccessKit/accesskit | 3 |
| G10 | accesskit_macos main 的 `node.rs` / `event.rs` | 他人 · 源码 | 0.27.2（2026-11-14 取） | 取自 `raw.githubusercontent.com/AccessKit/accesskit/main/adapters/macos/src/`（2026-11-14；本地只在会话临时目录，未入库，可随时重取） | 8 |
| G11 | AccessKit `CONTRIBUTING.md`（不收外部 PR，先开 issue） | 他人 · 文档 | 2026-11-14 取 | github.com/AccessKit/accesskit | 8 |
| G12 | Zed `CONTRIBUTING.md`（PR 上限、先讨论、AI 政策）与 PR 模板 | 他人 · 文档 | `15d136b` | MAIN `resources/zed-20261114/CONTRIBUTING.md` | 8 |
| G13 | crates.io：`gpui` 0.2.2、`gpui-pre` 0.3.12（所有者非 Zed） | 他人 · 元数据 | 2026-11-14 查 | crates.io API | 7 |
| G14 | Zed 设置页输入框 `settings_ui/src/components/input_field.rs`（TextInput + TextRun 先例） | 他人 · 源码 | `15d136b` | MAIN `resources/zed-20261114/` | 5 |
| G15 | AccessKit Windows 适配层 | 他人 · 源码 | accesskit_windows 0.34.0（本机缓存）；main 同处复核（2026-11-14） | `~/.cargo/registry/src/index.crates.io-*/accesskit_windows-0.34.0/src/node.rs:681-690`（位置）、`:717`（展开）、`:766`（选区事件）、`:1325-1326` | 8、草稿 |
| G16 | AccessKit AT-SPI 适配层 | 他人 · 源码 | main（2026-11-14 取） | `github.com/AccessKit/accesskit/blob/main/adapters/atspi-common/src/node.rs:395-401`（位置）；`events.rs` / `adapter.rs`（光标移动事件）；`expanded`、`keyboard_shortcut`、`custom_action` 零命中 | 8、草稿 |
| G17 | Chromium macOS 无障碍：`ui/accessibility/platform/ax_platform_node_cocoa.mm`（`:326,343` 方法表；`:1740-1756` AXARIAPosInSet / AXARIASetSize 及注释；`:1867-1869,2307` AXKeyShortcutsValue；`:2593-2605`）、`browser_accessibility_cocoa.mm`（`:3439-3495` accessibilityCustomActions） | 他人 · 源码 | main（2026-11-14 取；行号随 main 变） | `chromium.googlesource.com/chromium/src/+/refs/heads/main/<路径>`；本地只在会话临时目录，未入库 | 草稿 |
| G18 | Chromium 旧版 `content/browser/accessibility/browser_accessibility_manager_mac.mm`（选区通知 userInfo） | 他人 · 源码 | 修订 `0d26e79ae48f` | `chromium.googlesource.com/chromium/src/+/0d26e79ae48f/content/browser/accessibility/browser_accessibility_manager_mac.mm`（搜索结果，**未逐行读**） | 5、P12 草稿 |
| G19 | WebKit `AXObjectCacheMac.mm`；Mozilla bug 1653421、1686995 | 他人 · 源码 / bug | WebCore-7611.2.7.0.4；2026-11-14 访问 | `opensource.apple.com/source/WebCore/WebCore-7611.2.7.0.4/accessibility/mac/AXObjectCacheMac.mm.auto.html`；`bugzilla.mozilla.org/show_bug.cgi?id=1686995`、`=1653421`（搜索结果摘要，**未逐行读**） | 5、P12 草稿 |
| G20 | exp009 实验数据 | 自有 · 实测 | 2026-11-14，7 次运行 R1–R7（运行表在其 README） | `docs/experiments/exp009-gpui-head/out/`（不入库）与其 README | 3–7 |

## gcomp

| # | 来源 | 类型 | 版本 / 日期 | 在哪 | 用来答 |
|---|---|---|---|---|---|
| C1 | Zed 老版本（gpui、ui、terminal、agent_ui、settings、script/bundle-mac） | 他人 · 源码 | `79fe6a6` 2026-10-23 | MAIN `resources/zed/` | 1、2、4、5、6、7 |
| C2 | Zed 主干（复核 ui 标注、list、keymap、editor 无 TextInput） | 他人 · 源码 | `15d136b` 2026-11-14 | MAIN `resources/zed-20261114/`（ggpui 拉的） | 2、4、5 |
| C3 | gpui-component / gpui-kit（base、component、kit、shell） | 他人 · 源码（Apache-2.0） | `4379eeb` 2026-11-15（gpui-kit 0.9.1，钉 gpui-pre =0.3.12） | MAIN `resources/gpui-component-20261114/`（浅克隆 49 MB，只读） | 2、3、4、5、6 |
| C4 | Quill（GPUI 写的 Telegram 桌面客户端） | 他人 · 源码 + 决定记录（MIT） | `d4a18b7` 2026-11-14 | MAIN `resources/quill-20261114/`（浅克隆 151 MB，只读） | 1–8 |
| C5 | accesskit_macos 文本范围实现 | 他人 · 源码 | 0.26.3 | 本机 `~/.cargo/registry/src/index.crates.io-*/accesskit_macos-0.26.3/src/node.rs` | 3 |
| C6 | awesome-gpui 应用总表 | 他人 · 文档 | 2026-11-14 访问 | https://github.com/zed-industries/awesome-gpui | 1 |
| C7 | gpui-pre / gpui-kit / gpui-fast / gpui 的 crates 页面 | 他人 · 文档 | gpui-pre 0.3.12（2026-11-09，描述「snapshot of zed@d2a4fb6」）· gpui-kit 0.9.1 · gpui-fast 0.1.15 · gpui 0.2.2（2025-10） | https://docs.rs/crate/gpui-pre/latest 等 | 2 |
| C8 | Zed AccessKit 接入 PR #56065、关闭开关 PR #57954 | 他人 · PR | 2026-05-27 / 05-28 合并 | https://github.com/zed-industries/zed/pull/56065 | 2、6 |
| C9 | coop（GPUI 写的 Nostr 消息客户端，GPL-3.0，git 依赖 Zed 主干不锁 rev） | 他人 · 源码（Cargo.toml） | 2026-11-14 访问 | https://github.com/lumehq/coop | 1 |
| C10 | Zed 性能文章（120fps、帧预算）与厂商对比页 | 他人 · 文档（厂商口径） | 2024-02 / 2026-11-14 访问 | https://zed.dev/blog/120fps · https://zed.dev/blog/videogame · https://zed.dev/compare/vscode | 8 |
| C11 | Zed 截图回归 runner 与 macOS 开发文档 | 他人 · 源码 + 文档 | 2026-11-14 访问 | https://zed.dev/docs/development/macos.md | 6 |
| C12 | 本项目实验与方案 | 自有 | 2026-10-23~25 | `docs/experiments/exp002`、`exp003`、`exp004`；`docs/plan/001_arch-stack.md`；`scripts/dev-sign.sh` | 全部 |

## ga11y

> 短名即 `findings-a11y.md` 里的指代。访问日期均 2026-11-14（子代理 A / B / C / D 搜集，ga11y 汇总；SDK 头 ga11y 亲手 grep，`NSAccessibilityProtocols.h` 一条为子代理 A 查）。可信度见 findings 文件头。AppleVis 主站挡抓取，帖子经镜像 / 搜索摘要读到。

| # | 短名 | 来源 | 类型 | 在哪 |
|---|---|---|---|---|
| Y1 | `AXC.h` / `AX.h` / 转子头 | AppKit SDK 头：`NSAccessibilityConstants.h`、`NSAccessibility.h`、`NSAccessibilityCustomRotor.h` | 他人 · SDK | 本机 `xcrun --show-sdk-path` 下 `System/Library/Frameworks/AppKit.framework/Headers/` |
| Y2 | VO-general / VO-nav / VO-search / VO-quicknav | VoiceOver 用户指南：通用、导航、搜索命令、快速导航 | 他人 · 文档 | support.apple.com/guide/voiceover/general-commands-cpvokys01/mac · …/navigation-commands-cpvokys04/mac · …/search-commands-cpvokys08/mac · …/quick-nav-vo27943/mac |
| Y3 | Mac-shortcuts · FKA | Mac 键盘快捷键；全键盘访问 | 他人 · 文档 | support.apple.com/HT201236 · support.apple.com/guide/mac-help/mchlc06d1059/mac |
| Y4 | HIG-keyboards · HIG-focus | Apple 人机界面指南 键盘、焦点与选择 | 他人 · 文档 | developer.apple.com/design/human-interface-guidelines/keyboards · …/focus-and-selection |
| Y5 | AXMacOSX-guide | Apple 归档《Accessibility Programming Guide for OS X》增强标准控件 | 他人 · 文档 | developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/EnhancingtheAccessibilityofStandardAppKitControls.html |
| Y6 | AppKit-customaction / 转子文档 | `NSAccessibilityCustomAction`、`NSAccessibilityCustomRotor`、SwiftUI `accessibilityRotor` | 他人 · 文档 | developer.apple.com/documentation/appkit/nsaccessibilitycustomaction · …/nsaccessibilitycustomrotor |
| Y7 | NSTextInputClient | 输入法协议（`hasMarkedText`） | 他人 · 文档 | developer.apple.com/documentation/appkit/nstextinputclient |
| Y8 | WWDC23-10035 · WWDC23-10036 | 无障碍审计 / 构建无障碍应用（标识、公告优先级） | 他人 · 视频 | developer.apple.com/videos/play/wwdc2023/10035 · …/10036 |
| Y9 | apple-ax-dev-2013 | 公告被焦点朗读盖掉 | 他人 · 邮件列表 | lists.apple.com/archives/accessibility-dev/2013/Jul/msg00020.html |
| Y10 | WCAG-2.x（正文写作「WCAG 2.5.3」「WCAG-2.1.4」等，均指此条） | WCAG 2.2 理解文档：1.4.1、1.4.3、1.4.11、2.1.4、2.4.7、2.4.11、2.5.3、4.1.3 | 他人 · 标准 | w3.org/WAI/WCAG22/Understanding/（character-key-shortcuts、label-in-name 等） |
| Y11 | APG-* | ARIA 编写实践：listbox、feed、grid、treegrid、dialog-modal、alertdialog、combobox、toolbar、menu、tabs、keyboard-interface | 他人 · 标准 | w3.org/WAI/ARIA/apg/patterns/<名>/ · …/practices/keyboard-interface/ |
| Y12 | ARIA-1.2 · MDN | log / status / alert 角色；aria-relevant；读屏实现指南 | 他人 · 标准 / 文档 | w3.org/TR/wai-aria-1.2/ · developer.mozilla.org（ARIA_Screen_Reader_Implementors_Guide） |
| Y13 | Roselli-2026 | live 区域支持实测（2026-01） | 他人 · 专家 | adrianroselli.com/2026/01/live-region-support.html |
| Y14 | OHara-2022 | Are we live? | 他人 · 专家 | scottohara.me/blog/2022/02/05/are-we-live.html |
| Y15 | Soueidan-2 | 用 live 区域做无障碍通知（下） | 他人 · 专家 | sarasoueidan.com/blog/accessible-notifications-with-aria-live-regions-part-2 |
| Y16 | LevelAccess · Zellwk · Evinced | 语音软件与 ARIA；焦点 vs active-descendant；焦点方法冲突 | 他人 · 专家 / 社区 | levelaccess.com/blog/the-importance-of-keyboard-accessibility-… · zellwk.com/blog/element-focus-vs-aria-activedescendant · knowledge.evinced.com/system-validations/conflicting-focus-management-methods |
| Y17 | WebKit-167680 / 269026 / 209076 / 222190 / 271287 | Safari + VO 的 active-descendant 与 live 区 bug | 他人 · bug 跟踪 | bugs.webkit.org/show_bug.cgi?id=<号> |
| Y18 | Mozilla-1888310 | 公告 medium 改 high 后 VO 自我打断 | 他人 · bug 跟踪 | bugzilla.mozilla.org/show_bug.cgi?id=1888310 |
| Y19 | azukiazusa | 无障碍流式聊天界面 | 他人 · 社区 | azukiazusa.dev/en/blog/accessible-streaming-chat-ui |
| Y20 | UIA-notification · UIA-automationid | Windows UI Automation 通知处理方式；AutomationId | 他人 · 文档 | learn.microsoft.com/dotnet/api/system.windows.automation.automationnotificationprocessing · …automationelementinformation.automationid |
| Y21 | Playwright-locators / -aria-snapshot / -MCP | 按角色定位；无障碍快照；MCP 快照与引用 | 他人 · 文档 | playwright.dev/docs/locators · playwright.dev/docs/aria-snapshots · playwright.dev/mcp/snapshots |
| Y22 | Slack-2022 · Slack-SR · Slack-shortcuts · Slack-changelog · Salesforce-eng | Slack 无障碍改版博客；读屏帮助；快捷键；无障碍更新记录；工程博客 | 他人 · 文档 / 厂商博客 | slack.com/blog/news/slack-updates-accessible-equitable · slack.com/help/articles/360000411963 · slack.com/help/articles/201374536 · slack.com/help/articles/50668520513939 · engineering.salesforce.com/what-is-slacks-secret-for-enhancing-accessibility-… |
| Y23 | Discord-shortcuts · Discord-blog | Discord 键盘快捷键；全应用键盘导航工程文章 | 他人 · 文档 / 厂商博客 | support.discord.com（键盘快捷键页）· discord.com/blog/how-discord-implemented-app-wide-keyboard-navigation |
| Y24 | tdesktop | Telegram Desktop 读屏支持 PR #30306 / #31032 / #31171 / #31197 / #31198 / #31218 / #31432；`history_inner_widget_accessibility.cpp` | 他人 · 源码 / PR | github.com/telegramdesktop/tdesktop/pull/<号> |
| Y25 | Signal | Signal 桌面 issue #5296 / #6090 / #7368 / #7638 / #7745 / #7922，PR #7921 | 他人 · issue | github.com/signalapp/Signal-Desktop/issues/<号> |
| Y26 | Element | element-web #5696 / #34269 / #34324 / #34333；无障碍声明；Marco Zehe 文章 | 他人 · issue / 文档 / 专家 | github.com/element-hq/element-web/… · translated.element.io/legal/accessibility-statement · marcozehe.de/how-to-use-element-and-matrix-with-a-screen-reader/ |
| Y27 | AppleVis-* | Telegram mac、WhatsApp Mac、信息、Discord 的用户报告；189312（信息的播报开关） | 他人 · 社区 | atlas-entry.applevis.com/forum/macos-mac-apps/any-accessible-telegram-app · applevis.com/comment/189312 · applevis.com/bugs/mac/…（信息相关四条） |
| Y28 | GitHub-timeline | 读屏把时间线当列表导航（2026-10） | 他人 · 文档 | github.blog/changelog/2026-10-08-screen-readers-can-navigate-timelines-as-lists |
| Y29 | 其他先例 | Teams 读屏聊天、Zulip 快捷键、freeCodeCamp Discord 读屏、Telegram iOS 审计 | 他人 · 文档 / 社区 | support.microsoft.com/…/use-a-screen-reader-to-chat-in-microsoft-teams · zulip.com/help/keyboard-shortcuts · freecodecamp.org/news/using-discord-with-a-screen-reader/ · i2tc.ru/2024/01/31/accessibility-failures-in-telegram-for-ios-en/ |
| Y30 | Quill | GPUI 写的 Telegram 桌面客户端（MIT），`d4a18b7` | 他人 · 源码 + 决定记录 | 主工作区（本 worktree 之外的 `telegram accessibility ` 目录）`resources/quill-20261114/`（gcomp 拉，即 gcomp 一节的 C4） |
| Y31 | 本项目 | `.42cog/`（intent、real、cog）、`docs/plan/001`、`003`、`005`、`009`、`013`、`docs/plan/002`、`specs/gui/announce.md`、`keymap.md`、`focus.md`、`checklist.md`、`specs/tasks.md`、`specs/architecture-rules.md`、`specs/gui/upstream-patches.md`、`findings-gpui.md`（含 ggpui 转告）、`docs/research/001_stack/findings-gpui.md`、`specs/style/`（读法正本）、exp001/003/004 | 自有 | 本仓库 |
| Y32 | Apple Voice Control | 语音控制使用手册（显示编号 / 名称） | 他人 · 文档 | support.apple.com/guide/mac-help/mh40719/mac |
| Y33 | Apple 论坛 | 合并子元素后 accessibilityIdentifier 查不到 | 他人 · 社区 | developer.apple.com/forums/thread/728871 |
| Y34 | getstream 文档 | 聊天 SDK「正在输入」默认不播（**待核**：子代理 C 转述，具体页面未定位） | 他人 · 文档 | getstream.io（聊天 SDK 无障碍文档；未定位到页） |
| Y35 | `NSAccessibilityProtocols.h` · `StandardKeyBinding.dict` | 角色协议（静态文字要求 value）；系统文本编辑键绑定 | 他人 · SDK | AppKit `Headers/` · `/System/Library/Frameworks/AppKit.framework/Resources/StandardKeyBinding.dict`（自审时 `plutil -p` 核过 Opt-Shift-↑/↓） |
| Y36 | accesskit_macos 0.26.3 | `src/event.rs`（Live 触发公告条件） | 他人 · 源码 | 本机 `~/.cargo/registry/src/index.crates.io-*/accesskit_macos-0.26.3/` |
| Y37 | UMich-IT | 密歇根大学 IT 对 Slack 各平台读屏体验的评价 | 他人 · 社区 | teamdynamix.umich.edu/TDClient/30/Portal/KB/PrintArticle?ID=12980 |
