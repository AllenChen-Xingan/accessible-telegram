#!/usr/bin/env bash
# PreToolUse(Bash) 守门：拦合并 PR、直推 main、强推、删 main。
# 放行方式：人在对话里明确同意后，命令前加 ATG_CONFIRMED=1 重发（拦得住，不堵死）。
# 依据：specs/workflow.md 第 1、7 节。
set -euo pipefail
cmd="$(jq -r '.tool_input.command // empty')"
[ -z "$cmd" ] && exit 0
case "$cmd" in ATG_CONFIRMED=1\ *) echo "guard-git: 已按人确认放行：$cmd" >&2; exit 0 ;; esac

# 统一成单空格，便于匹配多种写法
c="$(printf '%s' "$cmd" | tr -s '[:space:]' ' ')"
deny() { echo "guard-git 拦下：$1。见 specs/workflow.md。人确认后可加前缀 ATG_CONFIRMED=1 重发。" >&2; exit 2; }

# 1. 合并 PR（gh 命令或直接调 API）
printf '%s' "$c" | grep -Eq 'gh( [^|;&]*)? pr merge' && deny "合并 PR 由人来点"
printf '%s' "$c" | grep -Eq 'gh api [^|;&]*pulls/[0-9]+/merge' && deny "合并 PR 由人来点"

if printf '%s' "$c" | grep -Eq 'git( -C [^ ]+)? push'; then
  push="$(printf '%s' "$c" | grep -Eo 'git( -C [^ ]+)? push[^|;&]*' | head -1)"
  # 2. 强推
  printf '%s' "$push" | grep -Eq -- '(^| )(-f|--force|--force-with-lease[^ ]*|\+[^ ]+)( |$)' && deny "禁止强推"
  # 3. 删除远程 main
  printf '%s' "$push" | grep -Eq -- '(--delete|-d|:)(main|master)( |$)|--delete [^ ]+ (main|master)( |$)' && deny "禁止删除 main"
  # 4. 显式推到 main
  printf '%s' "$push" | grep -Eq -- '( |:)(main|master|HEAD:main|HEAD:master)( |$)' && deny "禁止直推 main，请走 PR"
  # 5. 当前就在 main 上的裸 push
  branch="$(git -C "${CLAUDE_PROJECT_DIR:-.}" branch --show-current 2>/dev/null || true)"
  if [ "$branch" = main ] || [ "$branch" = master ]; then
    printf '%s' "$push" | grep -Eq -- 'push( -u| --set-upstream)?( origin)?$' && deny "当前在 main 上，禁止直推，请先切功能分支"
  fi
fi
exit 0
