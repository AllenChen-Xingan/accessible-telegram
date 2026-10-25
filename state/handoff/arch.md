# 交接 · arch（手下）

- **手上**：写 `docs/plan/001_arch-stack.md`（交代 `_tmp/20261025-batch/arch.md`）
- **做到哪**：完成，收尾。方案经三视角自审 + codex 跨谱系评审（7 条全采纳），评审记录 `docs/reviews/002_arch-codex.md`
- **下一步（给接班人）**：等人看方案第七节「待人拍板」10 条；人定了哪条，就回方案对应处改，并更新 R 表。第一刀的拆法在第六节，issue 由人写
- **改过的文件**：`docs/plan/001_arch-stack.md`（新）· `docs/plan/README.md`（+1 行）· `docs/reviews/002_arch-codex.md`（新）· `docs/reviews/README.md`（+1 行）· `docs/experiments/exp003-list-nav/README.md`（第 116 行改一处引用，大总管授权）· 本文件。均未提交

## 流水
- 2026-10-25 读交代；`common.md` 不存在，按交代「规矩」执行；SendMessage 报到大总管
- 2026-10-25 读 CLAUDE.md、.42cog 四份、decision.md、specs/{README,tasks,workflow}.md、docs/plan/README.md
- 2026-10-25 读 research 四份、exp001–004 README 与人工试用、exp003b-v2 交代、coord/board
- 2026-10-25 写 `docs/plan/001_arch-stack.md` 初稿（七节）；`docs/plan/README.md` 加 001 一行
- 2026-10-25 派 3 个对抗视角子代理审初稿（外行 / 老手对照实验 / 挑刺）
- 2026-10-25 三份评审回来：核对员（P1×1 P2×3 P3×4，主因 exp003 README 已补 v2 节）、老手挑刺（P0×1：状态两份 + 应答与更新两路无序；P1×5；P2×2）、外行与规矩（P1×2 P2×5 P3×2）
- 2026-10-25 重写方案全部修完：store 只做 reducer、单线程跑、合并与到达顺序无关（墓碑 + edit_date）；无界通道一帧一批；notify 输入显式 + 初始同步标记；speech 渲染时算；启动/关闭协议；补 FocusManager、Composer；按键监视器按 v2 收窄；术语表；图前文字版；表格改列表；第六节改为「拆法供人写 issue」并插入 model+reducer 一块；待人拍板 8 条各自写全选项与代价
- 2026-10-25 收大总管转来 codex 评审（`_tmp/20261025-batch/codex-arch-out.md`，P1×6 P2×1 + 引用抽查 1 不符）；逐条回源码核实（td_api.tl:3551/11217/11230/12502/12507/12758、tdlib-rs lib.rs:23,51,65-68、observer.rs、gpui element.rs:230-254,431、a11y.rs:331-339、list.rs:1087-1117），全部属实
- 2026-10-25 改方案：(会话id,消息id) 身份；序号 + 版本边界 + 代次 + 按字段合并；分批上限并让出主线程；收线程监督与故障广播；发送确认移到 core 入口（凭据绑定目标+内容）；日志去内容；loadChats 替 only_local；R10 定论、加 R13/R14；第七节加第 9（AI 经界面按确认）、10（测试收藏夹免确认）条
- 2026-10-25 写 `docs/reviews/002_arch-codex.md`，`docs/reviews/README.md` 加一行；发现 exp003 README「B」节同样误引 a11y.rs:331-339，非我负责文件，报大总管
- 2026-10-25 按大总管授权改 `docs/experiments/exp003-list-nav/README.md:116`：a11y.rs:331-339 → element.rs:230-254（只改引用与一句说明，结论未动）；写好交接，收尾
