---
name: path-trailing-space
description: 项目根路径末尾带一个空格（"telegram accessibility "），会让不加引号的构建脚本在空格处拆参数；只管构建 / 脚本路径问题，不管别的
type: feedback
---

**为什么**：2026-10-25 编 TDLib 1.8.69 第一次失败——它的链接参数 `-exported_symbols_list <路径>` 没加引号，路径在末尾空格处被拆开（patch 手下，`state/handoff/patch.md`）。shell 的 cwd 也常被重置回这个带空格的路径。

**怎么用**：
- 自己写的命令与脚本，路径一律加引号
- 第三方构建（cmake、C/C++ 链接、build.rs 里拼命令行的）遇到奇怪的「找不到文件 / 参数错」先怀疑这个；绕法：在草稿区或 `_build/` 外建一个**不带空格的软链接**指向项目根，从软链接路径编译
- 不改项目目录名（人的目录，改了会断别处的引用；要改先问人）
