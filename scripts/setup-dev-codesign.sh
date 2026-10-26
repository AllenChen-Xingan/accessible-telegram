#!/usr/bin/env bash
# setup-dev-codesign.sh — 检查本机有没有能给开发版签名的 Apple Development 身份；没有就告诉人怎么领。
#
# 为什么（issue #38，拍板 007「#38 钥匙串弹框」：用乙）：开发版读钥匙串要重编不弹框，签名得带团队号，
#   只有苹果签发的证书有；以前这里建的自签名证书 atg-dev 没有团队号，重编必弹（exp005）。
# 谁来跑：人或 AI 都行，只读，不改钥匙串、不弹框。命令：bash scripts/setup-dev-codesign.sh
# 做什么：按 scripts/dev-codesign-id.sh 的规则找身份（环境变量 / .env.local 的 ATG_CODESIGN_ID，或恰好一个 Apple Development），
#   找到就打印 SHA-1 前 8 位与有效期；找不到或不止一个就打印领证书 / 选证书的步骤，退出码 2。
# 不做什么：不建证书、不打印身份名（里面有人名）。
# atg-dev 已于 2026-10-26 删除（拍板 007 E，人亲手删），exp005 的 atg-dev 对照组不可复现。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! id="$(bash "${here}/dev-codesign-id.sh")"; then
  cat >&2 <<'EOF'

（原因见上一行。下面是各种情形的处理办法。）
没有身份——领 Apple Development 证书（免费 Apple ID 就行，不用建工程）：
  0. 要完整的 Xcode（App Store 下载，十几 GB；只装命令行工具不够，没有 Accounts 面板）。装好后下面几步约 2 分钟
  1. 打开 Xcode → 设置（⌘,）→ Accounts
  2. 左下角「+」→ Apple ID → 登录
  3. 选中个人团队（Personal Team）→ Manage Certificates…
  4. 左下角「+」→ Apple Development
  5. 回来再跑一次 bash scripts/setup-dev-codesign.sh
ATG_CODESIGN_ID 设错了（格式不对、过期、不是 Apple Development）：改环境变量或 .env.local 里那一行，或删掉它走自动选择
有两个以上 Apple Development 身份时（比如续签期间新旧并存）：上面已列出每个的 SHA-1 与有效期，
  把要用的那个写进仓库根 .env.local：ATG_CODESIGN_ID=<40 位 SHA-1>
只想跑单测、本机暂时没有身份：ATG_DEV_SIGN=0 cargo test --workspace（单测不碰钥匙串；不签名的程序读本地库钥匙会弹框或读不到）
EOF
  exit 2
fi

end="$(bash "${here}/dev-codesign-id.sh" --not-after "${id}")"

echo "开发签名身份就绪：Apple Development，SHA-1 前 8 位 ${id:0:8}${end:+，有效到 ${end}}"
echo "签名：bash scripts/dev-sign.sh <程序>；cargo run / cargo test 经 .cargo/config.toml 的 runner 自动签。"
echo "证书一年一续（看板 state/board.md 记着到期日）。续签后：新证书签的程序能否照读现有条目未测，"
echo "  先按 docs/experiments/exp005-keychain-acl/README.md「乙本体」第 4、7 条用实验条目演一遍，再迁真条目"
echo "  （探针与做法见 docs/experiments/exp006-keychain-migrate/README.md「续签时怎么用」）；"
echo "  旧证书别急着删。注意迁移过的条目新身份只能读、不能删（config-privacy.md 第一节第 3 小节）。"
