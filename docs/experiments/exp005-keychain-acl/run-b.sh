#!/usr/bin/env bash
# exp005 方案乙：用有团队号的 Apple Development 证书签名，验证——
#   ① 建条目 → 换两版（重编）→ 读：不问人；分区名单是 teamid:
#   ② 开强化运行时的几版同样能读；③ atg-dev / 临时签名的程序读不到（规约：开发证书以外拿不到）
#   ④ 注入：普通版能被借身份读，强化运行时版挡住
#   ⑤ 强化运行时下的动态库（替身 libdummy 模拟 libtdjson）：rpath + 同团队签名能载；未签名 / 临时签名的库拒载；DYLD_LIBRARY_PATH 被忽略
# 只碰服务名 atg.exp005 / 账户 exp-b；所有读写关交互（本该问人时返回 -25293，不弹框）。
# 隐私：证书名里有人的姓名——签名用证书 SHA-1，不打印证书名；指定需求引号内一律打码；团队号只留前 4 位；dyld 报错只留原因。
# 前提：本机有 Apple Development 身份（恰好一个，或设 ATG_CODESIGN_ID=<SHA-1>）；对照组 ③ 还要 atg-dev 证书，没有就跳过那一行。
# 用法：docs/experiments/exp005-keychain-acl/run-b.sh [产物目录]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:-$(mktemp -d)}"; mkdir -p "$OUT/lib" "$OUT/u/lib" "$OUT/v"
FW=(-framework Security -framework CoreFoundation)
mask() { sed -E 's/"[^"]*"/"…"/g; s/(teamid:[A-Z0-9]{4})[A-Z0-9]+/\1……/g'; }
reason() { grep -oE '(different Team IDs|no such file)' | sort -u | tr '\n' ' '; echo; }

IDS="$(security find-identity -v -p codesigning | grep 'Apple Development' | awk '{print $2}' | sort -u)"; ID="${ATG_CODESIGN_ID:-}"
# 没设 ATG_CODESIGN_ID 时，恰好一个 Apple Development 身份才用；零个或多个就报错（取错团队号会静默失败）
if [[ -z "$ID" && $(printf '%s\n' "$IDS" | grep -c . || true) -eq 1 ]]; then ID="$IDS"; fi
[[ -n "$ID" ]] || { echo "Apple Development 身份没有或不止一个：请设 ATG_CODESIGN_ID=<SHA-1>" >&2; exit 1; }
echo "身份：Apple Development（SHA-1 前 8 位 ${ID:0:8}）"

cleanup() {
  [[ -x "$OUT/rA" ]] && "$OUT/rA" del exp-b >/dev/null 2>&1 || true
  local n; n=$(security dump-keychain 2>/dev/null | grep -c '"svce"<blob>="atg.exp005' || true)
  echo "剩余 atg.exp005* 条目：${n} 条"
}
trap cleanup EXIT

# 替身库与各版程序：r* 普通签名，h* 强化运行时；都按 @rpath 找库，rpath = @executable_path/lib（程序所在目录下的 lib/）
clang -dynamiclib "$HERE/libdummy.c" -install_name @rpath/libdummy.dylib -o "$OUT/lib/libdummy.dylib"
codesign -s "$ID" --force "$OUT/lib/libdummy.dylib" 2>/dev/null
for n in A B C; do
  clang -O0 -DNONCE="\"b$n\"" "${FW[@]}" "$HERE/probe.c" -L"$OUT/lib" -ldummy -Wl,-rpath,@executable_path/lib -o "$OUT/r$n" 2>/dev/null
  cp "$OUT/r$n" "$OUT/h$n"
  codesign -s "$ID" -i atg.exp005 --force "$OUT/r$n" 2>/dev/null
  codesign -s "$ID" -i atg.exp005 --options runtime --force "$OUT/h$n" 2>/dev/null
done
# 对照：atg-dev 签名版与临时签名版
clang -O0 -DNONCE='"bX"' "${FW[@]}" "$HERE/probe.c" -o "$OUT/xdev" 2>/dev/null
HAVE_DEV=1; codesign -s atg-dev -i atg.exp005 --force "$OUT/xdev" 2>/dev/null || { HAVE_DEV=0; echo "（没有 atg-dev 证书：③ 跳过 atg-dev 对照）"; }
clang -O0 -DNONCE='"bY"' "${FW[@]}" "$HERE/probe.c" -o "$OUT/xadhoc" 2>/dev/null
clang -dynamiclib "${FW[@]}" "$HERE/inject.c" -o "$OUT/inject.dylib" 2>/dev/null

TEAM="$(codesign -d -vv "$OUT/rA" 2>&1 | sed -n 's/^TeamIdentifier=//p')"
echo "团队号前 4 位：${TEAM:0:4}"
for n in A B C; do echo "r$n cdhash=$(codesign -d -vvv "$OUT/r$n" 2>&1 | sed -n 's/^CDHash=//p')"; done
echo "指定需求结构：$(codesign -d -r- "$OUT/rA" 2>&1 | grep designated | mask)"

echo "① 建、原版读、两个重编版读（通过 = status=0）"
"$OUT/rA" add exp-b || true
for n in A B C; do "$OUT/r$n" get exp-b || true; done
echo "   分区名单：$(security dump-keychain -a "$HOME/Library/Keychains/login.keychain-db" 2>/dev/null | awk '/^keychain: /{if(blk ~ /"exp-b"/ && blk ~ /"atg\.exp005"/) print blk; blk=""} {blk=blk $0 "\n"} END{if(blk ~ /"exp-b"/ && blk ~ /"atg\.exp005"/) print blk}' | grep -A2 'partition_id' | grep description | mask || true)"
echo "② 强化运行时版读（通过 = status=0）"
for n in A B C; do "$OUT/h$n" get exp-b || true; done
echo "③ atg-dev 版 / 临时签名版读（通过 = -25293）"
[[ $HAVE_DEV == 1 ]] && { "$OUT/xdev" get exp-b || true; }; "$OUT/xadhoc" get exp-b || true
echo "④ 注入：普通版（预期 inject: … len=32）/ 强化版（预期注入不生效，输出是探针自己的）"
EXP005_INJECT_OP=get EXP005_INJECT_ACCOUNT=exp-b DYLD_INSERT_LIBRARIES="$OUT/inject.dylib" "$OUT/rB" get exp-b 2>&1 || true
EXP005_INJECT_OP=get EXP005_INJECT_ACCOUNT=exp-b DYLD_INSERT_LIBRARIES="$OUT/inject.dylib" "$OUT/hB" get exp-b 2>&1 || true
echo "⑤ 强化版的库加载（预期：未签名库、临时签名库 → different Team IDs；强化版只给 DYLD_LIBRARY_PATH → no such file；普通版 → status=0）"
# u/ = rpath 处放未签名或临时签名的库；v/ = rpath 处没有库，只靠 DYLD_LIBRARY_PATH 指向已签名的库
cp "$OUT/hB" "$OUT/u/"; clang -dynamiclib "$HERE/libdummy.c" -install_name @rpath/libdummy.dylib -o "$OUT/u/lib/libdummy.dylib"
# 被拒载时 dyld 让进程 abort；放进 bash -c，免得外层 shell 打印作业崩溃信息
try() { bash -c '"$@"; exit $?' _ "$@" 2>&1; }
echo -n "   未签名库：";   try "$OUT/u/hB" get exp-b | reason || true
codesign -s - --force "$OUT/u/lib/libdummy.dylib" 2>/dev/null
echo -n "   临时签名库："; try "$OUT/u/hB" get exp-b | reason || true
cp "$OUT/hB" "$OUT/rB" "$OUT/v/"
echo -n "   强化版 + DYLD_LIBRARY_PATH（rpath 处无库）："; DYLD_LIBRARY_PATH="$OUT/lib" try "$OUT/v/hB" get exp-b | reason || true
echo -n "   普通版 + 同上："; DYLD_LIBRARY_PATH="$OUT/lib" "$OUT/v/rB" get exp-b 2>&1 || true
