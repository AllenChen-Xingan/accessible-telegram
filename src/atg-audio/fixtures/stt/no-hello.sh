#!/bin/sh
# 假辅助程序：不出版本行，一直睡（应在握手期限被杀）
cat > /dev/null
exec sleep 60
