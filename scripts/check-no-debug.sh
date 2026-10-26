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
#         测试代码 = `#[cfg(test)]` 紧跟**内联**模块 `mod … {`（花括号在同一行）起头，约定放在文件末尾；
#         从它起**到这个模块的花括号配平为止**不扫（逐字符，同一行闭合之后照扫）；按 Rust 词法区分字符串、
#         字符字面量、行注释、块注释，注释里的内容不参与判定；`tests/` 目录不扫。单独一个带 `#[cfg(test)]` 的函数或 use 不算，后面照扫。
#         **`#[cfg(test)] mod X;` 这种声明不算**、不跳（它若写在文件顶部，会让后面整个文件逃过扫描；
#         测试模块放单独文件的，那个文件照扫）。脚本每次先自测这一条（见下「自测」）
#
# 只扫本项目自己的 crate：`src/atg-*`。`src/` 下别的目录（如将来搬进来的 `src/tdlib-rs` 补丁库）
# 是上游代码，不归这条规矩管。
#
# 已知拦不到：`{}` 打印一个手写了「Display = Debug」的类型；宏生成的代码。审代码时人看。
# 词法只做到够用：不认宏里自造的语法（如 `macro_rules!` 里不配对的花括号字面 token）；
# 那种写法放进测试模块会让跳过范围偏——测试模块按约定放文件末尾，偏也只偏在文件尾。
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
hits="$(grep -lE '^\s*tdlib-rs\s*=|package\s*=\s*"tdlib-rs"' src/atg-*/Cargo.toml | grep -v '^src/atg-core/' || true)"
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
#    按 Rust 词法逐字符过一遍（普通字符串含跨行、原始字符串 r#"…"#、字符字面量 vs 生命周期、行注释、
#    可嵌套的块注释），每行得到两份：「代码 + 字符串」（找 {:?}，它在格式字符串里）和「只有代码」
#    （数花括号、认测试模块、找 assert_eq! / main）。只有代码态里的 /* 才开块注释。
#    awk 一律在 LC_ALL=C 下按字节跑：分隔符都是 ASCII，UTF-8 的多字节字符不会被误认；
#    换成按字符的区域设置时，macOS 的 awk 遇到中文可能报 multibyte conversion failure
#    跳过规则：代码里「#[cfg(test)] [pub] mod X {」（同一行，或 #[cfg(test)] 单独一行、下一行 mod X {）
#    起头的内联模块，**逐字符跳到它的花括号配平为止**，同一行闭合之后的部分照扫；`mod X;` 声明不跳
scan_d='
  function lex(s, doskip,   i, n, c, c2, rest, m, h, k) {
    CS = ""; CO = ""; n = length(s); i = 1
    while (i <= n) {
      c = substr(s, i, 1); c2 = substr(s, i, 2)
      if (bc > 0) {
        if (c2 == "/*") { bc++; i += 2; continue }
        if (c2 == "*/") { bc--; i += 2; continue }
        i++; continue
      }
      if (rawh >= 0) {
        h = "\""; for (k = 0; k < rawh; k++) h = h "#"
        if (substr(s, i, length(h)) == h) { if (!sk) CS = CS h; rawh = -1; i += length(h); continue }
        if (!sk) CS = CS c; i++; continue
      }
      if (instr) {
        if (c == "\\") { if (!sk) CS = CS substr(s, i, 2); i += 2; continue }
        if (c == "\"") { instr = 0 }
        if (!sk) CS = CS c; i++; continue
      }
      if (c2 == "//") break
      if (c2 == "/*") { bc = 1; i += 2; continue }
      rest = substr(s, i)
      if (match(rest, /^b?r#*"/) && (i == 1 || substr(s, i - 1, 1) !~ /[A-Za-z0-9_]/)) {
        m = substr(rest, 1, RLENGTH); rawh = gsub(/#/, "#", m)
        if (!sk) { CS = CS m; CO = CO "\"\"" }
        i += RLENGTH; continue
      }
      if (c == "\"") { instr = 1; if (!sk) { CS = CS c; CO = CO "\"\"" }; i++; continue }
      if (c == "'"'"'" && match(rest, /^'"'"'(\\[^'"'"']+|[^'"'"'\\])'"'"'/)) {
        if (!sk) { CS = CS substr(rest, 1, RLENGTH); CO = CO "'"'"''"'"'" }
        i += RLENGTH; continue
      }
      if (sk) {
        if (c == "{") depth++
        if (c == "}") { depth--; if (depth <= 0) sk = 0 }
        i++; continue
      }
      CS = CS c; CO = CO c; i++
    }
  }
  function save() { s_bc = bc; s_instr = instr; s_rawh = rawh; s_sk = sk; s_depth = depth }
  function restore() { bc = s_bc; instr = s_instr; rawh = s_rawh; sk = s_sk; depth = s_depth }
  function hit(line) {
    if (CS ~ /\{[^{}]*\?\}/ || CO ~ /(^|[^A-Za-z0-9_])(debug_)?assert_(eq|ne)!/ \
        || CO ~ /fn[[:space:]]+main[[:space:]]*\([^)]*\)[[:space:]]*->[[:space:]]*Result/)
      print FILENAME ":" FNR ": " line
  }
  FNR == 1 { bc = 0; instr = 0; rawh = -1; sk = 0; depth = 0; pend = 0 }
  {
    save(); lex($0, 0)
    if (!s_sk && (CO ~ /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/ \
        || (pend && CO ~ /^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/))) {
      restore(); sk = 1; depth = 0; pend = 0; lex($0, 1); hit($0); next
    }
    pend = (!sk && CO ~ /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/)
    hit($0)
  }
'

# 自测：跳过规则本身不许漏（2026-10-25 曾因顶部的 `#[cfg(test)] mod tests;` 声明险些让整个 store 逃过扫描；
# codex 005 第十一轮又指出开头的空测试模块、块注释里的同样一行也会让后面全跳过）
selftest="$(mktemp -d)"
trap 'rm -rf "$selftest"' EXIT
# 必须报：
printf '%s\n' '#[cfg(test)]' 'mod tests;' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_decl.rs"
printf '%s\n' '#[cfg(test)] mod tests;' 'fn f(x: u8) -> String { format!("{x:?}") }' > "$selftest/must_decl_inline.rs"
printf '%s\n' '#[cfg(test)] mod tests {}' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_empty_mod_first.rs"
printf '%s\n' '/*' '#[cfg(test)] mod tests {' '*/' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_in_block_comment.rs"
printf '%s\n' '/* #[cfg(test)] mod tests { */' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_in_inline_comment.rs"
printf '%s\n' '#[cfg(test)]' 'mod tests {' '    fn g() -> &'"'"'static str { "{" }' '}' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_after_test_mod.rs"
printf '%s\n' 'const S: &str = "/*";' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_comment_start_in_string.rs"
printf '%s\n' '// 注释里写了 /* 但不是块注释' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_comment_start_in_line_comment.rs"
printf '%s\n' '#[cfg(test)] mod tests {} fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_after_close_same_line.rs"
printf '%s\n' '#[cfg(test)]' 'mod tests {' '    const R: &str = r#"}" " {"#;' '}' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_after_raw_string_mod.rs"
printf '%s\n' '#[cfg(test)]' 'mod tests {' '    fn g<'"'"'a>(x: &'"'"'a str) -> &'"'"'a str { x }' '}' 'fn f(x: u8) -> String { format!("{:?}", x) }' > "$selftest/must_after_lifetime_mod.rs"
# 必须不报：
printf '%s\n' 'fn f() {}' '#[cfg(test)]' 'mod tests {' '    fn g(x: u8) -> String { format!("{:?}", x) }' '}' > "$selftest/ok_inline.rs"
printf '%s\n' 'fn f() {}' '#[cfg(test)] mod tests {' '    fn g(x: u8) -> String { format!("{:?}", x) }' '    fn h() -> char { '"'"'}'"'"' }' '}' > "$selftest/ok_inline1.rs"
printf '%s\n' 'fn f() {}' '#[cfg(test)]' 'mod tests {' '    const S: &str = "/* {";' '    fn g(x: u8) -> String { format!("{:?}", x) }' '}' > "$selftest/ok_comment_like_string_in_mod.rs"
printf '%s\n' 'fn f() -> &'"'"'static str { "a\"b" }' '/* 块注释里 {:?} 不算' '   assert_eq!(1, 2) */' 'fn g() {}' > "$selftest/ok_block_comment.rs"
for f in "$selftest"/must_*.rs; do
  if ! out="$(LC_ALL=C awk "$scan_d" "$f" 2>&1)" || [[ -z "$out" ]]; then
    echo "✗ 自测失败：$(basename "$f") 里测试模块之外的 {:?} 逃过了扫描"
    fail=1
  fi
done
# 实扫用的是「一次喂多个文件」的调用：同样的调用方式对自测文件也要报得出来
must_files=("$selftest"/must_*.rs)
if ! out="$(LC_ALL=C awk "$scan_d" "${must_files[@]}" 2>&1)" \
    || [[ "$(grep -c '^' <<<"$out")" -lt "${#must_files[@]}" ]]; then
  echo "✗ 自测失败：多文件一起扫时漏报（实扫用的就是这种调用）"
  fail=1
fi
for f in "$selftest"/ok_*.rs; do
  if ! out="$(LC_ALL=C awk "$scan_d" "$f" 2>&1)" || [[ -n "$out" ]]; then
    echo "✗ 自测失败：$(basename "$f") 里测试模块被误扫"
    fail=1
  fi
done

files=()
while IFS= read -r -d '' f; do files+=("$f"); done < <(find src/atg-* -path '*/src/*' -name '*.rs' -print0)
# awk 自己出错（语法、区域设置……）必须让脚本失败，不能当成「没扫到问题」
if ! hits="$(LC_ALL=C awk "$scan_d" "${files[@]}")"; then
  echo "✗ 扫描本身出错（awk 失败），结果不可信"
  fail=1
fi
if [[ -n "$hits" ]]; then
  echo "✗ 非测试代码里有会走 Debug 打印的写法（{:?} 一类 / assert_eq! / main 返回 Result）："
  echo "$hits"
  fail=1
fi

if [[ $fail -eq 0 ]]; then
  echo "✓ check-no-debug：自测过 · a crate 根 lint 齐 · b tdlib-rs 没出 td 模块 · c 日志宏无 ?字段 · d 非测试代码无 {:?} / assert_eq! / main 返回 Result（unwrap、expect、print 系由 clippy 拦）"
fi
exit $fail
