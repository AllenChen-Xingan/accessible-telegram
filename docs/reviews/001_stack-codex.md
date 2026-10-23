# 评审 001 · 选型分析（codex · GPT 系 · read-only）

> 2026-10-23。对象：`docs/research/001_stack/`。处理：P1×2、P2×3 全部并入 `decision.md` 与 `gap.md`。

发现以下问题；未修改文件。文档路径省略 `docs/research/001_stack/`，源码路径沿用原文简称。

- **P1｜decision.md:14、18**：两个实验均不验证跨屏列表导航；方向键方案改变了命门1的 VoiceOver 逐项移动要求，不能以播报、延迟实验通过代替列表验收。
- **P1｜decision.md:58**：`local-tdlib` 不能单独解决缺新 API；`tdlib-rs/build.rs:310` 仍从仓库内 `tl/api.tl` 生成绑定，且 `:93` 硬编码 1.8.61 的动态库文件名，升级还须同步 schema 和构建配置。
- **P2｜gap.md:37**：比较对象写成“TDLib 1.8.69 + tdlib-rs”，实际下载由 `build.rs:21、199-205` 锁定 1.8.61；必须按交付版本复核，不能直接继承新版本源码结论。
- **P2｜gap.md:20、61**：缓存存在不等于冷启动快；实验只测新消息到通知，既漏测冷启动，也不能据整段超时就归因 TDLib、改选 grammers，需分段计时和对照。
- **P2｜findings-gpui.md:12、18**：accesskit_macos 的焦点通知、公告触发、优先级及重复抑制均**无法核实**；本地只能证实 GPUI 写树出口，不能确认 macOS 端行为。

核实且属实的引用：

- `element.rs:427-442`：激活后，有 ID、角色才建节点。
- `div.rs:2378-2386`：将已聚焦节点报给树。
- `a11y.rs:564-583`：写入树焦点。
- `a11y.rs:364`：公开可变父节点。
- `uniform_list.rs:475-492`：按可见区渲染。
- `auth.rs:271`：付费分支确为 `unimplemented!()`。
- `td_api.tl:12758`：支持仅取本地历史。
- `build.rs:196-215`：确有 Release 下载路径。

结论「修改」