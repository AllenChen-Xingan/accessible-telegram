# 现状页 · 手下 signing（批次 b16，issue #38 落地）

交代：`_tmp/20261026-b16/signing.md`　工作目录：本 worktree（分支 `feat/38-dev-signing`）　正本：`docs/experiments/exp005-keychain-acl/README.md`「乙本体」

## 手上
开发版改用 Apple Development 身份签名 + 迁移 `atg.tdlib-db-key` 的 test、main。

## 做到哪
- ✓ 报到
- ✓ worktree 里软链 `.env.local`、`_build` → MAIN（⚠️ `_build` 软链在 `git status` 里显示为未跟踪：`.gitignore` 的 `_build/` 只匹配目录。提交时别 `git add -A`）
- ✓ `scripts/dev-codesign-id.sh`（选身份：环境变量 / `.env.local` 的 `ATG_CODESIGN_ID`，否则恰好一个 Apple Development；核身份必须是有效的 Apple Development）· `scripts/dev-sign.sh` · `scripts/dev-sign-run.sh`（runner；CI 里或 `ATG_DEV_SIGN=0` 不签）· `.cargo/config.toml` · `scripts/setup-dev-codesign.sh` 改成只读检查
  - 自测：选中身份前 8 位 DE6F6A93；atg-dev 的 SHA-1、乱写的 SHA-1、非 40 位 → 退出码 2；签出 `Identifier=atg.dev`、带团队号；runner 能跑
- ▶ 迁移演练（实验条目 `atg.exp006` / `exp-mig`，探针在会话 scratchpad，不进仓库）
  - ✓ ① atg-dev 版（A）建条目 → A 读 `0 len=32`；Apple Dev 版 B、C 静默读 → `-25293`。分区 `cdhash:adb5399e…`，访问名单 atg-dev 需求（= 真条目现状）
  - ✓ ② B 开交互读 → 人点「始终允许」→ `0 len=32`
  - ✓ ③ 访问名单加了 Apple Dev 需求（atg-dev 那条还在），分区名单变成 `cdhash:adb5399e…, teamid:XWFX…`；没点过框的 C、D 静默读 → `0 len=32`（**不弹**）。反例：临时签名版、同身份但标识 `atg.other` → `-25293`
  - ⚠️ **新发现**：迁移后新身份**能读不能删**（`SecItemDelete` → `-25244` errSecInvalidOwnerEdit，属主仍是建条目的 atg-dev 版）。keystore 从不删，不影响日常；但 E（删 atg-dev 证书）之后，要删这两条真条目得人在框里输密码；也意味着 exp005「退路：删了重建」用新身份做不了
  - ✓ 清理：实验条目由原建立者 A 删掉，`atg.exp006` 剩 0
- ▶ 真条目迁移（大总管定甲：scratchpad 只读小程序，直接 `get_generic_password`，不走 get_or_create；`-i atg.dev` + 同身份签）
  - ✓ 名额领到；编好三版 `atg-migrate-read-1/2/3`（cdhash a40f5949 / c95b8cdb / c7d6afdf），都签好；还没读真条目
  - ✓ 2026-10-26 10:43 人点两次「始终允许」：1 号开交互读 test → `0 len=32`，main → `0 len=32`
  - ✓ 重编的 2、3 号静默读 test、main → 四次都 `0 len=32`，**不弹**。两条的分区名单都加上了 `teamid:XWFX…`（旧 cdhash 留着）
  - ✓ 名额已还；真条目只读，没建、没改、没删
  - 剩：验收时用真 atg-cli 跑一次（乙，要领名额、连服务器）

- ✓ 规约 / 注释 / N2 指针 / 文档签名写法（src/README、atg-cli/README、live_wait_phone 头注释、login-main.sh）
- ✓ 名额内：fmt ✓ · clippy ✓ · `cargo test --workspace` 255 passed / 0 failed（11 个测试程序）· check-no-debug ✓ · check-manifests ✓；抽查测试程序签名 `Identifier=atg.dev`、团队号前 4 位 XWFX（runner 生效）。名额已还
- ▶ 自审第 1 轮（atg-review）

- ✓ 自审第 1 轮：P1×1（没身份时 `cargo test` 必红、绕法没写）+ P2×7 + P3 若干，已修：runner 失败时提示 `ATG_DEV_SIGN=0`、README / testing 修订记录写明；多身份时列出每个 SHA-1 与有效期；规约写明**迁移过来的真条目仍认旧 atg-dev 需求与旧 cdhash、新身份不能删**，「拿不到」测试限定为新建条目；README 写首次迁移要点「始终允许」；续签说明；术语指针；只限 Apple 芯片目标；规则写法统一
- ✓ 自审第 2 轮：**无 P0/P1**；P2×5 已修——领证书前要完整 Xcode；config-privacy「正式发布签名照样在两边访问名单里」与已定机制矛盾 → 改为「正式版第一次读要人允许一次，留到正式版定」（⚠️ 规约冲突，报大总管）；testing.md 正文补 runner / `ATG_DEV_SIGN=0` 前提；atg-cli README 指向 login.md 的作废签名命令 → 改指 src/README；runner 先认 `LOCAL_TDLIB_PATH/lib`，带 TDLib 的程序找不到库时提示。P3 修了大部分（机制提到最前、属主解释、静默签名指针、集成测试自起的二进制不经 runner、反例加「同身份别的标识」、有效期读法去重为 `dev-codesign-id.sh --not-after`、失败提示按原因、README 解释 runner 与迁移适用范围）；没修：testing §4 标题「CI 每次都跑」与「钥匙访问（本机，非 CI）」行的归类矛盾（原有，不在本次范围）
- ✓ 自审第 3 轮：**无 P0/P1** → 第 2、3 轮连续干净，**达到停止线**。P2 已修：规约引用的迁移演练出处指向本页（探针待收录）；testing §4 前提句改为「除『钥匙访问』行外不碰钥匙串」，写明该行与 `live_` 层必须签名；**更正**「删 atg-dev 证书后删条目要输密码」——能不能删看建条目的原版 atg-dev 程序在不在，与删不删证书无关（推断，未测），E 题面随之更正。P3 已修：`cargo test …` 字面省略号改 `--workspace`；README 把 `ATG_DEV_SIGN=0` 挪到 cargo test 旁、写明谁会读到 test / main；正式版一句写出推理
- 待 codex 补审（#32）

- ✓ 乙（大总管授权、领名额）：TDLib 版 atg-cli 经 cargo run（runner 签名）跑 `auth state`：test → `wait_phone`、main → `ready`，各约 1 秒，**不弹**；名额已还
- ✓ exp006 收录（大总管授权）：`docs/experiments/exp006-keychain-migrate/`（README + `probe.c` + `acl.sh` + `migread/` 源码，无编译产物）；索引加一行；config-privacy / testing / setup 脚本的出处指针改指 exp006
- ✓ 删 atg-dev（人定 E：删）：本会话的 `security delete-identity` 被自动权限检查拦下（没绕过），**人亲手删除**（2026-10-26）；大总管核 find-identity 只剩 Apple Development，我又核一遍；`migread` 3 号静默读 test、main → 各 `0 len=32`，不弹。exp005 README（E 补记、run.sh / run-b.sh 两行）、exp006 README、setup 脚本已注明「atg-dev 已于 2026-10-26 删除，对照组不可复现」

## 下一步
大总管提交 → 验收后在本页写「已收尾」

## 交大总管
- 看板：续签提醒（2027-10-26 前）由大总管写；看板第 27 行还写「开发证书 atg-dev」，已过时。`setup-dev-codesign.sh` 末尾已指向看板
- 迁移演练的探针（C 版 `atg.exp006`、Rust 只读小程序）在会话 scratchpad，没进仓库；续签时要再演，建议收进 `docs/experiments/exp006-keychain-migrate/`（不在我的可写清单里，等你定）

## 要人定的事
- **E（删 atg-dev 证书）现在可以定了**（迁移验证已做完）：①删（`security delete-identity`，要人点框）②继续留。代价：①exp005 `run.sh` / `run-b.sh` 的 atg-dev 对照组跑不了；②多一张不用的证书（dev-codesign-id 已拒收它，误用不了）。删不删都**不影响**迁移过的条目：旧 atg-dev 程序照能读、新身份照不能删（第 3 轮自审更正了我先前「删了以后删条目要输密码」的说法）。建议：①删，不急
