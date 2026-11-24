//! 纯函数层的无障碍树快照（`docs/plan/012_gui-ssot.md` 第四节）：夹具 → core 投影 → 界面投影 → GPUI 层格式文本。
//! 夹具借 `atg-core` 的读法夹具（`src/atg-core/fixtures/speech/`，只有一处：`testing.md` 第一节第 3 小节）；
//! 场景目录（`fixtures/scenarios/`）建好后改用场景。
//! 改了表或读法：`INSTA_UPDATE=always cargo test -p atg-gui-spec`，**逐行看过差异**再入库。

use atg_core::speech::SpeechCtx;
use atg_core::tdjson::World;
use atg_core::tree::{Doc, MsgId};
use atg_gui_spec::a11y::{Role, TABLE};
use atg_gui_spec::aid;
use atg_gui_spec::project::{
    A11yNode, NodeState, OpenChat, Region, UiState, ViewInput, header, project, render,
};

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

/// 每个节点：可操作的角色有合法标识和非空名称；整棵树里标识不重复（`a11y-tree.md` 第一节第 1 条、第二节）
fn check_rules(root: &A11yNode) {
    let operable: Vec<Role> = TABLE
        .iter()
        .filter(|s| s.operable)
        .map(|s| s.role)
        .collect();
    let mut seen = std::collections::HashSet::new();
    fn walk(n: &A11yNode, operable: &[Role], seen: &mut std::collections::HashSet<String>) {
        if let Some(a) = &n.aid {
            assert!(aid::is_valid(a), "标识不合规：{a}");
            assert!(seen.insert(a.clone()), "标识重复：{a}");
        }
        if operable.contains(&n.role) {
            assert!(n.aid.is_some(), "{:?} 缺标识", n.role);
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
        msgs_anchor: Some(first_msg(&msgs)),
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
        msgs_anchor: None,
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
        msgs_anchor: None,
    };
    check_rules(&dump(&w, None, &ui));
}

#[test]
fn anchor_from_another_chat_is_ignored() {
    // 锚点指向别的会话的消息：不设 active、不选中任何项（锚点按 id，不按下标：012 第一节第 5 条）
    let w = world("basic");
    let msgs = messages_of(&w, 101);
    let other = first_msg(&messages_of(&w, -1001));
    let ui = UiState {
        region: Region::MessageList,
        chats_anchor: Some(101),
        msgs_anchor: Some(other),
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
    let w = world("basic");
    let msgs = messages_of(&w, -1001);
    let ui = UiState {
        region: Region::ChatList,
        chats_anchor: Some(-1001),
        msgs_anchor: None,
    };
    let tree = dump(&w, Some((-1001, &msgs)), &ui);
    let items = &tree.children[1].children[0].children;
    assert_eq!(items.len(), msgs.len());
    for (i, it) in items.iter().enumerate() {
        assert!(it.states.contains(&NodeState::Pos(i + 1, items.len())));
    }
}
