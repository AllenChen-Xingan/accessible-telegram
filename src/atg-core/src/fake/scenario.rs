//! 场景夹具：读 `fixtures/scenarios/<名>/` 一整套数据，装进 [`FakeBackend`]。
//! **格式与解析规则的正本**：`specs/contracts/testing.md` 第一节第 3 小节（哪些文件可无、顺序、哪些目录项跳过、名字与路径怎么分、
//! 会触发 `fixture_bad` 的情形）。这里不另写一份，只实现它。`atg-cli --fake <场景>` 现在用它；`atg-gui --fixture <场景>`
//! （M3.1 后面的 PR，还没有）将用同一份。
//!
//! 正式代码（`atg-cli` 的二进制会走到）：不 panic、不 unwrap。发布构建里给了裸场景名回 `internal{fixture_needs_path}`，
//! 其余读不了、形状不对一律回 `internal{fixture_bad}`（码登记在 `specs/contracts/errors.md` 第一节 `internal` 行）。
//! [`Scenario::install`] 只调 [`FakeBackend`] 已有的公开编排方法，不给 `Backend` 加任何能力（arch 审定，2026-11-24）。

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
                    // 只认 `<会话 id>.jsonl`：点开头的（访达的 .DS_Store、编辑器的 .swp）、不是 .jsonl 的（101.jsonl~）、
                    // 子目录一律跳过；形状像夹具（.jsonl）而文件名不是整数的才算坏
                    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
                        continue;
                    };
                    if name.starts_with('.') || !name.ends_with(".jsonl") || !path.is_file() {
                        continue;
                    }
                    let chat_id = name
                        .trim_end_matches(".jsonl")
                        .parse::<i64>()
                        .map_err(|_| BAD)?;
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

    /// 装进假后端：已登录的假账号、自己的 id、已连接、主列表的会话、每个会话的历史。`start` 之前调。
    ///
    /// 用户更新推两处，不是冗余：`set_chats` 管「拉整份列表」（`chats_load`）时推什么，`set_chat` 管「按 id 取一个会话」
    /// （`chat_get`，`history` 先调它）时推什么。两边都要带上用户，会话项的名字、消息的发送人才认得出
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> crate::config::TempDirGuard {
        crate::config::TempDirGuard::new(tag).expect("临时目录")
    }

    fn put(d: &Path, rel: &str, body: &str) {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().expect("有上级")).expect("建目录");
        std::fs::write(p, body).expect("写文件");
    }

    const ME: &str = r#"{"@type":"user","id":9,"first_name":"我自己"}"#;
    const CHAT: &str = r#"{"@type":"chat","id":101,"title":"林小雨"}"#;
    const MSG: &str = r#"{"@type":"message","id":1,"chat_id":101}"#;

    fn is_bad(r: Result<Scenario, CoreError>) -> bool {
        matches!(
            r,
            Err(CoreError::Internal {
                what: "fixture_bad"
            })
        )
    }

    #[test]
    fn shipped_scenarios_load() {
        for name in ["basic-3-chats", "empty"] {
            let Ok(p) = Scenario::resolve(OsStr::new(name)) else {
                panic!("{name} 解析不了");
            };
            let Ok(sc) = Scenario::load(&p) else {
                panic!("{name} 装不上");
            };
            assert_eq!(sc.chat_count(), if name == "empty" { 0 } else { 3 });
        }
    }

    #[test]
    fn only_me_json_is_required() {
        let d = dir("sc-min");
        put(d.path(), "me.json", ME);
        let Ok(sc) = Scenario::load(d.path()) else {
            panic!("只有 me.json 也该装得上");
        };
        assert_eq!(sc.chat_count(), 0);
    }

    #[test]
    fn junk_in_history_is_skipped() {
        // 访达、编辑器留下的东西不许把整个场景弄坏
        let d = dir("sc-junk");
        put(d.path(), "me.json", ME);
        put(d.path(), "chats.jsonl", CHAT);
        put(d.path(), "history/101.jsonl", MSG);
        put(d.path(), "history/.DS_Store", "\0\0binary");
        put(d.path(), "history/.101.jsonl.swp", "x");
        put(d.path(), "history/101.jsonl~", "x");
        put(d.path(), "history/notes.txt", "x");
        put(d.path(), "history/sub/202.jsonl", MSG);
        let Ok(sc) = Scenario::load(d.path()) else {
            panic!("history/ 里的杂项应跳过");
        };
        assert_eq!(sc.histories.len(), 1);
        assert_eq!(sc.histories[0].0, 101);
    }

    #[test]
    fn every_bad_shape_is_fixture_bad() {
        // 每一条拒绝路径各一例（规约 testing.md 第一节第 3 小节列的情形）
        let cases: &[(&str, &[(&str, &str)])] = &[
            ("缺 me.json", &[("chats.jsonl", CHAT)]),
            ("me.json 不是 JSON", &[("me.json", "{")]),
            ("me.json 不是 user", &[("me.json", CHAT)]),
            (
                "me.json 没有整数 id",
                &[("me.json", r#"{"@type":"user","id":"9"}"#)],
            ),
            (
                "users 一行不是对象",
                &[("me.json", ME), ("users.jsonl", "[1]")],
            ),
            (
                "users 一行不是 user",
                &[("me.json", ME), ("users.jsonl", CHAT)],
            ),
            (
                "chats 一行坏 JSON",
                &[("me.json", ME), ("chats.jsonl", "{\"@type\"")],
            ),
            (
                "chats 一行不是 chat",
                &[("me.json", ME), ("chats.jsonl", ME)],
            ),
            (
                "history 文件名不是整数",
                &[("me.json", ME), ("history/abc.jsonl", MSG)],
            ),
            (
                "history 一行不是 message",
                &[("me.json", ME), ("history/101.jsonl", CHAT)],
            ),
            (
                "消息的 chat_id 和文件名不符",
                &[("me.json", ME), ("history/102.jsonl", MSG)],
            ),
        ];
        for (what, files) in cases {
            let d = dir("sc-bad");
            for (rel, body) in *files {
                put(d.path(), rel, body);
            }
            assert!(is_bad(Scenario::load(d.path())), "{what} 应报 fixture_bad");
        }
    }

    #[test]
    fn names_and_paths() {
        assert!(is_name("basic-3-chats") && is_name("empty"));
        for not_name in ["", "./basic", "Basic", "a_b", "a b", "/tmp/x", "场景"] {
            assert!(!is_name(not_name), "{not_name}");
        }
        // 不是名字的原样当路径
        assert_eq!(
            Scenario::resolve(OsStr::new("./myscene")).ok(),
            Some(PathBuf::from("./myscene"))
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn name_resolves_under_this_crate_in_debug_builds() {
        let Ok(p) = Scenario::resolve(OsStr::new("empty")) else {
            panic!("开发构建里名字该能解析");
        };
        assert!(p.ends_with("fixtures/scenarios/empty"));
    }
}
