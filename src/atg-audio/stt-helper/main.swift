// atg-stt：本机端侧语音转文字的辅助程序（#59；exp008 探针的正式版）。由 atg-audio 的 stt 模块起子进程调用。
//
// 协议（正本 docs/experiments/exp008-local-stt/design.md 第三节第 1、6 条；版本 PROTO）：
//   启动参数：--parent-pid <父进程 pid> --rate <采样率> --deadline-s <秒> --locale <zh-CN> [--locale <en-US>]
//   标准输入：单声道 PCM，每个采样一个 32 位浮点、小端（f32le；本机 Apple 芯片即小端，按本机字节序直接读），
//            写完关闭（EOF = 输入写完，不代表父进程死了）
//   标准输出，一行一个 JSON：
//     {"kind":"hello","proto":1}                                        —— 参数核对完、兜底线程起来后立刻出
//     {"kind":"locale","locale":"zh-CN","status":"text"|"empty"|"failed","conf":0.95,
//      "segments":[{"text":"…","start_s":0.0,"end_s":1.2}],"error":"…"}  —— 每个语种一行
//     {"kind":"done","count":N}                                         —— 全部报完，退出码 0
//   failed 的 error 码：unavailable（模型没装 / 不支持这个语种）· results_failed · analyze_failed · no_audio
//   其他退出码：2 参数不对（已出过 hello）· 3 父进程没了或到了总期限（兜底，下面「规矩」第二条）
//   标准错误：不写（Rust 那边接 null）
// 规矩：
//   - 每个语种各自捕获错误，互不连累；失败只带码，不带系统说明文字（可能含内容）
//   - 父进程不是 --parent-pid 给的那个（起来之前就没了，或中途没了、被系统收养）立刻退出；到总期限自己退出（第二次 Ctrl+C 时 Rust 来不及收尾，靠这两条兜底）
//   - 转写文字只进标准输出，别处一个字都不写
import AVFoundation
import Foundation
import Speech

let PROTO = 1
/// 「模型没装 / 不支持这个语种」的码（Rust 据此判 stt_unavailable；两个值都由 Rust 那边的测试比对）
let UNAVAILABLE = "unavailable"

func emit(_ obj: [String: Any]) {
    guard let d = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]),
          let s = String(data: d, encoding: .utf8) else { return }
    print(s)
    fflush(stdout)
}

/// 参数全由 Rust 给（采样率、期限的正本在 atg-audio，这里不留默认值）
struct Args {
    var parent: pid_t
    var rate: Double
    var deadline: Double
    var locales: [String]
}

func parseArgs() -> Args? {
    var parent: pid_t?, rate: Double?, deadline: Double?, locales: [String] = []
    let v = CommandLine.arguments
    var i = 1
    while i < v.count {
        guard i + 1 < v.count else { return nil }
        switch v[i] {
        case "--parent-pid": guard let x = pid_t(v[i + 1]), x > 1 else { return nil }; parent = x
        case "--rate": guard let x = Double(v[i + 1]), x > 0 else { return nil }; rate = x
        case "--deadline-s": guard let x = Double(v[i + 1]), x > 0 else { return nil }; deadline = x
        case "--locale": locales.append(v[i + 1])
        default: return nil
        }
        i += 2
    }
    guard let parent, let rate, let deadline, !locales.isEmpty else { return nil }
    return Args(parent: parent, rate: rate, deadline: deadline, locales: locales)
}

/// 兜底：父进程不是给定的那个或到总期限 → 退出。先当场核对一次（父进程在本程序起来之前就没了也能发现），再起线程。
/// 期限用单调时钟（系统校时不影响），与 Rust 那边的 Instant 同一类
func startWatchdog(parent: pid_t, deadline: Double) {
    if getppid() != parent { exit(3) }
    let start = DispatchTime.now().uptimeNanoseconds
    Thread.detachNewThread {
        while true {
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1e9
            if getppid() != parent || elapsed > deadline { exit(3) }
            Thread.sleep(forTimeInterval: 0.2)
        }
    }
}

/// 错误分类：模型没装 / 不支持 → UNAVAILABLE，其余给调用处的码。分析器与结果流两处共用
func code(_ e: Error, otherwise: String) -> String {
    if let s = e as? SFSpeechError, s.code == .noModel || s.code == .cannotAllocateUnsupportedLocale { return UNAVAILABLE }
    return otherwise
}

/// 一个语种的结果
func collect(_ m: SpeechTranscriber, locale: String) async -> [String: Any] {
    var segments: [[String: Any]] = []
    var confSum = 0.0, confN = 0
    do {
        for try await r in m.results {
            let text = String(r.text.characters)
            for run in r.text.runs {
                if let c = run.transcriptionConfidence { confSum += c; confN += 1 }
            }
            segments.append([
                "text": text,
                "start_s": r.range.start.seconds,
                "end_s": r.range.end.seconds,
            ])
        }
    } catch {
        return ["kind": "locale", "locale": locale, "status": "failed", "error": code(error, otherwise: "results_failed")]
    }
    let hasText = segments.contains { !(($0["text"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var out: [String: Any] = ["kind": "locale", "locale": locale, "status": hasText ? "text" : "empty"]
    if hasText {
        out["segments"] = segments
        out["conf"] = confN > 0 ? confSum / Double(confN) : 0.0
    }
    return out
}

@main struct Main {
    static func main() async {
        guard let args = parseArgs() else {
            // 参数不对也先报版本（对得上版本、却退出码 2 → Rust 判进程失败；对不上 → 不可用、提示重编）
            emit(["kind": "hello", "proto": PROTO])
            exit(2)
        }
        // hello 在兜底线程起来之后才出：Rust 读到它，就知道父进程核对过、兜底在跑
        startWatchdog(parent: args.parent, deadline: args.deadline)
        emit(["kind": "hello", "proto": PROTO])

        let data = FileHandle.standardInput.readDataToEndOfFile()
        let n = data.count / 4
        guard n > 0,
              let fmt = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: args.rate, channels: 1, interleaved: false),
              let buf = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: AVAudioFrameCount(n)) else {
            // 没有声音：每个语种都报失败（码），仍按协议收尾
            for l in args.locales { emit(["kind": "locale", "locale": l, "status": "failed", "error": "no_audio"]) }
            emit(["kind": "done", "count": args.locales.count])
            exit(0)
        }
        buf.frameLength = AVAudioFrameCount(n)
        data.withUnsafeBytes { raw in
            let src = raw.bindMemory(to: Float.self)
            if let base = src.baseAddress, let dst = buf.floatChannelData?[0] { dst.update(from: base, count: n) }
        }

        // 每个语种一个转写器、各自一个分析器：一个出错不连累另一个
        var lines: [[String: Any]] = []
        await withTaskGroup(of: (Int, [String: Any]).self) { g in
            for (k, l) in args.locales.enumerated() {
                g.addTask {
                    let m = SpeechTranscriber(locale: Locale(identifier: l), transcriptionOptions: [],
                                              reportingOptions: [], attributeOptions: [.transcriptionConfidence, .audioTimeRange])
                    do {
                        let conv = try await AnalyzerInputConverter.converter(compatibleWith: [m])
                        var inputs = try conv.convert(buf, at: nil)
                        inputs += try conv.flush()
                        let (stream, cont) = AsyncStream<AnalyzerInput>.makeStream()
                        for x in inputs { cont.yield(x) }
                        cont.finish()
                        let analyzer = SpeechAnalyzer(modules: [m])
                        async let r = collect(m, locale: l)
                        try await analyzer.start(inputSequence: stream)
                        try await analyzer.finalizeAndFinishThroughEndOfInput()
                        return (k, await r)
                    } catch {
                        return (k, ["kind": "locale", "locale": l, "status": "failed", "error": code(error, otherwise: "analyze_failed")])
                    }
                }
            }
            var got: [(Int, [String: Any])] = []
            for await x in g { got.append(x) }
            lines = got.sorted { $0.0 < $1.0 }.map { $0.1 }
        }
        for l in lines { emit(l) }
        emit(["kind": "done", "count": lines.count])
        exit(0)
    }
}
