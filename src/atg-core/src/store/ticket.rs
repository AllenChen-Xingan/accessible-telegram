//! 凭据：发请求 / 开始一件慢活之前从 store 取一张，应答带着它回来，reducer 据此判新旧、判还作不作数。

use super::Store;
use crate::model::*;
use crate::tree::FileRef;

/// 发历史请求时记下的东西：哪个会话、当时的代次、当时最后一条更新的序号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryTicket {
    pub chat_id: ChatId,
    pub generation: u64,
    pub since_seq: u64,
    /// 请求编号，每张凭据加一：同一序号下，后发的请求的应答更新
    pub request: u64,
}

/// 转写凭据：**从 store 里读出这条语音的那一刻**取（不是转写开始时），贯穿下载、解码、转写、提交。
/// 记的是「哪条消息、哪个音频文件」——转写的对象就是那段音频，所以只看提交时它的有效内容还是不是同一个文件
/// （人定 2026-11-13，#59 设计变更，`docs/experiments/exp008-local-stt/design.md` 第二节第 3 条）。
/// 「同一个音频文件」= TDLib 文件 id 相同：本次运行里同一个文件的 id 不变（下载不换），换了内容就是另一个文件、
/// 另一个 id；id 只在本次运行有效（`tree::FileRef`），所以凭据和转写表都只在内存。自己发的语音发送成功后 id 变没变
/// **未实测**：变了就按「内容变了」作废——不会出错，只是要再转一次。
/// 没有内容，`Debug` 照常派生
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptTicket {
    /// 取凭据时这条消息的 id（之后换过 id 的，裁决时沿换 id 链走到现在的）
    pub id: MsgId,
    pub file: FileRef,
}

impl Store {
    /// 发历史请求前取一张凭据（请求编号加一，所以要 `&mut`）。会话不是当前窗口的，应答回来只进缓存
    pub fn ticket(&mut self, chat_id: ChatId) -> HistoryTicket {
        self.requests += 1;
        HistoryTicket {
            chat_id,
            generation: self.generation,
            since_seq: self.last_seq,
            request: self.requests,
        }
    }

    /// 取转写凭据：这条是 store 里的活消息、有效内容是语音才有，并给出该交给下载的那个文件
    pub fn transcript_ticket(&self, id: MsgId) -> Option<TranscriptTicket> {
        let file = super::transcripts::voice_file(self.message(id)?)?;
        Some(TranscriptTicket { id, file })
    }
}
