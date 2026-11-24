# 上游 issue 草稿（AccessKit）

> 2026-11-14 · 手下 ggpui 起草 · 状态：**草稿，未发**。大总管准写（b19）。
> 为什么是 issue 不是 PR：AccessKit 不收外部贡献者的 PR，要先开 issue 写实现方案、考虑各平台对等、对照 Chromium；方案被认可才给仓库成员资格（AccessKit `CONTRIBUTING.md`，2026-11-14 取）。
> **发之前**：描述由人改定（本项目规矩；Zed 的 AI 政策也要求面向维护者的文字别全靠模型）。草稿里凡写「VoiceOver 会……」的地方，都还没人试过，发前要么人试一次、要么改成「我们预期」。
> 编号照补丁正本 `specs/gui/upstream-patches.md`。

| 草稿 | 补丁号 | 内容 | 发前要先做的 |
|---|---|---|---|
| `accesskit-macos-missing-properties.md` | P2 · P3 · P4 | macOS 适配层不送 `expanded`、`position_in_set` / `size_of_set`、`keyboard_shortcut` | 人开 VO 听一次「第 N 项共 M 项」是否读出（可用 exp003 改成设 AXARIAPosInSet 的小实验） |
| `accesskit-macos-custom-actions.md` | P1 | `custom_actions` 已在数据结构里，macOS 不映射成 `NSAccessibilityCustomAction` | 确认 Windows / AT-SPI 也没实现（读源码：没有），在 issue 里说清只先做 macOS 还是三平台一起 |
| `accesskit-macos-selection-userinfo.md` | P12 | 选区变化通知不带 userInfo，VO 移光标读字 / 词可能不对 | **先人试**：做个输入框，开 VO 按 ←/→、Option+←/→，听读不读字 / 词。问题不存在就不发 |

建议发的顺序：先 P2/P3/P4（各平台对等论据最强：Windows 已有展开和位置，AT-SPI 已有位置），拿到成员资格后再提 P1、P12。注意：P3 对本项目的主要用处是给读 macOS 层的 agent，不是给 VoiceOver 读（人定不读位置，001 拍板 2）；草稿里照实写了。

出处登记在 `../sources.md` `## ggpui`：G15（AccessKit Windows）、G16（AT-SPI）、G17–G18（Chromium，含行号；本地副本只在会话临时目录、未入库，可按网址重取）、G19（WebKit、Mozilla；只读了搜索摘要）。草稿正文里凡没人试过的 VoiceOver 行为都写成「we expect / as we understand」。
