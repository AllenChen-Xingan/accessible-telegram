# 现状页 · patch（批次 b3）

> 交代：`_tmp/20261025-b3/patch.md` · 共同规矩：`_tmp/20261025-b3/common.md` · issue #3

## 手上
tdlib-rs 补丁：用上 TDLib 1.8.69 + 生成器递归修补 + 解析失败日志不带内容。补丁仓 `../tdlib-rs`（分支 `atg/tdlib-1.8.69`，**未提交**，git 写操作归大总管）。

## 做到哪
- [x] 读 build.rs / gen / parser
- [x] 编 TDLib 1.8.69 → `_build/20261025-161449/tdlib/`（203 秒；第一次因路径空格失败，见 004 第三节）
- [x] `tl/api.tl` 同步、旧的存 `api_1.8.61.tl`、版本号全改
- [x] 生成器修补 `tdlib-rs-gen/src/metadata.rs`（裸类型环 → E0072）+ 3 单测
- [x] 隐私修补 `tdlib-rs/src/lib.rs` + 3 单测
- [x] 例子 `examples/version_check.rs`：1.8.69 · waitTdlibParameters · 正常退出 ×3
- [x] fmt / clippy / 42 单测过（追加后 45）；连测试服务器到 waitPhoneNumber（凭据出现 0 次）；名额已还
- [x] `ATG.md`（补丁仓）、`docs/plan/004_tdlib-patch.md`、`docs/plan/README.md` 一行
- [x] 三视角对抗自审（代码 · 隐私攻击 · 事实核对）：无 P0/P1；已修：receive 不再崩、测试防并发、例子出错也正常关闭 + 总时限、文档 7 处

## 拍板后追加（005 补丁 ①②，已完成）
- [x] ① `tdlib_rs::execute` 绑定 `td_execute` + 单测；例子建客户端前降日志级别，stderr 24 → 0 行
- [x] ② 生成函数改走 `crate::from_response`，崩溃文本只留类型名 + 单测 + 生成器单测
- [x] fmt / clippy / 45 单测过；ATG.md、004 已更新；名额已还
- ③ 禁 `{:?}` 归 atg-core，004 第五节已写

## 下一步（给接手的人）
- 大总管提交补丁仓改动、推 fork；之后 004 里依赖写法改 `rev = "<提交>"`
- 复跑测试服务器：见 004 第五节第一条

## 流水
- 2026-10-25 16:14 报到；领名额；clone 源码；后台 cmake
- 16:17 第一次编译失败（路径空格拆了 `-exported_symbols_list` 参数）；改用草稿区软链接重编，16:21 成功，名额还
- 改 tl、版本号、lib.rs、例子；领 cargo 名额
- cargo：E0072 → 修生成器；日志位置恒 0:0 → 改为只记类别（实测 serde_path_to_error 也无路径）；例子退出 segfault 139 → 加关闭与 join
- 全过，名额还；写 ATG.md / 004 / README 行；起三视角自审
- 自审修复后复跑全过；.env.local 就绪后连测试服务器成功（waitPhoneNumber，未登录）；文档更新；名额还
- 人拍板 005：做补丁 ①②；45 单测过；文档更新；名额还
