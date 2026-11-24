//! 无障碍树契约表：每类元素进树时的角色、名称从哪来、标识种类、角色描述、能不能操作。
//! 规约（v0 正本移交到这里的那几行）：`specs/gui/a11y-tree.md` 第三节「元素表」。
//!
//! 只收 M3.1 用到的元素；其余行（输入框、按钮、搜索、对话框……）仍以规约为正本，随用到它们的块进表。
//! 角色用 AccessKit 的名字（`accesskit::Role` 的变体名），本 crate 不依赖 accesskit，界面那边一一对上
//! （`match` 不写 `_ =>`）。macOS 层各角色映射成什么，等 M3.1 的真窗口导出实测后再进表。

use crate::aid::IdKind;
use crate::text::TextId;

/// 角色（AccessKit 名字）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// 根节点；macOS 上是不带标题的 AXGroup，窗口名走 NSWindow 标题（`a11y-tree.md` 元素表第一行）
    Window,
    /// 地标：导航（会话区）
    Navigation,
    /// 地标：主要（消息区）
    Main,
    /// 列表（AXList）
    List,
    /// 列表项（macOS 上是 AXGroup，exp003 实测）
    ListItem,
    /// 状态（常驻用户状态）
    Status,
}

impl Role {
    pub const ALL: [Role; 6] = [
        Role::Window,
        Role::Navigation,
        Role::Main,
        Role::List,
        Role::ListItem,
        Role::Status,
    ];

    /// 导出、快照、自描述里的写法（同 AccessKit 变体名）
    pub fn name(self) -> &'static str {
        match self {
            Role::Window => "Window",
            Role::Navigation => "Navigation",
            Role::Main => "Main",
            Role::List => "List",
            Role::ListItem => "ListItem",
            Role::Status => "Status",
        }
    }
}

/// 名称从哪来（`a11y-tree.md` 第一节第 2 条：只从读法或文案表来）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameFrom {
    /// 没有名称（根节点：窗口名在 NSWindow 标题上，这里给的是标题的文案）
    WindowTitle(TextId),
    /// 文案表里的一条
    Text(TextId),
    /// `atg_core::speech::chat_brief`
    ChatBrief,
    /// `atg_core::speech::brief`（消息、系统消息、撤回占位、状态都是它）
    Brief,
}

impl NameFrom {
    pub fn key(self) -> String {
        match self {
            NameFrom::WindowTitle(t) => format!("window_title:{}", t.key()),
            NameFrom::Text(t) => format!("text:{}", t.key()),
            NameFrom::ChatBrief => "speech:chat_brief".into(),
            NameFrom::Brief => "speech:brief".into(),
        }
    }
}

/// 契约表里的一类元素
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Window,
    RegionChats,
    RegionMessages,
    ChatList,
    ChatItem,
    MessageList,
    /// 消息项、系统消息项、撤回占位共用这一行（同一个标识种类，撤回后标识不变、名称变）
    MessageItem,
    StatusBar,
}

/// 契约表的一行
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementSpec {
    pub element: Element,
    pub key: &'static str,
    pub role: Role,
    pub id: IdKind,
    pub name: NameFrom,
    /// 角色描述（macOS AXRoleDescription）；只在应用自己设时有
    pub role_description: Option<TextId>,
    /// 能操作（能获焦或能被选中）：`check-a11y-tree.py` 对这类元素查角色、名称、标识三样（第一节第 1 条）
    pub operable: bool,
}

/// 契约表（顺序 = 树里从上到下的顺序）
pub const TABLE: [ElementSpec; 8] = [
    ElementSpec {
        element: Element::Window,
        key: "window",
        role: Role::Window,
        id: IdKind::Window,
        name: NameFrom::WindowTitle(TextId::AppTitle),
        role_description: None,
        operable: false,
    },
    ElementSpec {
        element: Element::RegionChats,
        key: "region_chats",
        role: Role::Navigation,
        id: IdKind::RegionChats,
        name: NameFrom::Text(TextId::Chats),
        role_description: None,
        operable: false,
    },
    ElementSpec {
        element: Element::ChatList,
        key: "chat_list",
        role: Role::List,
        id: IdKind::Chats,
        name: NameFrom::Text(TextId::Chats),
        role_description: None,
        operable: true,
    },
    ElementSpec {
        element: Element::ChatItem,
        key: "chat_item",
        role: Role::ListItem,
        id: IdKind::Chat,
        name: NameFrom::ChatBrief,
        role_description: None,
        operable: true,
    },
    ElementSpec {
        element: Element::RegionMessages,
        key: "region_messages",
        role: Role::Main,
        id: IdKind::RegionMessages,
        name: NameFrom::Text(TextId::Messages),
        role_description: None,
        operable: false,
    },
    ElementSpec {
        element: Element::MessageList,
        key: "message_list",
        role: Role::List,
        id: IdKind::Messages,
        name: NameFrom::Text(TextId::Messages),
        role_description: None,
        operable: true,
    },
    ElementSpec {
        element: Element::MessageItem,
        key: "message_item",
        role: Role::ListItem,
        id: IdKind::Msg,
        name: NameFrom::Brief,
        role_description: Some(TextId::MessageRole),
        operable: true,
    },
    ElementSpec {
        element: Element::StatusBar,
        key: "status_bar",
        role: Role::Status,
        id: IdKind::Status,
        name: NameFrom::Brief,
        role_description: None,
        operable: false,
    },
];

/// 查一类元素的契约。下标与 [`TABLE`] 的顺序对应，单测 `spec_points_at_its_own_row` 逐个核
pub fn spec(e: Element) -> &'static ElementSpec {
    let i = match e {
        Element::Window => 0,
        Element::RegionChats => 1,
        Element::ChatList => 2,
        Element::ChatItem => 3,
        Element::RegionMessages => 4,
        Element::MessageList => 5,
        Element::MessageItem => 6,
        Element::StatusBar => 7,
    };
    &TABLE[i]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aid::IdKind;

    const ELEMENTS: [Element; 8] = [
        Element::Window,
        Element::RegionChats,
        Element::RegionMessages,
        Element::ChatList,
        Element::ChatItem,
        Element::MessageList,
        Element::MessageItem,
        Element::StatusBar,
    ];

    // 新加 Element 变体时这里编不过，逼着把它加进 ELEMENTS
    fn _exhaustive(e: Element) {
        match e {
            Element::Window
            | Element::RegionChats
            | Element::RegionMessages
            | Element::ChatList
            | Element::ChatItem
            | Element::MessageList
            | Element::MessageItem
            | Element::StatusBar => {}
        }
    }

    #[test]
    fn every_element_exactly_once() {
        for e in ELEMENTS {
            assert_eq!(TABLE.iter().filter(|s| s.element == e).count(), 1, "{e:?}");
        }
        assert_eq!(TABLE.len(), ELEMENTS.len());
    }

    #[test]
    fn spec_points_at_its_own_row() {
        for e in ELEMENTS {
            assert_eq!(spec(e).element, e);
        }
    }

    #[test]
    fn keys_and_ids_unique() {
        for (i, a) in TABLE.iter().enumerate() {
            for b in &TABLE[i + 1..] {
                assert_ne!(a.key, b.key);
                assert_ne!(a.id, b.id, "{} 与 {} 用了同一个标识种类", a.key, b.key);
            }
        }
        // 每个标识种类都有元素用
        for k in IdKind::ALL {
            assert!(TABLE.iter().any(|s| s.id == k), "{k:?} 没有元素用");
        }
    }

    #[test]
    fn operable_elements_have_a_name_source() {
        for s in TABLE.iter().filter(|s| s.operable) {
            assert!(!matches!(s.name, NameFrom::WindowTitle(_)), "{}", s.key);
        }
    }
}
