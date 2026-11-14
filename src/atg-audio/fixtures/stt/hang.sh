#!/bin/sh
# 出了版本行，之后不关标准输出、也不退出（应在总期限被杀）
echo '{"kind":"hello","proto":1}'
cat > /dev/null
exec sleep 60
