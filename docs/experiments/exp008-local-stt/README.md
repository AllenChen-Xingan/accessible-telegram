# exp008 · 本机端侧语音转文字（SpeechAnalyzer / SpeechTranscriber）

> 2026-11-13 · 手下 media · issue #59（人原话：用 Mac 本机 Apple 端侧模型，按一下才转）
> 夹具全部是 `say` 合成的编造句子（`fixtures/make.sh`），不含任何真实语音；探针 `probe.swift`（一次性代码）

## 假设

1. 本机 Speech 框架的 `SpeechTranscriber` 能离线转写普通话与英语，速度远快于实时
2. Rust 能用某种方式调到它
3. 中英混说能用

## 做法

- 探针 `probe.swift`：标准输入读 48 kHz 单声道 f32 PCM → `AnalyzerInputConverter` 转成分析器要的格式 → `SpeechAnalyzer` 挂一个或两个 `SpeechTranscriber`（`zh-CN` / `en-US`，开 `transcriptionConfidence`）→ 每个语种输出一行 JSON（文本、平均置信度、耗时）
- 编译：`swiftc -O -parse-as-library probe.swift -o probe`（1.9 s，领了名额）
- 夹具：`sh fixtures/make.sh <目录>`——`say` 合成 → Opus 24 kbps 编码 → 系统 AudioToolbox 解码（exp007）→ f32。也就是**和正式管线同一条解码路径**
- 运行：`./probe --list`；`./probe --rate 48000 --locale zh-CN --locale en-US < zh.f32`

## 数据

本机：Apple M5 · macOS 27.0.1 · Swift 6.4（Command Line Tools SDK）

**可用性**（`--list`）

| 项 | 结果 |
|---|---|
| `SpeechTranscriber.isAvailable` | true |
| 已装语种 | en_AU/CA/GB/IE/IN/NZ/SG/US/ZA · **zh_CN** · zh_TW |
| 支持语种 | 45 种（另有 yue_CN、zh_HK 未装） |
| 推荐输入格式 | 16 kHz · 单声道 · Int16（zh-CN、en-US 相同）；`AnalyzerInputConverter` 从 48 kHz f32 转，自带 |
| `AssetInventory.status` | `supported`（不是 `installed`）——但实际转写成功，**不能拿它当门槛** |
| 授权弹窗 | **没有**。两种启动方式都测了：① 终端里直接跑；② 包成最小 `.app`（临时签名、Info.plist 不写任何用途说明）由 `open` 经 LaunchServices 拉起、不经终端（排除「授权算在终端头上」）——都不弹，TCC 日志里没有这个包的记录。**没测**：Apple Development 签名版（正式代码时顺带验） |
| 联网 | **未断网实测**（断网会影响同机其他工作线）。依据只有 Apple 对 SpeechAnalyzer「端侧」的说明与资源已装在本机；列为待人验证项 |

**准确度与速度**（Opus 往返后的合成音；`analyze_s` = 分析器开跑到出结果，`total_s` = 进程从读输入到出结果，不含进程启动；主要工作在系统服务里做。第一个跑的是普通话短句双跑，没有单独区分冷启动与热跑——**冷启动未测**）

| 夹具 | 时长 | zh-CN 结果 | 置信度 | en-US 结果 | 置信度 | 耗时（analyze / total） |
|---|---|---|---|---|---|---|
| 普通话短句 | 5.7 s | 「今天下午 3点在图书馆二楼开会 ，记得带上笔记本电脑。」**零错字** | 0.95 | 拼音乱码 | 0.18 | 双跑 0.35 / 0.39 s |
| 英语 | 3.3 s | 「Pease bring the cortel report theing at tea clock.」 | 0.67 | 「Please bring the quarterly report to the meeting at 3 o'clock.」**零错** | 0.96 | 双跑 0.14 / 0.19 s |
| 中英混说 | 7.2 s | 「我明天要交 Corter Liryport，然后跟 David开个 meeting地点在 starbugs。」 | 0.92 | 拼音乱码 | 0.24 | 单 zh 0.14 / 0.18 s · 双跑 0.21 / 0.26 s |
| 普通话长段 | 39.9 s | 文字**零错字**；但前两句后**标点全丢**（一口气连成一串） | 0.97 | 乱码 | 0.11 | **单 zh 0.54 / 0.59 s（≈74 倍实时，两次一致）** · 双跑 1.10 / 1.15 s（≈35 倍实时） |

中英混说的错误集中在英文专名和短语（quarterly report、Starbucks），中文部分和常见英文词（David、meeting）正确。

**语种判断**：Speech 公开接口里**没有语种识别模块**（`swiftinterface` 只有 `SpeechTranscriber` / `DictationTranscriber` / `SpeechDetector`；资源目录里的语种识别模型只供系统内部用）。替代：**同一个分析器同时挂 zh、en 两个转写器，取平均置信度高者**——四个夹具全部判对，差距都很大（0.95/0.18 · 0.96/0.67 · 0.92/0.24 · 0.97/0.11）。「判对」= 选中的那份文字正是上表零错 / 错最少的那份（混说选 zh：中文全对、英文专名错，而 en 那份是拼音乱码）。「平均置信度」= 结果里各段（run）置信度的简单平均，不按时长加权。

## Rust 怎么调（代价对比）

`SpeechAnalyzer` 是 Swift actor + async 接口，**没有 Objective-C 出口**，Rust 的 objc2 系列绑定够不着，必须经 Swift。

| 方案 | 做法 | 好处 | 代价 |
|---|---|---|---|
| **甲 独立 Swift 辅助程序**（已定） | 一个约 150 行的 Swift 可执行（位置按 arch 定：`src/atg-audio/stt-helper/`，见 `design.md`），由本探针改成正式版：标准输入收 PCM，标准输出按协议出结果（正式协议是每语种一行加结束行，正本 `design.md` 第三节第 1 条；本探针的「每语种一行 JSON、出错整个退出 2」只是实验用）；Rust 起子进程、经管道喂数据 | 构建简单（单文件 `swiftc`，约 2 s）；系统框架崩了不连累 CLI / GUI；探针已实测；PCM 走管道**不落盘** | 多一个要签名、要随 `.app` 打包的可执行；每次转写多起一个进程（中英混说单 zh 时 `time` 量整进程 0.18 s 含分析，启动开销可忽略）；要定位它的路径 |
| 乙 Swift 静态库 + C 接口 | `@_cdecl` 导出 C 函数，`build.rs` 调 `swiftc` 编静态库，Rust 链接 | 单一二进制 | 链接 Swift 运行时与并发库（`-L /usr/lib/swift` 等）；async 要在 C 边界用信号量转同步；每次 `cargo build` 可能触发 Swift 编译（名额）；Speech 内部崩溃直接带走 GUI |
| 丙 老接口 `SFSpeechRecognizer`（有 ObjC 出口，Rust 可直接调） | objc2 绑定 | 不用 Swift | 老接口要授权弹窗、对端侧支持有限且 Apple 已转向新接口；没实测，不建议 |

## 结论

1. **成立。** 端侧转写可用、无弹窗（离线未断网实测）；普通话、英语在合成音上零错字；按已定的双跑方案约 35 倍实时（40 s 音频 1.15 s 出结果）。**这只是转写这一段**；「按一下」到出字还要加下载（语音没下时）和解码（exp007，可忽略），冷启动另计，端到端未测。
2. 输入：推荐 16 kHz 单声道 Int16，但分析器自带转换器，**直接喂 exp007 解出的 48 kHz f32 即可**——解码只做一份，播放与转写共用同一份 PCM。
3. Rust 调用：建议**方案甲（独立 Swift 辅助程序）**。
4. 中英混说：中文模型能用，英文专名会错；纯英语用中文模型会错得多（见上表）——语种策略大总管已定双跑取高（2026-11-13，`design.md` 第三节第 1 条）。
5. 长段标点会丢（`design.md` 第三节第 2 条）。

## 边界

- **时间**：macOS 27.0.1 实测；Speech 新接口要 macOS 26+。
- **规模**：只测了 4 段 TTS 合成音（3–40 s），**是干净的朗读音**。真人语音的准确度**未测**，不能从合成音结果外推；待测条件：口音、噪声、语速、口语停顿、方言。正式代码阶段用人放进「收藏夹」的语音（人自己录的）再测一次，只报统计（字数、耗时、置信度），不记内容。
- 置信度在两个模型之间不是校准过的同一把尺，「取高者」在四个夹具上差距都很大才判对；接近时（比如真正一半中一半英）可能判反，未测。
- 没测：冷启动（开机后第一次）、断网、Apple Development 签名版、并发（同时转几条）、超长语音（> 5 分钟）、`ignoresResourceLimits`、系统内存紧张时的 `insufficientResources`。

## 审查

- 2026-11-13 多视角自审（atg-review，范围 = 本目录与 exp007）：补 `.app` 方式的授权补测；「离线」降为未实测；补 `total_s`、冷启动未测；夹具计数 3 → 4；速度结论换成已定的双跑数字；判对标准与平均方式写明
- codex 017：见 `docs/reviews/017_exp007-008-codex.md`
