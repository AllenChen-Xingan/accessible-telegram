//! 含内容的类型的 `Debug`：只打印类型名，正文、说明文字、标题、人名、链接一个字都不出
//! （`specs/contracts/config-privacy.md` 验收判据：「`{:?}` 整个消息也漏不出内容」）。
//!
//! 做法：这些类型不派生 `Debug`，改在这里统一实现成 `类型 { <redacted> }`。
//! 本 crate 自己的单元测试（`cfg(test)`）仍派生完整的 `Debug`，断言失败时看得到差在哪；
//! 集成测试和一切下游（CLI、GUI）拿到的都是这里的脱敏版。`tests/privacy.rs` 用暗号测住。
//!
//! 加了带文字字段的新类型：派生里去掉 `Debug`、加 `#[cfg_attr(test, derive(Debug))]`，再登记到下面。

use crate::model as m;
use crate::tree as t;

macro_rules! redacted_debug {
    ($($name:literal => $ty:ty),* $(,)?) => {$(
        impl ::std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(concat!($name, " { <redacted> }"))
            }
        }
    )*};
}

redacted_debug!(
    "Doc" => t::Doc,
    "Message" => t::Message,
    "Head" => t::Head,
    "Topic" => t::Topic,
    "Sender" => t::Sender,
    "ReplyTo" => t::ReplyTo,
    "Forward" => t::Forward,
    "Origin" => t::Origin,
    "Content" => t::Content,
    "Attachment" => t::Attachment,
    "Span" => t::Span,
    "Service" => t::Service,
    "UserRef" => t::UserRef,
    "Action" => t::Action,
    "Tombstone" => t::Tombstone,
    "ChatItem" => t::ChatItem,
    "ChatLast" => t::ChatLast,
    "AlertChat" => t::AlertChat,
    "Alert" => t::Alert,
    "Reaction" => t::Reaction,
    "ReactionKind" => t::ReactionKind,
    "EntityKind" => t::text::EntityKind,
    "LinkRef" => t::text::LinkRef,
    "model::User" => m::User,
    "model::OriginId" => m::OriginId,
    "model::ReplyRef" => m::ReplyRef,
    "model::Body" => m::Body,
    "model::Message" => m::Message,
    "model::Chat" => m::Chat,
    "model::CoreEvent" => m::CoreEvent,
);
