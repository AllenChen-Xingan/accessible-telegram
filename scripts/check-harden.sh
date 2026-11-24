#!/usr/bin/env bash
# check-harden.sh — 核开发版的强化运行时真挡住了注入（#42）。不碰钥匙串、不连 Telegram，产物全在 mktemp 目录，退出时删。
#
# 做什么（依据 docs/experiments/exp005-keychain-acl/README.md 取证 9、10 ⑤）：
#   ① 编一个只在载入时往临时文件写标记的 libmark.dylib，和一个几行的启动器（系统 shell 会删掉 DYLD_* 变量，
#      所以要一个非系统程序来设 DYLD_INSERT_LIBRARIES 再 exec）
#   ② 把要核的程序拷两份：一份 ATG_DEV_HARDEN=0 签（对照，注入应该成功——证明这个检查本身有效），
#      一份照常签（强化运行时，注入应该不生效）；各跑一次 `<程序> version`
#   ③ 程序链了 libtdjson 时再核两件：rpath 里有 ~/<dev-link>/lib（现为 ~/.atg-dev/tdlib/lib，正本 src/tdlib-build/dev-link）且照常能起；把 rpath 换成一个放着
#      临时签名 libtdjson 的目录，强化运行时版应被 dyld 拒载（库验证：不同团队号）
# 用法：bash scripts/check-harden.sh [atg-cli 的路径]     # 只收 atg-cli；默认 target/debug/atg-cli（先 cargo build -p atg-cli [--features tdlib]）
# 前置：clang（Xcode 命令行工具）、开发签名身份；核带 TDLib 的程序时还要先编过 TDLib（bash src/tdlib-build/build.sh，
#   它顺带建好软链接 ~/.atg-dev/tdlib）。本机跑，不在 CI（CI 不签名）；什么时候跑见 specs/contracts/testing.md 第一节第 4 小节「强化运行时」
# 退出码：0 全部符合预期 · 1 有一项不符 · 2 用法错 / 准备失败
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "${here}/.." && pwd)"
bin="${1:-${CARGO_TARGET_DIR:-${root}/target}/debug/atg-cli}"
prep() { echo "check-harden: 准备失败：$*" >&2; exit 2; }
[[ -f "${bin}" ]] || prep "没有这个程序：${bin}"
# 只核 atg-cli：它的 `version` 子命令不读钥匙串、不起后端、不联网。别的程序（如 live_wait_phone）不认参数，
# 照跑会真起后端，所以不收（codex 063 第 1 轮 #1）。同理清掉 live 开关，免得继承进来
[[ "$(basename "${bin}")" == "atg-cli" ]] || prep "只核 atg-cli（它的 version 没有副作用）；给的是 $(basename "${bin}")"
unset ATG_LIVE

tmp="$(mktemp -d "${TMPDIR:-/tmp}/atg-harden.XXXXXX")" || prep "建临时目录失败"
trap 'rm -rf "${tmp}"' EXIT
fail=0
ok() { echo "  ✓ $*"; }
bad() { echo "  ✗ $*"; fail=1; }

cat >"${tmp}/mark.c" <<'C' || prep "写 mark.c 失败"
#include <fcntl.h>
#include <stdlib.h>
#include <unistd.h>
__attribute__((constructor)) static void mark(void) {
  const char *p = getenv("ATG_HARDEN_MARK");
  if (p) { int fd = open(p, O_WRONLY | O_CREAT | O_TRUNC, 0600); if (fd >= 0) { write(fd, "injected\n", 9); close(fd); } }
}
C
cat >"${tmp}/launch.c" <<'C' || prep "写 launch.c 失败"
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
/* 用法：launch <dylib> <程序> [参数…]：设 DYLD_INSERT_LIBRARIES 后 exec，非系统程序，变量不会被删 */
int main(int argc, char **argv) {
  if (argc < 3) return 2;
  setenv("DYLD_INSERT_LIBRARIES", argv[1], 1);
  execv(argv[2], argv + 2);
  perror("execv");
  return 2;
}
C
clang -dynamiclib -o "${tmp}/libmark.dylib" "${tmp}/mark.c" || prep "编 libmark 失败"
clang -o "${tmp}/launch" "${tmp}/launch.c" || prep "编启动器失败"

cp "${bin}" "${tmp}/plain" && cp "${bin}" "${tmp}/hard" || prep "拷程序失败"
ATG_DEV_HARDEN=0 bash "${here}/dev-sign.sh" "${tmp}/plain" 2>/dev/null || prep "签对照版失败"
bash "${here}/dev-sign.sh" "${tmp}/hard" || prep "签强化运行时版失败"

# 先把输出存进变量再匹配：开着 pipefail，`… | grep -q` 一匹配就提前退出，前一个命令收到 SIGPIPE，整条管道算失败，结论会反过来
has() { [[ "$1" == *"$2"* ]]; }
runtime_re='flags=0x[0-9a-f]+\([^)]*runtime'   # flags=0x10000(runtime)，或几个标志并列时其中有 runtime
echo "→ 签名标志"
# 读不出签名信息算准备失败（退 2），不能把空输出当成「没有 runtime」（codex 063 第 1 轮 #3）
sig_hard="$(codesign -dv "${tmp}/hard" 2>&1)" || prep "读强化运行时版的签名信息失败"
sig_plain="$(codesign -dv "${tmp}/plain" 2>&1)" || prep "读对照版的签名信息失败"
[[ "${sig_hard}" == *"flags="* && "${sig_plain}" == *"flags="* ]] || prep "签名信息里没有 flags 一项，认不出"
if [[ "${sig_hard}" =~ ${runtime_re} ]]; then ok "强化运行时版带 runtime 标志"; else bad "强化运行时版没有 runtime 标志"; fi
if [[ "${sig_plain}" =~ ${runtime_re} ]]; then bad "对照版也带了 runtime（ATG_DEV_HARDEN=0 没生效）"; else ok "对照版不带 runtime"; fi

inject() { # $1=程序；标记文件出现 = 注入生效
  rm -f "${tmp}/mark"
  ATG_HARDEN_MARK="${tmp}/mark" "${tmp}/launch" "${tmp}/libmark.dylib" "$1" version >/dev/null 2>&1 || true
  [[ -f "${tmp}/mark" ]]
}
# 两版都先得能正常起，否则「没出标记」分不清是挡住了注入还是程序根本没起来（codex 063 第 1 轮 #4）
echo "→ 照常起（version）"
if "${tmp}/plain" version >/dev/null 2>&1; then ok "对照版照常起"; else bad "对照版起不来，下面的注入结论不可信"; fi
if "${tmp}/hard" version >/dev/null 2>&1; then ok "强化运行时版照常起"; else bad "强化运行时版起不来（带 TDLib 时：libtdjson 没签、没静态链 OpenSSL，或软链接不对），下面的注入结论不可信"; fi
echo "→ 注入（DYLD_INSERT_LIBRARIES）"
if inject "${tmp}/plain"; then ok "对照版被注入（说明检查有效）"; else bad "对照版也没被注入：检查本身不灵，结论不可信"; fi
if inject "${tmp}/hard"; then bad "强化运行时版被注入了"; else ok "强化运行时版挡住了注入"; fi

libs="$(otool -L "${bin}")" || prep "otool -L 读不了程序"
if has "${libs}" libtdjson; then
  echo "→ TDLib 动态库"
  # 软链接路径的正本是 src/tdlib-build/dev-link（#170），和程序里的 rpath 同一个来源
  f="${root}/src/tdlib-build/dev-link"
  [[ -r "${f}" ]] || prep "读不了 src/tdlib-build/dev-link"
  # 规则（三处同一条，见 src/tdlib-build/README.md）：整个文件 = 一行路径 + 可有可无的一个换行；不 trim。
  # `$(cat; printf x)` 再去掉 x：保住文件末尾的换行原样（`$( )` 本身会吞掉所有末尾换行，那样多出的空行就查不出来）
  rel="$(cat "${f}"; printf x)"
  rel="${rel%x}"
  rel="${rel%$'\n'}"
  [[ -n "${rel}" && "${rel}" != /* && "${rel}" != *..* && "${rel}" != *[[:space:]]* ]] || prep "src/tdlib-build/dev-link 的内容不对"
  rp="${HOME}/${rel}/lib"
  loads="$(otool -l "${bin}")" || prep "otool -l 读不了程序"
  if has "${loads}" "path ${rp} "; then ok "rpath 有 ${rp}"; else bad "rpath 里没有 ${rp}"; fi
  # 只许这一条：别的 rpath（如 tdlib-rs 的 OUT_DIR、@loader_path）可能先被搜到，那里同用户可写（codex 063 第 2 轮）
  n_rpath="$(awk '/cmd LC_RPATH/{n++} END{print n+0}' <<<"${loads}")"
  if [[ "${n_rpath}" == 1 ]]; then ok "rpath 只有这一条"; else bad "rpath 有 ${n_rpath} 条，不只 ${rp}"; fi
  # 换库：临时签名的 libtdjson 副本放进另一个目录，把 rpath 改指那里。准备失败按约定退 2，不和「有一项不符」混
  real="$(cd "${rp}" && pwd -P)" || prep "软链接 ${rp} 不在（先 bash src/tdlib-build/build.sh 或 build.sh link）"
  mkdir -p "${tmp}/fake-lib" && cp "${real}"/libtdjson.*.dylib "${tmp}/fake-lib/" || prep "拷 libtdjson 失败"
  for f in "${tmp}"/fake-lib/libtdjson.*.dylib; do codesign -s - --force "${f}" 2>/dev/null || prep "临时签名失败"; done
  cp "${bin}" "${tmp}/swapped" || prep "拷程序失败"
  install_name_tool -rpath "${rp}" "${tmp}/fake-lib" "${tmp}/swapped" 2>/dev/null || prep "改 rpath 失败"
  bash "${here}/dev-sign.sh" "${tmp}/swapped" || prep "签换库版失败"
  if "${tmp}/swapped" version >/dev/null 2>"${tmp}/swapped.err"; then
    bad "换成临时签名的 libtdjson 后照常起了（库验证没生效）"
  elif grep -q 'Team ID\|not valid for use in process\|code signature' "${tmp}/swapped.err"; then
    ok "换成临时签名的 libtdjson 被 dyld 拒载"
  else
    bad "换库后起不来，但不是库验证拒载（看 dyld 报错：$(head -c 200 "${tmp}/swapped.err" | tr '\n' ' ')）"
  fi
else
  echo "→ 这个程序没链 libtdjson，跳过 TDLib 那几项（要核就 cargo build -p atg-cli --features tdlib）"
fi

if (( fail )); then echo "✗ 有不符合预期的项"; exit 1; fi
echo "✓ 全部符合预期"
