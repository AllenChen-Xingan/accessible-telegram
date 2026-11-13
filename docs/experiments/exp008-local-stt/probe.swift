// exp008 一次性探针：标准输入读 f32le 单声道 PCM（采样率由 --rate 给），用 SpeechTranscriber 转写。
// 用法：probe --list | probe --rate 48000 --locale zh-CN [--locale en-US] < x.f32
// 输出 JSON 行：每个 locale 的最终文本、平均置信度、耗时。
import AVFoundation
import Foundation
import Speech

@main struct Probe {
    static func main() async {
        let args = CommandLine.arguments
        if args.contains("--list") { await list(); return }
        var rate = 48000.0
        var locales: [String] = []
        var i = 1
        while i < args.count {
            if args[i] == "--rate" { rate = Double(args[i + 1])!; i += 2 }
            else if args[i] == "--locale" { locales.append(args[i + 1]); i += 2 }
            else { i += 1 }
        }
        let t0 = Date()
        let data = FileHandle.standardInput.readDataToEndOfFile()
        let n = data.count / 4
        let fmt = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: rate, channels: 1, interleaved: false)!
        let buf = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: AVAudioFrameCount(n))!
        buf.frameLength = AVAudioFrameCount(n)
        data.withUnsafeBytes { raw in
            let src = raw.bindMemory(to: Float.self)
            buf.floatChannelData![0].update(from: src.baseAddress!, count: n)
        }
        let modules = locales.map {
            SpeechTranscriber(locale: Locale(identifier: $0), transcriptionOptions: [],
                              reportingOptions: [], attributeOptions: [.transcriptionConfidence])
        }
        do {
            let st = await AssetInventory.status(forModules: modules)
            let best = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: modules, considering: fmt)
            FileHandle.standardError.write("status=\(st) best=\(String(describing: best))\n".data(using: .utf8)!)
            let conv = try await AnalyzerInputConverter.converter(compatibleWith: modules)
            var inputs = try conv.convert(buf, at: nil)
            inputs += try conv.flush()
            let (stream, cont) = AsyncStream<AnalyzerInput>.makeStream()
            for x in inputs { cont.yield(x) }
            cont.finish()
            let analyzer = SpeechAnalyzer(modules: modules)
            let tReady = Date()
            let collectors = modules.map { m in
                Task { () -> (String, Double, Int) in
                    var text = ""; var confSum = 0.0; var confN = 0
                    for try await r in m.results {
                        text += String(r.text.characters)
                        for run in r.text.runs {
                            if let c = run.transcriptionConfidence { confSum += c; confN += 1 }
                        }
                    }
                    return (text, confN > 0 ? confSum / Double(confN) : -1, confN)
                }
            }
            try await analyzer.start(inputSequence: stream)
            try await analyzer.finalizeAndFinishThroughEndOfInput()
            for (k, c) in collectors.enumerated() {
                let (text, conf, runs) = try await c.value
                let out: [String: Any] = ["locale": locales[k], "text": text, "conf": conf, "runs": runs,
                                          "audio_s": Double(n) / rate,
                                          "analyze_s": Date().timeIntervalSince(tReady),
                                          "total_s": Date().timeIntervalSince(t0)]
                let j = try JSONSerialization.data(withJSONObject: out, options: [.sortedKeys])
                print(String(data: j, encoding: .utf8)!)
            }
        } catch {
            FileHandle.standardError.write("error: \(error)\n".data(using: .utf8)!)
            exit(2)
        }
    }

    static func list() async {
        print("isAvailable", SpeechTranscriber.isAvailable)
        print("installed", await SpeechTranscriber.installedLocales.map(\.identifier).sorted())
        let sup = await SpeechTranscriber.supportedLocales.map(\.identifier)
        print("supported count", sup.count, sup.filter { $0.hasPrefix("zh") || $0.hasPrefix("en") || $0.hasPrefix("yue") }.sorted())
        for l in ["zh-CN", "en-US"] {
            let m = SpeechTranscriber(locale: Locale(identifier: l), preset: .transcription)
            print(l, await AssetInventory.status(forModules: [m]),
                  await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [m]) as Any)
        }
        print("reserved", await AssetInventory.reservedLocales.map(\.identifier), "max", AssetInventory.maximumReservedLocales)
    }
}
