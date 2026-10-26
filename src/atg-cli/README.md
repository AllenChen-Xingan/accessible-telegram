# atg-cli

无障碍 Telegram 的命令行入口，也是 AI 和脚本的首选接口：标准输出一行一个 JSON（外壳、结尾行、退出码照 `specs/contracts/cli.md`），给人看加 `--format text`。依赖 `atg-core`，不依赖 GPUI。**现在是骨架**：`version`（或 `--version`）和 `--help` 能用，契约里的其余子命令都已占名，执行时输出一行 `kind:"error"`、`code:"not_implemented"` 并以退出码 1 退出；还没接 TDLib。跑法：`cargo run -p atg-cli -- version`，或 `cargo run -p atg-cli -- --format text chats`；测试 `cargo test -p atg-cli`（跑真二进制，对照契约检查外壳与退出码）。
