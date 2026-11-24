#!/usr/bin/env bash
# dev-sign.sh — 用本人的 Apple Development 身份、固定标识 atg.dev 给开发版程序签名（只签，不运行）。
#
# 为什么（issue #38，exp005「乙本体」第 1–2 条）：读本地库钥匙要过钥匙串两道关——访问名单认「标识 + 证书」，
#   分区名单认团队号 `teamid:…`。用同一身份、同一标识签，重编多少次都不弹框。
#   `cargo build` 的产物是临时签名，读钥匙必弹，所以跑之前一定要签。
# 谁调：`cargo run` / `cargo test` 经 runner（scripts/dev-sign-run.sh，见 .cargo/config.toml）自动调；
#       只 `cargo build` 时手动调：bash scripts/dev-sign.sh target/debug/atg-cli [更多程序…]
# 身份怎么选：见 scripts/dev-codesign-id.sh（环境变量 / .env.local 的 ATG_CODESIGN_ID，或恰好一个 Apple Development）
# 强化运行时（#42）：默认开（`--options runtime`），挡住 `DYLD_INSERT_LIBRARIES` 注入——没开时，同用户任何进程都能把
#   代码注入这一版、借它的签名静默读钥匙（exp005 取证 9）。开了以后库验证生效、`DYLD_LIBRARY_PATH` 被忽略：
#   TDLib 改靠程序里的 rpath（~/.atg-dev/tdlib/lib，见 src/tdlib-build/build.sh）找，libtdjson 要用同一身份签过。
#   调试（lldb 挂不上强化运行时版）时临时设 ATG_DEV_HARDEN=0 签一版；不加 get-task-allow 权限（加了同用户就能挂上来，等于又开了注入的门）。
#   核：bash scripts/check-harden.sh
# 不做什么：不打印身份名。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

[[ $# -ge 1 ]] || { echo "用法：bash scripts/dev-sign.sh <程序> [程序…]" >&2; exit 2; }

id="$(bash "${here}/dev-codesign-id.sh")"

opts=(--options runtime)
if [[ "${ATG_DEV_HARDEN:-1}" == 0 ]]; then
  opts=()
  echo "dev-sign: ATG_DEV_HARDEN=0，这一版不开强化运行时（可被注入，只给调试用）" >&2
fi

for bin in "$@"; do
  [[ -f "${bin}" ]] || { echo "dev-sign: 没有这个文件：${bin}" >&2; exit 2; }
  # codesign 成功时不出声；失败时它的报错里可能带身份名，只留退出码和一句说明
  if ! codesign -s "${id}" -i atg.dev ${opts[@]+"${opts[@]}"} --force "${bin}" 2>/dev/null; then
    echo "dev-sign: 签名失败（身份 SHA-1 前 8 位 ${id:0:8}）：${bin}。手动看原因：codesign -s \"\$(bash scripts/dev-codesign-id.sh)\" -i atg.dev ${opts[*]+${opts[*]}} --force <程序>" >&2
    exit 2
  fi
done
