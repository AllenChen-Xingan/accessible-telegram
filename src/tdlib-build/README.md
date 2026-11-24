# src/tdlib-build

- `build.sh`：按锁定的提交拉 TDLib 源码、编 `libtdjson`，维护常驻软链接（用法见文件头）
- `dev-link`：常驻软链接**相对家目录**的路径，唯一正本（#170）。`rpath.rs`、`build.sh`、`scripts/check-harden.sh` 三处都读它
- `rpath.rs`：`atg-cli`、`atg-core` 共用的 Cargo 构建脚本（两个包的 `Cargo.toml` 都写 `build = "../tdlib-build/rpath.rs"`）：开了 `tdlib` 特性时，往程序里写 rpath `$HOME/<dev-link>/lib`

## `dev-link` 的规则（三处读者都照这一条，不 trim）

- 整个文件 = 一行路径，后面可以有、也可以没有**一个**换行；此外不许有任何字符（不许第二行、不许末尾多一个空行、不许注释）
- 路径不能为空，不能以 `/` 开头，不能含 `..`，不能含任何空白（空格、tab、回车、换行）
- 不合规则：只有开了 `tdlib` 特性（在 macOS 上）时，构建脚本才查它，查出来不对就让 `cargo build` 失败；默认构建和 CI 不写 rpath，内容不对也照样编得过。**文件整个删掉**例外：构建脚本是在编译时把它读进来的（`include_str!`），所以默认构建也会编不过，这算明显的错，不另做兜底。另外，`build.sh` 报错退出（退出码 1）；`check-harden.sh` 按准备失败退出（退出码 2）
- 改了它：重编带 `tdlib` 特性的程序（cargo 会自动重跑构建脚本），再跑 `bash src/tdlib-build/build.sh link` 把软链接挪过去
