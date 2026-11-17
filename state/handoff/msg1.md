# 现状页 · msg1

## b20 接手（2026-11-16）
交代：MAIN `_tmp/20261116-b20/msg1.md`。大总管 `allenchen-37`。顺序：#57 pending 收尾（029）→ #88 待发标记提前删（先写红测试）→ #8 编辑 / 删除自己刚发的（030、031，直发不确认）。
- **做到哪**：大总管提交 WIP 53088ce 并 merge origin/main；12 个冲突已解（无标记），Touch ID 残留已改直发，**未编译**，已申请名额。之前：已报到。工作区未提交改动 = codex 029 第 2 轮两条 P2 的修复（清缓存误报撤销成功 → `unknown`/`gone`；待发不占删旧配额），**未编译**
- **合 main 预计冲突**：core `send.rs`（main 拆成子模块，分支改了约 100 行）、`confirm_box.rs`（main 删、分支改：修改/删除冲突）；还有 fake / td / store/mod / error / lib、CLI send / serve / session / args / main / forward / deadlines、契约 actions / cli / errors / config-privacy、reviews/README、文件行数基线
- **Touch ID 删除波及我的代码**：pending 撤销确认框文字（confirm_box）、`delete.rs` 与其测试里的 HumanAuth / 确认文字、`serve/l2.rs`、`pending.rs`、`exp_real.rs`、CLI session / send 里的 Authenticator 接线 → 全改成直发
- **下一步**：合完 → 去掉确认、编译测试（领名额）→ codex 029 第 3 轮 → #88
- **要人定的事**：见下「要人定的事（b20）」
- **#88 计划（读了 main 的 `send_limits.rs`、`td.rs::detached`、`link::Shared::event`，未动代码）**
  - 病根两处：①调用方 30 秒等不及丢掉等待 → `Pending` 守卫 drop、标记删了，可 `detached` 线程还在等应答，晚到的受限写盘失败时 `mark_pending` 也写不进（磁盘满）→ 重启放行；②交出成功（进 TDLib 待发队列）后，真正的成败靠 `updateMessageSendFailed` 晚到，那时标记早删了
  - 修法 A（治①）：`detached` 线程开跑前自己领一个守卫（inflight+1），登记完应答再放；调用方先放也删不掉标记
  - 修法 B（治②）：交出成功回来的临时 id 各领一个守卫，存进 `SendLimits`；`Shared::event` 收到该 id 的 SendSucceeded / SendFailed（先 note 再放）才放；临时 id 换正式 id 要跟上
  - 红测试：A——`SendLimits` 层模拟「调用方守卫先放、晚到受限写盘失败、线程守卫后放」→ 重启后 `check` 应拒（现在会放行）；B——假后端：交出成功 → 堵住限制文件 → 推 SendFailed(受限) → 新进程 `load` 应拒
  - 待定 Q1（见下）

## 要人定的事（b20）
- **Q1 #88：进程正常退出时还有「交出了、结果没回来」的消息，标记留不留？**
  - 选项 a：留——最稳，下个进程拒发，要人看过账号状态后手动删标记。代价：断网时等超时这种常见情况也会留人工活，和现在「常见情况自己清」的承诺相反
  - 选项 b：删——下个进程启动时 TDLib 续发，失败由它自己登记。代价：续发结果回来之前，下个进程可能已经又发了一条（最坏是多一次被拒，不会发错人）
  - 选项 c：留，但标记里写「正常退出、结果未知」，下个进程看到这种标记先等 TDLib 续发的结果（或等一个期限）再放行。代价：多一个状态、多些代码
  - 建议 c 太重，先 b，并在契约里写明；默认 b

## b18 记录（发送这一侧收尾）

交代：MAIN `_tmp/20261113-b18/msg1.md`。分支 `feat/57-pending`。顺序：#57 pending（评审 029）→ 删除（030）→ 编辑（031）。

## 手上
**#57 `atg-cli pending` + `pending cancel CHAT_ID`**：自审 4 轮，每轮各有 P1，均已修（见下「自审记录」）；第 4 轮修后待编译验证，再跑第 5 轮。全量（第 1 轮修后）487 过、clippy 净、tdlib 特性编译过。实测（大总管已同意：正式档案 main 收藏夹 + 直发，「atg 实验 待发撤销 N」3 次）代码在 `src/atg-cli/src/pending/exp_real.rs`（`#[ignore]` + 环境变量），等名额

## 已核的事实（TDLib 1.8.69 源码，`resources/td/td/telegram/MessagesManager.cpp`）
1. 开着消息库时，上次留下的待发消息在 TDLib 启动重放 binlog 时**当场续发**（`:35925-35930`），并推 `updateNewMessage`。任何一条命令（含 `pending` 本身）一启动就续发
2. 续发时**临时 id 和 `date` 都重编**（`:35851-35856`），`sending_id` 不落盘
3. 超过 24 小时（`MAX_RESEND_DELAY`，`MessagesManager.h:1708`）的不再续发、直接变失败（非收藏夹）
4. 删除待发消息只停本地请求（`:21330-21345`）；服务器已收到的，成功应答回来时 TDLib 自动 `revoke=true` 删（`:27848-27851` → `:8484-8490`）；会话不可访问时放弃
5. `deleteMessages` 先把临时 id 换成发出后的正式 id（`get_persistent_message_id`，`:8415`、`:21075-21087`；登记在 `:26949`、`:28347`）→ 撤销用 **`revoke=false`**（大总管同意），赶上恰好发出的只删自己这边，CLI 报 `sent_deleted_here`

## 大总管定（2026-11-14）
- Q1 `pending_cancel` 按 L2 过闸（同删除）
- Q2 `PendingMessage` 时间改 `date`，契约写明续发后 id 与时间都会变

## 改了什么（#57）
- core：`store/query.rs`（新：`pending_sends`，迁入 `chat_list` / `next_unread_chat`）· `delete.rs`（新：`DeletePlan`、`prepare_cancel`、`cancel`，以后给所有人删除也放这里）· `send.rs` 交出口加 `SealedKind::Delete` / `Answer::Deleted`；确认文字函数迁到 `confirm_box.rs` · `td.rs` 加 `deleteMessages` 交出，建客户端前两步迁到 `td/real/setup.rs` · `fake/submit.rs`（新：重发迁入 + 删除）· `error.rs` 加 `not_pending`
- CLI：`pending.rs`（新：列表 + 撤销）· `pending/exp_real.rs`（新：实测）· `deadlines::CANCEL_RACE_WAIT` 1 秒· `serve/l2.rs`（新：执行方的发送类分派，send / resend 迁入）· args / main / session / forward 接线 · `deadlines::CANCEL_WAIT` 5 秒
- 契约：actions（待发出的消息、PendingMessage）· cli（pending 行、send 行结果未知）· errors（not_pending）· config-privacy（能转 pending）

## 报大总管 / 要人定的
- 超线文件变小（fake / send / store/mod / td / serve），要跑 `python3 scripts/check-file-size.py --tighten` 收紧基线——`scripts/` 不在我可写范围，请大总管跑或授权
- 架构守则第 3 条：`pending` 文字格式的结尾一句是在 CLI 里拼的统计句（同 send / resend 现状），没有「命令结尾统计」树节点；建议另开 issue 统一处理
- 误操作留痕：跑文件行数检查时用了一次 `git add -N`（只登记意向），发现违规立刻 `git reset` 撤回，工作区恢复原状

## 自审记录（atg-review）
- 第 1 轮 P1×3：一条 `messages_deleted` 只认第一条（`send::verdict` 改返回多条）· 文字格式缺 id · `result` 配退出码 10（改退出码 0 + `all_cancelled`）
- 第 2 轮 P1：跨进程临时 id 重编 → `pending cancel CHAT_ID` 不收 id、同进程取全部待发（大总管同意）
- 第 3 轮 P1：删除被换成正式 id → `revoke=false` + `sent_deleted_here`（大总管同意）
- 第 4 轮 P1：换 id 的删除与发出成功同批到、第二次等待等不到 → 下落按 store 判（`cancel_outcome`，有单测）
- codex 029 第 1 轮（`docs/reviews/029_pending-57-codex.md`）：P1×3 属实已修（待发不参与删旧 + 超 100 整批拒；`sent_deleted_here` 只认墓碑；带附件的整批拒 `not_text`），P2×1 已开 issue #109（撤销交出后超时被报成普通 network）
- 剩余 P2（待开 issue）：确认框只能放三种结局里的两句；`DeletePlan` 等 M2.7 再加 revoke 字段；`serve/l2.rs` 名字与内容（含只读列表）不完全一致；文字格式结尾统计句在 CLI 里拼（架构守则第 3 条，同 send / resend 现状）

## 下一步
人定 A：不再自审。名额：编译 + 测试 + tdlib 编译 + 实测 → 大总管提交 → codex 029 第 2 轮（脚本交大总管代跑）→ 报「可以提交」
