//! 打印界面自描述（JSON）。正本与更新步骤见 `atg_gui_spec::schema`。
//! `--md-elements`：改打印契约表的 Markdown 段（贴进 `specs/gui/a11y-tree.md` 第三节，替换起止标记之间的那段）

// 与 atg-core 同一行（拍板 005 补丁 ③ `docs/plan/005_decisions-b3.md`，`scripts/check-no-debug.sh` 检查每个 crate 根都带着）
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
    let out = if std::env::args().nth(1).as_deref() == Some("--md-elements") {
        atg_gui_spec::schema::elements_md()
    } else {
        atg_gui_spec::schema::render()
    };
    // 管道被对端关了（如 `| head`）就安静退出；别的写错误（磁盘满……）要报出来、非零退出，
    // 免得重定向生成金样时留下半截文件却像成功了
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(out.as_bytes())
        .and_then(|()| stdout.flush())
        && e.kind() != std::io::ErrorKind::BrokenPipe
    {
        eprintln!("gui-schema：写标准输出失败：{e}");
        std::process::exit(1);
    }
}
