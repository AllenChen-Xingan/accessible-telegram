//! 固定文案表：`TextId` → 中文。只放**不带任何数据**的控件名和固定提示；带数据的句子（「3 条新消息」「已连接」）
//! 是读法，走树 + `atg_core::speech`（`docs/plan/012_gui-ssot.md` 第三节第 4 小节）。
//!
//! 界面代码里不写中文字面量，一律从这里取（`check-gui-strings.py` 将来扫）。不上本地化框架（卡 G7）。
//! 名称以画面上看得见的文字开头、不含角色词：写「发送」不写「发送按钮」（`specs/gui/a11y-tree.md` 第一节第 8 条）。

/// 文案编号。JSON、文档里用 [`TextId::key`]（snake_case）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextId {
    /// 窗口标题的固定部分（macOS 层是 NSWindow 标题，不在 GPUI 根节点上）
    AppTitle,
    /// 会话区地标、会话列表的名称
    Chats,
    /// 消息区地标、消息列表的名称
    Messages,
    /// 消息项的角色描述（VoiceOver 读「消息」不读「组」；卡 G15 人选 B，`docs/plan/013_decisions-b19-gui.md` B20-3）
    MessageRole,
}

impl TextId {
    pub const ALL: [TextId; 4] = [
        TextId::AppTitle,
        TextId::Chats,
        TextId::Messages,
        TextId::MessageRole,
    ];

    pub fn zh(self) -> &'static str {
        match self {
            TextId::AppTitle => "无障碍 Telegram",
            TextId::Chats => "会话",
            TextId::Messages => "消息",
            TextId::MessageRole => "消息",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            TextId::AppTitle => "app_title",
            TextId::Chats => "chats",
            TextId::Messages => "messages",
            TextId::MessageRole => "message_role",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique_and_texts_non_empty() {
        for (i, a) in TextId::ALL.iter().enumerate() {
            assert!(!a.zh().is_empty());
            assert!(a.key().bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
            for b in &TextId::ALL[i + 1..] {
                assert_ne!(a.key(), b.key());
            }
        }
    }

    #[test]
    fn no_role_words_in_names() {
        // 第一节第 8 条：名称不含角色词
        for t in TextId::ALL {
            for w in ["按钮", "列表", "输入框", "搜索框"] {
                assert!(!t.zh().contains(w), "{t:?}");
            }
        }
    }
}
