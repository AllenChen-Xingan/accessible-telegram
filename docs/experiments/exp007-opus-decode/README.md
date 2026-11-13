# exp007 · macOS 27 能不能直接解 Telegram 的 OGG/Opus

> 2026-11-13 · 手下 media · issue #4（M2.6）、#59 · 夹具全部是 `say` 合成的编造句子，不含任何真实语音

## 假设

roadmap M2.6 写「先核 macOS 能否直接解 Opus / OGG（未实测）」；inventory 第 163 行写「macOS AVFoundation 不原生解 Opus/OGG，需解码库或先转码」。假设：**macOS 27 的 AudioToolbox 已能原生读 OGG 封装的 Opus**，不需要第三方解码库。

## 做法

本机：Apple M5 · macOS 27.0.1（26A434）· ffmpeg（Homebrew，带 libopus，只用来造夹具和做对照）。

1. `afconvert -hf` 看系统登记的格式
2. `say` 合成中、英两句 → ffmpeg（libopus）编成和 Telegram 语音同样的格式（单声道、48 kHz、Opus 编码、OGG 封装；编码器不是 Telegram 客户端的那一个），共 4 个文件、3 组参数：zh、en 各一份 32 kbps 20 ms 帧 · zh 16 kbps 60 ms 帧 · en 16 kHz 输入 24 kbps
5. 另编一份 MP3（64 kbps）、一份 M4A（AAC 64 kbps），看同一入口能不能解
3. 用系统 `afinfo` 读头、`afconvert` 解成 PCM；同一文件再用 ffmpeg 解两份做对照：**显式指定 `libopus` 解码器**一份、ffmpeg 自带的原生 `opus` 解码器（不指定时的默认）一份，做抽样波形对照（口径见下方 `snr.py` 一段）
4. 截去后半个文件（模拟没下完），看系统怎么处理

复现命令（在任意空目录里）：

```sh
afconvert -hf | grep -E "Oggf|opus"
say -v Tingting -o zh.aiff "今天下午三点在图书馆二楼开会，记得带上笔记本电脑。"
ffmpeg -y -i zh.aiff -ac 1 -ar 48000 -c:a libopus -b:a 32k -application voip zh.ogg
afinfo zh.ogg
time afconvert -f WAVE -d LEI16@48000 -c 1 zh.ogg af_zh.wav     # 系统解码
ffmpeg -y -c:a libopus -i zh.ogg -ac 1 -ar 48000 -c:a pcm_s16le lib_zh.wav   # libopus 对照（-c:a 放在 -i 前 = 选解码器）
ffmpeg -y -i zh.ogg -ac 1 -ar 48000 -c:a pcm_s16le ff_zh.wav                # ffmpeg 原生 opus 解码器（默认）
head -c 10000 zh.ogg > trunc.ogg && afconvert -f WAVE -d LEI16@48000 trunc.ogg t.wav; echo $?
```

波形对照用同目录 `snr.py`（一次性代码，标准库 `wave`）：在 ±400 样本内找最佳对齐后算波形信噪比，**抽样计算**——跳过开头 1000 个、末尾约 500 个样本，中间每隔 7 个取一个。所以下面的 dB 数是这段抽样的波形信噪比，不是完整波形的逐样本核验。复现（接上面的命令，en、zh60 同法生成）：

```sh
python3 -I snr.py af_zh.wav lib_zh.wav     # 系统 vs libopus：零偏移、约 62 dB
python3 -I snr.py af_zh.wav ff_zh.wav      # 系统 vs ffmpeg 原生 opus：偏移 1、约 18 dB
ffmpeg -y -i zh.aiff -ar 48000 -c:a pcm_s16le orig_zh.wav
python3 -I snr.py orig_zh.wav af_zh.wav    # 原声 vs 系统：约 9.8 dB
python3 -I snr.py orig_zh.wav lib_zh.wav   # 原声 vs libopus：约 9.8 dB
# 其余夹具：en 用 -v Samantha 合成同一套；zh60：-b:a 16k -frame_duration 60；en16k：先 -ar 16000、-b:a 24k
```

**术语**：OGG 是封装（装数据的盒子），Opus 是编码（压缩方法）；PCM 是解压后的原始采样；预跳过（priming）是 Opus 文件开头要丢掉的 312 个样本，处理错了声音会错位；信噪比（dB）越大两段波形越像，每多 10 dB 误差能量小 10 倍。

## 数据

| 项 | 结果 |
|---|---|
| 系统登记 | `'Oggf' = Ogg (.opus, .ogg, .oga)`，`data_formats: 'opus' 'flac' 'vorb'` |
| `afinfo zh.ogg` | `1 ch, 48000 Hz, opus`；时长 5.73 s；`274806 valid frames + 312 priming + 402 remainder`（Opus 的预跳过 312 样本被正确识别） |
| 解码速度 | 5.7 s 音频 0.02 s（整个 `afconvert` 进程，含启动） |
| 帧数 | 4 个文件（3 组参数）下，系统、libopus、ffmpeg 原生三者解出的有效样本数**完全一致**（zh 274806、en 160440） |
| 系统 vs libopus（抽样） | **零偏移**，波形信噪比 zh 62.0 dB · en 65.7 dB · zh60（16 kbps 60 ms 帧）37.3 dB：几乎一样（误差能量是信号的百万分之一量级；60 ms 帧那份差一些）。是不是同一份实现本实验分不出 |
| 系统 vs ffmpeg 原生 opus（抽样） | 相差 1–2 个样本，18–23 dB：输出不逐位一致（第 1–11 轮写的「与 libopus 比 18–23 dB」其实是与这个原生解码器比——当时没指定解码器，codex 017 第 12 轮指出，2026-11-13 补测改正） |
| 各自与原声比 | 系统、libopus 都是 9.8 dB（有损语音编码只求听着像、不求波形一样，所以离原声只有 10 dB 左右）。只能说两者量级相同；**不能推出系统解码有没有额外失真、听感是否一样**——有损编码下波形信噪比不是听感判据，听感留到人试听 |
| MP3 / M4A | 都能读（`.mp3` 44.1 kHz、`aac` 44.1 kHz），`afconvert` 退出码 0，解出时长与原文件一致（5.77 s / 5.73 s） |
| 截断文件 | `afinfo` 估出 2.0 s，`afconvert` 退出码 0，解出已下的那 2 秒 |

## 结论

1. **成立。** macOS 27 的 AudioToolbox 原生解 OGG/Opus，Telegram 常见的几种帧长与码率都能解，预跳过处理正确，速度可忽略。**不需要第三方解码库**（原计划要比的纯 Rust 方案因此不再比；备选见下）。
2. inventory 第 163 行原写「macOS AVFoundation 不原生解 Opus/OGG」。本实验测的是更底层的 AudioToolbox（AVFoundation 的 `AVAudioFile` 读文件也是经它；**AVFoundation 自己的接口没测**）。正式代码直接用 AudioToolbox，所以那句已按大总管定改成「AudioToolbox 原生解」（2026-11-13）。
3. 系统同一入口也解 MP3 / M4A（上表，各一个合成文件）——`inputVoiceNote` 允许语音是这两种格式（inventory 第 167 行），所以一个入口全覆盖。
4. 没下完的文件能解出已下的部分，不崩，**但也不报错**——解码器分不出文件完不完整。所以正式代码必须先由 TDLib 确认下载已完成（`file.local.is_downloading_completed`）再解码，否则会静默放出 / 转出半截（`design.md` 第二节第 1 条）。

**给正式代码的建议**：`atg-audio` 用 AudioToolbox 的 C 接口 `ExtAudioFile`（`ExtAudioFileOpenURL` → 设客户端格式为 32 位浮点、单声道 → `ExtAudioFileRead`）从 Rust 直接 FFI 调，不经 Swift、不加依赖；解出的 PCM 播放与转写共用（解码只做一份）。

**备选（未实测，仅当系统解码将来出问题时再比）**：`ogg` 解封装 + libopus 绑定（要编 C 库）；纯 Rust 的 Opus 解码器目前没有成熟的。

## 边界

- **时间边界**：macOS 27.0.1（26A434）实测。macOS 26 及更早**没测**——本项目只跑这台机器，不影响；将来要支持更早系统得重测。
- **规模边界**：夹具是 TTS 合成、3–7 秒、4 个 Opus 文件（3 组参数）加 MP3、M4A 各一；没有用真实 Telegram 客户端录的文件（隐私规矩不许用真实语音）。正式代码阶段用人放进「收藏夹」的语音再验一次（先报大总管）。
- `afconvert` 是命令行工具，走的是同一个 AudioToolbox；Rust 直接调 `ExtAudioFile` 的行为在正式代码里由解码单测确认。

## 审查

- 2026-11-13 多视角自审（atg-review，范围 = 本目录与 exp008）：本文件的修正——参数计数、MP3/M4A 补测、AVFoundation 与 AudioToolbox 的关系、截断文件「不报错」传到设计、术语说明、对照脚本入库
- codex 017：见 `docs/reviews/017_exp007-008-codex.md`
