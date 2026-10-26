#!/bin/bash
# dev-sign-run.sh — cargo 的 runner：先给编出来的程序签名，再运行它（.cargo/config.toml 里设）。
#
# 为什么（issue #38，exp005「乙本体」第 2 条）：免得忘签。cargo 运行程序前先调 runner，
#   所以 `cargo run` / `cargo test` 直接运行的程序都会先经 scripts/dev-sign.sh 签好再跑。
#   集成测试里自己再起的二进制（CARGO_BIN_EXE_*）不经 runner、不签名；要碰钥匙串的得自己签。
# ⚠️ 本脚本由系统 shell 执行，macOS 启动系统程序时会删掉所有 DYLD_* 变量，DYLD_LIBRARY_PATH 传不进来。
#   所以 TDLib 库目录改用 ATG_TDLIB_LIB 传；没设就依次用 "$LOCAL_TDLIB_PATH/lib"（编译时链接的那份，变量名不带
#   DYLD_ 不会被删）、`src/tdlib-build/build.sh path` 打印的「最近编好的 TDLib 前缀/lib」。都没有就不设，
#   不带 TDLib 的程序照常跑；带 TDLib 的程序会在标准错误提示一句。exec 的被测程序不是系统程序，变量能传到。
#   同理 cargo 自己设的 DYLD_FALLBACK_LIBRARY_PATH 也到不了被测程序（本项目没有要它的动态库）。
# 不签名的情形：设了 ATG_DEV_SIGN=0，或在 CI 里（环境变量 CI 非空；CI 没有开发身份，测试也不碰钥匙串）。
# 只挂在 Apple 芯片本机目标（aarch64-apple-darwin）上：Intel Mac 或 --target 别的平台不经本脚本，要手动 dev-sign.sh。
# 用法：cargo 自动调，形如 dev-sign-run.sh <程序> [参数…]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "${here}/.." && pwd)"

[[ $# -ge 1 ]] || { echo "用法：dev-sign-run.sh <程序> [参数…]" >&2; exit 2; }

if [[ "${ATG_DEV_SIGN:-1}" != 0 && -z "${CI:-}" ]]; then
  if ! bash "${here}/dev-sign.sh" "$1"; then
    echo "dev-sign-run: 没签成，不运行。只跑单测、本机暂时没有开发身份时可设 ATG_DEV_SIGN=0 跳过签名（单测不碰钥匙串）。" >&2
    exit 2
  fi
fi

if [[ -z "${ATG_TDLIB_LIB:-}" && -n "${LOCAL_TDLIB_PATH:-}" ]]; then
  ATG_TDLIB_LIB="${LOCAL_TDLIB_PATH}/lib"
fi
if [[ -z "${ATG_TDLIB_LIB:-}" ]]; then
  if prefix="$(bash "${root}/src/tdlib-build/build.sh" path 2>/dev/null)"; then
    ATG_TDLIB_LIB="${prefix}/lib"
  elif otool -L "$1" 2>/dev/null | grep -q libtdjson; then
    echo "dev-sign-run: 这个程序要 libtdjson，但找不到 TDLib 库目录（ATG_TDLIB_LIB、LOCAL_TDLIB_PATH 都没设，本仓库 _build 下也没有编好的）。" >&2
  fi
fi
if [[ -n "${ATG_TDLIB_LIB:-}" ]]; then
  export ATG_TDLIB_LIB
  export DYLD_LIBRARY_PATH="${ATG_TDLIB_LIB}"
fi

exec "$@"
