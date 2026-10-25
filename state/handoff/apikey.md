# 交接 · apikey（批次 b3 · 2026-10-25）

## 手上
用 Claude in Chrome 申请 Telegram api_id / api_hash，写入 `.env.local`。

## 做到哪
- 已读交代与 common.md，已向大总管报到
- 已加载 Chrome 工具；`tabs_context_mcp` 报「Browser extension is not connected」，`list_connected_browsers` 返回空 → **卡住，未打开任何页面，未填任何东西**

## 卡点（待人处理）
1. Chrome 扩展未连接：需人打开 Chrome、确认 Claude 扩展已装并登录同一 claude.ai 账号（首次装可能要重启 Chrome）
2. 登录方式调整：我的安全规则不允许代填登录码（属于认证凭据）。改为：我开好标签页到 my.telegram.org，**人自己在该页填手机号和登录码**登录；登录后由我进 API development tools 读值/建应用、写 `.env.local`、登出、关标签
3. 若需新建应用（提交表单），请人在对话里确认一次

## 下一步
等大总管回复「扩展已连上 + 同意上述流程」后重试 `tabs_context_mcp`。

## 流水
- 报到 → 加载工具 → 扩展未连接 → 停下回报
- 收到大总管转来的手机号（未落盘）；复查扩展仍未连接；回复：号码与登录码请人本人在页面填，等「扩展已连上」
- 扩展已连上；在 Claude 标签组开 my.telegram.org 登录页；请人自己登录，等「已登录」
- 人已登录；/apps 无既有应用；填好表单（title「atg client」因最少 5 字符，short name atgclient，Desktop），停在提交前，等人自己点 Create application
- 人已提交建应用；ATG_API_ID 写入 .env.local（600，已确认 git 忽略）；api_hash 被浏览器工具的敏感值保护拦下，留空行请人自己粘；暂不登出，等「hash 已填」
