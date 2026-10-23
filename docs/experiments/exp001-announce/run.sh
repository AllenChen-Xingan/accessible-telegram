#!/bin/sh
# 一条命令跑起来。从任何目录调用都行。
#   ./run.sh          手动模式（开 VoiceOver 试听，见 人工试用.md）
#   ./run.sh --auto   自动模式（进程内激活无障碍 + 截获通知，打印结果表后退出）
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
export CARGO_TARGET_DIR="$ROOT/_build/20261023-154000"
cd "$HERE"
exec cargo run -- "$@"
