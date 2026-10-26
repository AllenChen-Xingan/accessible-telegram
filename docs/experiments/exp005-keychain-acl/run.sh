#!/usr/bin/env bash
# exp005 复现：只碰服务名 atg.exp005 的实验条目；所有读写前关交互，本该问人时直接失败（本机返回 -25293），不弹框。
# 前提：已有开发证书 atg-dev（scripts/setup-dev-codesign.sh），codesign 用它不弹框。
# 用法：docs/experiments/exp005-keychain-acl/run.sh [产物目录]   （默认 mktemp；不进仓库）
# 复现：取证 3（重编后读不到）· 方案丁（小助手重编 2 次、调用方换版、临时签名被拒）· 注入读取。
# 不复现：方案丙 / 己 / 庚的单次实测、强化运行时变体（它建的条目只有小助手自己能删）——命令见 README。
# 退出时（含中途失败）尽力删掉本次建的条目。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:-$(mktemp -d)}"; mkdir -p "$OUT"
FW=(-framework Security -framework CoreFoundation)
sign() { codesign -s atg-dev -i "$2" --force "$1" 2>/dev/null; }
cdh() { codesign -d -vvv "$1" 2>&1 | sed -n 's/^CDHash=//p'; }

# 本机 atg-dev 证书的 SHA-1（小助手按它核调用方）
CERT_SHA1="$(security find-certificate -c atg-dev -Z 2>/dev/null | sed -n 's/^SHA-1 hash: //p' | head -1 | tr 'A-F' 'a-f')"
[[ -n "$CERT_SHA1" ]] || { echo "找不到证书 atg-dev" >&2; exit 1; }

cleanup() {
  [[ -x "$OUT/pA" ]] && "$OUT/pA" del exp-run-direct >/dev/null 2>&1 || true
  if [[ -x "$OUT/h/keyhelper" && -f "$OUT/inject.dylib" ]]; then
    EXP005_INJECT_OP=del EXP005_INJECT_ACCOUNT=exp-run-helper DYLD_INSERT_LIBRARIES="$OUT/inject.dylib" \
      "$OUT/h/keyhelper" get exp-run-helper >/dev/null 2>&1 || true
  fi
  local n; n=$(security dump-keychain 2>/dev/null | grep -c '"svce"<blob>="atg.exp005"' || true)
  echo "剩余 atg.exp005 条目：${n} 条（本次建的应已清掉；若有以前留下的、强化运行时小助手建的条目，会算在这里）"
}
trap cleanup EXIT

# 调用方：同一份源码换 NONCE 编几版 = 模拟重编（cdhash 不同），都用 atg-dev + 固定标识签；pD 留作临时签名
for n in A B C; do clang -O0 -DNONCE="\"$n\"" "${FW[@]}" "$HERE/probe.c" -o "$OUT/p$n" 2>/dev/null; sign "$OUT/p$n" atg.exp005; done
clang -O0 -DNONCE='"D"' "${FW[@]}" "$HERE/probe.c" -o "$OUT/pD" 2>/dev/null
clang -dynamiclib "${FW[@]}" "$HERE/inject.c" -o "$OUT/inject.dylib" 2>/dev/null
echo "指定需求：$(codesign -d -r- "$OUT/pA" 2>&1 | grep designated)"
echo "cdhash：A=$(cdh "$OUT/pA") B=$(cdh "$OUT/pB")"

echo "— 取证 3：直接读（固定标识 + 同证书）"
"$OUT/pA" add exp-run-direct || true
"$OUT/pA" get exp-run-direct || true
"$OUT/pB" get exp-run-direct || true      # 预期 -25293：分区名单只有 pA 的 cdhash

echo "— 方案丁：取钥匙小助手（两步编译：先 .o 再链接，产物逐字节相同）"
mkdir -p "$OUT/h"
buildh() { (cd "$OUT/h" && rm -f kh.o keyhelper && clang -O0 -g0 -DCALLER_REQ="\"certificate leaf = H\\\"$CERT_SHA1\\\"\"" -c "$HERE/keyhelper.c" -o kh.o 2>/dev/null \
  && clang -g0 kh.o "${FW[@]}" -o keyhelper && sign keyhelper atg.exp005.keyhelper && echo "helper cdhash=$(cdh keyhelper)"); }
export EXP005_HELPER="$OUT/h/keyhelper"
buildh; "$OUT/pB" viahelper exp-run-helper || true   # 第一次：条目不存在，小助手建（created）
buildh; "$OUT/pC" viahelper exp-run-helper || true   # 小助手重编过、换了调用方：预期 get status=0
buildh; "$OUT/pB" viahelper exp-run-helper || true   # 再重编一次：预期 get status=0
"$OUT/pD" viahelper exp-run-helper || true           # 临时签名调用方：预期 caller rejected -67050

echo "— 注入读取：借小助手（未开强化运行时）的身份读，只打印长度"
EXP005_INJECT_OP=get EXP005_INJECT_ACCOUNT=exp-run-helper DYLD_INSERT_LIBRARIES="$OUT/inject.dylib" \
  "$OUT/h/keyhelper" get exp-run-helper || true      # 预期 status=0 len=32，不弹框
