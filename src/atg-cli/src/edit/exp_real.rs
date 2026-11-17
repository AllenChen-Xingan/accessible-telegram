//! 实测（#8 B）：真后端、正式档案 `main` 的**收藏夹**、直发，内容是编的「atg 实验 编辑 N」。收藏夹里的消息不限时可改，
//! 编辑只对自己（`CLAUDE.md` 第五节：测试只发收藏夹）。
//!
//! 每轮：发一条「atg 实验 编辑 N」到收藏夹、等发出 → 按编辑流程过闸、改成「atg 实验 编辑 N 改后」→ 看应答里的正文与
//! 编辑时间 → 再取这一条，核对服务器上的正文。只打印统计（每轮成没成、耗时），不打印别的消息。
//!
//! 默认不跑：`#[ignore]`，且要设 `ATG_EXP_EDIT=1`。跑法（要重活名额、要大总管回「可以」）：
//! `ATG_EXP_EDIT=1 cargo test -p atg-cli --features tdlib exp_edit -- --ignored --nocapture`

use super::*;
use crate::args::Cli;
use atg_core::link::block_on;
use atg_core::model::{Body, MsgId};
use atg_core::send::{self, Draft, Waited};
use clap::Parser;

const ROUNDS: u32 = 3;

fn plain(m: &atg_core::model::Message) -> Option<String> {
    match &m.content {
        Body::Content(c) if c.attachment.is_none() => {
            Some(atg_core::tree::text::plain_text(&c.body))
        }
        _ => None,
    }
}

#[test]
#[ignore]
fn exp_edit() {
    if std::env::var("ATG_EXP_EDIT").as_deref() != Ok("1") {
        eprintln!("没设 ATG_EXP_EDIT=1，跳过");
        return;
    }
    let cli = Cli::try_parse_from(["atg-cli", "pending"]).unwrap();
    let g = cli.global;
    assert!(!g.test_dc && g.profile == "main");
    let profile = crate::session::open_profile(&g).unwrap_or_else(|_| panic!("档案打不开"));
    let b = crate::session::td_backend().unwrap_or_else(|_| panic!("后端起不来"));
    let r = history::with_backend(&b, &profile, Instant::now(), |b, stream, feed| {
        chats::ensure_ready(b)?;
        let saved = crate::send::resolve(b, history::ChatRef::Saved)?;
        let mut ok = 0;
        for n in 1..=ROUNDS {
            let draft = Draft {
                text: format!("atg 实验 编辑 {n}"),
                reply_to: None,
            };
            let plan = block_on(send::prepare_text(b, saved, &draft, SendOrigin::Cli))
                .map_err(Fail::Core)?;
            // 只许发收藏夹（`CLAUDE.md` 第五节）
            assert!(plan.target().saved, "目标不是收藏夹，停");
            let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
            let (_, _, w) =
                send::send_and_wait(b, &plan, token, stream, send::RESULT_TIMEOUT, &mut |env| {
                    feed.apply(env)
                })
                .map_err(Fail::Core)?;
            // 前置发送没成：实测不成立，判失败（codex 031 #1）
            let Waited::Succeeded(sent) = w else {
                panic!("第 {n} 轮：收藏夹那条没发出，实测不成立");
            };
            let id = sent.id.message_id;
            let new = format!("atg 实验 编辑 {n} 改后");
            let t0 = Instant::now();
            let ep = block_on(edit::prepare_edit(b, saved, id, &new, SendOrigin::Cli))
                .map_err(Fail::Core)?;
            assert!(ep.target().saved, "目标不是收藏夹，停");
            let tk = b.gate().issue(&ep, false).map_err(Fail::Core)?;
            let (_, m) = block_on(edit::edit_text(b, &ep, tk)).map_err(Fail::Core)?;
            let ms = t0.elapsed().as_millis();
            let again = block_on(b.message_get(MsgId {
                chat_id: saved,
                message_id: id,
            }))
            .map_err(Fail::Core)?;
            let good = plain(&m).as_deref() == Some(new.as_str())
                && m.edited.is_some()
                && plain(&again).as_deref() == Some(new.as_str());
            ok += u32::from(good);
            eprintln!("第 {n} 轮：准备到改完 {ms} 毫秒；应答与重取都是新文、带编辑时间：{good}");
            assert!(good, "第 {n} 轮没改成");
        }
        eprintln!("改成 {ok} 轮（共 {ROUNDS} 轮）");
        assert!(ok == ROUNDS, "不是每轮都改成：{ok}/{ROUNDS}");
        Ok(())
    });
    if let Err(e) = r {
        let code = match &e {
            Fail::Core(atg_core::error::CoreError::Internal { what }) => format!("internal {what}"),
            Fail::Core(c) => c.code().to_string(),
            Fail::Usage(u) => format!("usage: {u}"),
            _ => "other".to_owned(),
        };
        panic!("实测中途出错：{code}");
    }
}
