# 评审 056 · #153 `atg-cli resend` 的 MSG_ID 吞掉后面的选项（codex · gpt-6-astra · read-only · diff 类）

> 对象：`8a98ab3...HEAD`（检查点 2ab84e4、08ba8f3、4b04028）。`src/atg-cli/src/args.rs`：`ResendArgs.ids` 去掉 `allow_hyphen_values`；新文件 `src/atg-cli/src/args/tests.rs`，8 条 clap 解析测试，含一条结构测试。依据文件：`args.rs`。effort medium。
> **状态：第 2 轮（2026-11-17）· 总评「通过」· 连续两轮无 P0/P1 → 满足停止线，收敛。留作 issue 的 P2：无。**
> 之前：
> - **先写会红的测试**：main 上 resend 3 条红，都报 `invalid value '--dry-run'（或 '--format'）for '<MSG_ID>...'`。实际后果是用法错误（退出码 2），不会真重发，#153 已降为 P2
> - **修**：`ids` 去掉 `allow_hyphen_values`。说明写成 `//` 注释，不进 `--help`（第一版用了 `///`，会改帮助文字，当场改掉）
> - **多视角自审 1 轮**：P0/P1 零。P2 两条已改：测试头注释说成「会真重发」，与实测不符；分支的交接页带进了别的线的内容。P3 已收尾，并新增结构测试 `no_variadic_positional_swallows_options`：遍历全部子命令，能收多个值的位置参数都不许开 `allow_hyphen_values`，只放过占位 `Rest` 的 `args`
> - **变异检验**：把 resend 那行临时加回 `allow_hyphen_values`，红 4 条（resend 3 条，加上结构测试报 `["atg-cli resend ids"]`）
> - **全套**：fmt、两种 clippy、`cargo test --workspace`（634 过、0 败、5 忽略）、check-no-debug 全过

## 第 1 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-153-codex/c153-codex-1.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：连续第 1 轮 · 留作 issue 的 P2：无

- 无发现
- 无法核实 2 条：
  - 没显式写 `num_args` 的 `Vec` 位置参数，结构测试抓不抓得到。说明：现在 `args.rs` 里能收多个值的位置参数都显式写了 `num_args`（resend、delete、`Rest`），结构测试覆盖得到；以后不写 `num_args` 的 `Vec`，clap 派生默认按 1.. 处理，见下面「留意」
  - 交接页还写着「待复编、待变异检验」：已改成实际结果（检查点 4b04028）

### 是否再审

再审一轮，确认是否连续第 2 轮无 P0/P1。

## 第 2 轮（2026-11-17）· 总评「通过」

原文：`_tmp/20261117-153-codex/c153-codex-2.md`（不入库）。`result: P0×0 P1×0 P2×0 P3×0`。
本轮 P0/P1：0 · 停止线：**满足**（连续第 2 轮） · 留作 issue 的 P2：无

- 无发现。无法核实 2 条：
  - 不写 `num_args` 的 `Vec` 在 clap 里的默认值（codex 只读，拿不到 clap 源码，没法独立证明）
  - 运行结果只有文字记录（以 CI 为准）

### 是否再审

不再审，按停止线收敛。

### 留意（不算问题）

结构测试靠 `get_num_args()` 判断「能收多个值」。以后有人加了不写 `num_args` 的 `Vec` 位置参数、又开了 `allow_hyphen_values`，要是 clap 那时的默认值不是 1..，这条测试就可能漏掉。现在仓库里没有这种写法；真加的时候，按惯例显式写 `num_args`。
