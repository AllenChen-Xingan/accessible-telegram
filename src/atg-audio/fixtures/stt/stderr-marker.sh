#!/bin/sh
# 在标准错误写暗号、然后正常出结果：暗号不许出现在任何输出与错误里（标准错误接 null）
echo '暗号XYZZY' >&2
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"failed","error":"analyze_failed"}'
echo '暗号XYZZY' >&2
echo '{"kind":"done","count":1}'
exit 1
