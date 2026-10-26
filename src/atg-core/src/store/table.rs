//! 存内容的地方：消息表、孤儿补丁、受限登记、换 id 链。
//!
//! **结构性约束**（codex 005 第十轮，受限原文连续三轮从不同入口漏进 store 之后改的）：
//! 字段全是这个子模块私有的，store 的其余代码只能经下面这几个入口写内容——
//! [`Table::put_live`]、[`Table::modify_live`]、[`Table::add_orphan`]——每个入口落盘前都过
//! [`Table::clean`] / [`Table::clean_body`]：
//! - 消息本身受限（自带 `restriction`，或此前登记过）→ 原内容、临时内容换成受限节点
//! - 回复的对象受限 → 引用清掉
//! - 孤儿补丁里的内容 / 临时内容，所属消息已知受限 → 换成受限节点
//!
//! 限制**新登记**时（[`Table::note`]），已存的所有活消息、该消息的补丁都重新过一遍。
//! 读只给不可变引用。墓碑只存发送人和时间，没有内容。

use super::{Entry, FieldChange, MAX_ORPHANS, Patch, Tomb};
use crate::model::*;
use std::collections::{HashMap, HashSet};

/// 整个表的 `Debug` 在非测试构建里只打类型名：里面有消息原文、限制原因（`config-privacy.md` 验收判据）
#[derive(Clone, Default)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Table {
    messages: HashMap<MsgId, Entry>,
    orphans: HashMap<MsgId, Patch>,
    /// 见过的受限消息及原因。独立于消息表：消息被删旧、被撤回、整条被拒收后仍记得。
    /// 受限消息很少，不设上限
    restricted: HashMap<MsgId, String>,
    /// 换掉的旧 id → 新 id
    retired: HashMap<MsgId, MsgId>,
}

impl Table {
    // ───── 读 ─────

    pub(super) fn get(&self, id: MsgId) -> Option<&Entry> {
        self.messages.get(&id)
    }
    pub(super) fn contains(&self, id: MsgId) -> bool {
        self.messages.contains_key(&id)
    }
    pub(super) fn is_live(&self, id: MsgId) -> bool {
        matches!(self.messages.get(&id), Some(Entry::Live(_)))
    }
    pub(super) fn is_tomb(&self, id: MsgId) -> bool {
        matches!(self.messages.get(&id), Some(Entry::Tombstone(_)))
    }
    pub(super) fn has_orphan(&self, id: MsgId) -> bool {
        self.orphans.contains_key(&id)
    }
    pub(super) fn is_retired(&self, id: MsgId) -> bool {
        self.retired.contains_key(&id)
    }
    /// 换掉的旧 id 最终换到了哪个 id；没换过是 `None`
    pub(super) fn retired_to(&self, id: MsgId) -> Option<MsgId> {
        self.retired.contains_key(&id).then(|| self.current(id))
    }
    /// 沿换 id 链走到现在的 id（走到底，判环）
    pub(super) fn current(&self, id: MsgId) -> MsgId {
        let mut cur = id;
        let mut seen = HashSet::new();
        while seen.insert(cur) {
            match self.retired.get(&cur) {
                Some(&next) => cur = next,
                None => break,
            }
        }
        cur
    }
    pub(super) fn is_restricted(&self, id: MsgId) -> bool {
        self.restricted.contains_key(&self.current(id))
    }

    // ───── 写（只有这几个入口碰内容） ─────

    /// 看到一条消息对象（不论最后存不存）：带限制就登记；新登记的，把已存的内容全部重新清一遍
    pub(super) fn note(&mut self, m: &Message) {
        let Some(reason) = &m.restriction else {
            return;
        };
        let id = self.current(m.id);
        if self.restricted.contains_key(&id) {
            return;
        }
        self.restricted.insert(id, reason.clone());
        self.reclean_all();
    }

    /// 存一条活消息（整条替换）
    pub(super) fn put_live(&mut self, mut m: Message) {
        self.note(&m);
        self.clean(&mut m);
        self.messages.insert(m.id, Entry::Live(m));
    }

    /// 改一条活消息；改完再清。不是活消息返回 `false`
    pub(super) fn modify_live(&mut self, id: MsgId, f: impl FnOnce(&mut Message)) -> bool {
        let Some(Entry::Live(mut m)) = self.messages.remove(&id) else {
            return false;
        };
        f(&mut m);
        self.clean(&mut m);
        self.messages.insert(id, Entry::Live(m));
        true
    }

    pub(super) fn put_tomb(&mut self, id: MsgId, t: Tomb) {
        self.messages.insert(id, Entry::Tombstone(t));
    }

    /// 没见过原消息的墓碑补记发送人和时间（只这两项，没有内容）
    pub(super) fn fill_tomb(&mut self, id: MsgId, sender: SenderId, date: Time) {
        if let Some(Entry::Tombstone(t)) = self.messages.get_mut(&id)
            && t.sender.is_none()
        {
            t.sender = Some(sender);
            t.date = Some(date);
        }
    }

    pub(super) fn remove(&mut self, id: MsgId) {
        self.messages.remove(&id);
    }

    pub(super) fn retire(&mut self, old: MsgId, new: MsgId) {
        if old != new {
            self.retired.insert(old, new);
            // 引用旧 id 的回复，现在指向的可能是受限消息：重新清一遍
            if self.is_restricted(new) {
                self.reclean_all();
            }
        }
    }

    /// 给还没到的消息记一处字段改动（内容先清）。满了先挤掉最旧的，返回被挤掉的 (id, 序号)
    pub(super) fn add_orphan(
        &mut self,
        id: MsgId,
        seq: u64,
        ch: FieldChange,
    ) -> Option<(MsgId, u64)> {
        let mut evicted = None;
        if !self.orphans.contains_key(&id) && self.orphans.len() >= MAX_ORPHANS {
            evicted = self
                .orphans
                .iter()
                .min_by_key(|(id, p)| (p.seq, **id))
                .map(|(id, p)| (*id, p.seq));
            if let Some((oldest, _)) = evicted {
                self.orphans.remove(&oldest);
            }
        }
        let ch = match ch {
            FieldChange::Content(c) => FieldChange::Content(self.clean_body(id, c)),
            FieldChange::Ephemeral(e) => FieldChange::Ephemeral(e.map(|b| self.clean_body(id, b))),
            other @ (FieldChange::Edited(_) | FieldChange::Reactions(_) | FieldChange::Opened) => {
                other
            }
        };
        let p = self.orphans.entry(id).or_default();
        p.seq = seq;
        match ch {
            FieldChange::Content(c) => p.content = Some((seq, c)),
            FieldChange::Ephemeral(e) => p.ephemeral = Some((seq, e)),
            FieldChange::Edited(t) => p.edited = Some((seq, t)),
            FieldChange::Reactions(r) => p.reactions = Some((seq, r)),
            FieldChange::Opened => p.opened = Some(seq),
        }
        evicted
    }

    /// 取走补丁（内容已清过；补到消息上后还要经 [`Table::put_live`]）
    pub(super) fn take_orphan(&mut self, id: MsgId) -> Option<Patch> {
        self.orphans.remove(&id)
    }

    pub(super) fn drop_orphan(&mut self, id: MsgId) {
        self.orphans.remove(&id);
    }

    // ───── 清 ─────

    fn restricted_content(reason: &str) -> Content {
        Content {
            body: vec![],
            attachment: Some(Attachment::Restricted {
                reason: reason.to_owned(),
            }),
            sensitive: false,
        }
    }

    fn clean(&self, m: &mut Message) {
        if m.restriction.is_none()
            && let Some(reason) = self.restricted.get(&self.current(m.id))
        {
            m.restriction = Some(reason.clone());
        }
        m.enforce();
        if let Some(ReplyRef::Message { target, quote, .. }) = &mut m.reply_to
            && quote.is_some()
            && self.is_restricted(*target)
        {
            *quote = None;
        }
    }

    fn clean_body(&self, id: MsgId, b: Body) -> Body {
        match self.restricted.get(&self.current(id)) {
            Some(reason) => Body::Content(Self::restricted_content(reason)),
            None => b,
        }
    }

    fn reclean_all(&mut self) {
        let ids: Vec<MsgId> = self
            .messages
            .iter()
            .filter(|(_, e)| matches!(e, Entry::Live(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.modify_live(id, |_| {});
        }
        let orphan_ids: Vec<MsgId> = self.orphans.keys().copied().collect();
        for id in orphan_ids {
            if !self.is_restricted(id) {
                continue;
            }
            let Some(mut p) = self.orphans.remove(&id) else {
                continue;
            };
            p.content = p.content.map(|(s, b)| (s, self.clean_body(id, b)));
            p.ephemeral = p
                .ephemeral
                .map(|(s, e)| (s, e.map(|b| self.clean_body(id, b))));
            self.orphans.insert(id, p);
        }
    }
}

#[cfg(not(test))]
impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Table { <redacted> }")
    }
}
