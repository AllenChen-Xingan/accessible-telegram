#!/bin/sh
# 假辅助程序：zh 有字、en 空。内容是编的
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"text","conf":0.9,"segments":[{"text":"今天下午开会","start_s":0.0,"end_s":1.5}]}'
echo '{"kind":"locale","locale":"en-US","status":"empty"}'
echo '{"kind":"done","count":2}'
