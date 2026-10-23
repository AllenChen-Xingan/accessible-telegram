#!/usr/bin/env bash
# 把 .github/labels.yml 同步到 GitHub（只增改，不删已有标签）。需要 gh 已登录、仓库已有远程。
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY' | while IFS=$'\t' read -r name color desc; do
import re
for line in open(".github/labels.yml", encoding="utf-8"):
    m = re.match(r'- \{ name: "([^"]+)",\s*color: "([0-9a-f]{6})",\s*description: "([^"]*)" \}', line.strip())
    if m: print("\t".join(m.groups()))
PY
  gh label create "$name" --color "$color" --description "$desc" --force
done
