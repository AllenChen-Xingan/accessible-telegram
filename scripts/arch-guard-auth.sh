#!/usr/bin/env bash
# arch-guard 的授权判断（#60，codex 019 门槛第 2 轮 #1）：要不要给 check-file-size.py 传 --allow-loosen。
#
# 不信触发事件里存的标签快照：GitHub「重跑」用的是旧事件，标签摘掉以后重跑带标签那次，快照里还有标签。
# 所以每次（含重跑）都向 GitHub 现查：① PR 现在的头提交是不是这次要检查的提交（不是 → 旧运行，不放行）；
# ② PR 现在有没有 baseline:loosen 标签。任何一步查不到 → 不放行（不放行只意味着「门槛有改动就红」，不会误拦正常 PR）。
#
# 输入（环境变量）：REPO（owner/name）· PR（编号）· EVENT_HEAD（这次运行要检查的提交）· GH_TOKEN（只读）
# 输出：标准输出一行，`--allow-loosen` 或空；说明写标准错误。退出码恒为 0。
set -uo pipefail

say() { printf '%s\n' "$*" >&2; }

head_now=$(gh api "repos/${REPO}/pulls/${PR}" --jq .head.sha 2>/dev/null) || head_now=""
if [ -z "$head_now" ]; then
  say "查不到 PR #${PR} 现在的头提交：不放行"
  exit 0
fi
if [ "$head_now" != "$EVENT_HEAD" ]; then
  say "这次运行检查的是 ${EVENT_HEAD}，PR 现在的头提交是 ${head_now}（旧运行被重跑？）：不放行"
  exit 0
fi
labels=$(gh api "repos/${REPO}/issues/${PR}/labels" --jq '.[].name' 2>/dev/null) || {
  say "查不到 PR #${PR} 现在的标签：不放行"
  exit 0
}
if printf '%s\n' "$labels" | grep -qx 'baseline:loosen'; then
  say "PR #${PR} 现在带着 baseline:loosen：放行门槛改动"
  printf '%s\n' "--allow-loosen"
else
  say "PR #${PR} 现在没有 baseline:loosen"
fi
exit 0
