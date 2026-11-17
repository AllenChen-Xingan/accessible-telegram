# 004 · gui · 图形界面：唯一真相源、GPUI 无障碍现状、无障碍先例、组件架构

> 2026-11-14 · 批次 b19 · 状态：**进行中**（四个调研窗口：ggpui 写 findings-gpui、ga11y 写 findings-a11y、gcomp 写 findings-components、gssot 写取舍卡与 `specs/gui/`）
> 每条决策点的完整取舍卡在 [`decision.md`](decision.md)；已拍板的见 `docs/plan/013_decisions-b19-gui.md`
> 起因（人原话，2026-11-14）：「先详细定图形界面的唯一真香园，single source of truth 然后和各种东西 / 然后在把研究以及best practice 以及各种gpui 的东西，做好，包括完善的无障碍支持」
> 已有底子（不重复调研）：`../001_stack/findings-gpui.md`（2026-10-23 GPUI 无障碍核查）· `docs/plan/001_arch-stack.md` 第二节 · 实验 exp001 / exp003 / exp004

## 文件

| 文件 | 谁写 | 回答什么 |
|---|---|---|
| `findings-gpui.md` | ggpui | 上游 GPUI 最新的无障碍接口；比 2026-10-23 那份新增、改名、修掉了什么；编译成本 |
| `findings-a11y.md` · `checklist-candidate.md` | ga11y | macOS / VoiceOver 无障碍最佳实践与先例；验收清单候选 |
| `findings-components.md` | gcomp | GPUI 应用怎么组织实体、组件、按键；有哪些可借鉴的开源先例（含 Quill） |
| `sources.md` | 四人各写一节 | 来源登记 |
| `decision.md` | gssot | 取舍卡：把三份调研和正本方案（`docs/plan/012_gui-ssot.md`）合成给人拍板的题目 |
| 正本方案 | gssot | `docs/plan/012_gui-ssot.md`；规约 `specs/gui/` |

## 决策点（每条落在 `decision.md` 一张卡；⏳ = 调研还没答完）

| # | 决策点 | 靠哪份调研 | 影响到 |
|---|---|---|---|
| G1 | 八张界面表放哪：不依赖 GPUI 的小 crate / `atg-gui` 里设可选特性 / 数据文件 | findings-gpui（编译成本）· findings-components | 012 第一节 |
| G2 | CI 怎么对付 GPUI（工作区通配会把 `atg-gui` 拉进 `cargo test --workspace`） | findings-gpui | `ci.yml`、根 `Cargo.toml` |
| G3 | 稳定标识的写法 | findings-a11y（agent 先例） | `specs/gui/a11y-tree.md` 第二节 |
| G4 | 动作比对并进 `check-sources.py` 还是另起脚本 | — | 门槛文件 |
| G5 | D 键：展开隐藏内容与读详情合一还是分开 | findings-a11y | `specs/gui/keymap.md`  **已定 → 013 B19-6** |
| G6 | 进会话落在第一条未读时说不说「以下为未读」；未读分隔进不进 ↑↓ 顺序 | findings-a11y | `specs/gui/focus.md` · `a11y-tree.md`  **已定 → 013 B19-7** |
| G7 | 文案不上本地化框架 | — | 012 第三节第 4 小节 |
| G8 | 正本移交：建表 PR 起 md 表改成生成物 | — | `specs/gui/README.md` |
| G9 | 借鉴 Quill 哪些、不借鉴哪些 | findings-components | 组件清单、实体划分 |
| G10 | GPUI 跟哪个版本（Zed 主干某 commit / gpui 发布版 / Quill 用的那份） | findings-gpui · findings-components | 全部  **已定 → 013 B19-10** |
| G11 | 草案按键（`keymap.md` 第五节标「草案」的那些）——**取向已定**（B19-3、B19-4，`docs/plan/013_decisions-b19-gui.md`），具体键待人过目 | findings-a11y（保留键） | `keymap.md` |
| G12 | `intent.md`「不做什么」改写措辞（B19-1 带出） | — | `.42cog/intent.md`  **已定 → 013 B19-9** |
| G13 | 新消息播报详略设置（汇总 / 只计数 / 关） | findings-a11y | `announce.md` 第五节  **已定 → 013 B19-8** |
| G14 | 加载更早消息后说不说「已加载 N 条」 | findings-a11y（候选清单 A14） | `announce.md` · `focus.md`  **已定 → 013 B19-12** |
| G15 | 消息项角色名「组」换不换成「消息」（`role_description`） | findings-gpui 第 2.2 节 | `a11y-tree.md` |
| G16 | 外部 Focus 要不要拦（暂按 GPUI 默认，真碰到再问） | findings-gpui | `focus.md` 第三节 |
| G17 | 搜索结果的焦点模型（combobox 在 GPUI 上做不出；默认 ↓ 进结果列表） | findings-gpui 第 4 节 | `keymap.md` · `focus.md` · `a11y-tree.md` |
