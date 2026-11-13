//! 用户状态（T10，`Doc::Status`）的读法：断网、连接中、同步中、已连接……（`errors.md` 第一节第 3 小节；说法待人实听）

use crate::tree::UserStatus;

pub(super) fn brief(s: UserStatus) -> &'static str {
    match s {
        UserStatus::Fault => "连接已中断，需要重启",
        // `specs/contracts/errors.md` 第一节第 3 小节：这一态不主动播报；但焦点落在状态项上时仍要有一句名称。说法待人实听定标
        UserStatus::Stopping => "正在退出",
        UserStatus::SignedOut => "未登录",
        UserStatus::SigningIn => "正在登录",
        UserStatus::UnsupportedStep => "这一步本应用暂不支持，请先在官方客户端完成",
        UserStatus::Offline => "网络已断开，正在等待网络",
        UserStatus::Connecting => "正在连接",
        UserStatus::Syncing => "正在同步",
        UserStatus::Online => "已连接",
    }
}
