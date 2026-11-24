# 评审 072 · #170 常驻软链接路径只留一份正本、两个 build.rs 合成一份（codex · gpt-6-astra · read-only · diff 类）

> 对象：`origin/main...HEAD`（第 1 轮 HEAD = 4793072：检查点 ea8c9f3、自审修正 4793072）。
> - 软链接相对家目录的路径，唯一正本是纯文本 `src/tdlib-build/dev-link`
> - 两个逐字相同的 `build.rs` 合成共用构建脚本 `src/tdlib-build/rpath.rs`：两个 `Cargo.toml` 都写 `build = "../tdlib-build/rpath.rs"`
> - `build.sh`、`check-harden.sh` 改成读 dev-link
> - 规则写在 `src/tdlib-build/README.md`。不合规则时：开了 tdlib 特性的构建失败（默认构建和 CI 不查）；`build.sh` 退 1；`check-harden` 退 2
>
> effort medium。依据：两个 `Cargo.toml`。隐私扫描：干净。
> 之前：
> - **多视角自审 1 轮**（ea8c9f3）：P0/P1 零，P2×4（都是同一件事：三处校验规则不一致，Rust 侧只给警告），P3×8。已改（4793072）：三处统一成一条规则；Rust 侧改成让构建失败；`build.sh` 只在动软链接时才读 dev-link；补了文档
> - **实测**（领名额跑）：test 772 过
>   - 默认构建 LC_RPATH 为 0
>   - 第二次构建是 Fresh；touch dev-link、touch rpath.rs、改 HOME，两个包的构建脚本都重跑
>   - dev-link 改坏后，默认构建照样编过，tdlib 构建失败，报的是我们写的提示；改回去后 git diff 干净
>   - 没有 HOME 时默认构建编过
>   - rpath 只有一条；check-harden 全过

## 第 1 轮（2026-11-24）· 总评「修改」

原文：MAIN 下 `_tmp/20261124-b21/r170-codex/r170-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×2 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- **P2 #1 属实，已改**：shell 用 `[[:space:]]` 判空白，结果随 locale 变。`LC_ALL=C` 下，含不换行空格（U+00A0）的路径会被放过，可 Rust 的 `char::is_whitespace` 会拒。
- **P2 #2 属实，已改**：逗号能通过三处校验，可 `-Wl,-rpath,…` 会把逗号当分隔符拆开，结果链接失败，或者写进去的 rpath 不对。
- 两条一起改：三处都换成**字符白名单**，只许 ASCII 字母、数字和 `.` `_` `/` `-`。
  - Rust 侧按字节判
  - 两份 shell 在 `(LC_ALL=C; [[ … =~ ^[A-Za-z0-9._/-]+$ ]])` 里判
  - 另外照旧：不许以 `/` 开头、不许含 `..`
  - 空白（含非 ASCII 空白）、回车、换行、逗号、非 ASCII 字符一律拒
  - README 写明了规则和原因
- 作者核过：15 种样例在 C 和 en_US.UTF-8 两种 locale 下，三处结论一致。样例有：正常、没有末尾换行、空、绝对路径、`..`、空格、开头空格、两个换行、两行、CRLF、tab、不换行空格、逗号、汉字、`a_b-1.2/c`。Rust 那一列是用同一个表达式另编的小程序跑的，真的构建脚本再实测一次，见下
- 抽查属实 5 条：rerun-if-changed 两条路径从两个包的根目录看都成立；两个 Cargo.toml；默认构建在校验之前就早退；`build.sh` 的赋值能停住，`path` 分支不读 dev-link；末尾换行的处理三处一致
- 无法核实 1 条：测试与实测结果没有日志可读，codex 这一轮没编译

### 是否再审

本轮无 P0/P1，是连续第 1 轮。改完编译实测之后，再审一轮确认。
