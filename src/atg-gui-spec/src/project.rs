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
use std::collections::HashMap;

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

/// 纯界面状态（只放 id 和枚举，不放下标、不放读法字符串：012 第二节第 2 行）。不放下标是因为新消息插进来、往上加载历史、
/// 有消息被撤回时，下标会漂、id 不会
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiState {
    pub region: Region,
    /// 会话列表选中哪个
    pub chats_anchor: Option<i64>,
    /// 每个会话里选中哪条：会话 id → 那个会话里的消息 id（会话 id 只存在键里，不再在值里存第二份）。
    /// 离开再回来保持（012 第三节第 2 小节、exp003b-v2）。锚点那条不在这次传进来的 `messages`（core 的窗口只是一段）
    /// 里时，什么都不选、不写 `active=`；要不要回落到最近的一条，由 M3.4 的焦点规则定（`specs/gui/focus.md`）
    pub msgs_anchor: HashMap<i64, i64>,
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
    /// 是契约表里的哪类元素（角色、能不能操作都按它那一行）
    pub element: Element,
    pub role: Role,
    pub aid: Option<String>,
    pub name: Option<String>,
    pub role_description: Option<&'static str>,
    pub states: Vec<NodeState>,
    pub children: Vec<A11yNode>,
}

/// 一个节点的名称要用的数据。调用方只交数据，**名称用哪条读法、哪条文案由契约表的 `name` 一列定**（[`name_of`]），
/// 不由调用方挑函数——这样表就是名称来源的唯一正本
enum Data<'a> {
    /// 不带数据（文案来源、根节点）
    None,
    Chat(&'a ChatItem),
    Doc(&'a Doc),
}

/// 照契约表的 `name` 一列算名称。来源是读法（`chat_brief` / `brief`）而数据给错了（比如给的是一条消息）就没有名称；
/// 来源是文案时不看数据，多给了也照出文案。没名称的兜底：可操作元素由本 crate 的投影测试 `tests/project.rs` 的
/// `check_rules` 抓（它只对可操作角色查名称，只跑那几个夹具）；不可操作的（如状态栏）只能靠快照里它那一行变了才看得出。
/// 规约里的 `check-a11y-tree.py` 还没写（M3.1 的树导出那一块，#169），现在不兜这一层
fn name_of(e: Element, data: Data, cx: &SpeechCtx) -> Option<String> {
    match (spec(e).name, data) {
        // 根节点不带名称（窗口名在 NSWindow 标题上）
        (NameFrom::WindowTitle(_), _) => None,
        (NameFrom::Text(t), _) => Some(t.zh().to_owned()),
        // 提醒那段要 `speech` 给拼接口（#172）；接上之前只给文案，不在界面里拼句子（`specs/architecture-rules.md` 第 3 条）
        (NameFrom::TextWithAlert(t), _) => Some(t.zh().to_owned()),
        (NameFrom::ChatBrief, Data::Chat(c)) => Some(speech::chat_brief(c)),
        (NameFrom::Brief, Data::Doc(d)) => Some(speech::brief(d, cx)),
        (NameFrom::ChatBrief, Data::None | Data::Doc(_))
        | (NameFrom::Brief, Data::None | Data::Chat(_)) => None,
    }
}

impl A11yNode {
    /// 角色、角色描述、名称来源都取契约表那一行；标识种类和那一行对不上就不出标识。契约表每一行都带标识种类（没有
    /// 「纯装饰容器」这一类），所以本 crate 投出的节点个个该有标识——这比规约第四节（装饰容器可以不带）严；
    /// `tests/project.rs` 的 `check_rules` 按这条要求每个节点都有标识，种类不符就红
    fn of(e: Element, a: Aid, data: Data, cx: &SpeechCtx) -> Self {
        let s = spec(e);
        A11yNode {
            element: e,
            role: s.role,
            aid: (a.kind() == s.id).then(|| aid(a)),
            name: name_of(e, data, cx),
            role_description: s.role_description.map(|t| t.zh()),
            states: Vec::new(),
            children: Vec::new(),
        }
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

/// 一个列表：容器 + 项；选中的那项带 `selected`，容器带 `active=`；区里有焦点时容器带 `focused`。
/// 比对锚点、写 `active=` 都用项自己的 `aid`（标识只有这一处来源）：项没出标识时它不会被选中，容器也不会指向它。
/// 能不能获焦、能不能被选中，看契约表的 `operable` 一列：容器不可操作就不带 `focused` / `active=`，项不可操作就不带
/// `selected`、也不会被容器指向（表是正本，这里不另做判断）
fn list(
    mut container: A11yNode,
    items: Vec<A11yNode>,
    anchor: Option<Aid>,
    focused: bool,
) -> A11yNode {
    let n = items.len();
    let anchor = anchor.map(aid);
    let container_operable = spec(container.element).operable;
    if focused && container_operable {
        container.states.push(NodeState::Focused);
    }
    let target = |it: &A11yNode| anchor.is_some() && it.aid == anchor && spec(it.element).operable;
    if container_operable
        && let Some(a) = &anchor
        && items.iter().any(target)
    {
        container.states.push(NodeState::Active(a.clone()));
    }
    for (i, mut item) in items.into_iter().enumerate() {
        if target(&item) {
            item.states.push(NodeState::Selected);
        }
        item.states.push(NodeState::Pos(i + 1, n));
        container.children.push(item);
    }
    container
}

/// 投影出整棵树
pub fn project(input: &ViewInput, ui: &UiState) -> A11yNode {
    let mut root = A11yNode::of(Element::Window, Aid::Window, Data::None, input.cx);

    // 会话区
    let chats: Vec<A11yNode> = input
        .chats
        .iter()
        .filter_map(chat_item)
        .map(|c| {
            A11yNode::of(
                Element::ChatItem,
                Aid::Chat(c.chat_id),
                Data::Chat(c),
                input.cx,
            )
        })
        .collect();
    let chat_list = list(
        A11yNode::of(Element::ChatList, Aid::Chats, Data::None, input.cx),
        chats,
        ui.chats_anchor.map(Aid::Chat),
        ui.region == Region::ChatList,
    );
    let mut region_chats =
        A11yNode::of(Element::RegionChats, Aid::RegionChats, Data::None, input.cx);
    region_chats.children.push(chat_list);
    root.children.push(region_chats);

    // 消息区
    let mut region_msgs = A11yNode::of(
        Element::RegionMessages,
        Aid::RegionMessages,
        Data::None,
        input.cx,
    );
    if let Some(open) = &input.open {
        let items: Vec<A11yNode> = open
            .messages
            .iter()
            .filter_map(|d| message_id(d).map(|id| (id, d)))
            .map(|(id, d)| A11yNode::of(Element::MessageItem, Aid::Msg(id), Data::Doc(d), input.cx))
            .collect();
        // 只取这个会话自己的锚点；别的会话的锚点留在表里，切回去时还在
        let anchor = ui.msgs_anchor.get(&open.chat_id).map(|&message_id| {
            Aid::Msg(MsgId {
                chat_id: open.chat_id,
                message_id,
            })
        });
        region_msgs.children.push(list(
            A11yNode::of(
                Element::MessageList,
                Aid::Messages(open.chat_id),
                Data::None,
                input.cx,
            ),
            items,
            anchor,
            ui.region == Region::MessageList,
        ));
    }
    root.children.push(region_msgs);

    // 状态栏
    if let Some(status) = input.status {
        let doc = Doc::Status(Status { status });
        root.children.push(A11yNode::of(
            Element::StatusBar,
            Aid::Status,
            Data::Doc(&doc),
            input.cx,
        ));
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
    use super::*;
    use atg_core::tree::{Doc, Status, UserStatus};

    fn cx() -> SpeechCtx {
        SpeechCtx {
            utc_offset: 0,
            offset_at: None,
            listen_key: None,
            expand_key: None,
            reveal: false,
        }
    }

    #[test]
    fn wrong_id_kind_gives_no_id() {
        // 标识种类和契约表那一行不符（会话项给了消息标识）→ 不出标识
        let m = MsgId {
            chat_id: 1,
            message_id: 2,
        };
        assert_eq!(
            A11yNode::of(Element::ChatItem, Aid::Msg(m), Data::None, &cx()).aid,
            None
        );
        assert_eq!(
            A11yNode::of(Element::StatusBar, Aid::Chats, Data::None, &cx()).aid,
            None
        );
        assert!(
            A11yNode::of(Element::ChatItem, Aid::Chat(1), Data::None, &cx())
                .aid
                .is_some()
        );
    }

    #[test]
    fn wrong_data_for_speech_source_gives_no_name() {
        // 表说读 chat_brief / brief，给的数据对不上 → 没有名称；文案来源不看数据
        let doc = Doc::Status(Status {
            status: UserStatus::Online,
        });
        assert_eq!(
            A11yNode::of(Element::ChatItem, Aid::Chat(1), Data::Doc(&doc), &cx()).name,
            None
        );
        assert_eq!(
            A11yNode::of(Element::StatusBar, Aid::Status, Data::None, &cx()).name,
            None
        );
        assert!(
            A11yNode::of(Element::StatusBar, Aid::Status, Data::Doc(&doc), &cx())
                .name
                .is_some()
        );
        assert_eq!(
            A11yNode::of(Element::ChatList, Aid::Chats, Data::Doc(&doc), &cx())
                .name
                .as_deref(),
            Some(crate::text::TextId::Chats.zh())
        );
    }

    fn items(ids: &[i64]) -> Vec<A11yNode> {
        ids.iter()
            .map(|&i| A11yNode::of(Element::ChatItem, Aid::Chat(i), Data::None, &cx()))
            .collect()
    }

    #[test]
    fn anchor_not_in_list_selects_nothing() {
        // 锚点那一项不在这次的列表里（滚出窗口、被删）→ 不设 active=、不选中，不悬空
        let c = A11yNode::of(Element::ChatList, Aid::Chats, Data::None, &cx());
        let l = list(c, items(&[1, 2]), Some(Aid::Chat(9)), true);
        assert!(l.states.contains(&NodeState::Focused));
        assert!(!l.states.iter().any(|s| matches!(s, NodeState::Active(_))));
        assert!(
            l.children
                .iter()
                .all(|it| !it.states.contains(&NodeState::Selected))
        );
    }

    #[test]
    fn item_without_id_is_never_selected() {
        // 项没出标识（种类不符）→ 不被选中，容器也不指向它
        let c = A11yNode::of(Element::ChatList, Aid::Chats, Data::None, &cx());
        let mut its = items(&[1]);
        its[0].aid = None;
        let l = list(c, its, Some(Aid::Chat(1)), true);
        assert!(!l.states.iter().any(|s| matches!(s, NodeState::Active(_))));
        assert!(!l.children[0].states.contains(&NodeState::Selected));
    }

    #[test]
    fn non_operable_container_gets_no_focus_or_active() {
        // 契约表说不可操作的元素（这里拿状态栏当容器）不带 focused / active=
        let c = A11yNode::of(Element::StatusBar, Aid::Status, Data::None, &cx());
        let l = list(c, items(&[1]), Some(Aid::Chat(1)), true);
        assert!(l.states.is_empty(), "{:?}", l.states);
    }

    #[test]
    fn escape_keeps_one_line() {
        assert_eq!(escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
        assert!(!escape("一\n二\r三").contains('\n'));
    }
}
