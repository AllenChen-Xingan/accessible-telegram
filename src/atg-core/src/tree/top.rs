//! 消息以外的顶层节点（`docs/plan/010_complexity-budget.md`：`tree/mod.rs` 拆成消息节点与其余顶层节点两个文件；
//! `ChatItem`、`Alert` 以后照 010 搬进来）。现在有：用户状态（`Doc::Status`）。

use serde::Serialize;

/// 用户状态：内核、授权、连接三条线合成的一个值（合成规则归 `specs/contracts/errors.md`
/// 第一节第 3 小节）。读法只读这个，不直接读 TDLib 的连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    pub status: UserStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Fault,
    /// 内核或授权在关闭中 / 已关闭：人自己关的，不播报（infra 新增，`errors.md` 第一节第 3 小节）
    Stopping,
    SignedOut,
    SigningIn,
    UnsupportedStep,
    Offline,
    Connecting,
    Syncing,
    Online,
}
