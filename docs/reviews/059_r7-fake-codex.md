# 评审 059 · #60 R7 第八刀：拆 core `fake.rs`（只挪不改）；R3a 收尾（codex · gpt-6-astra · read-only · diff 类）

> 对象：`f5ca5b2...HEAD`（检查点 5ab87e3、bc94287）。
> - `fake.rs` 1021 → 548，拆出三个文件：`fake/script.rs`（测试用的设置方法）、`fake/login.rs`（`FakeAuth` 和登录）、`fake/read.rs`（会话与历史）。`Backend` 的 15 个方法在根上一行转给 `do_*`，签名不变，函数体照原样；`fake/submit.rs` 没动
> - R3a 收尾：cli `tests/cli.rs` 的 `send_usage_paths`、`send_file/tests.rs` 的 `env` 换成 `testkit::temp_dir`
> - 基线删 `fake.rs`；改 010 的三行、`testkit.rs` 头注释、交接页
>
> 依据文件：拆前原文件、`--color-moved` 里没被识别为挪动的行、新旧几个 fake 文件、`testkit.rs`、010。effort medium。
> **状态：第 1 轮（2026-11-17）· 总评「通过」· 连续第 1 轮无 P0/P1。**
> 之前：
> - **多视角自审 1 轮**（5ab87e3）：P0/P1 零；P2×1，P3×8 里改了 5 条（bc94287），另 3 条不改，理由见交接页
> - **全套**（5ab87e3）：fmt、两种 clippy（`-D warnings`）、`cargo test --workspace`（713 过、0 败、5 忽略）、check-no-debug、check-file-size、check-sources 全过

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-r7fake-codex/r7fk-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现。codex 核实了这几点：
  - 三个子模块里的函数体和原文件逐字一致（login 17 个、read 7 个、script 26 个），没有漏挪的
  - 15 个转发的签名不变，参数和顺序都对
  - `FakeAuth` 经根上重新导出，字段不变；登录的私有助手仍是私有；`do_*` 都是 `pub(super)`；`HANG` 和各设置方法照旧能用
  - R3a 收尾两处的目录布局、断言不变
- 抽查「不符」1 条：010 写 `script.rs` 156 行，实际 158 行。属实：自审时给它补了两行头注释，没同步改行数。010 和交接页已改成 158
- 无法核实 1 条：测试结果只有文字记录，codex 只读、不跑测试，以 CI 为准

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。
