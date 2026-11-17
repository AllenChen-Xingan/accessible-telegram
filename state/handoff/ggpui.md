# 现状页 · ggpui（b19 · GPUI 无障碍接口现状）

大总管：`telegram-accessibility-98`（2026-11-14 更正；之前误报 -dd 的已补报）
补丁编号一律用 `P<n>`，正本 `specs/gui/upstream-patches.md`（旧草稿里的 G/A 编号已作废；sources.md 里的 G1–G20 是**来源登记号**，不是补丁号）

## 手上 · 做到哪

- `docs/research/004_gui/findings-gpui.md`：第 1–8 节写完
- `docs/research/004_gui/sources.md` 的 `## ggpui`：G1–G20
- `docs/research/004_gui/upstream-drafts/`：AccessKit issue 草稿 3 份（P2+P3+P4、P1、P12），未发
- `docs/experiments/exp009-gpui-head/`：7 次运行 R1–R7 已跑完（运行表在其 README）；名额已还
- 自审：第 1 轮 4 条 P1、第 2 轮 5 条 P1 已修（第 2 轮里「冷编译 29 秒」那条要重测才算清）
- 克隆：MAIN `resources/zed-20261114/`（`15d136b`，191 MB，只读）

## 下一步

1. 等名额，重测一次真冷编译（删 MAIN `_build/20261114-exp009/` 后全量日志存档）
2. 第 3 轮 atg-review；连续两轮无 P0/P1 后跑 codex 041
3. 已告诉 gssot：`specs/gui/a11y-tree.md` 第 11 条和 `specs/gui/README.md` NavList 行抄了我旧写法「滚动一律 scroll_to_reveal_item」，要改成「逐项 ↓ 用 scroll_to_reveal_item，跳跃用 scroll_to(ListOffset)」

## 要人定的事

1. **光标移动时 VO 读不读字 / 词，要人试一次**（findings 第 5 节；补丁 P12）
   - 题面：accesskit_macos 发「选区变了」不带 userInfo（exp009 7/7 实测），VO 可能移光标不读字 / 词
   - 选项：A. GUI 做到输入框（M4.1）时人试一次 · B. exp009 已有最小输入框，人开 VO 按 ←/→ 试 1 分钟
   - 代价：A 晚发现，若确实不读要补 accesskit_macos（AccessKit 不收外部 PR，周期长）；B 现在要人 1 分钟
   - 建议：B；默认：A
2. **消息项读成「组」要不要换叫法**（findings 第 2.2 节，`set_role_description`）
   - 选项：A. 保持「组」（List/ListItem 已拍板）· B. 角色描述设成「消息」/「会话」
   - 代价：B 是非标准叫法，VO 用户可能更顺也可能困惑；要人听
   - 建议：等 GUI 骨架出来人听一次再定；默认：A
3. **先给上游提哪几个补丁**（findings 第 8.3 节）
   - 建议：**P10**（应用级焦点报成窗口，#66691）+ **P6**（测试平台激活无障碍的开关）先做；fork / 推送 / 开 PR 由大总管经人同意，PR 描述人写（Zed 的 AI 政策也这么要求）
   - 默认：先只做 P10
4. **GPUI 的 Metal 着色器怎么编**（findings 第 7 节，实测）
   - 题面：本机 `xcode-select` 指向 Command Line Tools，没有 `metal`；Xcode 里也缺 Metal Toolchain 组件。默认特性编不过
   - 选项：A. `gpui_platform` 开 `runtime_shaders`（运行时编着色器，不改系统）· B. 人装 Metal Toolchain（`xcodebuild -downloadComponent MetalToolchain`，并把 `xcode-select` 指到 Xcode，或构建时设 `DEVELOPER_DIR`）
   - 代价：A 每次启动多一次着色器编译，冷启动变慢多少**没量**（`.42cog/intent.md` 的「速度很快」要量）；CI 同样要定。B 改系统、约数百 MB 下载，但与 Zed 官方构建一致
   - 建议：开发期 A，打 `.app` 前量一次 A 的冷启动代价再定；默认：A
5. **（搁置，暂不问人）外部辅助程序发「设焦点」时 atg-gui 怎么办**（findings 第 4 节）
   - 为什么搁置：焦点漂移 3/7 次，原因未定（推断：VO 光标跟随，或人当时在操作电脑）；取证两次都没复现。不再追，GUI 开发时用调试日志再看
   - 坐实是外部请求后才问：A. 照常移（GPUI 默认）· B. 只在 VoiceOver 开着时移 · C. 记日志不移；建议 A + 被外部移走时不播报、不改选中项
