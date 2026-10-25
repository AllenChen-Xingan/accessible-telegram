#!/bin/zsh
# 一条命令跑起来。用法：
#   ./run.sh                  人工试用（默认：Tab 进出列表、列表内 ↑/↓、不读位置、绕过输入法）
#   ./run.sh --pos-in-label   对照：「第 N 项，共 M 项」拼进名称
#   ./run.sh selftest         exp003 原自测：四种模式，GPUI 内部模拟按键，结果写 out/
#   ./run.sh selftest-tab     exp003b-v2 自测：Tab 进出列表 + ↑/↓ + VO 落到列表，绕/不绕输入法各一轮，结果写 out/v2-*
#   ./run.sh selftest-b       exp003b 自测：真实按键 ×2（绕/不绕输入法）+ 模拟 VO+←/→ ×6，结果写 out/b-*
# 人工试用时的按键与读屏动作记录在 out/live-<时间>.jsonl
# ⚠️ 会编译 GPUI，算重活，先向大总管申请名额
set -e
HERE="${0:A:h}"
ROOT="${HERE:h:h:h}"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
export CARGO_TARGET_DIR="$ROOT/_build/20261023-154000"
cd "$HERE"
cargo build
BIN="$CARGO_TARGET_DIR/debug/exp003-list-nav"
run_set() { # $1 = 参数串
  local rc=0
  mkdir -p out
  echo "== $1"
  "$BIN" ${=1} || rc=$?
  return 0
}
if [[ "$1" == "selftest" ]]; then
  for m in "" "--pos-in-label" "--listbox" "--listbox --pos-in-label"; do
    name="selftest${m// /}"; name="${name//--/-}"
    run_set "$m --selftest out/$name"
  done
  exit 0
fi
if [[ "$1" == "selftest-tab" ]]; then
  run_set "--key-fix on  --selftest-tab out/v2-tab-keyfix-on"
  run_set "--key-fix off --selftest-tab out/v2-tab-keyfix-off"
  exit 0
fi
if [[ "$1" == "selftest-b" ]]; then
  # A. 真实按键：不绕输入法 / 绕输入法 / 对照的 GPUI 内部模拟
  run_set "--key-fix off --selftest-real out/b-real-keyfix-off"
  run_set "--key-fix on  --selftest-real out/b-real-keyfix-on"
  run_set "--key-fix on  --selftest      out/b-internal-keyfix-on"
  # B. 模拟 VO+→ / VO+←：各方向单独、组合
  run_set "--sync none                --selftest-vo out/b-vo-none"
  run_set "--sync none   --overdraw 5 --selftest-vo out/b-vo-overdraw5"
  run_set "--sync scroll              --selftest-vo out/b-vo-scroll"
  run_set "--sync focus               --selftest-vo out/b-vo-focus"
  run_set "--sync both                --selftest-vo out/b-vo-both"
  run_set "--sync both   --overdraw 5 --selftest-vo out/b-vo-both-overdraw5"
  exit 0
fi
exec "$BIN" "$@"
