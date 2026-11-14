#!/bin/sh
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"failed","error":"analyze_failed"}'
echo '{"kind":"locale","locale":"en-US","status":"empty"}'
echo '{"kind":"done","count":2}'
