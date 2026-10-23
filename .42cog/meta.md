# 无障碍 Telegram（macOS · VoiceOver 优先） — 项目身份

<meta>
  <document-id>atg-meta</document-id>
  <version>1.0.0</version>
  <project>无障碍 Telegram（macOS · VoiceOver 优先）</project>
  <type>Project Metadata</type>
  <created>2026-10-23</created>
</meta>

## Document Purpose

这个系统的**项目身份**：它是什么、归谁、在哪、跟谁交接。

---

<project-info>

## 身份

- **系统名**：无障碍 Telegram（macOS · VoiceOver 优先） ｜ **系统码**：`atg` ｜ **开仓**：2026-10-23
- **一句话定位**：一个只靠键盘和 VoiceOver 就能顺畅聊天的 macOS Telegram 客户端，用 Rust + GPUI 写
- **负责人**：Allen Chen
- **状态**：在建

## 位置

- **作品区**：`src/` —— 装 Rust workspace 的各个 crate
- **远程仓库**：https://github.com/AllenChen-Xingan/accessible-telegram（公开）
- **与默认六组的差异**：根目录多一份 `Cargo.toml`（workspace）与 `rust-toolchain.toml`，`target/` 不入库

## 交接

```
Telegram 服务器 / GPUI 上游 ──给我──▶  无障碍 Telegram  ──我给出──▶ 我自己（日常使用）/ 可能的 GPUI 无障碍 PR
```

| | 是谁 | 交接什么 | 怎么交 |
|---|---|---|---|
| 上游 | Telegram（MTProto API） | 消息与账号数据 | grammers 库 |
| 上游 | Zed / GPUI | 界面框架与 AccessKit 接入 | Cargo git 依赖 |
| 下游 | 我自己 | 能用的 App | 本地构建 |
| 下游 | Zed 仓库（可选） | 发现的 GPUI 无障碍缺口 | 上游 PR / issue |

## 依赖

| 依赖 | 版本 | 缺了会怎样 |
|---|---|---|
| Rust 工具链（rustup / cargo） | 见 `rust-toolchain.toml` | 什么都编不了（**当前未安装**） |
| Xcode 命令行工具 | 随系统 | GPUI 的 macOS 后端编不过（需要 Metal 相关工具） |
| GPUI | zed 主干 git 依赖（锁 commit） | 没有界面与无障碍 |
| grammers | 待定版本 | 连不上 Telegram |
| Telegram `api_id` / `api_hash` | my.telegram.org 申请 | 无法登录 |
| macOS VoiceOver | 系统自带 | 无法做实效验证 |

</project-info>

---

元数据变了就地改，不留历史——**历史在提交链里**。
