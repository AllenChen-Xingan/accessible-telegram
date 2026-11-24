# exp010-ui-closure · Zed `ui` crate 拖进来多少东西（静态算，不编译）

> 2026-11-14 · 手下 gcomp · 给 `docs/research/004_gui/findings-components.md` 第二节第 1 小节 ① 的数字作依据。

## 做法

`closure.py <Zed 仓根> <起点>[,<起点>…] [--upper] [--minus <基线>[,…]]`：从 Zed 根 `Cargo.toml` 的 `[workspace.dependencies]` 找每个 `workspace = true` 依赖的路径（另认直接写 `path` 的依赖），递归读各 crate 的依赖表，统计每个**Zed 工作区内部** crate 的许可证和 `src/` 下 `.rs` 行数；外部（crates.io / git）依赖只计个数、不展开——外部包不可能反过来依赖 Zed 未发布的内部 crate，所以不展开不会漏掉内部 crate。

- 默认模式：`[dependencies]` + 条件里**正面**写了 macos / unix / apple 的 `[target.*.dependencies]`（含 `not(` 的一律不算，免得把 `cfg(not(target_os = "macos"))` 算进来），跳过 `optional = true`。这是对 cfg 的粗判，不是真求值
- `--upper`（上界）：再加 `optional` 依赖、`[build-dependencies]`、**所有** target 的依赖与构建依赖。特性不解析，所以「只要可能被打开」的都算进来
- `--minus`：减去基线 crate 的闭包，得到「多出来的」。**基线一律按默认模式算**（`--minus-upper` 才按上界）：所以 `ui --upper --minus gpui,gpui_platform` 是「加了 ui、把所有可选特性都打开，比一份普通 GPUI 构建多出什么」的**静态差集估算**（避免了「两个上界相减扣掉 ui 才打开的东西」，codex 043 第 3 轮 #1）；基线的 cfg 只是粗判，**不保证是严格上界**。重型 crate 的排除只看不减基线的 `out-ui-upper.txt`
- 不算 `[dev-dependencies]`：它们只在编这个 crate 自己的测试时用，别人依赖它时不编

```
cd /tmp && python3 -I <本目录>/closure.py "<MAIN>/resources/zed-20261114" ui --upper --minus gpui,gpui_platform
```

## 结果（Zed `15d136b`，2026-11-14）

基线取 `gpui` + `gpui_platform`：任何 GPUI 应用本来就要编它们（本项目 exp001–004 都依赖这两个）。

| 起点 | 模式 | 比基线多出的内部 crate | 行数 | 输出 |
|---|---|---|---|---|
| `ui` | 默认 | 7 个，全是 GPL：ui 31642、theme 5957、component 530、icons 360、syntax_theme 345、ui_macros 239、menu 37 | 39110 | `out-ui-minus-base.txt` |
| `ui` 上界 − 基线默认 | 上界 | 9 个：同样这 7 个 GPL，加 2 个 Apache 的 `bench_metrics` 2234 行、`zlog` 1710 行（gpui 的可选特性带进来的，普通构建不开） | 43054 | `out-ui-minus-base-upper.txt` |
| `ui`（不减基线） | **上界** | 21 个；**重型 crate 一个不中** | 约 15.2 万 | `out-ui-upper.txt` |
| `ui`（不减基线） | 默认 | 19 个 | 约 14.8 万 | `out-ui.txt` |
| `editor`（`ui_input` 要它注入） | 默认 | 97 个 | 约 74.7 万 | `out-editor.txt` |

settings、editor、workspace、project、language、client、theme_settings **在 `ui` 的完整上界闭包里也一个都没有**（`out-ui-upper.txt` 末行 `heavy-crate hits: []`，不靠减基线）。

## 复核记录（codex 043 第 1、2 轮 #3 / #1）

codex 指出 `closure.py` 不解析特性、跳过可选依赖和构建依赖、target 按子串匹配（会把 `not(...)` 也算进来，只会多算）。离线 `cargo tree --offline --locked -p ui` 跑不了（Zed 有 git 依赖未缓存），所以改为**直接核排除结论**：

- 闭包里 7 个新增 GPL crate（ui、theme、component、icons、menu、syntax_theme、ui_macros）的 `Cargo.toml` 全文：内部依赖里**没有** `optional = true` 的（只有 `syntax_theme` 的 serde / serde_json 两个外部可选依赖）；`[features]` 只开 gpui / syntax_theme 的 `test-support`；没有 `[build-dependencies]`
- 闭包全部 19 个 crate 的 `Cargo.toml` 里检索 `settings`、`editor`、`workspace`（作依赖名）、`theme_settings`、`project`、`language`、`client`：**只在 `ui` 的 `[dev-dependencies]` 出现**（`settings`、`theme_settings`，第 36–37 行），正常构建不拉
- gpui 的 `[build-dependencies]` 只有可选的外部 `embed-resource`（Windows 用）

第 2 轮 codex 仍指出「只在已发现的 crate 里搜终点名，排除不了经漏掉的中间包抵达」——所以脚本加了上界模式与基线差集。第 3 轮又指出「两个上界相减」会扣掉 ui 才打开的东西——所以基线改按默认模式，并单独存一份不减基线的上界闭包查重型 crate（见上「做法」「结果」）。

结论（静态差集，不是严格证明）：普通构建比 GPUI 基线多 7 个 GPL crate、约 3.9 万行；可选特性全开时的静态差集是 9 个、约 4.3 万行（基线的 cfg 是粗判，差集不保证是严格上界——codex 043 第 4 轮 #2）；`ui` 的完整上界闭包里没有 settings / editor / workspace 等重型 crate。本项目实际开哪些特性、编哪个平台，接进工程时用 `cargo tree` 再确认一次。剩下没做的：特性精确解析与外部 crate 的数量 / 体积（要联网 `cargo tree`，以后接进工程时顺手量）
