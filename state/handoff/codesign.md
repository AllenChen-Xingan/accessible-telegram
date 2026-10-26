# 交接 · codesign（批次 b11，2026-10-25）

交代：`_tmp/20261025-b11/codesign.md`

## 状态：完成 ✅

- 通读 `scripts/setup-dev-codesign.sh`：只做头部注释写的四步，未改脚本
- 运行 `bash scripts/setup-dev-codesign.sh`（默认名 `atg-dev`），退出码 0；第 ③ 步密码框由人输入，我未操作

## 脚本完整输出（不含密码）

```
① 生成私钥与自签证书（atg-dev，10 年）
subject=CN=atg-dev
notAfter=Oct 23 02:42:37 2036 GMT
X509v3 Extended Key Usage: critical
    Code Signing
② 导入登录钥匙串（只授权 /usr/bin/codesign 使用私钥）
③ 设为「信任用于代码签名」——系统会弹框要你的登录密码
④ 确认：
  1) B1C8869CA718E08BBB17F17058D66646F051C245 "atg-dev"
完成。以后签名用：codesign -s "atg-dev" <程序>
```

## 没做的
- 无 git 写操作、未跑 cargo、未碰钥匙串其他条目
