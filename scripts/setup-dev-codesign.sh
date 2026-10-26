#!/usr/bin/env bash
# setup-dev-codesign.sh — 在本机登录钥匙串里建一张自签名「代码签名」开发证书（默认名 atg-dev）。
#
# 为什么：拍板 005 N2「开发证书也管正式钥匙、不弹框」需要一张稳定的签名身份；
#         没有它，每次 cargo 重编出的程序签名都变，读钥匙串会反复弹「允许访问」。
# 谁来跑：人本人（会往钥匙串里加证书；第 3 步系统会弹框要登录密码确认信任）。
#         命令：bash scripts/setup-dev-codesign.sh [证书名]
# 做什么：① openssl 生成 RSA-2048 私钥 + 自签证书（EKU = codeSigning，10 年）
#         ② 导入登录钥匙串，只授权 /usr/bin/codesign 使用私钥
#         ③ 设为「信任用于代码签名」  ④ 列出确认
# 不做什么：不改别的证书；私钥只进钥匙串，临时文件退出即删；已存在同名有效身份就直接退出。
set -euo pipefail

NAME="${1:-atg-dev}"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
DRY="${ATG_CODESIGN_DRY_RUN:-0}"   # =1 只生成并检查证书，不碰钥匙串（给 AI 自测用）

if [[ ! "${NAME}" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "证书名只许字母数字 . _ -：${NAME}" >&2; exit 1
fi

if [[ "${DRY}" != 1 ]] && security find-identity -v -p codesigning 2>/dev/null | grep -q "\"${NAME}\""; then
  echo "已有有效的代码签名身份「${NAME}」，不重复创建："
  security find-identity -v -p codesigning | grep "\"${NAME}\""
  exit 0
fi

OPENSSL="$(brew --prefix openssl@3 2>/dev/null)/bin/openssl"
[[ -x "${OPENSSL}" ]] || { echo "找不到 homebrew 的 openssl@3（brew install openssl@3）" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
chmod 700 "${TMP}"

cat > "${TMP}/req.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = ${NAME}
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
EOF

echo "① 生成私钥与自签证书（${NAME}，10 年）"
"${OPENSSL}" req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "${TMP}/key.pem" -out "${TMP}/cert.pem" -config "${TMP}/req.cnf" 2>/dev/null
"${OPENSSL}" x509 -in "${TMP}/cert.pem" -noout -subject -enddate -ext extendedKeyUsage

if [[ "${DRY}" == 1 ]]; then
  echo "（ATG_CODESIGN_DRY_RUN=1：只生成并检查，不导入钥匙串）"; exit 0
fi

# p12 的临时口令只活在本进程，导入后随临时目录删除
P12_PASS="$("${OPENSSL}" rand -hex 16)"
"${OPENSSL}" pkcs12 -export -legacy -inkey "${TMP}/key.pem" -in "${TMP}/cert.pem" \
  -name "${NAME}" -out "${TMP}/id.p12" -passout "pass:${P12_PASS}"

echo "② 导入登录钥匙串（只授权 /usr/bin/codesign 使用私钥）"
security import "${TMP}/id.p12" -k "${KEYCHAIN}" -P "${P12_PASS}" -T /usr/bin/codesign >/dev/null

echo "③ 设为「信任用于代码签名」——系统会弹框要你的登录密码"
security add-trusted-cert -r trustRoot -p codeSign -k "${KEYCHAIN}" "${TMP}/cert.pem"

echo "④ 确认："
if security find-identity -v -p codesigning | grep "\"${NAME}\""; then
  echo "完成。以后签名用：codesign -s \"${NAME}\" <程序>"
else
  echo "没找到有效身份「${NAME}」——可能信任那一步被取消了。可在「钥匙串访问」里找到 ${NAME}，打开「信任」→「代码签名：始终信任」。" >&2
  exit 1
fi
