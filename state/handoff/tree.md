# 交接 · tree（手下，批次 b2）

- **手上**：内容树（AST）设计 v0 + `src/atg-core` 原型（交代 `_tmp/20261025-b2/tree.md`）
- **做到哪**：**完成，收尾。** 规约、原型、架构方案三样都写完；三视角对抗自审（老手挑刺 / 架构与规矩 / 外行与读屏用户）全部处理；35 个测试 + clippy + fmt 本机全过
- **下一步（给接班人）**：等人定 `specs/content-tree.md` 第十节 7 条读法；人在 `src/atg-core/tests/snapshots/*brief*.snap` 上改字即定标。之后：未读分隔节点（api 升 M）→ 第一刀第 2 块 model + reducer 时把 `tdjson::World::docs()` 的投影搬过去 → 接 `atg-cli speech --fixture`
- **改过的文件**：`specs/content-tree.md`（新）· `src/atg-core/**`（新）· `docs/plan/001_arch-stack.md`（相关段落 + 修订记录）· 本文件。均未提交；仓根 `Cargo.lock` 是编译生成的（大总管说入库）。夹具生成脚本在会话 scratchpad，不入库；夹具 JSON 本身入库

## 流水
- 2026-10-25 读交代与 common.md；SendMessage 报到大总管
- 2026-10-25 读 CLAUDE.md、.42cog 四份、001_arch-stack.md、specs/{README,tasks,workflow}.md、coord.md、CI 配置
- 2026-10-25 grep td_api.tl：textEntity 偏移是 UTF-16（:113）、实体嵌套规则（:118-122）、TextEntityType 24 种（:6309-6383）、message（:3598）、各附件类型（:571-643）、MessageContent 108 种
- 2026-10-25 发现：`.42cog/cog.md` 实体表 crate 段写着「还没建」——按交代不自己改，建好 atg-core 后报大总管
- 2026-10-25 回 infra 五条（外壳 v/kind/data、判别字段 "type"、id 用数字、JSON 时间 RFC3339、读法放外壳 speech）：基本同意，补「一个版本号」「超 2^53 的 int64 写字符串」「Rust 里存 Unix 秒，序列化时转」
- 2026-10-25 收 api 内容类型清单（`docs/research/002_features/content-types.md`），五条提醒全采纳，回复 api
- 2026-10-25 写 `src/atg-core`：`tree/{mod,text,time}.rs` · `speech.rs` · `json.rs` · `tdjson.rs` · `tests/{snapshots,coverage}.rs` · 夹具 `tests/fixtures/{basic,entities}.json`（scratchpad 脚本生成，内容全编）
- 2026-10-25 领名额：test 30 个全过、clippy（加一处 large_enum_variant 豁免）、fmt 过；归还名额。报大总管 cog.md「还没建」→ 大总管已改
- 2026-10-25 写 `specs/content-tree.md`；改 `docs/plan/001_arch-stack.md` 第一节第 2 小节、第三节、第六节第 1/2/6 块，文末加「修订记录」
- 2026-10-25 读法小改（频道项不重复读名字、去句末句号逗号、JSON 键保序 `preserve_order`）——**这次改动后还没重跑测试**，要再领一次名额
- 2026-10-25 收 infra 四问（连接节点、类型名、身份序列化、unknown）：`Connection` 改 `Status{UserStatus}`；正式类型名写进规约第三节第 7 小节；消息身份一律嵌套对象；回复 infra。告知 infra：errors.md 的草拟读法列改成指针
- 2026-10-25 收 api v0.1/v0.2：加 `topic`、`links()`、受限内容 `restricted` + `sensitive`、有效内容（ephemeral）、`mute_inherited`、document `size`；未读分隔记为下一个。最后一条回复发不出（api 会话已关），写进最终回报
- 2026-10-25 三视角自审回来：代码 P1×1 P2×6 P3×4、架构 P0×1 P1×4 P2×4 P3×1、读法 P1×4 P2×5 P3×2。修：交叉时切格式不切链接、排序含种类本身（结果与输入顺序无关、去重不漏）、嵌套上限 32、坏输入 panic×2、时间溢出、`ChatLast` 窄枚举、更新改 Content 的字段表、系统消息「只存 id」、版本规则补「other→专门节点不加一」、GUI 换 id 待办、读法说法统一与逗号分隔、去英文类型名、术语表、待拍板补选项与代价。未修（转人或以后）：同一分钟重复读时间、连续相同 emoji 合并、撤回占位读发送人（进待拍板第 7 条）
- 2026-10-25 第二次领名额：35 个测试全过，快照逐个对过差异；clippy 消一处参数过多；fmt 过；归还
