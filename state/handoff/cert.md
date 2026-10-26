# 现状页 · 手下 cert（批次 b15）

- **手上**：领 Apple Development 免费证书，给 #38 方案乙实测用（交代 `_tmp/20261026-b15/cert.md`）
- **结果**（2026-10-26）：✅ 证书已建
  - 签名身份：`Apple Development`，SHA-1 前 8 位 `DE6F6A93`；团队号前 4 位 `XWFX`（免费个人团队）；有效期到 2027-10-26
  - 试签的小程序 `codesign -dvv` 显示 `TeamIdentifier=XWFX…`，证书链到 Apple Root CA → 分区应能记成 `teamid:`（交给 keychain 实测）
  - 没弹钥匙串框、没弹协议框；没注册设备（命令行工具不要描述文件）
- **过程**：
  - 第 1 步冒烟 ✗（System Events `-1743`，没自动化权限）→ 人选了 A：人自己在 Xcode 登录
  - 最小 C 命令行工程 + `xcodebuild -allowProvisioningUpdates build`，自动签名时 Xcode 自动建了证书
  - 坑：Xcode 27 的团队列表不在 `com.apple.dt.Xcode`，在 `com.apple.dt.xcodebuild` 的 `IDEProvisioningTeamByIdentifier`
  - 坑：系统默认开发目录是 CommandLineTools，用 `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` 逐条指定，没改系统设置
- **清理**：`cert-proj/` 已删；DerivedData 里的 `certprobe-*` 已删。Xcode 保持登录。Xcode 设置窗口我关不了（没界面脚本权限），由人自己关
- 只写了 `cert-proj/`（已删）和本页；没碰已有钥匙串条目、没动 `atg-dev`；没做 git 写操作
- **待验收**
