#!/bin/bash
# exp009：跑已编好的自测二进制（编译要先领名额：见 README「跑法」）。
# 会开一个小窗口、抢前台 5–20 秒后自动退出；30 秒还没退就强杀。人关窗即停。
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
MAIN="$HERE/../../../../../telegram accessibility "
BIN="${CARGO_TARGET_DIR:-$MAIN/_build/20261114-exp009}/debug/exp009-gpui-head"
[ -x "$BIN" ] || { echo "没找到 $BIN，先编：见 README"; exit 1; }
mkdir -p "$HERE/out"
# 共同规矩第 16 条：VoiceOver 开着就不跑（会被念出来），除非人同意后设 EXP009_VO_OK=1
VO=$(defaults read com.apple.universalaccess voiceOverOnOffKey 2>/dev/null || echo unknown)
if [ "$VO" = "1" ] && [ "${EXP009_VO_OK:-}" != "1" ]; then
  echo "VoiceOver 开着：先经大总管问人；同意后 EXP009_VO_OK=1 ./run.sh ..."; exit 2
fi
"$BIN" "$@" > "$HERE/out/stdout.txt" 2> "$HERE/out/stderr.txt" &
PID=$!
( sleep 30; kill "$PID" 2>/dev/null && echo "超时强杀" >> "$HERE/out/stderr.txt" ) &
GUARD=$!
wait "$PID"; CODE=$?
kill "$GUARD" 2>/dev/null
echo "退出码 ${CODE}；结果 out/result-<模式>.json，树 out/tree-end-<模式>.json"
exit $CODE
