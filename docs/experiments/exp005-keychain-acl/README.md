# exp005-keychain-acl · 开发版重编后读钥匙串为什么弹框，怎么根治（issue #38）

> 2026-10-26 · 手下 keychain · macOS 27.0.1（26A434）· 登录钥匙串（传统文件钥匙串）· 开发证书 `atg-dev`（自签名，SHA-1 `B1C8869C…F051C245`）；第二轮加测 Apple Development 证书（团队号前 4 位 `XWFX`）
> **结果：方案乙（改用 Apple Development 证书）实测通过，是根治办法。实测中发现签名用的私钥对 codesign 不弹框，同用户任何进程都能自己签一个「看起来是 atg」的程序、不弹框拿到正式钥匙（「静默签名」，比现状宽，详见「安全口径」）。人已在知情后拍板（2026-10-26）：接受静默签名（A1），采用乙；详见「已定」。**
> **时间边界**：只在这一版系统上测过；分区名单是 macOS 10.12 起的行为，苹果可能再改。**规模边界**：用 C 小探针模拟，接口与 `security-framework` 的 `set_generic_password` / `get_generic_password` 相同（`SecItemAdd` / `SecItemCopyMatching`）；没跑 Rust 程序，没碰真条目 `atg.tdlib-db-key`。

## 先说几个词

- **cdhash**：程序内容的哈希。源码改一个字、重编一次就变
- **指定需求**（designated requirement）：签名里写的「怎样算同一个程序」。用 atg-dev 签时是「标识叫 X，且证书是这一张」，跟内容无关，重编不变
- **临时签名**（ad-hoc）：没有证书的签名，`cargo build` 的产物默认就是这种，指定需求只能是 cdhash
- **团队号**（Team ID）：苹果给开发者账号的编号，只有苹果签发的证书才带；自签名证书没有
- **钥匙串条目的两道关**：① **访问名单**——记「哪些程序可以读」，按指定需求认；② **分区名单**（partition list，macOS 10.12 起）——记「哪一类签名可以读」，有团队号记 `teamid:…`，苹果自带工具记 `apple-tool:`，什么都没有就只能记 `cdhash:…`。两道都过才不问人
- **强化运行时**（hardened runtime，`codesign --options runtime`）：签名时打开的一组保护，其中一条是忽略 `DYLD_INSERT_LIBRARIES`（往程序里塞别的代码的环境变量）
- **库验证**（library validation）：开了强化运行时，程序只许载入苹果签的库、或与自己**同团队号**签的库；别的一律拒载（dyld 报 `different Team IDs`）。程序依赖的库所依赖的库也算
- **rpath / `@rpath`**：写在程序里的「去哪找动态库」。`libtdjson` 自称 `@rpath/libtdjson…`，所以程序里得有一条 rpath 指向它所在的目录。`DYLD_LIBRARY_PATH` 是运行时用环境变量补指路，强化运行时会忽略它
- **`anchor apple generic`**：指定需求里的「证书链通到苹果的根」，即苹果签发的证书
- **正式档案 / 测试档案**：「正式档案」是连 Telegram 正式服务器、存着人真实账号的那套本地库，钥匙条目 `atg.tdlib-db-key` / `main`，下称「正式钥匙」；「测试档案」连测试服务器，条目 `… / test`
- **钥匙串弹框**：macOS 问「某程序想访问钥匙串条目」的对话框，按钮有「拒绝 / 允许 / 始终允许」，有时要输登录密码。「允许」只放行这一次，「始终允许」会改条目的访问控制
- **Apple Development 与 Developer ID**：都是苹果签发、带团队号的证书。Apple Development 是开发用（免费 Apple ID 也能领）；Developer ID 是对外分发正式版用，要付费开发者账号。两者的团队号是否相同本文未核
- **签名身份**：一张证书 + 配套的私钥，私钥也存在钥匙串里、也有自己的访问名单。本文用证书的 **SHA-1**（证书指纹）指代身份，因为身份名里有人名
- **本文引用的拍板**：**N2**（`docs/plan/005_decisions-b3.md`）——开发证书也管正式档案的钥匙、开发版不弹框，人已知「AI 用同一证书签的程序都能拿到正式钥匙」；**P5**（`specs/contracts/config-privacy.md` 第一节第 3 小节末）——人接受「同一用户下的其他程序能读下载下来的明文文件」；**001-8**（`docs/plan/001_arch-stack.md` 第七节第 8 条）——本地库加密，钥匙放钥匙串

## 假设

开工前大家以为：开发版都用同一张证书 `atg-dev`、同一个固定标识签名（`codesign -s atg-dev -i atg.dev`），重编后读钥匙就不弹框（`keystore.rs` 头注释、`state/handoff/tdlib.md`、拍板 005 N2）。#18 冒烟时这个假设被现实推翻：重编后的 `atg-cli` 读 `atg.tdlib-db-key` / `main` 时弹框。

## 做法

C 小探针（`probe.c`）建、读、删服务名 `atg.exp005` 的实验条目。每次读写前先调 `SecKeychainSetUserInteractionAllowed(false)` 关掉交互：本该问人时直接失败，不弹框。本机实测返回 **-25293**（errSecAuthFailed）。「重编」的做法是：同一份源码换一个编译期常量编两版，cdhash 不同，签名方式完全相同。

⚠️ **推断**：「关交互时返回 -25293」就等于「开着交互时会弹框」，这一步没有在开交互的状态下对实验条目验证，因为验证就得弹框。依据是同一配置的真条目在 #18 冒烟时确实弹了框，而且 -25293 只在两道关有一道没过时出现：建条目的那一版读出来是 0。

## 结论

1. **根因**：访问名单按指定需求认，重编也能过；**分区名单却记成 `cdhash:<建条目那一版>`**。自签名证书没有团队号，只能这么记，所以重编后一定过不了。**只改签名方式，在自签名证书下根治不了。**
2. 另一个独立原因：`cargo build` 的产物是临时签名，忘了重签就必弹。重签了也过不了第 1 条那道关。
3. **推断**：「始终允许」是把**发起读取的程序那一类签名的记号**加进分区名单——自签名只能记 cdhash，所以下次重编照弹；有团队号的程序就记 `teamid:…`（这一半是迁移的前提，见「乙本体」第 4 条）。都没实测（要人点框），前一半和 #18 之后「点过始终允许、重编又弹」的现象一致。
4. **第二轮（乙）**：改用有团队号的 Apple Development 证书后，分区名单记 `teamid:…`，重编两版都读到、不弹框，开强化运行时也一样；atg-dev / 临时签名版读不到（取证 10）。**这是根治办法**；人知情后已定采用（A1 + 乙，见「已定」）。乙本体只是换签名身份；若再开强化运行时（#42：另开的「开强化运行时、堵注入」issue），TDLib 的加载要改四处，那部分没在真程序上测过

## 取证（命令与输出摘要）

条目前的甲—辛是方案代号，含义见下文「方案对比」表。

1. **现在的 `atg-cli` 是临时签名**：`codesign -d -r- -vv target/debug/atg-cli` → `flags=0x20002(adhoc,linker-signed)` · `designated => cdhash H"f38b…"`
2. **atg-dev 签出的指定需求稳定**：两版 cdhash 分别是 `73bf…`、`812d…`，用 `codesign -s atg-dev -i atg.exp005` 签过后都是 `designated => identifier "atg.exp005" and certificate leaf = H"b1c8…"`，`TeamIdentifier=not set`
3. **重编版读不到**：A 版建、A 版读 → `status=0 len=32`；B 版读 → `-25293`。临时签名版同样是 `-25293`（`run.sh` 可复现）
4. **原因在分区名单**：`security dump-keychain -a` 的输出用 awk 只留 `atg.exp005` 的块：
   ```
   decrypt … 一栏  applications: …/signA  requirement: identifier "atg.exp005" and certificate leaf = H"b1c8…"   ← 稳定
   partition_id    description: cdhash:73bf37238a231357e3d3f59fe6c5a4dd41c86f71                             ← 建条目那一版
   change_acl      applications (0)                                                                        ← 改访问控制要人输登录密码
   ```
5. **丙**：`security -i` 从标准输入读入 `add-generic-password -s atg.exp005 -a exp-tool -X <实验随机值>`，再用 `security find-generic-password -w` 读 → 两次都拿到 64 个十六进制字符（32 字节），不弹框；分区是 `apple-tool:`，访问名单是 `identifier "com.apple.security" and anchor apple`
6. **己**：`probe adddp`（临时签名版和 atg-dev 版）→ `-34018`（errSecMissingEntitlement，缺权限声明）。签名时带上 `keychain-access-groups` 权限 → 程序一启动就被系统强杀（退出码 137 = SIGKILL：自签名程序不允许声明受限权限）
7. **庚**：`probe addnp`：去掉分区 ACL 那一步 `SecACLRemove` 返回 0，写回 `SecKeychainItemSetAccess` 返回 `-25293`。`probe addacc`：建条目时自带访问对象，分区照样记成 `cdhash:c8c7…`
8. **丁**：见下表与 `run.sh`。小助手前后编了 3 次（换目录也算），cdhash 都是 `489f…`；强化运行时版两次都是 `13b0…`。**非得两步编译**的原因：一步 `clang x.c -o y` 时链接器拿到的是随机命名的临时 `.o`，产物里有随它变的字节（实测两次不同）；先编成固定名字的 `.o` 再链接，产物逐字节相同
9. **注入**：给没开强化运行时、能过两道关的程序设 `DYLD_INSERT_LIBRARIES`，注入的代码用这个程序的身份**读到了钥匙**（`len=32`），也能**删掉**条目，都不弹框（`inject.c`）。同样的注入打到强化运行时版小助手上不起作用：注入代码没运行，小助手照常拒绝了 shell 调用方
10. **乙（第二轮，2026-10-26 人领了免费证书后）**：Apple Development 证书（SHA-1 前 8 位 `DE6F6A93`，团队号前 4 位 `XWFX`，有效到 2027-10-26）。`run-b.sh` 可复现：
    - 指定需求：`identifier "…" and anchor apple generic and certificate leaf[subject.CN] = "…" and certificate 1[field.1.2.840.113635.100.6.2.1] /* exists */`（引号内打码：证书名里有人名）
    - ① A 版建条目 → **分区名单 `teamid:XWFX……`**；A、B、C 三版（cdhash 各不同）读 → 全是 `status=0 len=32`。**不弹框**，包括 codesign 第一次用 Xcode 私钥也没弹
    - ② 同证书开强化运行时的三版读 → 全是 `status=0`
    - ③ atg-dev 签名版、临时签名版读 → `-25293`（规约「开发证书以外拿不到」成立）
    - ④ 注入：普通版 → 注入代码读到 `len=32`；强化运行时版 → 注入不生效
    - ⑤ 强化运行时下的动态库（替身 `libdummy.dylib` 模拟 `libtdjson`，按 `@rpath` 找）：同证书签的库 → 载入；**未签名或临时签名的库 → 拒载**（dyld：`different Team IDs`）；rpath 处没有库、只给 `DYLD_LIBRARY_PATH` → 找不到（强化运行时忽略这个变量）；普通版 + `DYLD_LIBRARY_PATH` → 照常。（①② 能跑，本身就说明同证书签的库按 rpath 载入了）
    - （①–⑤ 与 `run-b.sh` 输出的编号一一对应，探针都链接了替身库；脚本输出里的 `r*` = 普通签名，`h*` = 开了强化运行时）
11. **真程序的库加载现状**（只读查看，2026-10-26）：`otool -L` 看 `_build/20261025-171733/tdlib/lib/libtdjson.1.8.69.dylib`——自称 `@rpath/libtdjson.1.8.69.dylib`，动态链接 `/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib`、`libcrypto.3.dylib`；`codesign -dv` 看这两个 OpenSSL 库：`Signature=adhoc`、`TeamIdentifier=not set`。`otool -l target/debug/examples/live_wait_phone`（已有的 TDLib 版程序）：**没有任何 `LC_RPATH`**

## 方案对比

「实测」= 本目录小实验跑过；**通过 = 重编两次都不弹框**。

| | 方案 | 实测 | 能否根治 | 要改哪些文件 | 对正式发布版 | 安全代价 |
|---|---|---|---|---|---|---|
| 甲 | 只固定签名（同证书 + 固定标识，或显式 `-r=` 写指定需求），或用 cargo runner 每次编完自动签 | ✗ 不通（取证 3） | 否：指定需求本来就稳定，挡人的是分区名单 | — | — | — |
| 乙 | 改用**有团队号**的苹果开发证书（Apple ID 免费「个人团队」的 Apple Development 证书），分区记 `teamid:…` | ✓ **通过**（取证 10）：重编两版都读到，不弹框；开强化运行时也通过；atg-dev / 临时签名版被拒 | 能 | 见下「采用乙的具体做法 · 乙本体」（签名脚本与 cargo runner、setup 脚本、迁移、文档与规约措辞；A1 / A2 下代码逻辑不动，A3 要加「开发版拒开正式档案」）；开强化运行时另有四处（#42） | **未核**：免费个人团队的团队号和以后付费 Developer ID 的团队号是否相同；不同的话，正式版读开发版建的条目要允许一次 | 证书有效一年（到 2027-10-26），要人在 Xcode 里续；指定需求认的是证书名 + 苹果根，续签后名字不变，**推断**仍认（未测）。⚠️ codesign 用这把私钥不弹框，**同用户任何进程都能自己签一个「这个身份 + 标识 `atg.dev`」的程序**拿到钥匙，比现状宽（见「安全口径」）。安全边界与丙基本相同（待人定 A1 / A3 下；A2 效果有限）；乙多的不多，见「安全口径 · 乙 vs 丙」。不开强化运行时的版本还可被注入（取证 9、10；#42 = 另开的「开强化运行时」issue） |
| 丙 | 条目只经 `/usr/bin/security` 建与读，钥匙从 `security -i` 的标准输入进，不上命令行；分区记 `apple-tool:` | ✓ 不弹（取证 5） | 能 | `keystore.rs` 改成起 `security` 子进程 | 无影响 | ⚠️ **同用户任何进程**一条 `security find-generic-password -w` 就能不弹框拿到钥匙；违反 `config-privacy.md`「开发证书以外的签名拿不到」，选它就得改规约和 `testing.md` 的对应测试。与乙在待人定 A1 / A3 下边界基本相同；乙唯一多出的收紧办法 A2 效果有限。比乙差的是：没签名的程序（一段随手写的脚本）也能拿到。可选，但我略偏乙（见待人定 B） |
| 丁 | **取钥匙小助手**：一个很少改的小程序独占条目；每次重编的开发版把它当子进程起，小助手核过调用方的签名再交出钥匙 | ✓ **机制通过**：小助手重编 3 次 cdhash 相同，调用方换了 2 版都读到（`run.sh`）；临时签名调用方被拒（`-67050` = 签名不满足要求）。强化运行时版同样通过 | 能（小助手源码不改的前提下；改了要人再允许一次） | 新增小助手 crate（`src/atg-keyhelper/`）、`keystore.rs`（`Keychain` 改成问小助手）、编译 + 签名 + 安装小助手的脚本、`config-privacy.md` 第 3 小节、`testing.md` 对应测试 | 正式版可直接读（Developer ID 有团队号），也可把小助手打进 `.app`；正式版第一次读开发版建的条目时要允许一次 | ⚠️ **原型的核身份不安全**（见下节「方案丁的已知弱点」）；正式实现的核法**未实测**。小助手**必须开强化运行时**，否则它就是固定的注入靶子（取证 9）。能拿钥匙的程序从现在的「人点过允许的那几版」扩大到「任何一版 atg-dev 签名的程序」；同用户进程又能静默签 atg-dev 程序，所以实际边界与乙相同（同用户任何进程，见「安全口径」） |
| 戊 | 开发版钥匙不进钥匙串，放档案目录里的 `0600` 文件 | 未测（不经过钥匙串，不会弹框） | 能 | `keystore.rs` 加文件版、`config-privacy.md` | 正式版仍用钥匙串 | ❌ 同用户进程都能读；和拍板 001-8「钥匙放钥匙串」冲突。只能当测试档案的退路 |
| 己 | 数据保护钥匙串（`kSecUseDataProtectionKeychain` / 访问组） | ✗ 不通（取证 6） | 否 | — | 带苹果描述文件（provisioning profile）的正式版可用 | 要苹果签发的描述文件；免费个人团队能不能用于命令行程序未核 |
| 庚 | 程序自己改条目：建完后去掉分区 ACL，或建时自带访问对象 | ✗ 不通（取证 7） | 否 | — | — | 改访问控制要人输登录密码 |
| 辛 | 人每次用 `security set-generic-password-partition-list` 把新 cdhash 加进去（只管分区那一道；「始终允许」推断还会顺带改访问名单，见结论 3） | 未测（要登录密码） | 否：每次重编都要人做 | — | — | 每次都要人输密码 |

### 方案丁的已知弱点（自审提出，未实测）

原型（`keyhelper.c`）只在启动时按**父进程 pid** 核一次签名，然后把钥匙写进继承来的标准输出。「核的是谁」和「钥匙交给谁」没有绑在一起：自审指出，同用户进程不靠注入也能让小助手核到一个合法的父进程，而钥匙却落到自己手里。所以原型**只证明了「可复现小程序 + 分区稳定」这个机制**，不是安全实现。

正式实现要做的（都没实测）：
- 用 `socketpair` + `LOCAL_PEERTOKEN` 拿对端的**审计令牌**再核签名。审计令牌里有进程版本号，进程一 `exec` 就变，所以核的身份和通道是绑在一起的
- 补对抗测试：除了「临时签名的老实调用方被拒」，还要测上面那类调用方
- 实现之前请换谱系评审（codex 恢复后）专门审这一块

## 注入风险（独立于选哪个方案）

取证 9 说明：能过两道关的程序只要没开强化运行时，同用户任何进程都能注入它，**不弹框地读到钥匙**，不需要会签名。这比 N2 当时「人已知」的范围（同证书签的程序能拿到）更大。

- **现状**：能过关的只有人点过「始终允许」、文件还在磁盘上的那几版。现在的 `atg-cli` 开发版签名都不带 `--options runtime`
- **乙**：每一版同证书签的程序都能过关，不开强化运行时的那一版就可被注入（取证 10 实测）；开了就挡住。但乙下还有一条强化运行时挡不住、而且更省事的路：同用户进程静默自签（见「安全口径」）。所以 #42 在乙下值不值得做，取决于待人定 A（签名要不要人确认）
- **丁**：小助手开了强化运行时之后，注入被挡住（已实测）。剩下的路：注入某一版 atg-dev 签名的 `atg-cli` 再让它去问小助手，或者直接静默签一个 atg-dev 程序去问（同乙）
- ~~第一轮写的出路「rpath + `disable-library-validation` 权限」~~：**已被乙取代**。有了团队号，库用同一身份签就能载入（取证 10），**不要加 `disable-library-validation`**（加了就放宽库验证，同用户可写目录里被换掉的库也能进来）。具体做法见「开强化运行时要做的」；实现归 #42

## 安全口径：换成乙以后谁能拿到正式钥匙（要人知道，决定下面的拍板）

- **能直接读钥匙的**：用这个 Apple Development 身份、标识 `atg.dev` 签名的任何一版程序，**开不开强化运行时都能读**（取证 10 ①②）。强化运行时管的只是「这个程序能不能被别人注入借身份」
- **真正的边界是「同用户能不能静默签名」**：签名身份的私钥也在钥匙串里、也有访问名单。取证 10 里 codesign 用这把私钥没弹框（实测）；**推断**是 Xcode 领证书时把它设成了对 codesign「始终允许」。所以同用户的任何进程都能自己编一个程序、用这个身份加 `-i atg.dev` 签上，两道关都过，**不弹框拿到钥匙**。强化运行时堵不住这条
- **这比现状宽**：atg-dev 的私钥同样能被 codesign 静默使用（本实验 `run.sh` 每次用 atg-dev 签名都没弹框；另见 `state/coord.md` 2026-10-25），但现有条目的分区名单记的是 cdhash，新签出来的程序对不上，**静默签名在现状下拿不到钥匙**；能拿到的只有人点过允许、还在磁盘上的那几版（以及注入它们）。换成乙之后，范围扩大到**同用户任何进程**。N2 当时人知道的是「AI 用同证书签的程序都能拿到」，没明说「任何进程都能静默签」
- **丁也一样**：丁的小助手认「atg-dev 签名的调用方」，同用户进程照样能静默签一个 atg-dev 程序去问它。所以乙、丁在这一点上边界相同
- **乙 vs 丙**：私钥保持静默签名（下面待人定 A1，A3 下的测试钥匙同理）时，乙和丙的安全边界基本相同，都是同用户任何进程；丙下更省事（一条命令）。乙多出来的不多：① 开发版、正式版走同一套机制；② 没签名、也不懂签名的程序（比如一段随手写的脚本）拿不到，而丙下一条 `security` 命令就够；③ 名义上能收紧（待人定 A2），但效果有限，见 A2
- atg-dev 签名、临时签名、未签名的程序拿不到（实测，取证 10 ③）
- Xcode 里别的项目：用别的标识签，按访问名单里的指定需求（含标识）**推断**读不到（未测）；但它们同样能用静默签名那条路

## 已定（人，2026-10-26）

第一次：先测乙，通了就用乙（乙已实测通过）；注入风险另开 issue（#42）。
第二次（看过「安全口径」之后，由大总管转达）：
- **A → A1**：接受静默签名；开发版照常用正式账号；按 P5 的口径改写 N2（写明「同用户任何进程都能静默签名拿到正式钥匙」）
- **B → 乙**：免费 Apple Development 证书方案
- **C**：#42（强化运行时）缓做，乙先单独落地（开发版照旧靠 `DYLD_LIBRARY_PATH`；经 cargo runner 跑时用 `ATG_TDLIB_LIB`，见「乙本体」第 2 条）
- **D**：`test`、`main` 两个档案都迁移，人各点一次「始终允许」（先用实验条目验证，见「乙本体」第 4 条）
- **E**：atg-dev 证书先留着，迁移验证做完再删。（补记：迁移验证做完后人定删，**atg-dev 已于 2026-10-26 删除，对照组不可复现**——人亲手 `security delete-identity`；迁移见 `../exp006-keychain-migrate/`）

落地按「采用乙的具体做法 · 乙本体」第 1–5、7 条做（第 6 条是 A3 专用，不做）；「开强化运行时要做的」归 #42，缓做。

## 待人定（题面存档；按这个顺序定，后面的取决于前面的）

以下为拍板时的原题面，结果见上「已定」。

**A. 正式钥匙给不给开发版、给到什么程度**（决定其余几题）。题面：乙下同用户任何进程能静默签名拿到正式钥匙，比现状宽。选项：
- A1 接受：同 P5「同用户其他程序能读明文文件」的口径，把范围改写进 N2。代价：乙在安全上只比丙强一点（见「安全口径 · 乙 vs 丙」），#42 的收益也很小（它只堵注入这一条路，而攻击者有更省事的静默签名可走，堵了注入意义不大）
- A2 签名每次要人确认：把这把私钥对 codesign 的「始终允许」去掉（在「钥匙串访问」里改这把私钥的访问控制；具体操作和之后弹不弹都**未测**）。代价：每次编译都要点框；一次 `cargo test` 会编出好几个测试程序，每个都签一次、各点一次。**效果有限**：这个框只说「codesign 想使用某把钥匙」，不显示要签的是哪个程序、谁发起的；同用户的恶意进程调一次 codesign，弹出来的框和正常编译一模一样，人一天点几十次分不出来。所以 A2 实际只挡「人没点」的情形。真要靠它，得另做「签名前先核要签的是什么」的设计（例如私钥只给一个固定的小签名器用），本实验没做
- A3 折中（未测）：开发版只用测试档案，正式档案的钥匙不给开发身份。等于推翻 N2。做法：`main` 条目不迁到开发身份（见 D），开发版打开正式档案时直接拒绝；正式档案留给以后的正式版，在那之前人日常用别的客户端。代价：开发期不能用真账号冒烟（#18 那类只读冒烟就做不了）

我不替人选：更在意省事、接受「同用户进程能拿到」，A1；更在意正式账号、开发期不需要真账号，A3（三个里唯一真正把正式钥匙挡在开发版之外的）；A2 代价高、效果有限，不建议。

**B. 还用不用乙**（随 A）。
- A 选 A1 时：乙与丙安全基本相同。两边的改动量：乙——签名脚本与 cargo runner、`setup-dev-codesign.sh`、迁移、规约措辞（`config-privacy.md`、`testing.md`、N2、`keystore.rs` 注释），代码逻辑不动（见下「乙本体」）；丙——`keystore.rs` 改成起 `security` 子进程（钥匙经标准输入进出），迁移同样要做，`config-privacy.md`「开发证书以外的签名拿不到」那条和 `testing.md` 对应测试要删改。两边工作量相近，安全上也基本相同（乙多的见「安全口径 · 乙 vs 丙」，不多）。建议仍用乙，理由是与正式版同一套机制，且没签名的程序拿不到（规约那条要改写成「认本人的 Apple Development 身份；没用它签名的程序拿不到」，对静默签名不成立，要写明）；选丙也说得过去
- A 选 A2 时：只有乙能做（丙没有对应的框），但 A2 本身不建议，所以这不算乙的实质优势
- A 选 A3 时：正式钥匙两边都不给开发版；测试钥匙在乙、丙下都是同用户进程能拿到（乙下攻击者得先自己签一次名，丙下一条命令）。改动量在上面基础上，两边都要加一段代码：开发版遇到正式档案直接拒绝打开，并加对应测试（见「乙本体」第 6 条）

**C. #42（强化运行时）怎么排**（随 A）。它要改 TDLib 的加载方式（四处，都没在真程序上测过，见下「开强化运行时要做的」）。
- A1：收益很小，建议**缓做**，乙先单独落地，开发版照旧靠 `DYLD_LIBRARY_PATH`（取证 10 ⑤ 实测普通签名版能用；经 cargo runner 跑时要绕开系统 shell 删变量的坑，见「乙本体」第 2 条）
- A2：必需（A2 只管签名那条路，注入不用签名，得靠强化运行时堵），要做就和乙合成一块做；但 A2 本身不建议
- A3：开发版碰不到正式钥匙，#42 可缓做

**D. 迁移现有真条目**（随 A）：新版第一次读时人点「始终允许」——A1 / A2 迁 `test`、`main` 两条（两次框）；**A3 只迁 `test`**，`main` 不交给开发身份。万一「始终允许」不灵，退路是删了重建（可能多弹一次框）。详见「乙本体」第 4 条。要人知道：这几次框会在实现那块开跑时出现

**E. atg-dev 证书留不留**（不随 A；但要等 D 的迁移验证做完）。选项：删掉（`security delete-identity`，要人点框；以后不会误用；代价：本实验 `run.sh`、`run-b.sh` 的 atg-dev 对照组跑不了）· 留着不用。建议先留，迁移验证（要用它建旧式条目）做完再删

## 采用乙的具体做法（B 定了用乙之后；另起一块实现，本实验不改代码）

### 乙本体（标了「随 A」的条目除外，A 怎么选都要做）

1. **签名身份**：所有开发版改用 Apple Development 身份签名，**用 SHA-1 引用**（身份名里有人名，不写进仓库和日志）。SHA-1 从 `.env.local` 的 `ATG_CODESIGN_ID` 读；没设时，现场找 `Apple Development` 身份，**恰好一个才用，零个或多个就报错退出**，让人把 SHA-1 写进 `.env.local`（续签期间新旧证书并存、或多了别的团队，取错团队号会悄悄回到弹框）。标识照旧固定为 `-i atg.dev`：`codesign -s "$ATG_CODESIGN_ID" -i atg.dev --force <程序>`。TDLib 库的**载入方式不变**（取证 10 ⑤：不开强化运行时，`DYLD_LIBRARY_PATH` 照常），只是经 cargo runner 跑时传路径的变量要换名，见第 2 条
2. **免得忘签**：加 `scripts/dev-sign.sh <程序…>`（只签名），再加 `scripts/dev-sign-run.sh`（先签、再运行），在 `.cargo/config.toml` 设 `[target.aarch64-apple-darwin] runner = "scripts/dev-sign-run.sh"`——cargo 的 runner 是它运行编出来的程序前先调用的包装命令，这样 `cargo run` / `cargo test` 都会先签再跑；只 `cargo build` 时手动调 `dev-sign.sh`。⚠️ **runner 是 shell 脚本，系统 shell（`/usr/bin/env`、`/bin/bash` 都是系统程序）启动时会删掉 `DYLD_LIBRARY_PATH`**，库路径就传不到被测程序（仓库里踩过：`state/handoff/ipc.md`、`src/atg-cli/live/login.exp`）。照现有办法：库目录改用 `ATG_TDLIB_LIB` 传进来，脚本里 `export DYLD_LIBRARY_PATH="$ATG_TDLIB_LIB"` 再 `exec` 被测程序（被测程序不是系统程序，变量能传到）。`ATG_TDLIB_LIB` 的值是「编好的 TDLib 前缀」下的 `lib/`：前缀由 `bash src/tdlib-build/build.sh path` 打印（现在是 `_build/20261025-171733/tdlib`；现有文档里把它记作 `LOCAL_TDLIB_PATH`，tdlib-rs 编译时也读这个变量）。建议由 `dev-sign-run.sh` 自己跑一次 `build.sh path` 算出来，不用人每次 export；也可以写进仓库根 `.cargo/config.toml` 的 `[env]`，但前缀目录名带时间戳、换了要跟着改。`src/README.md`、`src/atg-cli/README.md`、`src/atg-core/examples/live_wait_phone.rs` 头注释里「`DYLD_LIBRARY_PATH=… target/debug/…`」那类写法，凡是改经 cargo 跑的都要同步改。开了强化运行时以后改靠 rpath，就不需要这一步。（runner 的做法本实验没测）
3. **`setup-dev-codesign.sh`**：不再建自签名证书，改成检查 Apple Development 身份在不在（规则同第 1 条），不在就提示人：打开 Xcode → 设置 → Accounts → 加 Apple ID → 选个人团队 → Manage Certificates → 加 Apple Development（本次人就是这么领的；不用建工程）
4. **现有真条目迁移**（随 A：A1 / A2 迁 `test` / `main` 两条；A3 只迁 `test`，`main` 不动）：两道关都对不上——分区名单是旧版的 `cdhash:…`，访问名单是 atg-dev 的需求。新签名版第一次读时各弹一次框，人点「始终允许」。**推断**（结论 3 的后一半）：这一下会把新程序加进访问名单、把它那一类签名的记号 `teamid:…` 加进分区名单，此后不再弹（未测：要碰真条目）。**这是实现那块要先验证的头号问题**：atg-dev 版建实验条目 → Apple Development 版读、人点「始终允许」→ 换一版再读，看弹不弹。**万一加进去的仍是 cdhash**，退路（未测）：迁移程序既然已经读到钥匙，就用同样的 32 字节**删掉旧条目、以新身份重建**——新条目由 Apple Development 版建，分区就是 `teamid:…`（取证 10 ①）；删除时可能再弹一次框；钥匙值不变，本地库照常能打开。（「人跑 `set-generic-password-partition-list`」不可行：它只改分区名单，访问名单那道关照样拦）
5. **规约与注释**：`config-privacy.md` 第一节第 3 小节——「认一张本机开发证书」改成「认本人的 Apple Development 身份（团队号）」，并按 A 题写明谁能拿到钥匙；「正式发布签名照样在访问名单里」加「团队号是否与开发身份相同未核」；「用哪种访问控制，开工时实测再定，未核」改成已定（传统钥匙串 + 团队号分区，见本实验）。A3 时还要写明「开发身份拿不到正式钥匙」并加对应测试。`testing.md` 第一节第 4 小节里「开发证书」的措辞，以及「开发证书以外拿不到」的测试（已有实测依据，取证 10 ③）。`keystore.rs` 头注释。拍板 005 N2 的措辞
6. **（随 A · A3）开发版拒开正式档案**：开发版（以签名身份或构建方式区分，实现时定）遇到 `main` 档案直接报错退出，不去读钥匙串（否则会读那条没迁的旧条目、照样弹框）；加对应测试
7. **证书续期**：一年一续（下次 2027-10-26 前）。建议在 `state/board.md` 记一笔。续签后旧条目能不能照读**未测**，到时先用实验条目测

### 开强化运行时要做的（#42；何时做见待人定 C）

签名加 `--options runtime`。开了以后库验证生效、`DYLD_LIBRARY_PATH` 失效，TDLib 的加载要改下面四处（依据：取证 10 ⑤、取证 11）——**都没在真程序上测过，是 #42 最大的风险**：

1. **`libtdjson` 依赖的 OpenSSL 过不了库验证**：`libtdjson.1.8.69.dylib` 动态链接 Homebrew 的 `libssl.3.dylib`、`libcrypto.3.dylib`，这两个是临时签名、无团队号（取证 11）。按取证 10 ⑤ 的机制，几乎肯定被拒载（推断：替身库没有这层依赖）。出路（未测）：① 编 TDLib 时把 OpenSSL 静态链进去（CMake 的 `OPENSSL_USE_STATIC_LIBS=TRUE`）——静态链接就是把库的代码直接编进 `libtdjson`，运行时不再单独载入 OpenSSL，也就不受库验证管，**建议这个**；② 把两个库拷进 TDLib 前缀、`install_name_tool -change` 改引用、用同一身份签。**不要**用 `disable-library-validation`（加了就放宽库验证，同用户可写目录里被换掉的库也能进来）
2. **`libtdjson` 本体要用同一身份签**（取证 10 ⑤：未签名 / 临时签名的库被拒载）。签真文件 `libtdjson.1.8.69.dylib`（`libtdjson.dylib` 是软链接）；放在 `src/tdlib-build/build.sh` 编完之后、算 sha256 写 `build-info.txt` **之前**（签名会改文件字节）；已有的 `_build/20261025-171733` 前缀没签过，要补签（同时重算哈希）或重编
3. **程序里要自己加 rpath**：已有的 TDLib 版程序里没有任何 `LC_RPATH`（取证 11），这正是现在各处都要 `DYLD_LIBRARY_PATH` 的原因。tdlib-rs `build.rs:193` 的 `cargo:rustc-link-arg=-Wl,-rpath,…` 只作用于 tdlib-rs 包自己的产物，不传给下游的 atg-cli；而且它指的是 `OUT_DIR/tdlib/lib`（`build.rs` 先把 `LOCAL_TDLIB_PATH` 整个拷到 `OUT_DIR`，即 cargo 给每个包的构建产物目录）。所以**要在跑 TDLib 的包（atg-cli，以及 atg-core 的 example / 测试）里自己加 rpath**：例如仓库根 `.cargo/config.toml` 的 `[target.aarch64-apple-darwin] rustflags = ["-C", "link-arg=-Wl,-rpath,<TDLib lib 目录>"]`（rustflags = 传给 Rust 编译器的额外参数），或各包自己的 `build.rs`。目录要常驻、最好不带空格（项目路径末尾的空格曾让第三方构建拆错参数，见 `state/memory/20261025-path-trailing-space.md`；rpath 里带尾空格行不行未核，绕开最稳）：`build.sh` 里那个不带空格的软链接是临时的、用完即删，得另建一个常驻的（如 `~/Library/Application Support/atg/dev/tdlib` → 实际前缀），由 `build.sh` 维护。实现后用 `otool -l <程序> | grep -A2 LC_RPATH` 核
4. 其余直接跑程序的写法（`src/atg-cli/live/login-main.sh`、`login.exp` 等）里的 `DYLD_LIBRARY_PATH` / `ATG_TDLIB_LIB` 都可以去掉

## 清理记录

用过 13 个账户名：`exp-adhoc` `exp-signed` `exp-np` `exp-acc` `exp-tool` `exp-helper` `exp-det` `exp-inj` `exp-hr` `exp-run-direct` `exp-run-helper`（这两个由 `run.sh` 建，跑了三次，每次自清）`exp-b` `exp-b2`（乙）；`exp-dp-*` 一条也没建成（`-34018` / 137）。除 `exp-hr` 外都由各自的建立者删除（直接删，或注入建立者后删）。
其中 `exp-b` 由 `run-b.sh` 建、跑完自清；`exp-b2` 是第二轮手工测试建的。
**`exp-hr`**（强化运行时小助手建的，注入不进去，小助手又没有删除功能）：2026-10-26 经人同意，跑 `security delete-generic-password -s atg.exp005 -a exp-hr`，人在弹框里点「允许」后删除。
**收尾核对**：`security dump-keychain | grep -c '"svce"<blob>="atg.exp005'` → **0**。
全程没有读、改、删 `atg.tdlib-db-key` 或任何非 `atg.exp005` 的条目；`dump-keychain` 的输出只在管道里用 awk 过滤，只显示 `atg.exp005` 的块。

## 文件

- `probe.c`：探针（add / get / del / addnp / addacc / adddp / viahelper），只打印状态码和长度
- `keyhelper.c`：方案丁原型（核调用方签名 → 读 / 建条目）；**核身份不安全，见上**
- `inject.c`：注入取证与清理（读只打印长度 / 删；挂在小助手的 `getppid` 与探针的 `SecKeychainSetUserInteractionAllowed` 上）
- `run.sh`：一条命令复现取证 3、方案丁、注入读取，退出时自清（包括中途失败）。证书哈希现场读出。产物目录默认用 `mktemp`，不进仓库。**atg-dev 已于 2026-10-26 删除，对照组不可复现**：`run.sh` 整个依赖 atg-dev，跑不了
- `run-b.sh`：一条命令复现方案乙（取证 10 ①–⑤），要本机有 Apple Development 身份（恰好一个，或设 `ATG_CODESIGN_ID`）；③ 的 atg-dev 对照要 atg-dev 证书，没有就跳过；不打印证书名，团队号只留前 4 位。**atg-dev 已于 2026-10-26 删除，对照组不可复现**：③ 的 atg-dev 对照会自动跳过
- `libdummy.c`：替身动态库，模拟 `libtdjson` 在强化运行时下的加载

```sh
docs/experiments/exp005-keychain-acl/run.sh     # 约 10 秒，不弹框
docs/experiments/exp005-keychain-acl/run-b.sh   # 约 10 秒，不弹框（方案乙）
```

## 审查

多视角自审（atg-review）：作废 1 次（传错参数）+ 有效 8 轮——第 1 轮审第一版报告，第 2–8 轮审方案乙部分；第 7、8 轮连续无 P0/P1，达到项目的停止线（`CLAUDE.md` 第二节：连续两轮没有 P0/P1 就停），两轮的 P2 都已修。每轮要点记在 `state/handoff/keychain.md`。待 codex 补审（#32）。
