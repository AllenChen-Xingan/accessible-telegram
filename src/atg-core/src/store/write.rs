//! Store 改状态的那一半：按事件分派（`update`）、整条快照与墓碑、字段变更、删除、换 id、窗口与历史应答。
//! 合并规则见模块根 `store/mod.rs` 的头注释；只读的访问器在模块根，纯查询在 `query.rs`。

use super::*;

impl Store {
    // ───── 更新 ─────

    pub(super) fn update(&mut self, seq: u64, event: CoreEvent) {
        match event {
            CoreEvent::NewMessage(m) => {
                let id = m.id;
                self.upsert_snapshot(seq, m);
                self.show_in_window(id);
            }
            CoreEvent::MessageContentChanged { id, content } => {
                self.change(seq, id, FieldChange::Content(content));
            }
            CoreEvent::MessageEphemeralChanged { id, ephemeral } => {
                self.change(seq, id, FieldChange::Ephemeral(ephemeral));
            }
            CoreEvent::MessageEdited { id, edit_date } => {
                // TDLib 用 0 表示没有编辑时间；与整条消息的映射一致，存成 None
                let t = (edit_date.0 > 0).then_some(edit_date);
                self.change(seq, id, FieldChange::Edited(t));
            }
            CoreEvent::MessageContentOpened { id } => self.change(seq, id, FieldChange::Opened),
            CoreEvent::MessageInteractionChanged { id, reactions } => {
                self.change(seq, id, FieldChange::Reactions(reactions));
            }
            CoreEvent::MessagesDeleted {
                chat_id,
                message_ids,
                is_permanent,
                from_cache,
            } => self.delete(seq, chat_id, &message_ids, is_permanent && !from_cache),
            CoreEvent::SendSucceeded { old_id, message }
            | CoreEvent::SendFailed {
                old_id, message, ..
            } => self.rename(seq, old_id, message),
            CoreEvent::ChatAdded {
                mut chat,
                last_message,
            } => {
                // 同一会话重复到达：保留 store 里已维护的最后一条
                chat.last_message = self.chats.get(&chat.id).and_then(|c| c.last_message);
                let id = chat.id;
                self.chats.insert(id, chat);
                if let Some(m) = last_message {
                    self.set_last(seq, id, Some(m));
                }
            }
            CoreEvent::ChatPositionChanged { chat_id, position } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.positions.retain(|p| p.list != position.list);
                    if position.order != 0 {
                        c.positions.push(position);
                    }
                }
            }
            CoreEvent::ChatLastMessageChanged {
                chat_id,
                last_message,
                positions,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id)
                    && let Some(p) = positions
                {
                    c.positions = p.into_iter().filter(|p| p.order != 0).collect();
                }
                self.set_last(seq, chat_id, last_message);
            }
            CoreEvent::ChatDraftChanged { chat_id, positions } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.positions = positions.into_iter().filter(|p| p.order != 0).collect();
                }
            }
            CoreEvent::ChatReadInbox {
                chat_id,
                last_read_inbox_message_id,
                unread_count,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.last_read_inbox = last_read_inbox_message_id;
                    c.unread_count = unread_count;
                }
            }
            CoreEvent::ChatReadOutbox {
                chat_id,
                last_read_outbox_message_id,
            } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.last_read_outbox = last_read_outbox_message_id;
                }
            }
            CoreEvent::ChatMentionCountChanged { chat_id, count } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.unread_mention_count = count;
                }
            }
            CoreEvent::ChatTitleChanged { chat_id, title } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.title = title;
                }
            }
            CoreEvent::ChatNotificationChanged { chat_id, notify } => {
                if let Some(c) = self.chats.get_mut(&chat_id) {
                    c.notify = notify;
                }
            }
            CoreEvent::ScopeNotificationChanged { scope, notify } => {
                self.scope_notify.insert(scope, notify);
            }
            // 话题静音到期 TDLib 不再推更新：按收线程记下的收到时刻换成绝对到期时刻（store 自己不碰时钟）
            CoreEvent::ForumTopicChanged {
                chat_id,
                topic_id,
                last_read_inbox,
                notify,
                received_ms,
            } => {
                let state = ForumTopicState {
                    last_read_inbox,
                    notify,
                };
                self.topics
                    .insert((chat_id, topic_id), topic_notify(state, received_ms));
            }
            // 正在输入归 `notify`；补发结束只给订阅方看
            CoreEvent::ChatAction { .. } | CoreEvent::SyncDone => {}
            // 0 = 没有（登出后 TDLib 会把选项清掉）
            CoreEvent::MyIdChanged { user_id } => self.me = (user_id != 0).then_some(user_id),
            CoreEvent::UserChanged(u) => {
                self.users.insert(u.id, u);
            }
            CoreEvent::UserStatusChanged(s) => self.status = Some(s),
            // 三条线的原值由后端的 `status()` 给；store 只存合成后的那个
            CoreEvent::AuthStepChanged(_)
            | CoreEvent::NetStateChanged(_)
            | CoreEvent::CoreStateChanged(_) => {}
            CoreEvent::Unknown { .. } => {}
        }
    }

    /// 一整条消息（新消息、会话的最后一条）随更新到达：它就是这个序号时的完整状态，整条替换
    /// （更新按序到达，此前的字段更新都已含在里面）。墓碑不复活，换掉的旧 id 不复活
    fn upsert_snapshot(&mut self, seq: u64, m: Message) {
        self.table.note(&m);
        let id = m.id;
        if self.table.is_tomb(id) || self.table.is_retired(id) {
            return;
        }
        // 补丁、字段序号都不可能比同一条流里后到的整条消息新（更新按序），丢掉
        self.table.drop_orphan(id);
        self.fields.remove(&id);
        self.put_live(m);
        self.versions.insert(id, update_version(seq));
    }

    /// 存活消息（经 [`Table::put_live`] 清过）并维护每会话的活消息索引
    fn put_live(&mut self, m: Message) {
        let id = m.id;
        self.by_chat
            .entry(id.chat_id)
            .or_default()
            .insert(id.message_id);
        self.dirty.insert(id.chat_id);
        self.table.put_live(m);
    }

    fn put_tomb(&mut self, id: MsgId, t: Tomb) {
        if let Some(ids) = self.by_chat.get_mut(&id.chat_id) {
            ids.remove(&id.message_id);
        }
        self.table.put_tomb(id, t);
    }

    fn take(&mut self, id: MsgId) {
        if let Some(ids) = self.by_chat.get_mut(&id.chat_id) {
            ids.remove(&id.message_id);
        }
        self.table.remove(id);
    }

    /// 记一道屏障：凭据 `(since_seq, request)` 不比它新的应答，store 里没有的消息不收
    fn mark_evicted(&mut self, chat_id: ChatId, seq: Version) {
        let w = self.evicted_upto.entry(chat_id).or_default();
        *w = (*w).max(seq);
    }

    /// 非当前会话只留最近 `keep_per_chat` 条活消息；最后一条不删
    pub(super) fn prune(&mut self, chat_id: ChatId) {
        let keep = self.config.keep_per_chat;
        let last = self.chats.get(&chat_id).and_then(|c| c.last_message);
        let Some(ids) = self.by_chat.get(&chat_id) else {
            return;
        };
        // 超额只按能删的算：待发的不占配额，否则一堆待发会把随后的普通消息挤掉（codex 029 第 2 轮 #2）
        let excess = ids
            .iter()
            .filter(|&&m| self.table.prunable(chat_id, m, None))
            .count();
        let Some(excess) = excess.checked_sub(keep).filter(|&e| e > 0) else {
            return;
        };
        let victims: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|&m| self.table.prunable(chat_id, m, last))
            .take(excess)
            .collect();
        if victims.is_empty() {
            return;
        }
        for message_id in victims {
            let id = MsgId {
                chat_id,
                message_id,
            };
            self.take(id);
            self.fields.remove(&id);
            self.versions.remove(&id);
        }
        // 删掉的消息之后还可能收到更新（记成补丁）；早于此刻的请求的应答不许把旧内容带回
        // 此刻之前发出的请求（编号不超过现有的）都算旧
        self.mark_evicted(chat_id, (self.last_seq, self.requests));
    }

    fn show_in_window(&mut self, id: MsgId) {
        if let Some(w) = self.window.as_mut()
            && w.chat_id == id.chat_id
            && self.table.contains(id)
        {
            w.ids.insert(id.message_id);
        }
    }

    fn change(&mut self, seq: u64, id: MsgId, ch: FieldChange) {
        // 换掉的旧 id 不再有消息，也不记补丁（不然晚到的应答会借补丁把它带回来）
        if self.table.is_retired(id) || self.table.is_tomb(id) {
            return;
        }
        if self.table.is_live(id) {
            let f = self.fields.entry(id).or_default();
            match &ch {
                FieldChange::Content(_) => f.content = seq,
                FieldChange::Ephemeral(_) => f.ephemeral = seq,
                FieldChange::Edited(_) => f.edited = seq,
                FieldChange::Reactions(_) => f.reactions = seq,
                FieldChange::Opened => f.opened = seq,
            }
            self.table.modify_live(id, |m| match ch {
                FieldChange::Content(c) => m.content = c,
                FieldChange::Ephemeral(e) => m.ephemeral = e,
                FieldChange::Edited(t) => m.edited = t,
                FieldChange::Reactions(r) => m.reactions = r,
                FieldChange::Opened => m.mark_opened(),
            });
            return;
        }
        // 消息还没到：记成补丁（各字段带序号），消息随应答进来时只补比请求新的字段
        if let Some((evicted, eseq)) = self.table.add_orphan(id, seq, ch) {
            // 补丁之前发出的请求（序号早于补丁）不可信；补丁之后发出的已含这次更新
            self.mark_evicted(evicted.chat_id, (eseq.saturating_sub(1), u64::MAX));
        }
    }

    fn delete(&mut self, seq: u64, chat_id: ChatId, ids: &[i64], revoked: bool) {
        for &message_id in ids {
            let id = MsgId {
                chat_id,
                message_id,
            };
            self.table.drop_orphan(id);
            self.fields.remove(&id);
            self.versions.insert(id, update_version(seq));
            // 还没发出去的（待发送 / 发送失败）被删：是自己取消或重发（TDLib 重发时先发一条
            // 永久删除，`MessagesManager.cpp:25221-25223`；新旧的关联由 `Input::Resent` 给），不是撤回
            let unsent = match self.table.get(id) {
                Some(Entry::Live(m)) => (m.sending != SendingState::Sent).then_some(m.sending),
                Some(Entry::Tombstone(_)) | None => None,
            };
            if revoked && unsent.is_none() {
                let tomb = match self.table.get(id) {
                    Some(Entry::Tombstone(_)) => continue,
                    Some(Entry::Live(m)) => Tomb {
                        sender: Some(m.sender.clone()),
                        date: Some(m.date),
                        outgoing: m.outgoing,
                    },
                    None => Tomb {
                        sender: None,
                        date: None,
                        outgoing: false,
                    },
                };
                // 窗口里的 id 原样留着，投影成原位占位
                self.put_tomb(id, tomb);
            } else {
                // 只是清缓存 / 暂时不可访问 / 没发出去的：不是撤回，不留占位，从窗口拿掉；墓碑不受影响
                if self.table.is_live(id) {
                    self.take(id);
                    if let Some(w) = self.window.as_mut()
                        && w.chat_id == chat_id
                    {
                        w.ids.remove(&message_id);
                    }
                    if let Some(c) = self.chats.get_mut(&chat_id)
                        && c.last_message == Some(message_id)
                    {
                        c.last_message = None;
                    }
                }
            }
        }
    }

    /// 发送成功 / 失败：临时 id 换成新 id，内容取新的那份，位置（窗口、会话最后一条）原样沿用
    fn rename(&mut self, seq: u64, old: MsgId, m: Message) {
        self.table.note(&m);
        let new = m.id;
        // 旧的或新的已撤回：不复活。同一次换 id 重复到达（旧的已不在、早换到这个新 id）：不重复做
        if self.table.is_tomb(old)
            || self.table.is_tomb(new)
            || (!self.table.contains(old) && self.table.retired_to(old) == Some(new))
        {
            return;
        }
        self.take(old);
        if old != new {
            self.versions.insert(old, update_version(seq));
            self.table.retire(old, new);
        }
        self.table.drop_orphan(old);
        self.fields.remove(&old);
        self.fields.remove(&new);
        // 新 id 名下不该有补丁（新 id 是这一刻才有的）；有也是坏数据，丢掉
        self.table.drop_orphan(new);
        if old != new {
            self.parent.insert(new, old);
        }
        self.put_live(m);
        self.versions.insert(new, update_version(seq));
        if let Some(w) = self.window.as_mut()
            && w.chat_id == old.chat_id
        {
            w.ids.remove(&old.message_id);
        }
        // 会话正开着就放进窗口：旧 id 原在窗口里的原位换上（按 id 排序）；旧 id 没见过的也补进来
        self.show_in_window(new);
        if let Some(c) = self.chats.get_mut(&old.chat_id)
            && c.last_message == Some(old.message_id)
        {
            c.last_message = Some(new.message_id);
        }
    }

    fn set_last(&mut self, seq: u64, chat_id: ChatId, m: Option<Message>) {
        // 已换掉的旧 id 指向它现在的 id（旧快照本身下面会被拒收）
        let last = m.as_ref().map(|m| self.table.current(m.id).message_id);
        let Some(c) = self.chats.get_mut(&chat_id) else {
            return;
        };
        c.last_message = last;
        if let Some(m) = m {
            let id = self.current_id(m.id);
            self.upsert_snapshot(seq, m);
            // 最后一条未知期间，新消息可以不带 updateNewMessage 就进会话（`td_api.tl:11312`）：
            // 它作为最后一条到达时，会话正开着就放进窗口
            self.show_in_window(id);
        }
    }

    // ───── 窗口与历史 ─────

    pub(super) fn open_window(&mut self, chat_id: ChatId) {
        // 关掉的窗口里的撤回占位留下来，下次打开同一会话原位带回（拍板 006 甲3）
        if let Some(old) = self.window.take() {
            let tombs: Vec<i64> = old
                .ids
                .iter()
                .copied()
                .filter(|&m| {
                    self.table.is_tomb(MsgId {
                        chat_id: old.chat_id,
                        message_id: m,
                    })
                })
                .collect();
            self.window_tombs
                .entry(old.chat_id)
                .or_default()
                .extend(tombs);
            // 切走的会话变成「非当前」，按上限删旧
            self.dirty.insert(old.chat_id);
        }
        let carried: BTreeSet<i64> = self
            .window_tombs
            .get(&chat_id)
            .map(|set| {
                set.iter()
                    .copied()
                    .filter(|&m| {
                        self.table.is_tomb(MsgId {
                            chat_id,
                            message_id: m,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.generation += 1;
        self.window = Some(Window {
            chat_id,
            generation: self.generation,
            ids: carried.clone(),
            stale: false,
            carried,
            loaded_from: None,
        });
    }

    pub(super) fn history(&mut self, t: HistoryTicket, messages: Vec<Message>) {
        let current = self
            .window
            .as_ref()
            .is_some_and(|w| w.chat_id == t.chat_id && w.generation == t.generation);
        // 这份应答确认到的最旧 id（空应答 = 全部）
        let mut oldest_in_response: Option<i64> = None;

        for mut m in messages {
            let id = m.id;
            // 应答只管请求的那个会话；混进来的别的会话的消息不收
            if id.chat_id != t.chat_id {
                continue;
            }
            oldest_in_response =
                Some(oldest_in_response.map_or(id.message_id, |o| o.min(id.message_id)));
            self.table.note(&m);
            // 已换过 id 的旧 id：应答里那份是换 id 之前的，认作新 id（新 id 在就放进窗口）
            if let Some(new) = self.table.retired_to(id) {
                if current && self.table.contains(new) {
                    self.show_in_window(new);
                }
                continue;
            }
            let this = (t.since_seq, t.request);
            let newer_in_store = self.versions.get(&id).is_some_and(|&v| v > this);
            enum Have {
                UnseenTomb,
                Tomb,
                Live,
                Nothing,
            }
            let have = match self.table.get(id) {
                Some(Entry::Tombstone(tb)) if tb.sender.is_none() => Have::UnseenTomb,
                Some(Entry::Tombstone(_)) => Have::Tomb,
                Some(Entry::Live(_)) => Have::Live,
                None => Have::Nothing,
            };
            match have {
                // 没见过原消息就被永久删除的墓碑还待判定：应答说它是没发出去的（待发送 / 失败），
                // 那次删除是取消或重发，不是撤回——拿掉占位（删除的版本屏障留着，旧应答不会把它带回）；
                // 否则补记应答里的发送人和时间（只记这两项，原文不留）
                Have::UnseenTomb => {
                    if m.sending != SendingState::Sent {
                        self.take(id);
                        continue;
                    }
                    self.table.fill_tomb(id, m.sender.clone(), m.date);
                }
                // 撤回过的不加回来；当前窗口里仍在原位显示占位
                Have::Tomb => {}
                // store 的整条快照比应答新：整条不收
                Have::Live if newer_in_store => {}
                // 应答为底，请求之后被更新过的字段取 store 的
                Have::Live => {
                    let f = self.fields.get(&id).copied().unwrap_or_default();
                    let p = match self.table.get(id) {
                        Some(Entry::Live(old)) => Patch::from_live(old, f, t.since_seq),
                        Some(Entry::Tombstone(_)) | None => Patch::default(),
                    };
                    p.apply_to(&mut m, t.since_seq);
                    self.put_live(m);
                    self.versions.insert(id, this);
                }
                Have::Nothing if newer_in_store && !self.table.has_orphan(id) => {
                    // 请求发出后被非撤回的删除拿掉了（清缓存 / 不可访问）：应答里那份是旧的，不收
                    continue;
                }
                Have::Nothing
                    if self
                        .evicted_upto
                        .get(&id.chat_id)
                        .is_some_and(|&b| this <= b) =>
                {
                    // 请求之后这个会话丢过东西：这条可能正是丢掉的那条，旧内容不可信。
                    // 有没有新补丁都一样（新补丁只记了丢之后的字段）。不收，窗口重拉；补丁留给重拉
                    if current && let Some(w) = self.window.as_mut() {
                        w.stale = true;
                    }
                    continue;
                }
                Have::Nothing => {
                    if let Some(p) = self.table.take_orphan(id) {
                        // 补丁的字段序号转成这条消息的字段序号：之后更新的应答到了还能照样合并
                        self.fields.insert(id, p.seqs());
                        p.apply_to(&mut m, t.since_seq);
                    }
                    self.put_live(m);
                    self.versions.insert(id, this);
                }
            }
            if current && let Some(w) = self.window.as_mut() {
                w.ids.insert(id.message_id);
            }
        }
        // 推进加载边界：只由应答推进，新消息不动它
        if current && let Some(w) = self.window.as_mut() {
            let reached = oldest_in_response.unwrap_or(i64::MIN);
            w.loaded_from = Some(w.loaded_from.map_or(reached, |b| b.min(reached)));
        }
    }
}
