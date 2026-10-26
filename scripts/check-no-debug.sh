#!/usr/bin/env bash
# 拍板 005 补丁 ③（docs/plan/005_decisions-b3.md）：禁止对 TDLib 对象用 `{:?}` 整个打印
# （会带出消息、姓名、手机号）。
#
# 两层拦：
#   1. clippy：每个 crate 根都有
#        #![cfg_attr(not(test), deny(clippy::use_debug, clippy::dbg_macro, clippy::unwrap_used, clippy::expect_used))]
#      实测（clippy 1.99）：use_debug 只管 print! / eprint! / println! / eprintln! / write! / writeln!，
#      **不管 format! / panic! / assert_eq! / 日志宏**；dbg_macro 管 dbg!；
#      unwrap_used / expect_used 管「失败时用 Debug 打出错误值」
#   2. 本脚本：clippy 管不到的——
#      a. 每个 crate 根确实带着第 1 层那行（防止有人删掉或新 crate 忘了加）
#      b. tdlib-rs 只许在 `atg-core` 的 `td` 模块里用（类型不渗出去，别处就没有 TDLib 对象可打印）
#      c. 日志宏的 `?字段` 写法（tracing / log 用 Debug 打印字段）
#      d. 非测试源码里：任何 Debug 占位符 `{:?}` `{:#?}` `{x:?}` `{0:>8?}`……；`assert_eq!` / `assert_ne!`
#         （失败时 Debug 打印两边）；`fn main() -> Result`（返回 Err 时 Debug 打印错误）
#         测试代码 = `#[cfg(test)]` 紧跟 `mod …` 起头的模块，约定放在文件末尾，从它起到文件尾不扫；
#         `tests/` 目录不扫。单独一个带 `#[cfg(test)]` 的函数或 use 不算，后面照扫
#
# 只扫本项目自己的 crate：`src/atg-*`。`src/` 下别的目录（如将来搬进来的 `src/tdlib-rs` 补丁库）
# 是上游代码，不归这条规矩管。
#
# 已知拦不到：`{}` 打印一个手写了「Display = Debug」的类型；宏生成的代码。审代码时人看。
#
# 用法：bash scripts/check-no-debug.sh    （在仓库任意位置跑都行；项目路径末尾有空格，全程加引号）
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
fail=0

# a. crate 根带 lint（rustfmt 会把那行折成多行，所以整文件压成一行再比）
for manifest in src/atg-*/Cargo.toml; do
  dir="$(dirname "$manifest")"
  for f in "$dir/src/lib.rs" "$dir/src/main.rs"; do
    [[ -f "$f" ]] || continue
    flat="$(grep -v '^[[:space:]]*//' "$f" | tr -d ' \n\t')"
    want='#![cfg_attr(not(test),deny(clippy::use_debug,clippy::dbg_macro,clippy::unwrap_used,clippy::expect_used))]'
    if [[ "$flat" != *"$want"* ]]; then
      echo "✗ ${f}：缺 #![cfg_attr(not(test), deny(clippy::use_debug, clippy::dbg_macro, clippy::unwrap_used, clippy::expect_used))]"
      fail=1
    fi
  done
done

# b. tdlib-rs 只在 atg-core 的 td 模块里出现（Cargo.toml 的依赖声明除外）
hits="$(grep -rnE '(^|[^A-Za-z0-9_])tdlib_rs([^A-Za-z0-9_]|$)' src/atg-* --include='*.rs' \
  | grep -vE '^src/atg-core/src/td(\.rs|/)' || true)"
if [[ -n "$hits" ]]; then
  echo "✗ tdlib-rs 只许在 src/atg-core/src/td 里用："
  echo "$hits"
  fail=1
fi
hits="$(grep -lE '^\s*tdlib-rs\s*=' src/atg-*/Cargo.toml | grep -v '^src/atg-core/' || true)"
if [[ -n "$hits" ]]; then
  echo "✗ 只有 atg-core 许依赖 tdlib-rs：${hits}"
  fail=1
fi

# c. 日志宏里的 `?字段`（如 `info!(?update)`、`debug!(x = ?msg)`）
hits="$(grep -rnE '(^|[^A-Za-z0-9_])(trace|debug|info|warn|error|event|span|trace_span|debug_span|info_span|warn_span|error_span)!\(.*([(,=]|^)\s*\?[A-Za-z_]' \
  src/atg-* --include='*.rs' || true)"
if [[ -n "$hits" ]]; then
  echo "✗ 日志宏里用了 ?字段（Debug 打印）："
  echo "$hits"
  fail=1
fi

# d. 非测试源码里的 Debug 占位符 / assert_eq! / main 返回 Result
hits="$(find src/atg-* -path '*/src/*' -name '*.rs' -print0 | xargs -0 awk '
  FNR == 1 { skip = 0; pend = 0 }
  skip { next }
  /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*(pub[[:space:]]+)?mod[[:space:]]/ { skip = 1; next }
  /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/ { pend = 1; next }
  pend && /^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]/ { skip = 1; next }
  { pend = 0 }
  /^[[:space:]]*\/\// { next }
  /\{[^{}]*\?\}/ { print FILENAME ":" FNR ": " $0; next }
  /(^|[^A-Za-z0-9_])(debug_)?assert_(eq|ne)!/ { print FILENAME ":" FNR ": " $0; next }
  /fn[[:space:]]+main[[:space:]]*\([^)]*\)[[:space:]]*->[[:space:]]*Result/ { print FILENAME ":" FNR ": " $0 }
' || true)"
if [[ -n "$hits" ]]; then
  echo "✗ 非测试代码里有会走 Debug 打印的写法（{:?} 一类 / assert_eq! / main 返回 Result）："
  echo "$hits"
  fail=1
fi

if [[ $fail -eq 0 ]]; then
  echo "✓ check-no-debug：a crate 根 lint 齐 · b tdlib-rs 没出 td 模块 · c 日志宏无 ?字段 · d 非测试代码无 {:?} / assert_eq! / main 返回 Result（unwrap、expect、print 系由 clippy 拦）"
fi
exit $fail
