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
#         **`#[cfg(test)] mod X;` 这种声明本身不跳**（它若写在文件顶部，会让后面整个文件逃过扫描）。
#         **它声明的那个文件整份不扫**（大总管定 2026-11-13，配合 `specs/architecture-rules.md` 第 1 条
#         「行内测试挪到 foo/tests.rs」）：只认代码态里 `#[cfg(test)]` 与 `[pub] mod X;` 紧挨着（同一行，或
#         `#[cfg(test)]` 单独一行、下一行就是声明，中间不许夹别的属性、注释行），且路径精确对应——
#         `foo.rs` 声明的是 `foo/X.rs` 或 `foo/X/mod.rs`，`lib.rs` / `main.rs` / `mod.rs` 声明的是同目录的 `X.rs` 或
#         `X/mod.rs`。测试再按主题拆的（`foo/tests.rs` 或 `foo/tests/mod.rs` 里 `mod serve;` → `foo/tests/serve.rs`），
#         只要它的上级已经是这样认下的测试文件，就跟着不扫（逐级往下，路径同样精确对应）。
#         没被这样声明的 `tests.rs`、`tests/<主题>.rs` 照扫。脚本每次先自测这几条（见下「自测」）
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
    if (decl) return
    if (CS ~ /\{[^{}]*\?\}/ || CO ~ /(^|[^A-Za-z0-9_])(debug_)?assert_(eq|ne)!/ \
        || CO ~ /fn[[:space:]]+main[[:space:]]*\([^)]*\)[[:space:]]*->[[:space:]]*Result/)
      print FILENAME ":" FNR ": " line
  }
  FNR == 1 { bc = 0; instr = 0; rawh = -1; sk = 0; depth = 0; pend = 0; bd = 0 }
  {
    save(); lex($0, 0)
    if (!s_sk && (CO ~ /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/ \
        || (pend && CO ~ /^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/))) {
      restore(); sk = 1; depth = 0; pend = 0; lex($0, 1); hit($0); next
    }
    # decl=1 时只列出 `mod X;` 声明：「文件<TAB>X<TAB>t」（t=1：紧挨着 #[cfg(test)]；0：没有）
    # 只认文件顶层（bd = 行首时代码里花括号的层数）：内联 `mod m { … }` 里的声明指向 m/ 下，路径另算，干脆照扫
    if (decl && !s_sk && bd == 0 && CO ~ /^[[:space:]]*(#\[cfg\(test\)\][[:space:]]*)?(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*;[[:space:]]*$/) {
      d = CO; sub(/^.*mod[[:space:]]+/, "", d); sub(/[[:space:]]*;.*$/, "", d)
      print FILENAME "\t" d "\t" ((pend || CO ~ /^[[:space:]]*#\[cfg\(test\)\]/) ? 1 : 0)
    }
    pend = (!sk && CO ~ /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/)
    tmp = CO; bd += gsub(/\{/, "", tmp); tmp = CO; bd -= gsub(/\}/, "", tmp)
    hit($0)
  }
'

# 扫一批文件：先列出被 `#[cfg(test)] mod X;` 声明的测试文件（路径精确对应），从这批里去掉，余下的照扫。
# 结果打到标准输出；awk 出错返回非零
in_list() {
  local want="$1" x
  shift
  for x in "$@"; do [[ "$x" == "$want" ]] && return 0; done
  return 1
}

scan_files() {
  local decls f x t base dir skip=() rest=() grew=1
  decls="$(LC_ALL=C awk -v decl=1 "$scan_d" "$@")" || return 1
  # 逐级往下：第一轮只认 #[cfg(test)] 声明的；之后上级已在 skip 里的，它声明的 mod X; 也算
  while [[ $grew -eq 1 ]]; do
    grew=0
    while IFS=$'\t' read -r f x t; do
      [[ -n "$f" ]] || continue
      if [[ "$t" != 1 ]] && ! in_list "$f" ${skip[@]+"${skip[@]}"}; then continue; fi
      base="$(basename "$f" .rs)"
      case "$base" in
        lib | main | mod) dir="$(dirname "$f")" ;;
        *) dir="$(dirname "$f")/$base" ;;
      esac
      for x in "$dir/$x.rs" "$dir/$x/mod.rs"; do
        if ! in_list "$x" ${skip[@]+"${skip[@]}"}; then skip+=("$x"); grew=1; fi
      done
    done <<<"$decls"
  done
  for f in "$@"; do
    in_list "$f" ${skip[@]+"${skip[@]}"} && continue
    rest+=("$f")
  done
  [[ ${#rest[@]} -gt 0 ]] || return 0
  LC_ALL=C awk "$scan_d" "${rest[@]}"
}

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
# 单独文件的测试模块：被 `#[cfg(test)] mod X;` 紧挨着声明、路径对得上的才不扫
t="$selftest/crate/src"
mkdir -p "$t/foo" "$t/bar" "$t/baz" "$t/cmt" "$t/attr" "$t/sub" "$t/other"
dbg='fn g(x: u8) -> String { format!("{:?}", x) }'
printf '%s\n' 'fn f() {}' '#[cfg(test)]' 'mod tests;' > "$t/foo.rs"
printf '%s\n' "$dbg" > "$t/foo/tests.rs"                       # 不报：foo.rs 声明了
printf '%s\n' '#[cfg(test)] mod t2;' > "$t/lib.rs"
printf '%s\n' "$dbg" > "$t/t2.rs"                              # 不报：lib.rs 同目录
printf '%s\n' '#[cfg(test)]' 'pub mod tests;' > "$t/sub.rs"
mkdir -p "$t/sub/tests"; printf '%s\n' "$dbg" > "$t/sub/tests/mod.rs"   # 不报：X/mod.rs 形式
printf '%s\n' 'mod tests;' > "$t/bar.rs"
printf '%s\n' "$dbg" > "$t/bar/tests.rs"                       # 要报：没有 cfg(test)
printf '%s\n' "$dbg" > "$t/baz/tests.rs"                       # 要报：谁也没声明
printf '%s\n' '// #[cfg(test)]' 'mod tests;' > "$t/cmt.rs"
printf '%s\n' "$dbg" > "$t/cmt/tests.rs"                       # 要报：cfg(test) 在注释里
printf '%s\n' '#[cfg(test)]' '#[path = "x.rs"]' 'mod tests;' > "$t/attr.rs"
printf '%s\n' "$dbg" > "$t/attr/tests.rs"                      # 要报：中间夹了属性
printf '%s\n' '#[cfg(test)]' 'mod tests;' > "$t/other.rs"
printf '%s\n' "$dbg" > "$t/other_tests.rs"                     # 要报：路径对不上
printf '%s\n' '#[cfg(test)]' 'mod tests;' "$dbg" > "$t/self.rs"  # 要报：声明本身不让后文跳过
mkdir -p "$t/srv/tests" "$t/lone/tests"
printf '%s\n' '#[cfg(test)]' 'mod tests;' > "$t/srv.rs"
printf '%s\n' 'mod topic;' "$dbg" > "$t/srv/tests.rs"            # 不报：被声明的测试文件
printf '%s\n' "$dbg" > "$t/srv/tests/topic.rs"                 # 不报：测试文件按主题再拆
printf '%s\n' "$dbg" > "$t/srv/tests/stray.rs"                 # 要报：没人声明
printf '%s\n' 'mod tests;' > "$t/lone.rs"
printf '%s\n' 'mod topic;' > "$t/lone/tests.rs"
printf '%s\n' "$dbg" > "$t/lone/tests/topic.rs"                # 要报：上级没被 cfg(test) 声明
mkdir -p "$t/nest"
printf '%s\n' 'mod helper;' 'mod worker {' '    #[cfg(test)]' '    mod helper;' '}' > "$t/nest/lib.rs"
printf '%s\n' "$dbg" > "$t/nest/helper.rs"                     # 要报：cfg(test) 声明在内联模块里，不指它（codex 020 第 2 轮）
if ! out="$(scan_files "$t"/*.rs "$t"/*/*.rs "$t"/*/*/*.rs 2>&1)"; then
  echo "✗ 自测失败：单独测试文件那一段 awk 出错"
  fail=1
fi
for want in bar/tests.rs baz/tests.rs cmt/tests.rs attr/tests.rs other_tests.rs self.rs srv/tests/stray.rs lone/tests/topic.rs nest/helper.rs; do
  if ! grep -q "^$t/$want:" <<<"$out"; then
    echo "✗ 自测失败：$want 该报没报（没被 #[cfg(test)] 紧挨着声明、或路径不对的文件必须照扫）"
    fail=1
  fi
done
for nope in foo/tests.rs t2.rs sub/tests/mod.rs srv/tests.rs srv/tests/topic.rs; do
  if grep -q "^$t/$nope:" <<<"$out"; then
    echo "✗ 自测失败：$nope 是被声明的测试文件，不该扫"
    fail=1
  fi
done

files=()
while IFS= read -r -d '' f; do files+=("$f"); done < <(find src/atg-* -path '*/src/*' -name '*.rs' -print0)
# awk 自己出错（语法、区域设置……）必须让脚本失败，不能当成「没扫到问题」
if ! hits="$(scan_files "${files[@]}")"; then
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
