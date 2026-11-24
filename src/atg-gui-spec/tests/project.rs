//! 纯函数层的无障碍树快照（`docs/plan/012_gui-ssot.md` 第四节）：夹具 → core 投影 → 界面投影 → GPUI 层格式文本。
//! 夹具借 `atg-core` 的读法夹具（`src/atg-core/fixtures/speech/`，只有一处：`testing.md` 第一节第 3 小节）；
//! 场景目录（`fixtures/scenarios/`）建好后改用场景。
//! 改了表或读法：`INSTA_UPDATE=always cargo test -p atg-gui-spec`，**逐行看过差异**再入库（改表的全部步骤见 `atg_gui_spec::schema` 文件头）。

use atg_core::speech::SpeechCtx;
use atg_core::tdjson::World;
use atg_core::tree::{Doc, MsgId};
use atg_gui_spec::a11y::{Role, TABLE};
use atg_gui_spec::aid;
use atg_gui_spec::project::{
    A11yNode, NodeState, OpenChat, Region, UiState, ViewInput, header, project, render,
};
use std::collections::HashMap;

fn cx() -> SpeechCtx {
    // M3.1 还没绑展开、收听键：键从按键表来（012 第二节第 7 行），表没进之前不读按键提示
    SpeechCtx {
        utc_offset: 8 * 3600,
        offset_at: None,
        listen_key: None,
        expand_key: None,
        reveal: false,
    }
}

fn world(name: &str) -> World {
    let path = format!(
        "{}/../atg-core/fixtures/speech/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    World::from_json(&std::fs::read_to_string(&path).expect("读夹具")).expect("解析夹具")
}

fn messages_of(w: &World, chat_id: i64) -> Vec<Doc> {
    w.docs()
        .into_iter()
        .filter(|d| match d {
            Doc::Message(m) => m.id.chat_id == chat_id,
            Doc::Service(s) => s.id.chat_id == chat_id,
            Doc::Tombstone(t) => t.id.chat_id == chat_id,
            Doc::ChatItem(_)
            | Doc::Alert(_)
            | Doc::Status(_)
            | Doc::Playback(_)
            | Doc::Reactor(_)
            | Doc::Transcription(_)
            | Doc::Unknown(_) => false,
        })
        .collect()
}

fn first_msg(docs: &[Doc]) -> MsgId {
    // messages_of 只留消息、系统消息、撤回占位三种
    match &docs[0] {
        Doc::Message(m) => m.id,
        Doc::Service(s) => s.id,
        Doc::Tombstone(t) => t.id,
        _ => unreachable!("messages_of 只留列表项"),
    }
}

fn dump(w: &World, open: Option<(i64, &[Doc])>, ui: &UiState) -> A11yNode {
    let chats = w.chat_items();
    let cx = cx();
    let input = ViewInput {
        chats: &chats,
        open: open.map(|(chat_id, messages)| OpenChat { chat_id, messages }),
        status: w.store().status(),
        cx: &cx,
    };
    project(&input, ui)
}

fn snap(node: &A11yNode, fixture: &str) -> String {
    format!("{}\n{}", header("projection", fixture), render(node))
}

/// 每个节点：有合法标识（种类与契约表一致）；可操作的角色有非空名称；整棵树里标识不重复（`a11y-tree.md` 第一节第 1 条、第二节）
fn check_rules(root: &A11yNode) {
    let operable: Vec<Role> = TABLE
        .iter()
        .filter(|s| s.operable)
        .map(|s| s.role)
        .collect();
    let mut seen = std::collections::HashSet::new();
    fn walk(n: &A11yNode, operable: &[Role], seen: &mut std::collections::HashSet<String>) {
        // 契约表里每类元素都有标识种类，所以每个节点都该有标识；投影时种类和表对不上会不出标识，这里红
        let a = n
            .aid
            .as_ref()
            .unwrap_or_else(|| panic!("{:?} 没有标识（或标识种类与契约表不符）", n.role));
        assert!(aid::is_valid(a), "标识不合规：{a}");
        assert!(seen.insert(a.clone()), "标识重复：{a}");
        // 获焦、被选中、指向某项这三样只许出现在契约表里「能操作」的元素上（表的 operable 一列管行为）
        let acts = n.states.iter().any(|s| {
            matches!(
                s,
                NodeState::Focused | NodeState::Selected | NodeState::Active(_)
            )
        });
        assert!(
            !acts || atg_gui_spec::a11y::spec(n.element).operable,
            "{:?} 不可操作却带了焦点 / 选中",
            n.element
        );
        if operable.contains(&n.role) {
            assert!(
                n.name.as_deref().is_some_and(|s| !s.is_empty()),
                "{:?} {:?} 缺名称",
                n.role,
                n.aid
            );
        }
        for c in &n.children {
            walk(c, operable, seen);
        }
    }
    walk(root, &operable, &mut seen);
}

#[test]
fn basic_open_private_chat() {
    let w = world("basic");
    let msgs = messages_of(&w, 101);
    let ui = UiState {
        region: Region::MessageList,
        chats_anchor: Some(101),
        msgs_anchor: HashMap::from([(101, first_msg(&msgs).message_id)]),
    };
    let tree = dump(&w, Some((101, &msgs)), &ui);
    check_rules(&tree);
    insta::assert_snapshot!(snap(&tree, "speech/basic"));
}

#[test]
fn basic_chat_list_only() {
    let w = world("basic");
    let ui = UiState {
        region: Region::ChatList,
        chats_anchor: None,
        msgs_anchor: HashMap::new(),
    };
    let tree = dump(&w, None, &ui);
    check_rules(&tree);
    insta::assert_snapshot!(snap(&tree, "speech/basic"));
}

#[test]
fn chat_items_fixture_follows_rules() {
    // 会话项各种情形（C1–C15）名称都非空、标识合规
    let w = world("chat_items");
    let ui = UiState {
        region: Region::ChatList,
        chats_anchor: None,
        msgs_anchor: HashMap::new(),
    };
    check_rules(&dump(&w, None, &ui));
}

#[test]
fn each_chat_keeps_its_own_anchor() {
    // 锚点按会话各存一个、按 id 不按下标（012 第三节第 2 小节、第一节第 5 条）。照人的操作走一遍：在 A 选一条 →
    // 切到 B 再选一条（只写 B 那一格）→ 切回 A，A 的选中还在
    let w = world("basic");
    let a = messages_of(&w, 101);
    let b = messages_of(&w, -1001);
    let (a1, b1) = (first_msg(&a), first_msg(&b));
    let mut ui = UiState {
        region: Region::MessageList,
        chats_anchor: Some(101),
        msgs_anchor: HashMap::from([(101, a1.message_id)]),
    };
    assert_selected(&dump(&w, Some((101, &a)), &ui), a1);
    // 切到 B，在 B 里选一条
    ui.chats_anchor = Some(-1001);
    ui.msgs_anchor.insert(-1001, b1.message_id);
    assert_selected(&dump(&w, Some((-1001, &b)), &ui), b1);
    // 切回 A
    ui.chats_anchor = Some(101);
    assert_selected(&dump(&w, Some((101, &a)), &ui), a1);
}

/// 消息列表恰好选中 `want` 一项，容器的 active= 指向它
fn assert_selected(tree: &A11yNode, want: MsgId) {
    let list = &tree.children[1].children[0];
    let want = aid::aid(aid::Aid::Msg(want));
    assert!(
        list.states.contains(&NodeState::Active(want.clone())),
        "{want}"
    );
    let selected: Vec<_> = list
        .children
        .iter()
        .filter(|c| c.states.contains(&NodeState::Selected))
        .collect();
    assert_eq!(selected.len(), 1, "{want}");
    assert_eq!(selected[0].aid.as_deref(), Some(want.as_str()));
}

#[test]
fn chat_without_anchor_selects_nothing() {
    // 打开的会话没有锚点（别的会话有）：不设 active、不选中任何项
    let w = world("basic");
    let msgs = messages_of(&w, 101);
    let other = first_msg(&messages_of(&w, -1001));
    let ui = UiState {
        region: Region::MessageList,
        chats_anchor: Some(101),
        msgs_anchor: HashMap::from([(-1001, other.message_id)]),
    };
    let tree = dump(&w, Some((101, &msgs)), &ui);
    let list = &tree.children[1].children[0];
    assert!(list.states.contains(&NodeState::Focused));
    assert!(
        !list
            .states
            .iter()
            .any(|s| matches!(s, NodeState::Active(_)))
    );
    assert!(
        list.children
            .iter()
            .all(|c| !c.states.contains(&NodeState::Selected))
    );
}

#[test]
fn order_is_reading_order_and_positions_count_up() {
    // 树的顺序 = 读的顺序（a11y-tree.md 第一节第 3 条）：输入里混着不是消息的 Doc（连接状态、不认识的更新），
    // 投出来的消息项标识序列要和输入里消息的身份序列逐项一致，pos 从 1 数到总数
    let w = world("basic");
    let mixed: Vec<Doc> = w
        .docs()
        .into_iter()
        .filter(|d| match d {
            Doc::Message(m) => m.id.chat_id == -1001,
            Doc::Service(s) => s.id.chat_id == -1001,
            Doc::Tombstone(t) => t.id.chat_id == -1001,
            // 不是消息的留着，验证它们被滤掉、不打乱顺序也不算进总数
            Doc::ChatItem(_)
            | Doc::Alert(_)
            | Doc::Status(_)
            | Doc::Playback(_)
            | Doc::Reactor(_)
            | Doc::Transcription(_)
            | Doc::Unknown(_) => true,
        })
        .collect();
    let want: Vec<String> = messages_of(&w, -1001)
        .iter()
        .map(|d| match d {
            Doc::Message(m) => m.id,
            Doc::Service(s) => s.id,
            Doc::Tombstone(t) => t.id,
            _ => unreachable!("messages_of 只留列表项"),
        })
        .map(|id| aid::aid(aid::Aid::Msg(id)))
        .collect();
    assert!(mixed.len() > want.len(), "夹具里应混有不是消息的 Doc");
    let ui = UiState {
        region: Region::ChatList,
        chats_anchor: Some(-1001),
        msgs_anchor: HashMap::new(),
    };
    let tree = dump(&w, Some((-1001, &mixed)), &ui);
    let items = &tree.children[1].children[0].children;
    let got: Vec<String> = items.iter().filter_map(|it| it.aid.clone()).collect();
    assert_eq!(got, want);
    for (i, it) in items.iter().enumerate() {
        assert!(it.states.contains(&NodeState::Pos(i + 1, items.len())));
    }
}
