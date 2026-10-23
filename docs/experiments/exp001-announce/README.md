# exp001-announce · GPUI 的 Live 播报在 macOS 上发不发

> 2026-10-23 · 手下 exp001 · Zed `79fe6a6`（gpui + accesskit_macos 0.26.3）· rustc 1.98.1 · macOS Darwin 27.0

## 假设

设了 AccessKit `Live::Polite`（对照 `Live::Assertive`）的节点，值变化时：
1. VoiceOver 会播报新值
2. 焦点不移动
3. Polite 不打断正在朗读的内容，Assertive 打断
4. 同一句话连续出现两次，第二次不会播（`findings-gpui.md` 第 4 条），需要绕法

## 做法

- 一个最小 GPUI 窗口（`src/main.rs`）：消息列表 + 一个 `Role::Status` 播报节点。播报节点通过公开出口 `.a11y_synthetic_children(|b| b.parent_node()…)`（`gpui/src/elements/div.rs:1432`）拿到自己的 AccessKit 节点，设 `set_live` + `set_value`。**没改 GPUI。**
- 重复内容的两种绕法：**换节点**（每次播报换元素 id，走 accesskit 的 `node_added` 分支）和**零宽字符**（交替在末尾加 U+200B，让值变化）。
- **自动验证，不需要辅助功能权限**（`src/intercept.rs`）：
  - 激活：在进程内对 contentView 调一次 `accessibilityChildren`，accesskit_macos 会懒激活，GPUI 日志打印 `Accessibility activated`。
  - 截获：程序导出与 AppKit 同名的 `NSAccessibilityPostNotification` / `NSAccessibilityPostNotificationWithUserInfo`，先记录，再经 `dlsym(RTLD_NEXT)` 转给 AppKit。`nm -m` 显示这两个符号是二进制自己的 `(__TEXT,__text) external`，所以 accesskit_macos 的调用都会先经过这里。
  - 脚本：礼貌式/打断式 × 不绕/换节点/零宽字符，每组依次测试：切换模式、第一次说、同一句再说、换一句、只重绘不改值。最后用兜底路径直发两次同一句。
- 兜底路径：用 objc2 拿到 NSWindow，直接发 `NSAccessibilityAnnouncementRequestedNotification`（与 accesskit_macos 发的是同一个通知）。

**一条命令**（项目根目录）：

```sh
docs/experiments/exp001-announce/run.sh --auto   # 自动：约 25 秒，打印结果表后自动退出
docs/experiments/exp001-announce/run.sh          # 手动：开 VoiceOver 试听，见 人工试用.md
```

输出写进 `out/`：`result.md`（结果表）、`stdout.txt`、`stderr.txt`（每条截获通知带时间戳）、`tree-before.json` / `tree-after.json`（GPUI 无障碍树导出）。`out/` 不入库。

## 结论（跑出来的事实）

`run.sh --auto` 的结果：32 步中 30 步符合期望，节选如下（全表见 `out/result.md`）：

| 步骤 | 公告 | 优先级 | 改值→发通知 | 焦点变化通知 |
|---|---|---|---|---|
| 礼貌式 · 第一次说 | 1 条「新消息，礼貌式 · 不绕」 | 50（中） | 5.5 ms | 0 |
| 礼貌式 · 不绕 · 同一句再说 | **0** | | | 0 |
| 礼貌式 · 换节点 · 同一句再说 | 1 | 50 | 15.1 ms | 0 |
| 礼貌式 · 零宽字符 · 同一句再说 | 1 | 50 | 14.3 ms | 0 |
| 打断式 · 第一次说 | 1 | 90（高） | 9.1 ms | 0 |
| 打断式 · 不绕 · 同一句再说 | **0** | | | 0 |
| 打断式 · 换节点 / 零宽字符 · 同一句再说 | 各 1 | 90 | 12–13 ms | 0 |
| 只重绘不改值（6 组） | 0 | | | 0 |
| 兜底直发 × 2（同一句） | 各 1 | 50 | 0.2–0.3 ms | 0 |

1. **GPUI 原生出口能用，已由机器确认。** 设了 Live 的节点值一变，accesskit_macos 就发 `AXAnnouncementRequested`，文本正确。礼貌式优先级 50，打断式 90。从改值到通知发出 5–21 ms，比一帧略长。
2. **焦点不动，已由机器确认。** 激活之后整个脚本运行期间，没有出现任何 `AXFocusedUIElementChanged`。GPUI 的焦点节点前后都是 root（Application）。全程唯一一次焦点通知在 0.68 s，是激活时的初始焦点。
3. **同一句重复不播，已由机器确认。** 两个「不绕 · 同一句再说」就是表里那 2 个 ❌，与源码判断一致（accesskit `event.rs:299-305` 只在值变化时发）。**换节点**和**零宽字符**两种绕法都能补发。换节点会多一条 `AXUIElementDestroyed`，零宽字符会多一条 `AXValueChanged`。零宽字符会不会被 VoiceOver 读出怪声，**待人试听**。
4. 切换 Live 本身不发公告，前提是值同时被清空；值不空时改 Live 也会触发一次公告（`event.rs:301`）。封装时要注意。
5. **兜底能用。** 直发不受「值没变」的限制，同一句发两次就有两条。
6. **还没测、只能由人判断：** VoiceOver 实际有没有读出来，以及礼貌式会不会等长句读完、打断式会不会掐断长句（假设 1、3）。机器只能证明通知按正确的文本和优先级发出去了。步骤见 `人工试用.md` 第 2–13 步。

另外记一条：GPUI 的树导出（`debug_a11y_tree_json`）不会列出 `live` 字段，只看导出的树发现不了某个节点设了 Live。

## 改变了哪一个取舍

- **gap.md 里 exp001 那条「若不成立：播报改走 NSAccessibility 直发通知，或重新评估 GPUI」不触发。** 界面层继续用 GPUI，播报走原生 Live 出口，不需要 fork，也不需要直发。
- **薄壳里的「播报元素」定型：** 一个 `Role::Status` 节点，经 `a11y_synthetic_children` 设 Live + value。**默认用「换节点」绕法**，理由是不往文本里塞不可见字符。直发留作兜底，适合不依附于任何节点的系统级提示。
- **「打断 / 不打断」的取舍还悬着**，要等人用 `人工试用.md` 第 9、11 步试听后才能定：新消息默认用礼貌式还是打断式。

### 人工试听（2026-10-23，使用者本人，VoiceOver）

- 第 9 步 · 礼貌式（Polite，优先级 50）：**插进来播**——长句正在朗读时，新消息提示混入 / 打断了朗读
- 第 11 步 · 打断式（Assertive，优先级 90）：**掐断长句立即播**
- 其余步骤：使用者表示「应该可以了」，未逐条报告
- ⚠️ **假设 3 不成立**：礼貌式并不会等当前朗读结束。也就是说「新消息不打断正在读的内容」这件事，GPUI / AccessKit 的 Live 出口做不到，要另想办法（见现状页的后续建议，由大总管排期）
