//! TDLib 段：不登录、不联网，只用 TDLib 的离线测试方法量 tdjson 往返和吞吐。
//!
//!   td_bench           正常跑全部测量
//!   td_bench --cold    冷启动模式：只量「进程 main → TDLib 客户端第一次应答」，打印一行 COLD 后退出
//!
//! test* 方法在 setTdlibParameters 之前就能调（TDLib Td.cpp:190-209），所以不需要 api_id、不需要账号。

#[path = "../common.rs"]
mod common;

use common::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const WARMUP: usize = 200;
const N: usize = 5000;
const BURST: usize = 20000;
const BURST_ROUNDS: usize = 5;

fn main() {
    let t_main = epoch_ns();
    if std::env::args().any(|a| a == "--cold") {
        cold(t_main);
        return;
    }

    quiet_tdlib();
    let version = raw_execute(r#"{"@type":"getOption","name":"version"}"#).unwrap_or_default();
    println!("# TDLib version: {version}");

    let payload_1k: String = "消息".repeat(170); // 约 1 KB UTF-8，和一条普通文字消息的 JSON 差不多大
    let payload_json = serde_json::to_string(&payload_1k).unwrap();
    let call_string = format!(r#"{{"@type":"testCallString","x":{payload_json}}}"#);

    let client = tdlib_rs::create_client();
    // 第一发让 TDLib 起线程、吐初始更新
    let t = Instant::now();
    let (_, init_updates) = raw_roundtrip(client, r#"{"@type":"testCallEmpty"}"#, 0);
    println!(
        "# first request on fresh client: {:.3}ms, initial updates drained: {init_updates}",
        ms(t.elapsed())
    );

    // ---- 1. 直连 tdjson：同一线程 send → receive 往返 ----
    println!("\n## A. raw tdjson round trip (td_send → td_receive, same thread, incl. JSON parse to Value)");
    let mut extra = 1u64;
    for (name, req) in [
        ("testCallEmpty", r#"{"@type":"testCallEmpty"}"#.to_string()),
        ("testSquareInt", r#"{"@type":"testSquareInt","x":12345}"#.to_string()),
        ("testCallString(~1KB)", call_string.clone()),
    ] {
        let mut s = Vec::with_capacity(N);
        for i in 0..WARMUP + N {
            let t = Instant::now();
            let _ = raw_roundtrip(client, &req, extra);
            extra += 1;
            if i >= WARMUP {
                s.push(ms(t.elapsed()));
            }
        }
        report(&format!("raw {name}"), &s);
    }

    // ---- 2. 直连 tdjson：一口气发 BURST 个再全收回，看每秒能过多少条 ----
    println!("\n## B. raw tdjson burst throughput ({BURST} x testCallString ~1KB, send all then receive all)");
    let mut rates = vec![];
    for _ in 0..BURST_ROUNDS {
        let t = Instant::now();
        let body = &call_string[..call_string.len() - 1];
        for _ in 0..BURST {
            raw_send(client, &format!("{body},\"@extra\":{extra}}}"));
            extra += 1;
        }
        let mut got = 0;
        while got < BURST {
            let resp = raw_receive(5.0).expect("burst: TDLib 没回应");
            let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
            if v.get("@extra").is_some() {
                got += 1;
            }
        }
        let secs = t.elapsed().as_secs_f64();
        let rate = BURST as f64 / secs;
        println!("  round: {:.1}ms total, {:.0} responses/s", secs * 1000.0, rate);
        rates.push(rate);
    }
    println!("raw burst median: {:.0} responses/s", median(&rates));

    // ---- 3. 经 tdlib-rs：后台收线程 + async 请求（就是以后真实用法） ----
    println!("\n## C. via tdlib-rs (background receive thread + async fn, incl. serde into typed structs)");
    let running = Arc::new(AtomicBool::new(true));
    let recv_thread = {
        let running = running.clone();
        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let _ = tdlib_rs::receive();
            }
        })
    };
    {
        let mut s = Vec::with_capacity(N);
        for i in 0..WARMUP + N {
            let t = Instant::now();
            futures::executor::block_on(tdlib_rs::functions::test_call_empty(client)).unwrap();
            if i >= WARMUP {
                s.push(ms(t.elapsed()));
            }
        }
        report("tdlib-rs test_call_empty", &s);

        let mut s = Vec::with_capacity(N);
        for i in 0..WARMUP + N {
            let t = Instant::now();
            futures::executor::block_on(tdlib_rs::functions::test_call_string(
                payload_1k.clone(),
                client,
            ))
            .unwrap();
            if i >= WARMUP {
                s.push(ms(t.elapsed()));
            }
        }
        report("tdlib-rs test_call_string(~1KB)", &s);
    }
    {
        let mut rates = vec![];
        for _ in 0..BURST_ROUNDS {
            let t = Instant::now();
            let futs: Vec<_> = (0..BURST)
                .map(|_| tdlib_rs::functions::test_call_string(payload_1k.clone(), client))
                .collect();
            let res = futures::executor::block_on(futures::future::join_all(futs));
            assert!(res.iter().all(|r| r.is_ok()));
            let secs = t.elapsed().as_secs_f64();
            println!("  round: {:.1}ms total, {:.0} responses/s", secs * 1000.0, BURST as f64 / secs);
            rates.push(BURST as f64 / secs);
        }
        println!("tdlib-rs burst median: {:.0} responses/s", median(&rates));
    }
    running.store(false, Ordering::Relaxed);

    // ---- 4. 纯 CPU：把一条 updateNewMessage 的 JSON 反序列化成 tdlib-rs 类型 ----
    println!("\n## D. deserialize one updateNewMessage JSON into tdlib_rs::enums::Update (same path as tdlib_rs::receive)");
    let sample = sample_update_new_message(1 << 20, SAMPLE_TEXT);
    println!("# sample JSON size: {} bytes", sample.len());
    // 先确认能解出来
    let v: serde_json::Value = serde_json::from_str(&sample).unwrap();
    match serde_json::from_value::<tdlib_rs::enums::Update>(v) {
        Ok(tdlib_rs::enums::Update::NewMessage(_)) => println!("# sample parses as Update::NewMessage ✓"),
        Ok(other) => println!("# sample parsed as unexpected variant: {other:?}"),
        Err(e) => {
            println!("# sample FAILED to parse: {e}");
            return;
        }
    }
    let mut s = Vec::with_capacity(20000);
    for i in 0..1000 + 20000 {
        let t = Instant::now();
        let v: serde_json::Value = serde_json::from_str(&sample).unwrap();
        let u: tdlib_rs::enums::Update = serde_json::from_value(v).unwrap();
        std::hint::black_box(u);
        if i >= 1000 {
            s.push(ms(t.elapsed()));
        }
    }
    report("parse updateNewMessage (str→Value→Update)", &s);
    println!("=> single-thread decode capacity ≈ {:.0} updates/s", 1000.0 / median(&s));

    // 收线程最多 2 秒自己退出（tdlib_rs::receive 超时 2s）
    let _ = recv_thread.join();
}

/// 冷启动：main → 客户端第一次应答；顺带记第一条 updateAuthorizationState 到达时刻
fn cold(t_main: u128) {
    quiet_tdlib();
    let t_quiet = epoch_ns();
    let client = tdlib_rs::create_client();
    raw_send(client, r#"{"@type":"testCallEmpty","@extra":1}"#);
    let mut t_auth = 0u128;
    let t_resp;
    loop {
        let r = raw_receive(10.0).expect("cold: TDLib 没回应");
        let now = epoch_ns();
        if t_auth == 0 && r.contains("updateAuthorizationState") {
            t_auth = now;
        }
        if r.contains("\"@extra\":1") {
            t_resp = now;
            break;
        }
    }
    println!("COLD td_main={t_main} td_lib_ready={t_quiet} td_first_response={t_resp} td_auth_state={t_auth}");
}
