//! 稳定标识（`.accessibility_id(..)` → macOS AXIdentifier）的唯一写法。规约：`specs/gui/a11y-tree.md` 第二节。
//!
//! 写法 `<种类>[:<数字>]*`：种类是小写字母和连字符，数字是十进制整数、可带负号（超级群 id 以 `-100` 开头）。
//! 界面代码只经 [`aid`] 拿标识，不手拼字符串（`check-gui-strings.py` 将来扫）。种类和每种带几个数字
//! 由 [`IdKind`] 定，[`crate::schema`] 把这张表原样导出，`scripts/check-a11y-tree.py`（待写，PR-3）将照它核快照里的标识；现在只有本文件的单测在管。
//!
//! 现在只有 M3.1 用到的种类；加一种就加一个变体，[`IdKind::ALL`] 与 `match` 不写 `_ =>`，漏一处编不过。

use atg_core::tree::MsgId;

/// 标识的种类（不带数字）。契约表、自描述用它
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// 主窗口，一个进程一个
    Window,
    /// 地标：会话区
    RegionChats,
    /// 地标：消息区
    RegionMessages,
    /// 会话列表
    Chats,
    /// 会话项 `chat:<chat_id>`
    Chat,
    /// 消息列表 `messages:<chat_id>`：带会话 id，切会话时整列表换身份
    Messages,
    /// 消息项、系统消息项、撤回占位 `msg:<chat_id>:<message_id>`（撤回后原位留占位，标识不变）
    Msg,
    /// 状态栏
    Status,
}

impl IdKind {
    /// 全部种类，按树里从上到下的顺序（与 `a11y::TABLE` 一致）
    pub const ALL: [IdKind; 8] = [
        IdKind::Window,
        IdKind::RegionChats,
        IdKind::Chats,
        IdKind::Chat,
        IdKind::RegionMessages,
        IdKind::Messages,
        IdKind::Msg,
        IdKind::Status,
    ];

    /// 冒号前那一段
    pub fn prefix(self) -> &'static str {
        match self {
            IdKind::Window => "window",
            IdKind::RegionChats => "region-chats",
            IdKind::RegionMessages => "region-messages",
            IdKind::Chats => "chats",
            IdKind::Chat => "chat",
            IdKind::Messages => "messages",
            IdKind::Msg => "msg",
            IdKind::Status => "status",
        }
    }

    /// 冒号后带几个数字
    pub fn arity(self) -> usize {
        match self {
            IdKind::Window
            | IdKind::RegionChats
            | IdKind::RegionMessages
            | IdKind::Chats
            | IdKind::Status => 0,
            IdKind::Chat | IdKind::Messages => 1,
            IdKind::Msg => 2,
        }
    }
}

/// 一个具体的标识：种类 + 它要的数字。数字的个数由变体的形状保证，界面代码拼不错
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aid {
    Window,
    RegionChats,
    RegionMessages,
    Chats,
    Chat(i64),
    Messages(i64),
    /// 用**当前**的消息 id（发送成功换 id 时跟着换，与 `atg-cli` 输出对得上；`a11y-tree.md` 第二节末段）
    Msg(MsgId),
    Status,
}

impl Aid {
    pub fn kind(self) -> IdKind {
        match self {
            Aid::Window => IdKind::Window,
            Aid::RegionChats => IdKind::RegionChats,
            Aid::RegionMessages => IdKind::RegionMessages,
            Aid::Chats => IdKind::Chats,
            Aid::Chat(_) => IdKind::Chat,
            Aid::Messages(_) => IdKind::Messages,
            Aid::Msg(_) => IdKind::Msg,
            Aid::Status => IdKind::Status,
        }
    }

    fn numbers(self) -> Vec<i64> {
        match self {
            Aid::Window | Aid::RegionChats | Aid::RegionMessages | Aid::Chats | Aid::Status => {
                Vec::new()
            }
            Aid::Chat(c) | Aid::Messages(c) => vec![c],
            Aid::Msg(m) => vec![m.chat_id, m.message_id],
        }
    }
}

/// 标识的唯一写法
pub fn aid(a: Aid) -> String {
    let mut s = a.kind().prefix().to_owned();
    for n in a.numbers() {
        s.push(':');
        s.push_str(&n.to_string());
    }
    s
}

/// 一串文字是不是合法标识：种类认得、数字个数对、每段是十进制整数（可带负号、不带前导 0 和加号）
pub fn is_valid(s: &str) -> bool {
    let mut parts = s.split(':');
    let Some(prefix) = parts.next() else {
        return false;
    };
    let Some(kind) = IdKind::ALL.iter().find(|k| k.prefix() == prefix) else {
        return false;
    };
    let nums: Vec<&str> = parts.collect();
    nums.len() == kind.arity() && nums.iter().all(|n| is_int(n))
}

fn is_int(n: &str) -> bool {
    let digits = n.strip_prefix('-').unwrap_or(n);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && n != "-0"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(chat_id: i64, message_id: i64) -> Aid {
        Aid::Msg(MsgId {
            chat_id,
            message_id,
        })
    }

    #[test]
    fn writes_the_spec_examples() {
        // a11y-tree.md 第二节的例子
        assert_eq!(aid(msg(-100123, 4567)), "msg:-100123:4567");
        assert_eq!(aid(Aid::Chat(101)), "chat:101");
        assert_eq!(aid(Aid::Messages(101)), "messages:101");
        assert_eq!(aid(Aid::Window), "window");
        assert_eq!(aid(Aid::RegionChats), "region-chats");
    }

    #[test]
    fn every_written_id_is_valid_and_arity_matches() {
        let samples = [
            Aid::Window,
            Aid::RegionChats,
            Aid::RegionMessages,
            Aid::Chats,
            Aid::Chat(-1001),
            Aid::Messages(0),
            msg(101, 1),
            Aid::Status,
        ];
        // 每种都抽到了（加种类忘了加样本就红）
        for k in IdKind::ALL {
            assert!(samples.iter().any(|a| a.kind() == k), "{k:?} 没有样本");
        }
        for a in samples {
            let s = aid(a);
            assert!(is_valid(&s), "{s}");
            assert_eq!(s.split(':').count() - 1, a.kind().arity(), "{s}");
        }
    }

    #[test]
    fn prefixes_are_unique_and_well_formed() {
        for (i, a) in IdKind::ALL.iter().enumerate() {
            let p = a.prefix();
            assert!(!p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
            for b in &IdKind::ALL[i + 1..] {
                assert_ne!(p, b.prefix());
            }
        }
    }

    #[test]
    fn rejects_bad_ids() {
        // a11y-tree.md 第五节的反例：下标、只有消息 id
        for bad in [
            "item-3",
            "msg:1002",
            "msg:101:1002:3",
            "chat:",
            "chat:+1",
            "chat:01",
            "chat:-0",
            "chat:a",
            "Chat:1",
            "",
            "window:1",
        ] {
            assert!(!is_valid(bad), "{bad}");
        }
    }
}
