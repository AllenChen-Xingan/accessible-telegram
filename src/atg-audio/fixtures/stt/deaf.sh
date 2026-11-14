#!/bin/sh
# 出了版本行，之后既不读标准输入也不退出：写线程堵在满了的管道上（1 秒 PCM 约 190 KB，远超管道容量），应在总期限被杀、照样收尾
echo '{"kind":"hello","proto":1}'
exec sleep 60
