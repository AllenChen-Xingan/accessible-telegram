#!/usr/bin/env bash
# dev-codesign-id.sh — 找出开发版签名用的身份，只在标准输出打印它的 SHA-1（40 位十六进制）。
#
# 为什么（issue #38，exp005「乙本体」第 1 条）：钥匙串条目的分区名单要记 `teamid:…` 才能重编不弹框，
#   只有苹果签发、带团队号的 Apple Development 证书做得到；自签名的 atg-dev 不行。
# 规则：
#   ① 环境变量 ATG_CODESIGN_ID，没有就读仓库根 `.env.local` 里的 ATG_CODESIGN_ID（只读这一行，不 source 整个文件）
#   ② 都没设：现场找 Apple Development 身份，**恰好一个**才用；零个或多个就报错退出——
#      续签期间新旧证书并存、或多了别的团队时，取错团队号会悄悄回到弹框
#   ③ 不管哪条来的，都核一遍：必须是钥匙串里有效的 Apple Development 代码签名身份
# 不做什么：不打印身份名（里面有人名）。成功时只打印 SHA-1；有多个候选时在标准错误列出每个的 SHA-1 与有效期，供人挑。
# 用法：bash scripts/dev-codesign-id.sh        # 成功：标准输出一行 SHA-1，退出码 0；失败：标准错误说明，退出码 2
#       bash scripts/dev-codesign-id.sh --not-after <SHA-1>   # 只打印这张证书的有效期（读不到打印空）
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() { echo "dev-codesign-id: $*" >&2; exit 2; }

hint_setup="跑 bash scripts/setup-dev-codesign.sh 看怎么领证书"
hint_pick="把要用的那个（一般挑有效期最晚的）的 SHA-1 写进仓库根 .env.local：ATG_CODESIGN_ID=<40 位 SHA-1>"

# 有效的 Apple Development 身份的 SHA-1，一行一个（只取哈希列，不留名字）
apple_dev_ids() {
  security find-identity -v -p codesigning 2>/dev/null \
    | awk '/"Apple Development: / { print toupper($2) }' \
    | sort -u
}

# 证书有效期（按 SHA-1 从钥匙串导出公开证书，openssl 读结束日期；读不到打印空）
not_after() {
  security find-certificate -a -Z -p 2>/dev/null \
    | awk -v want="$1" '
        /^SHA-1 hash:/ { hit = (toupper($3) == want); next }
        hit && /BEGIN CERTIFICATE/ { on = 1 }
        on { print }
        on && /END CERTIFICATE/ { exit }' \
    | openssl x509 -noout -enddate 2>/dev/null | sed 's/^notAfter=//' || true
}

if [[ "${1:-}" == "--not-after" ]]; then
  not_after "$(printf '%s' "${2:-}" | tr '[:lower:]' '[:upper:]')"
  exit 0
fi

id="${ATG_CODESIGN_ID:-}"
if [[ -z "${id}" && -f "${root}/.env.local" ]]; then
  id="$(grep -E '^[[:space:]]*(export[[:space:]]+)?ATG_CODESIGN_ID=' "${root}/.env.local" | tail -n 1 \
        | sed -E 's/^[^=]*=//; s/[[:space:]]+#.*$//; s/^["'\'']//; s/["'\'']$//; s/[[:space:]]//g' || true)"
fi

ids="$(apple_dev_ids)"

if [[ -n "${id}" ]]; then
  id="$(printf '%s' "${id}" | tr '[:lower:]' '[:upper:]')"
  [[ "${id}" =~ ^[0-9A-F]{40}$ ]] || die "ATG_CODESIGN_ID 不是 40 位十六进制的 SHA-1。${hint_pick}"
  grep -qx "${id}" <<<"${ids}" \
    || die "ATG_CODESIGN_ID（前 8 位 ${id:0:8}）不是钥匙串里有效的 Apple Development 身份（过期、被删，或是 atg-dev 之类的自签名证书）。${hint_setup}"
  echo "${id}"
  exit 0
fi

n="$(grep -c . <<<"${ids}" || true)"
case "${n}" in
  1) echo "${ids}" ;;
  0) die "钥匙串里没有有效的 Apple Development 身份。${hint_setup}" ;;
  *)
    echo "dev-codesign-id: 钥匙串里有 ${n} 个 Apple Development 身份，不知道用哪个（续签期间新旧并存时常见）：" >&2
    while read -r c; do echo "  ${c}  有效到 $(not_after "${c}")" >&2; done <<<"${ids}"
    die "${hint_pick}" ;;
esac
