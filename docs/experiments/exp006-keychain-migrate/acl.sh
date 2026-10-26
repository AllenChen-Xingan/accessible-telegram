#!/bin/bash
# 只显示 atg.exp006 条目的访问名单与分区名单（打码团队号、证书哈希、证书名）
security dump-keychain -a 2>/dev/null | awk '
  /^keychain: / { if (hit) printf "%s", blk; blk=""; hit=0 }
  { blk = blk $0 "\n" }
  /"svce"<blob>="atg.exp006"/ { hit=1 }
  END { if (hit) printf "%s", blk }' \
 | grep -E 'authorizations|description:|requirement:|applications|"acct"' \
 | sed -E 's/(teamid:....)[A-Z0-9]*/\1…/g; s/(H")(.{8})[^"]*"/\1\2…"/g; s/(subject\.CN\] = )"[^"]*"/\1"<名>"/g; s/(cdhash:.{8})[0-9a-f]*/\1…/g'
