#!/usr/bin/env bash
# 编 atg-stt（本机端侧转写的 Swift 辅助程序，#59）到 _build/<时间戳>/atg-stt。照 src/tdlib-build 的先例：脚本编，不用 build.rs
# （arch 定 2026-11-13：免得每次 cargo 都拉起 swiftc、四条线并行时都背上 Swift 工具链）。
#
# 用法（在仓库任意位置跑都行）：
#   bash src/atg-audio/stt-helper/build.sh          # 编一次（约 2 秒，要领编译名额）；最后一行打印路径
#   bash src/atg-audio/stt-helper/build.sh path     # 只打印最近一次编好的路径（没有就退出码 1）
# 运行时怎么找到它、找到后怎么验（属主、权限）：正本在 src/atg-audio/src/stt/locate.rs 模块头（arch 定 2026-11-14）。
# 编完设成 0755：组或其他人可写的会被判 helper_unsafe、不执行。
# 前提：macOS 26 起的系统与 SDK（Xcode 26 起；SpeechAnalyzer / SpeechTranscriber 是这一版才有的），系统设置里装好要用的听写
#   语言模型。系统或 SDK 旧了会直接报一堆 Swift 编译错误，先查这两条。
# 签名：可选。编出来是临时签名，直接能用（exp008 实测不弹授权框）；要和 atg-cli 一样用 Apple Development 签，照 #38 跑
#   scripts/dev-sign.sh。
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
src="$root/src/atg-audio/stt-helper/main.swift"

latest() {
  # _build 下也有 TDLib 的时间戳目录：取「含 atg-stt 的」里最新的，不是最新的目录
  local p
  p="$(ls -1 "$root"/_build/[0-9]*-[0-9]*/atg-stt 2>/dev/null | sort | tail -1 || true)"
  [[ -n "$p" && -x "$p" ]] && echo "$p"
}

if [[ "${1:-}" == "path" ]]; then
  p="$(latest)" || { echo "✗ 还没编过：bash src/atg-audio/stt-helper/build.sh" >&2; exit 1; }
  echo "$p"; exit 0
fi

command -v swiftc >/dev/null || { echo "✗ 没有 swiftc（装 Xcode 命令行工具）" >&2; exit 1; }
out="$root/_build/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
swiftc -O -parse-as-library "$src" -o "$out/atg-stt"
# 不跟 umask 走：组或其他人可写的，stt::locate 判 helper_unsafe、不执行
chmod 0755 "$out/atg-stt"
echo "$out/atg-stt"
