# src/tdlib-build

- `build.sh`：按锁定的提交拉 TDLib 源码、编 `libtdjson`，维护常驻软链接（用法见文件头）
- `dev-link`：常驻软链接**相对家目录**的路径，唯一正本（#170）。`rpath.rs`、`build.sh`、`scripts/check-harden.sh` 三处都读它
- `rpath.rs`：`atg-cli`、`atg-core` 共用的 Cargo 构建脚本（两个包的 `Cargo.toml` 都写 `build = "../tdlib-build/rpath.rs"`）：开了 `tdlib` 特性时，往程序里写 rpath `$HOME/<dev-link>/lib`

## `dev-link` 的规则（三处读者都照这一条，不 trim）

- 整个文件 = 一行路径，后面可以有、也可以没有**一个**换行；此外不许有任何字符（不许第二行、不许末尾多一个空行、不许注释）
- 路径不能为空，字符只许 ASCII 字母、数字和 `.` `_` `/` `-`（白名单；按 C locale 判，不随系统语言变），不能含 `..`；按 `/` 分开的每一段都不能为空、不能是 `.`（所以不能以 `/` 开头或结尾，不能有 `//`）
  - 拒末尾的 `/`：`ln` 建软链接和 check-harden 拼 `…/lib` 时，带不带末尾 `/` 结果不一样
  - 所以空白（含不换行空格这类非 ASCII 空白）、回车、换行、逗号都不行。逗号也拒，是因为链接参数 `-Wl,-rpath,…` 会把逗号当成分隔符
- 不合规则：只有开了 `tdlib` 特性（在 macOS 上）时，构建脚本才查它，查出来不对就让 `cargo build` 失败；默认构建和 CI 不写 rpath，内容不对也照样编得过。**文件整个删掉**例外：构建脚本是在编译时把它读进来的（`include_bytes!`），所以默认构建也会编不过，这算明显的错，不另做兜底。内容不是 UTF-8（比如被存成 UTF-16）不算例外：按字节读进来，开了 `tdlib` 特性才解码、校验，默认构建照样编过另外，`build.sh` 报错退出（退出码 1）；`check-harden.sh` 按准备失败退出（退出码 2）
- 改了它：重编带 `tdlib` 特性的程序（cargo 会自动重跑构建脚本），再跑 `bash src/tdlib-build/build.sh link` 把软链接挪过去
