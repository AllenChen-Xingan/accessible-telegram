#!/bin/sh
# 一条命令跑起来。从任何目录调用都行。
#   ./run.sh          手动模式（开 VoiceOver 试用，见 人工试用.md）
#   ./run.sh --auto   自动模式（进程内激活无障碍 + 截获通知，七种模式各走一遍，打印结果表后退出）
# 可调：EXP004_IDLE_MS（模式 4 静候毫秒，默认 3000）· EXP004_MS_PER_CHAR（模式 5 每字估算毫秒，默认 180）
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
export CARGO_TARGET_DIR="$ROOT/_build/20261023-154000"
cd "$HERE"
exec cargo run -- "$@"
