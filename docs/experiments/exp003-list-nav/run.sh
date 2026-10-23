#!/bin/zsh
# 一条命令跑起来。用法：
#   ./run.sh                 人工试用（默认模式：位置只放 aria 属性）
#   ./run.sh --pos-in-label  人工试用（对照：「第 N 项，共 M 项」拼进名称）
#   ./run.sh selftest        自动验证：四种模式各跑一遍，结果写到 out/
# ⚠️ 会编译 GPUI，算重活，先向大总管申请名额
set -e
HERE="${0:A:h}"
ROOT="${HERE:h:h:h}"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
export CARGO_TARGET_DIR="$ROOT/_build/20261023-154000"
cd "$HERE"
cargo build
BIN="$CARGO_TARGET_DIR/debug/exp003-list-nav"
if [[ "$1" == "selftest" ]]; then
  mkdir -p out
  rc=0
  for m in "" "--pos-in-label" "--listbox" "--listbox --pos-in-label"; do
    name="selftest${m// /}"; name="${name//--/-}"
    echo "== 模式 ${m:-默认} → out/$name"
    "$BIN" ${=m} --selftest "out/$name" || rc=$?
  done
  exit $rc
fi
exec "$BIN" "$@"
