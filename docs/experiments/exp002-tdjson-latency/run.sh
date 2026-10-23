#!/bin/bash
# exp002 一条命令复现：在项目根目录执行  bash docs/experiments/exp002-tdjson-latency/run.sh
# 会弹出若干次小窗口（界面段与冷启动各一批），跑完自动关闭。结果写到 results/。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
EXP="$ROOT/docs/experiments/exp002-tdjson-latency"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
export CARGO_TARGET_DIR="$ROOT/_build/20261023-154000"
cd "$EXP"
# 默认 debug（GPUI 未优化 → 界面段数字是上界）；PROFILE=release bash run.sh 跑优化版
PROFILE="${PROFILE:-debug}"
if [ "$PROFILE" = release ]; then cargo build --release --bins; else cargo build --bins; fi
BIN="$CARGO_TARGET_DIR/$PROFILE"
mkdir -p results
"$BIN/td_bench" | tee results/${PROFILE}_td_bench.txt
"$BIN/ui_bench" --items 50  | tee results/${PROFILE}_ui_bench_50.txt
"$BIN/ui_bench" --items 500 | tee results/${PROFILE}_ui_bench_500.txt
"$BIN/cold_start" --runs 20 --items 50 | tee results/${PROFILE}_cold_start.txt
