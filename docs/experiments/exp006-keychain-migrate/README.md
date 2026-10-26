# exp006-keychain-migrate · 旧条目迁到 Apple Development 身份：点一次「始终允许」后重编还弹不弹（issue #38）

> 2026-10-26 · 手下 signing · macOS 27.0.1 · 登录钥匙串（传统文件钥匙串）· Apple Development 身份（SHA-1 前 8 位 `DE6F6A93`，团队号前 4 位 `XWFX`，有效到 2027-10-26）· 旧身份 `atg-dev`（自签名，SHA-1 前 8 位 `B1C8869C`）
> **结果：迁移可行。旧条目被新身份签的程序第一次读时弹一次框，人点「始终允许」后，之后重编的版本都静默读到、不弹框。真条目 `atg.tdlib-db-key` 的 `test`、`main` 已照此迁完。另发现：迁移后新身份能读、不能删。**
> **时间边界**：只在这一版系统上测过。**规模边界**：实验条目用 C 探针（`SecItemAdd` / `SecItemCopyMatching`，与 `security-framework` 同路）；真条目用 Rust 只读小程序（直接调 `security-framework` 的 `get_generic_password`）和真 `atg-cli auth state`。术语见 `../exp005-keychain-acl/README.md` 开头「先说几个词」。

## 假设

exp005 结论 3 的后一半、「乙本体」第 4 条：新签名版第一次读旧条目时人点「始终允许」，会把新程序加进访问名单、把 `teamid:…` 加进分区名单，此后重编不再弹（推断，未测）。

## 做法

实验条目：服务名 `atg.exp006`，账户 `exp-mig`。同一份 `probe.c` 换编译期常量编 A–F 六版（cdhash 各不同）：

| 版 | 签名 | 用途 |
|---|---|---|
| A | atg-dev，`-i atg.dev` | 建条目（= 真条目当年的建法） |
| B | Apple Development，`-i atg.dev`（`scripts/dev-sign.sh`） | 开交互读，人点「始终允许」 |
| C、D | 同 B | 没点过框，关交互静默读（= 「重编以后」） |
| E | 临时签名 | 反例 |
| F | Apple Development，`-i atg.other` | 反例：同身份、别的标识 |

关交互（`SecKeychainSetUserInteractionAllowed(false)`）时，本该问人的情形直接失败（本机 `-25293`），不弹框；开交互的 `getui` 才会弹。

真条目：`migread/` 编三版（编译期常量 `NONCE=1/2/3`），都用 `scripts/dev-sign.sh` 签。1 号开交互读 `test`、`main`（人各点一次「始终允许」），2、3 号关交互静默读。只读，只打印状态码与长度，从不新建、改、删。

## 结论（实测）

1. **迁移可行**：B 开交互读 → 人点「始终允许」→ `status=0 len=32`。之后 C、D 静默读都是 `0 len=32`：**不弹**。A（atg-dev 原版）照读
2. **机制**：「始终允许」把 Apple Development 的指定需求加进访问名单（atg-dev 那条还在），把 `teamid:XWFX…` 加进分区名单（旧 `cdhash:…` 还在）。`acl.sh` 看到：
   ```
   decrypt …  applications (2):  identifier "atg.dev" and anchor apple generic and certificate leaf[subject.CN] = "<名>" and …
                                  identifier "atg.dev" and certificate leaf = H"b1c8869c…"
   partition_id  description: cdhash:adb5399e…, teamid:XWFX…
   ```
3. **反例**：E（临时签名）、F（同身份、标识 `atg.other`）→ `-25293`，拿不到
4. **新身份能读、不能删**：D `del` → `-25244`（errSecInvalidOwnerEdit）；A（建条目的原版）`del` → `0`。条目的属主仍是建它的那一版。**推断**（未测）：要删迁移过的条目，得用建条目的原版程序，否则要人输登录密码；跟 atg-dev 证书删没删无关（程序自带签名；新签的 atg-dev 程序 cdhash 不在分区名单里）。所以 exp005 D 题的退路「删了重建」用新身份做不成
5. **真条目**（2026-10-26 10:43）：1 号读 `test`、`main` 各 `0 len=32`（人点两次「始终允许」）；2、3 号静默读四次全 `0 len=32`，**不弹**。两条的分区名单都加上了 `teamid:XWFX…`
6. **真程序**：TDLib 版 `atg-cli` 经 `cargo run`（runner 签名）跑 `auth state`：`test` → `wait_phone`、`main` → `ready`，各约 1 秒，**不弹**；本地库能打开（钥匙对）

## 清理记录

`atg.exp006` / `exp-mig` 由 A 删除；atg-dev 身份 2026-10-26 由人亲手删除，删后用 `migread` 3 号静默读 `test`、`main` → 各 `0 len=32`（不弹），`find-identity` 只剩 Apple Development；`security dump-keychain | grep -c '"svce"<blob>="atg.exp006'` → **0**。真条目全程只读。

## 文件

- `probe.c`：C 探针，`add|get|getui|del <exp*-账户名>`，只碰 `atg.exp006`
- `acl.sh`：只显示 `atg.exp006` 条目的访问名单与分区名单（团队号、证书哈希、证书名打码）。**不会弹框**
- `migread/`：Rust 只读小程序，`atg-migrate-read quiet|ui test|main`，只读真条目 `atg.tdlib-db-key`，只打印状态码与长度。⚠️ `ui` 会弹框，跑之前要先告诉人点哪个

```sh
# 探针（atg-dev 已于 2026-10-26 删除，对照组不可复现：A 版那种「atg-dev 建的旧式条目」建不出来。
# 续签时演「旧证书建 → 新证书读」，改用旧 Apple Development 证书签 A 版）
FW=(-framework Security -framework CoreFoundation)
clang -O0 -DNONCE='"B"' -c probe.c -o pB.o && clang "${FW[@]}" pB.o -o pB    # 先编成固定名 .o 再链接，产物才稳定（exp005 取证 8）
bash ../../../scripts/dev-sign.sh pB

# 只读小程序（产物放仓库外）
NONCE=1 cargo build --manifest-path migread/Cargo.toml --target-dir "$(mktemp -d)"
```

## 续签时怎么用（2027-10-26 前）

1. 领新证书后，旧、新两个 Apple Development 身份并存：用旧的签 A 版建 `atg.exp006`，新的签 B 版开交互读（人点「始终允许」），再编 C 静默读——团队号相同时推断不弹（未测）
2. 演过再用 `migread/` 对真条目做同样三步
3. 都过了，把 `.env.local` 的 `ATG_CODESIGN_ID` 改成新 SHA-1（或删掉旧证书，让自动选择只剩一个）

## 审查

- 随 #38 落地一起过 `atg-review`（三轮，第 2、3 轮无 P0/P1），记录见 `state/handoff/signing.md`
- 待 codex 补审（#32）
