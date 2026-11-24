//! 界面投影（第二步）：界面输入（会话列表、打开的会话里的消息、用户状态）+ 界面状态 [`UiState`] → 无障碍树 [`A11yNode`]，
//! 再按 `specs/gui/a11y-tree.md` 第四节的 GPUI 层格式渲染成文本。
//!
//! 这一层是「视图模型 → 每个元素该设的角色、名称、标识、状态」（`docs/plan/012_gui-ssot.md` 第四节），CI 里快照测它；
//! GPUI 那边只把 [`A11yNode`] 抄进 `.role()` / `.aria_*()` / `.accessibility_id()`，不自己再算一遍。
//!
//! - 名称一律来自 `atg_core::speech`（带数据的）或文案表（不带数据的），这里不拼句子（`specs/architecture-rules.md` 第 3 条）
//! - 位置照设 `pos=`，名称里不放位置（`a11y-tree.md` 第一节第 4 条）
//! - 两步投影别混：第一步在 core，`atg_core::project`（`chat_list` / `window`）把 `Store` 投成内容树 `Doc`；
//!   第二步在这里，`Doc` + 界面状态投成无障碍树。这里不碰 `Store`，不缓存任何东西
//! - 现在的 [`UiState`] 只有 M3.1 要的三样；完整字段表见 012 第三节第 2 小节，随用到的块再加

use crate::a11y::{Element, NameFrom, Role, spec};
use crate::aid::{Aid, aid};
use atg_core::speech::{self, SpeechCtx};
use atg_core::tree::{ChatItem, Doc, MsgId, Status, UserStatus};

/// 打开着的那个会话
pub struct OpenChat<'a> {
    pub chat_id: i64,
    /// 从旧到新（`atg_core::project::window` 的输出）；不是消息的 `Doc` 不进列表
    pub messages: &'a [Doc],
}

/// 投影的输入：都是 core 投影好的东西
pub struct ViewInput<'a> {
    /// 会话列表，按 store 的顺序（`atg_core::project::chat_list`）；不是会话项的 `Doc` 不进列表
    pub chats: &'a [Doc],
    pub open: Option<OpenChat<'a>>,
    pub status: Option<UserStatus>,
    pub cx: &'a SpeechCtx,
}

/// 焦点在哪一区（取值正本 `specs/gui/keymap.md` 第二节 `view` 一行；现在只用到两个）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    ChatList,
    MessageList,
}

/// 纯界面状态（只放 id 和枚举，不放下标、不放读法字符串：012 第二节第 2 行）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    pub region: Region,
    /// 会话列表选中哪个
    pub chats_anchor: Option<i64>,
    /// 打开的会话里选中哪条
    pub msgs_anchor: Option<MsgId>,
}

/// 节点状态（导出格式里「只写有的」那几样，M3.1 用到的）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    Focused,
    Selected,
    /// active descendant 指向谁（标识）
    Active(String),
    /// 第几 / 共几（只进 GPUI 层，VoiceOver 拿不到：exp003 结论 2）
    Pos(usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yNode {
    pub role: Role,
    pub aid: Option<String>,
    pub name: Option<String>,
    pub role_description: Option<&'static str>,
    pub states: Vec<NodeState>,
    pub children: Vec<A11yNode>,
}

impl A11yNode {
    fn of(e: Element, a: Aid, name: Option<String>) -> Self {
        let s = spec(e);
        A11yNode {
            role: s.role,
            aid: Some(aid(a)),
            name,
            role_description: s.role_description.map(|t| t.zh()),
            states: Vec::new(),
            children: Vec::new(),
        }
    }
}

fn text_name(e: Element) -> Option<String> {
    match spec(e).name {
        NameFrom::Text(t) => Some(t.zh().to_owned()),
        // 提醒那段要 `speech` 给拼接口（M4.3）；接上之前只给文案，不在界面里拼句子（`specs/architecture-rules.md` 第 3 条）
        NameFrom::TextWithAlert(t) => Some(t.zh().to_owned()),
        // 根节点不带名称（窗口名在 NSWindow 标题上）；读法来源的由调用处现算
        NameFrom::WindowTitle(_) | NameFrom::ChatBrief | NameFrom::Brief => None,
    }
}

/// 一个 `Doc` 若是消息列表里的一项，给出它的身份
fn message_id(d: &Doc) -> Option<MsgId> {
    match d {
        Doc::Message(m) => Some(m.id),
        Doc::Service(s) => Some(s.id),
        Doc::Tombstone(t) => Some(t.id),
        Doc::ChatItem(_)
        | Doc::Alert(_)
        | Doc::Status(_)
        | Doc::Playback(_)
        | Doc::Reactor(_)
        | Doc::Transcription(_)
        | Doc::Unknown(_) => None,
    }
}

/// 一个 `Doc` 若是会话列表里的一项，取出它
fn chat_item(d: &Doc) -> Option<&ChatItem> {
    match d {
        Doc::ChatItem(c) => Some(c),
        Doc::Message(_)
        | Doc::Service(_)
        | Doc::Tombstone(_)
        | Doc::Alert(_)
        | Doc::Status(_)
        | Doc::Playback(_)
        | Doc::Reactor(_)
        | Doc::Transcription(_)
        | Doc::Unknown(_) => None,
    }
}

/// 一个列表：容器 + 项；选中的那项带 `selected`，容器带 `active=`；区里有焦点时容器带 `focused`
fn list(
    mut container: A11yNode,
    items: Vec<(String, A11yNode)>,
    anchor: Option<&str>,
    focused: bool,
) -> A11yNode {
    let n = items.len();
    if focused {
        container.states.push(NodeState::Focused);
    }
    if let Some(a) = anchor
        && items.iter().any(|(id, _)| id == a)
    {
        container.states.push(NodeState::Active(a.to_owned()));
    }
    for (i, (id, mut item)) in items.into_iter().enumerate() {
        if anchor == Some(id.as_str()) {
            item.states.push(NodeState::Selected);
        }
        item.states.push(NodeState::Pos(i + 1, n));
        container.children.push(item);
    }
    container
}

/// 投影出整棵树
pub fn project(input: &ViewInput, ui: &UiState) -> A11yNode {
    let mut root = A11yNode::of(Element::Window, Aid::Window, None);

    // 会话区
    let chats: Vec<(String, A11yNode)> = input
        .chats
        .iter()
        .filter_map(chat_item)
        .map(|c| {
            let node = A11yNode::of(
                Element::ChatItem,
                Aid::Chat(c.chat_id),
                Some(speech::chat_brief(c)),
            );
            (aid(Aid::Chat(c.chat_id)), node)
        })
        .collect();
    let chat_anchor = ui.chats_anchor.map(|c| aid(Aid::Chat(c)));
    let chat_list = list(
        A11yNode::of(Element::ChatList, Aid::Chats, text_name(Element::ChatList)),
        chats,
        chat_anchor.as_deref(),
        ui.region == Region::ChatList,
    );
    let mut region_chats = A11yNode::of(
        Element::RegionChats,
        Aid::RegionChats,
        text_name(Element::RegionChats),
    );
    region_chats.children.push(chat_list);
    root.children.push(region_chats);

    // 消息区
    let mut region_msgs = A11yNode::of(
        Element::RegionMessages,
        Aid::RegionMessages,
        text_name(Element::RegionMessages),
    );
    if let Some(open) = &input.open {
        let items: Vec<(String, A11yNode)> = open
            .messages
            .iter()
            .filter_map(|d| message_id(d).map(|id| (id, d)))
            .map(|(id, d)| {
                let node = A11yNode::of(
                    Element::MessageItem,
                    Aid::Msg(id),
                    Some(speech::brief(d, input.cx)),
                );
                (aid(Aid::Msg(id)), node)
            })
            .collect();
        let anchor = ui
            .msgs_anchor
            .filter(|m| m.chat_id == open.chat_id)
            .map(|m| aid(Aid::Msg(m)));
        region_msgs.children.push(list(
            A11yNode::of(
                Element::MessageList,
                Aid::Messages(open.chat_id),
                text_name(Element::MessageList),
            ),
            items,
            anchor.as_deref(),
            ui.region == Region::MessageList,
        ));
    }
    root.children.push(region_msgs);

    // 状态栏
    if let Some(status) = input.status {
        let name = speech::brief(&Doc::Status(Status { status }), input.cx);
        root.children
            .push(A11yNode::of(Element::StatusBar, Aid::Status, Some(name)));
    }
    root
}

/// 快照文件第一行（两层共同；`a11y-tree.md` 第四节）。`gpui` 填 GPUI commit，纯函数层填 `projection`
pub fn header(gpui: &str, fixture: &str) -> String {
    format!(
        "# gui-schema v{} · gpui {gpui} · fixture {fixture}",
        crate::schema::GUI_SCHEMA_V
    )
}

/// 按 GPUI 层格式渲染：一行一个节点，缩进两格表示层级，顺序即子节点顺序
pub fn render(node: &A11yNode) -> String {
    let mut out = String::new();
    render_into(node, 0, &mut out);
    out
}

fn render_into(n: &A11yNode, depth: usize, out: &mut String) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(n.role.name());
    if let Some(a) = &n.aid {
        out.push(' ');
        out.push_str(a);
    }
    if let Some(name) = &n.name {
        out.push_str(&format!(" \"{}\"", escape(name)));
    }
    if let Some(r) = n.role_description {
        out.push_str(&format!(" roledesc=\"{}\"", escape(r)));
    }
    for s in &n.states {
        out.push(' ');
        match s {
            NodeState::Focused => out.push_str("focused"),
            NodeState::Selected => out.push_str("selected"),
            NodeState::Active(a) => out.push_str(&format!("active={a}")),
            NodeState::Pos(i, k) => out.push_str(&format!("pos={i}/{k}")),
        }
    }
    out.push('\n');
    for c in &n.children {
        render_into(c, depth + 1, out);
    }
}

/// 名称里的引号、反斜杠、换行转义，保证一个节点只占一行
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escape_keeps_one_line() {
        assert_eq!(escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
        assert!(!escape("一\n二\r三").contains('\n'));
    }
}
