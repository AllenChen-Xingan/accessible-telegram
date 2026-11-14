#!/bin/sh
# 坏 JSON 里带暗号：错误里不许出现它
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"text","segments":[{"text":"暗号XYZZY"'
