//! 实测（#57，M2.1 验收项「`pending_cancel` 的实际效果」）：真后端、正式档案 `main` 的**收藏夹**、直发模式，内容是编的
//! 「atg 实验 待发撤销 N」。大总管同意 2026-11-14（测试服务器登录不通 #35，改用正式档案收藏夹 + 直发）。
//!
//! 每轮：交出一条（不等结果）→ 拿到临时 id 立刻按撤销流程过闸、交出 `deleteMessages` → 5 秒内看它的下落
//! （`messages_deleted` = 撤下 / `send_succeeded` = 没赶上）→ 再等 3 秒，取收藏夹最近 20 条，数服务器上有没有这句话。
//! 只打印统计（每轮的下落、服务器上留下几条），不打印别的消息。
//!
//! 默认不跑：`#[ignore]`，且要设 `ATG_EXP_PENDING_CANCEL=1`。跑法（要重活名额、要大总管回「可以」）：
//! `ATG_EXP_PENDING_CANCEL=1 cargo test -p atg-cli --features tdlib exp_pending_cancel -- --ignored --nocapture`

use super::*;
use crate::args::Cli;
use atg_core::backend::HistoryReq;
use atg_core::delete;
use atg_core::link::block_on;
use atg_core::model::Body;
use atg_core::send::{self, Draft};
use clap::Parser;

const ROUNDS: u32 = 3;

#[test]
#[ignore]
fn exp_pending_cancel() {
    if std::env::var("ATG_EXP_PENDING_CANCEL").as_deref() != Ok("1") {
        eprintln!("没设 ATG_EXP_PENDING_CANCEL=1，跳过");
        return;
    }
    let cli = Cli::try_parse_from(["atg-cli", "--unsafe-direct-send", "pending"]).unwrap();
    let g = cli.global;
    assert!(!g.test_dc && g.profile == "main");
    let profile = crate::session::open_profile(&g).unwrap_or_else(|_| panic!("档案打不开"));
    let b = crate::session::td_backend().unwrap_or_else(|_| panic!("后端起不来"));
    let r = history::with_backend(&b, &profile, Instant::now(), |b, stream, feed| {
        chats::ensure_ready(b)?;
        let saved = crate::send::resolve(b, history::ChatRef::Saved)?;
        let mut texts = Vec::new();
        for n in 1..=ROUNDS {
            let text = format!("atg 实验 待发撤销 {n}");
            texts.push(text.clone());
            let draft = Draft {
                text,
                reply_to: None,
            };
            let plan = block_on(send::prepare_text(b, saved, &draft, SendOrigin::Cli))
                .map_err(Fail::Core)?;
            // 只许发收藏夹（`CLAUDE.md` 第五节）：计划的目标必须是自己的收藏夹
            assert!(plan.target().saved, "目标不是收藏夹，停");
            let token = b.gate().issue(&plan, false).map_err(Fail::Core)?;
            let sub = block_on(send::send_text(b, &plan, token)).map_err(Fail::Core)?;
            let temp = sub.temp();
            let t0 = Instant::now();
            let cancel = block_on(delete::prepare_cancel(
                b,
                saved,
                &[temp.message_id],
                SendOrigin::Cli,
            ));
            let cancel_result = match cancel {
                Err(e) => format!("准备时就拒了：{}", e.code()),
                Ok(cp) => {
                    let tk = b.gate().issue(&cp, false).map_err(Fail::Core)?;
                    match block_on(delete::cancel(b, &cp, tk)) {
                        Ok(_) => "已交出删除".to_owned(),
                        Err(e) => format!("交出时拒了：{}", e.code()),
                    }
                }
            };
            let ms = t0.elapsed().as_millis();
            let w = send::wait(stream, &[temp], CANCEL_WAIT, &mut |env| feed.apply(env));
            let outcome = match &w[0] {
                Waited::Unknown(atg_core::error::UnknownWhy::PendingDeleted) => "cancelled",
                Waited::Succeeded(_) => "already_sent",
                Waited::Failed { .. } => "failed",
                Waited::Unknown(_) => "unknown",
            };
            send::finish(b, &plan, sub, &w[0]);
            eprintln!("第 {n} 轮：发出到交出删除 {ms} 毫秒；{cancel_result}；下落 {outcome}");
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
        let page = block_on(b.history(HistoryReq {
            chat: saved,
            from: 0,
            offset: 0,
            limit: 20,
            local_only: false,
        }))
        .map_err(Fail::Core)?;
        let left = page
            .iter()
            .filter(|m| match m.effective() {
                Body::Content(c) => texts.contains(&atg_core::tree::text::plain_text(&c.body)),
                _ => false,
            })
            .count();
        eprintln!("收藏夹最近 20 条里留着的实验消息：{left} 条（共发 {ROUNDS} 条）");
        Ok(())
    });
    assert!(r.is_ok(), "实测中途出错");
}
