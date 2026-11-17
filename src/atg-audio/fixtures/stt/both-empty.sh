#!/bin/sh
# 假辅助程序：两个语种都跑成功、都没听出内容
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"empty"}'
echo '{"kind":"locale","locale":"en-US","status":"empty"}'
echo '{"kind":"done","count":2}'
