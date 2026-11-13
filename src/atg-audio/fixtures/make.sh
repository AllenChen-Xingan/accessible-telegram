#!/bin/sh
# atg-audio 的解码夹具：macOS `say` 合成的编造句子 → ffmpeg（libopus）编成 Telegram 语音同格式（单声道 48 kHz Opus，OGG 封装）。
# 内容是编的，不含任何真实语音。重新生成：sh src/atg-audio/fixtures/make.sh（要 ffmpeg）。
set -eu
cd "$(dirname "$0")"
t=$(mktemp -d)
say -v Tingting -o "$t/zh.aiff" "今天下午三点开会。"
ffmpeg -hide_banner -loglevel error -y -i "$t/zh.aiff" -ac 1 -ar 48000 -c:a libopus -b:a 16k -application voip zh.ogg
head -c 3000 zh.ogg > zh-truncated.ogg
rm -rf "$t"
