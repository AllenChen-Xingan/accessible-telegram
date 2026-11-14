#!/bin/sh
# 假辅助程序：同 ok-zh，但收完 PCM 后歇 1 秒再出结果——给测试留出「转写中」这段时间推更新。内容是编的
echo '{"kind":"hello","proto":1}'
cat > /dev/null
sleep 1
echo '{"kind":"locale","locale":"zh-CN","status":"text","conf":0.9,"segments":[{"text":"今天下午开会","start_s":0.0,"end_s":1.5}]}'
echo '{"kind":"locale","locale":"en-US","status":"empty"}'
echo '{"kind":"done","count":2}'
