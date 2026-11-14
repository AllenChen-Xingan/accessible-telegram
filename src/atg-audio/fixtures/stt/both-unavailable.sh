#!/bin/sh
echo '{"kind":"hello","proto":1}'
cat > /dev/null
echo '{"kind":"locale","locale":"zh-CN","status":"failed","error":"unavailable"}'
echo '{"kind":"locale","locale":"en-US","status":"failed","error":"unavailable"}'
echo '{"kind":"done","count":2}'
