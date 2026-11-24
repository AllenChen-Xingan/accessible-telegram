//! 打印界面自描述（JSON）。正本与更新步骤见 `atg_gui_spec::schema`。

// 与 atg-core 同一行（拍板 005 补丁 ③，`scripts/check-no-debug.sh` 检查每个 crate 根都带着）
#![cfg_attr(
    not(test),
    deny(
        clippy::use_debug,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::expect_used
    )
)]

use std::io::Write;

fn main() {
    let out = atg_gui_spec::schema::render();
    // 管道被对端关了（如 `| head`）就安静退出
    let _ = std::io::stdout().write_all(out.as_bytes());
}
