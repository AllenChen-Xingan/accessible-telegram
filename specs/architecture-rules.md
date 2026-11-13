# 架构守则（给每个写代码的手下，一页）

> 来源：issue #60 人定（意图原话在 issue 里）· 依据 skill `complexity-budget`、`single-source-truth` · 体检与重构清单 `docs/plan/010_complexity-budget.md`
> 能机判的都在 CI 的 `arch` job 里；判不了的，写进交接和 PR 描述，由 arch 线把关。
> 「X 线」= 一个手下会话负责的一条工作线（如 arch 线、media 线）；「大总管」= 派活、收报告的那个会话，有事都报它。「合并目标」= PR 要合进去的分支（`main`）；「crate」= Rust 的一个包（有自己的 `Cargo.toml`）；「棘轮」= 只能往一个方向拧：超标的文件只许变小，变小后基线跟着下调。
> **新代码守这十条**；存量的违例已在 010 的重构清单里排队，由 arch 线第二段收，别的线不必顺手大改（碰到自己正在改的那几行，顺手改对即可）。

1. **单文件 ≤ 600 行**（`src/`、`scripts/` 下被 git 跟踪的 `.rs .swift .py .sh .m .c .h`；只有上游的 `src/tdlib-rs` 和构建产物目录 `target`、`_build`、`node_modules` 除外，夹具、快照目录里的代码照管）。存量超标的记在 `scripts/file-size-baseline.json`，**只许变小**，由 CI 检查（`arch`、`arch-guard` 定为合并必过项——`scripts/github-ruleset.json` 在 GitHub 上应用之后红了才拦得住合并：人定 2026-11-13 B18-6，出处：大总管账本 `state/coord.md`（MAIN 工作区，2026-11-13 待定清单第 369 行 B18-6，本分支开出时还没有；批次收尾时进 `docs/plan/` 的拍板记录）；`scripts/github-ruleset.json`）。只看**已 `git add`** 的文件：本机新建还没 add 的长文件不会报，CI 上才红。本机先跑：`python3 scripts/check-file-size.py`；改了门槛脚本再跑自测 `python3 -m unittest discover -s scripts/tests`（其中 `ReviewTarget` 一组要 `node`，本机没有就跳过、CI 上必跑）：
   - 文件变大、新文件超线 → 红。按职责拆（类型 / 解析 / 渲染 / 测试）；行内测试挪出的做法：`foo.rs` 里留一行 `#[cfg(test)] mod tests;`，测试搬到 `foo/tests.rs`。**测试文件同样守 600 行**，超了就按主题分成 `foo/tests/<主题>.rs`。不许切成 `part1`、`part2`，不许为过线把代码挤成一行
   - **要往已超线的文件里加功能**：新代码放进新的子模块文件 `foo/<新模块>.rs`；`foo.rs` 里多出的 `mod` 声明那一行也算数，所以要**同时把一块已有的、职责相近的代码一起迁出去**（如 media 把 `Call` 枚举挪进 `fake/call.rs`），让超线文件净增 ≤ 0，跑 `check-file-size.py` 确认；不要先加了再求放宽
   - 文件变小了 → 也红，直到同一个 PR 里跑 `python3 scripts/check-file-size.py --tighten` 把基线收紧（否则以后能悄悄长回去）
   - **门槛本身被放宽或改动** → PR 上红：基线放宽（数变大、多了条目、文件改名，比如跑了 `--write-baseline`），或「门槛文件」有任何改动（清单的正本是 `scripts/check-file-size.py` 里的 `GATE_FILES`：门槛脚本、自测、两个工作流、授权脚本、`clippy.toml` 等，以那里为准）。这道检查是 `arch-guard`：它用 `pull_request_target` 跑**合并目标（main）上**那份工作流和脚本，PR 改不到它；PR 的代码只当数据读、不执行。放行：人或大总管给 PR 加 `baseline:loosen` 标签。标签加上后对这个 PR 一直有效，所以放宽之后再 push 新改动，加标签的人要摘掉标签重新看过再加。手下不自己放宽基线、不改门槛文件
   CI：`.github/workflows/arch.yml`

2. **一样东西一个正本**：错误码、退出码、子命令、版本号、参数范围、超时、要读出来的句子，各在代码里定义一次，其余地方引用、生成，或由机器比对。改契约里的表（`errors.md`、`cli.md`……）必须同时改代码正本，反之亦然。
   CI：`python3 scripts/check-sources.py`（正本比对 + 工作区成员检查；对不上就红，并说出差在哪）

3. **一切要读出来或打印出来的东西都先变成树**（`specs/content-tree.md`）：消息、提醒、状态、链接、播放进度、命令结尾的统计……先是 `tree::Doc` 的一个节点，JSON 由 `json` 出，句子**只**由 `speech` 出。CLI、GUI 里不许拼消息、事件、状态、统计的读法句子（参数用法报错的说明不算），也不许拿句子当协议（按中文前缀判断一行是什么）。加节点时渲染器不写 `_ =>`，让编译器逼你补全。

4. **一个能力只做一次**：动手前先在 `src/` 里搜有没有现成的（起后端 → 干活 → 关后端、`saved` 换成自己的 id、输出一行、测试造数据……，清单见 010 第四节）。有就复用；现成的不合用就改它，不另写一份。**要新建共享模块或改模块边界，先报大总管，由 arch 线看过再做**。

5. **CLI 是唯一能力来源**：功能先在 `atg-cli` 做通、有测试；GUI 只调 `atg-core` / CLI 已有的能力，只管展示、按键、焦点，不自己实现业务判断。直跑（这个 `atg-cli` 进程自己连 Telegram 干活）和转发（档案已被常驻的 `atg-cli serve` 或 GUI 占着，命令经本机套接字转给它执行）走**同一条**执行路径。

6. **注释和 README 不抄清单**：不在文件头、README、`--help` 里列「已做了哪些命令」「能转发哪些」「状态有几条线」。要说就写一句「见 X」，指向正本。引用拍板代号（F6、N5……）时带出处文件。

7. **少抽象、多具象**：新加 trait、泛型层、框架之前，在 PR 描述里写清它删掉了多少行重复代码；说不清就不加。抽象优先放在数据上（树的节点、登记表），不放在代码层级上。

8. **一轮只碰 ≤ 4 个实体**（数据库里的表、树的节点种类、模块这类名词）：超了就拆成几个 PR。PR 描述写明本轮动了哪几个实体，删了多少行。

9. **审查要给明确范围**：`atg-review` 是 `.claude/workflows/atg-review.js` 这个多视角自检工作流（在 Claude Code 里用 `/atg-review <范围>` 或工作流工具调用）。跑它时写明审什么——参数传字符串，或传 `{"target": "…"}` 都行；什么都不给会直接报错（`.claude/workflows/atg-review.js` 的 `resolveTarget`，#41：以前不给就悄悄去审整个 `src/`）。codex 评审同理，交代里写死范围。

10. **新 crate、新顶层目录**：`src/` 下以 `atg-` 开头的目录**必须**是 Rust crate（根 `Cargo.toml` 的 `members = ["src/atg-*"]` 会把它当成员，没有 `Cargo.toml` 的话整个工作区的 cargo 命令都会失败）。脚本、Swift 辅助程序这类东西放进用它的 crate 里的子目录，或放进不以 `atg-` 开头的目录（先例 `src/tdlib-build`）。
