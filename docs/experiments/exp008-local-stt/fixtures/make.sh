#!/bin/sh
# exp008 夹具：全部用 macOS `say` 合成的编造句子，经 ffmpeg libopus 24 kbps 编码（与 Telegram 语音同格式，编码器不同）再由系统 AudioToolbox 解码成 48 kHz 单声道 f32le（与正式管线同一解码路径）。
# 用法：sh make.sh <输出目录>。产物不入库（几百 KB，随时可再生）。
set -eu
out=$1; mkdir -p "$out"; cd "$out"
say -v Tingting -o zh.aiff  "今天下午三点在图书馆二楼开会，记得带上笔记本电脑。"
say -v Samantha -o en.aiff  "Please bring the quarterly report to the meeting at three o'clock."
say -v Tingting -o mix.aiff "我明天要交 quarterly report，然后跟 David 开个 meeting，地点在 Starbucks。"
say -v Tingting -o long.aiff "小明周末打算去西湖边骑自行车。他先去超市买了两瓶矿泉水和一袋面包，然后在地铁站等朋友。朋友迟到了二十分钟，说是因为公交车坏在半路上。他们骑到断桥的时候，天开始下小雨，于是找了一家茶馆坐下来喝龙井。茶馆老板说，这里的茶叶都是早上刚采的，价格比外面便宜一半。雨停以后，他们继续往前骑，一直骑到太阳落山才回家。晚上小明在日记里写道，这是今年最开心的一天。"
for f in zh en mix long; do
  ffmpeg -hide_banner -loglevel error -y -i $f.aiff -ac 1 -ar 48000 -c:a libopus -b:a 24k -application voip $f.ogg
  afconvert -f caff -d LEF32@48000 -c 1 $f.ogg $f.caf
  ffmpeg -hide_banner -loglevel error -y -i $f.caf -f f32le -c:a pcm_f32le $f.f32
done
