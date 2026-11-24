//! 场景夹具：读 `fixtures/scenarios/<名>/` 一整套数据，装进 [`FakeBackend`]。
//! 格式正本：`specs/contracts/testing.md` 第一节第 3 小节；`atg-cli --fake <场景>` 与 `atg-gui --fixture <场景>` 共用这一份。
//!
//! 一个场景目录：
//! - `me.json`（必有）：自己，TDLib `user` 对象；收藏夹的会话 id 同它的 id
//! - `users.jsonl`（可无）：别的用户，一行一个 TDLib `user`
//! - `chats.jsonl`（可无）：主列表里的会话，一行一个 TDLib `chat`，按列表顺序
//! - `history/<会话 id>.jsonl`（可无）：那个会话的消息，一行一个 TDLib `message`，从新到旧
//! - `timeline.jsonl`（定时推更新）还没做，用到它的块再加
//!
//! 正式代码（`atg-cli` 与 `atg-gui` 的二进制会走到）：不 panic、不 unwrap。读不了、形状不对一律回
//! `internal{fixture_bad}`；发布构建里给了裸场景名回 `internal{fixture_needs_path}`（两个码登记在
//! `specs/contracts/errors.md` 第一节 `internal` 行）。[`Scenario::install`] 只调 [`FakeBackend`] 已有的公开编排方法，
//! 不给 `Backend` 加任何能力（arch 审定，2026-11-24）。

use super::{FakeAuth, FakeBackend};
use crate::error::CoreError;
use crate::model::ChatList;
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const BAD: CoreError = CoreError::Internal {
    what: "fixture_bad",
};

/// 一套读好的场景数据
pub struct Scenario {
    me: Value,
    users: Vec<Value>,
    chats: Vec<Value>,
    /// （会话 id，从新到旧的消息）
    histories: Vec<(i64, Vec<Value>)>,
}

/// 只有 `[a-z0-9-]+` 算场景名，别的一律当路径
fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// 场景名在开发构建里去哪找：本 crate 的 `fixtures/scenarios/`（编译期定，夹具不进发布包）
#[cfg(debug_assertions)]
fn by_name(name: &str) -> Result<PathBuf, CoreError> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("scenarios")
        .join(name))
}

/// 发布构建不认裸场景名：不把构建机的绝对路径带进发布包
#[cfg(not(debug_assertions))]
fn by_name(_name: &str) -> Result<PathBuf, CoreError> {
    Err(CoreError::Internal {
        what: "fixture_needs_path",
    })
}

fn read(path: &Path) -> Result<Option<String>, CoreError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(BAD),
    }
}

/// 一行一个 JSON 对象；空行跳过。缺文件 = 空
fn lines(path: &Path) -> Result<Vec<Value>, CoreError> {
    let Some(src) = read(path)? else {
        return Ok(Vec::new());
    };
    src.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| match serde_json::from_str::<Value>(l) {
            Ok(v) if v.is_object() => Ok(v),
            _ => Err(BAD),
        })
        .collect()
}

/// 对象的 `@type` 是 `ty`、且带整数 `id`，取出 id
fn typed_id(v: &Value, ty: &str) -> Result<i64, CoreError> {
    if v["@type"] != ty {
        return Err(BAD);
    }
    v["id"].as_i64().ok_or(BAD)
}

impl Scenario {
    /// 命令行上给的场景：名字（`[a-z0-9-]+`）或目录路径
    pub fn resolve(arg: &OsStr) -> Result<PathBuf, CoreError> {
        match arg.to_str() {
            Some(s) if is_name(s) => by_name(s),
            _ => Ok(PathBuf::from(arg)),
        }
    }

    /// 读一个场景目录
    pub fn load(dir: &Path) -> Result<Scenario, CoreError> {
        let me: Value = match read(&dir.join("me.json"))? {
            Some(s) => serde_json::from_str(&s).map_err(|_| BAD)?,
            None => return Err(BAD),
        };
        typed_id(&me, "user")?;
        let users = lines(&dir.join("users.jsonl"))?;
        for u in &users {
            typed_id(u, "user")?;
        }
        let chats = lines(&dir.join("chats.jsonl"))?;
        for c in &chats {
            typed_id(c, "chat")?;
        }
        let mut histories = Vec::new();
        match std::fs::read_dir(dir.join("history")) {
            Ok(entries) => {
                for e in entries {
                    let path = e.map_err(|_| BAD)?.path();
                    let stem = path.file_stem().and_then(OsStr::to_str);
                    let chat_id = match (stem, path.extension().and_then(OsStr::to_str)) {
                        (Some(s), Some("jsonl")) => s.parse::<i64>().map_err(|_| BAD)?,
                        _ => return Err(BAD),
                    };
                    let msgs = lines(&path)?;
                    for m in &msgs {
                        typed_id(m, "message")?;
                        if m["chat_id"].as_i64() != Some(chat_id) {
                            return Err(BAD);
                        }
                    }
                    histories.push((chat_id, msgs));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(BAD),
        }
        // 目录读出来的顺序随文件系统，排一下，装进去的结果才稳定
        histories.sort_by_key(|(id, _)| *id);
        Ok(Scenario {
            me,
            users,
            chats,
            histories,
        })
    }

    /// 会话数（主列表）
    pub fn chat_count(&self) -> usize {
        self.chats.len()
    }

    /// 装进假后端：已登录的假账号、自己的 id、已连接、主列表的会话、每个会话的历史。`start` 之前调
    pub fn install(&self, b: &FakeBackend) {
        b.set_auth(FakeAuth {
            logged_in: true,
            ..FakeAuth::default()
        });
        // load 已核过 me 带整数 id
        if let Some(id) = self.me["id"].as_i64() {
            b.set_me(id);
        }
        // 场景一律是「已连接」：不推的话 `chats` / `history` 要等满同步时限（10 秒）才放弃
        b.push_net("connectionStateReady");
        let user_updates: Vec<Value> = std::iter::once(&self.me)
            .chain(&self.users)
            .map(|u| json!({"@type": "updateUser", "user": u}))
            .collect();
        let groups: Vec<Vec<Value>> = self
            .chats
            .iter()
            .enumerate()
            .map(|(i, c)| {
                // 用户更新跟第一个会话一起推（会话项的名字、消息的发送人都要用）
                let mut g = if i == 0 {
                    user_updates.clone()
                } else {
                    Vec::new()
                };
                g.push(json!({"@type": "updateNewChat", "chat": c}));
                g
            })
            .collect();
        for c in &self.chats {
            if let Some(id) = c["id"].as_i64() {
                let mut g = user_updates.clone();
                g.push(json!({"@type": "updateNewChat", "chat": c}));
                b.set_chat(id, g);
            }
        }
        b.set_chats(ChatList::Main, groups);
        for (id, msgs) in &self.histories {
            b.set_history(*id, msgs.clone());
        }
    }
}
