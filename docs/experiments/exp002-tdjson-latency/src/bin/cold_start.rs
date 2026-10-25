//! 冷启动：反复拉起 ui_bench --cold 和 td_bench --cold，从「父进程发起 spawn」量到各个里程碑。
//!
//!   cold_start [--runs N] [--items N]     （默认 20 次、50 项）
//!
//! 时间用 Unix 纪元纳秒跨进程对齐（同一台机器同一个时钟）。
//! 注意：第一次运行时磁盘缓存可能是冷的，单独列出；之后是「热缓存冷启动」（进程新起，文件已在内存）。

#[path = "../common.rs"]
mod common;

use common::*;
use std::collections::HashMap;
use std::process::Command;

fn parse_cold(line: &str) -> HashMap<String, u128> {
    line.trim_start_matches("COLD ")
        .split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .filter_map(|(k, v)| v.parse().ok().map(|v| (k.to_string(), v)))
        .collect()
}

fn run(bin: &std::path::Path, args: &[&str]) -> (u128, HashMap<String, u128>) {
    let t_spawn = epoch_ns();
    let out = Command::new(bin).args(args).output().expect("spawn failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("COLD "))
        .unwrap_or_else(|| panic!("no COLD line from {bin:?}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (t_spawn, parse_cold(line))
}

fn main() {
    let mut runs = 20usize;
    let mut items = "50".to_string();
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        match v[i].as_str() {
            "--runs" => {
                i += 1;
                runs = v[i].parse().unwrap()
            }
            "--items" => {
                i += 1;
                items = v[i].clone()
            }
            o => panic!("unknown arg {o}"),
        }
        i += 1;
    }
    let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let ui = dir.join("ui_bench");
    let td = dir.join("td_bench");

    // ---- TDLib 单独 ----
    let mut td_rows: Vec<(u128, HashMap<String, u128>)> = vec![];
    for _ in 0..runs {
        td_rows.push(run(&td, &["--cold"]));
    }
    let col = |rows: &[(u128, HashMap<String, u128>)], from: &str, to: &str| -> Vec<f64> {
        rows.iter()
            .map(|(spawn, m)| {
                let a = if from == "spawn" { *spawn } else { m[from] };
                (m[to] as f64 - a as f64) / 1e6
            })
            .collect()
    };
    println!("## TDLib-only process (td_bench --cold), runs={runs}");
    println!("# first run (maybe cold disk cache): spawn→first response {:.1}ms",
        col(&td_rows[..1], "spawn", "td_first_response")[0]);
    let w = &td_rows[1..];
    report("spawn → main()", &col(w, "spawn", "td_main"));
    report("main → TDLib client first response", &col(w, "td_main", "td_first_response"));
    report("main → updateAuthorizationState", &col(w, "td_main", "td_auth_state"));
    report("spawn → TDLib first response", &col(w, "spawn", "td_first_response"));

    // ---- GPUI 窗口 + 无障碍树 + 并行拉起 TDLib ----
    let mut ui_rows: Vec<(u128, HashMap<String, u128>)> = vec![];
    for _ in 0..runs {
        ui_rows.push(run(&ui, &["--cold", "--items", &items]));
    }
    println!("\n## GPUI window + a11y tree + TDLib in parallel (ui_bench --cold --items {items}), runs={runs}");
    println!("# first run (maybe cold disk cache): spawn→ax_ready {:.1}ms",
        col(&ui_rows[..1], "spawn", "ax_ready")[0]);
    let w = &ui_rows[1..];
    report("spawn → main()", &col(w, "spawn", "ui_main"));
    report("main → window opened", &col(w, "ui_main", "window_opened"));
    report("main → first render()", &col(w, "ui_main", "first_render"));
    report("main → first frame presented", &col(w, "ui_main", "first_present"));
    report("first present → a11y tree has list", &col(w, "first_present", "ax_ready"));
    report("spawn → first frame presented", &col(w, "spawn", "first_present"));
    report("spawn → a11y tree has list", &col(w, "spawn", "ax_ready"));
    report("spawn → TDLib first response", &col(w, "spawn", "td_ready"));
    let polls: Vec<f64> = w.iter().map(|(_, m)| m["ax_polls"] as f64).collect();
    println!("# AX polls until ready: median {}", median(&polls));
}
