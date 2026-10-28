// The app's engine path outside the app, for measuring it: the engine and models loaded as EngineHost loads them, a
// composer set up as ControllerCore sets it up, keys typed one at a time, with Core ML as in the app.  Accepts the zhuyin-cli options `zt eval latency` passes, so
//   ZT_CLI=.build/release/EngineBench tools/zt eval latency
// measures it the same way:
//   EngineBench --models DIR [--channel on|off|PATH] [--timing] "<keys>"
// prints each commit and, with --timing, the per-key latency, CPU, energy and memory summary.
import Foundation
import ZhuyinEngine

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

var arguments = Array(CommandLine.arguments.dropFirst())
var models: URL?
var timing = false
var channel: String?
var variantScores: (UInt8, Double)?
var channelSettings = ChannelSettings()
let channelClasses = ["lexicon", "taiwan", "word", "attested", "cns", "unihan", "series", "interjection", "taiwan-word",
                      "neutral", "sandhi"]
func classBits(_ list: String) -> UInt32 {
    list.split(separator: ",").reduce(0) { bits, name in
        if name == "none" { return bits }
        guard let index = channelClasses.firstIndex(of: String(name)) else { fail("unknown class \(name)") }
        return bits | (1 << UInt32(index))
    }
}
while let option = arguments.first, option.hasPrefix("--") {
    switch option {
    case "--models":
        guard arguments.count >= 2 else { fail("--models needs a directory") }
        models = URL(fileURLWithPath: arguments[1])
        arguments.removeFirst(2)
    case "--timing":
        timing = true
        arguments.removeFirst()
    case "--v3-variants":
        // As zhuyin-cli: standard, best, zero or typed, with the window after a comma (default 3).
        guard arguments.count >= 2 else { fail("--v3-variants needs standard, best, zero or typed") }
        let parts = arguments[1].split(separator: ",")
        guard let mode = ["standard", "best", "zero", "typed"].firstIndex(of: String(parts[0])) else {
            fail("--v3-variants needs standard, best, zero or typed")
        }
        variantScores = (UInt8(mode), parts.count > 1 ? Double(parts[1]) ?? 3 : 3)
        arguments.removeFirst(2)
    case "--channel-classes", "--channel-single-classes", "--channel-cap", "--channel-max-cost", "--channel-sandhi":
        // As zhuyin-cli.
        guard arguments.count >= 2 else { fail("\(option) needs a value") }
        let value = arguments[1]
        switch option {
        case "--channel-classes": channelSettings.classes = classBits(value)
        case "--channel-single-classes": channelSettings.singleClasses = classBits(value)
        case "--channel-cap": channelSettings.optionsPerSyllable = UInt32(value) ?? 4
        case "--channel-sandhi": channelSettings.sandhiReverse = value != "forward"
        default: channelSettings.maxCost = Double(value) ?? 8
        }
        arguments.removeFirst(2)
    case "--channel":
        // The reading channel: `on` for channel.bin beside the lexicon, a path, or `off`.
        guard arguments.count >= 2 else { fail("--channel needs on, off or a path") }
        channel = arguments[1] == "off" ? nil : arguments[1]
        arguments.removeFirst(2)
    case "--check-v3":
        // The v3 golden outputs through the app's character model (Rust with Core ML called back), as
        // zhuyin-cli --check-v3 does with ONNX Runtime.
        guard arguments.count >= 2 else { fail("--check-v3 needs a directory") }
        do {
            let directory = URL(fileURLWithPath: arguments[1])
            let check = try SyllableScorer(directory: directory).checkVectors(directory: directory)
            print("v3 test vectors: \(check.passed)/\(check.total) within tolerance; largest difference \(check.worst)")
            exit(check.passed == check.total ? 0 : 1)
        } catch { fail(error.localizedDescription) }
    default: fail("Usage: EngineBench --models DIR [--channel on|off|PATH] [--timing] \"<keys>\"")
    }
}
guard let models, arguments.count == 1 else { fail("Usage: EngineBench --models DIR [--timing] \"<keys>\"") }

let specials: [String: KeyInput] = [
    "space": .space, "bs": .backspace, "enter": .enter, "esc": .escape, "left": .left, "right": .right,
    "home": .home, "end": .end, "up": .up, "down": .down, "tab": .tab, "del": .forwardDelete,
]
var keys: [KeyInput] = []
var index = arguments[0].startIndex
let text = arguments[0]
while index < text.endIndex {
    if text[index] == "<", let end = text[index...].firstIndex(of: ">") {
        let name = String(text[text.index(after: index)..<end])
        guard let key = specials[name] else { fail("Unknown key escape <\(name)>") }
        keys.append(key)
        index = text.index(after: end)
    } else {
        keys.append(text[index] == " " ? .space : .char(text[index]))
        index = text.index(after: index)
    }
}

// EngineHost.reload: the lexicon with lm/ beside it, lm_chat/, reranker/ (configured), v3/.
func exists(_ directory: String, _ file: String) -> URL? {
    let url = models.appendingPathComponent(directory)
    return FileManager.default.fileExists(atPath: url.appendingPathComponent(file).path) ? url : nil
}
let loadStart = Date()
Engine.chatModelDirectory = exists("lm_chat", "lm.bin") ?? exists("lm_chat", "bigram.tsv")
Engine.channelSettings = channelSettings
Engine.readingChannel = channel.map { $0 == "on" ? models.appendingPathComponent("channel.bin") : URL(fileURLWithPath: $0) }
let engine: Engine
do { engine = try Engine(lexiconURL: models.appendingPathComponent("lexicon.tsv"), userDataDirectory: nil) }
catch { fail("engine: \(error.localizedDescription)") }
func load<T>(_ name: String, _ body: () throws -> T?) -> T? {
    do { return try body() } catch { fail("\(name): \(error.localizedDescription)") }
}
let reranker = load("reranker") { try exists("reranker", "vocab.json").map { try NeuralReranker.configured(directory: $0) } }
let characterModel = load("v3") { try exists("v3", "chars.json").map { try SyllableScorer(directory: $0) } }
if timing {
    FileHandle.standardError.write(Data("models: reranker \(reranker != nil), character \(characterModel != nil)\n".utf8))
}
if timing {
    FileHandle.standardError.write(Data(String(format: "engine load: %.3f ms\n", Date().timeIntervalSince(loadStart) * 1000).utf8))
}

// ControllerCore.init with the app's default preferences (learning on, no user data directory), with what the
// composition commits as the text before it.  ZHUYIN_BENCH_LEARNING=off measures with learning off.
var recentText = ""
var options = ComposerOptions()
options.learningEnabled = ProcessInfo.processInfo.environment["ZHUYIN_BENCH_LEARNING"] != "off"
let composer = Composer(engine: engine, options: options)
if let (mode, window) = variantScores { composer.setVariantScores(mode: mode, window: window) }
if let characterModel {
    composer.characterWeight = SyllableScorer.appWeight
    composer.characterScorer = { syllables, candidates in
        characterModel.scores(context: recentText, syllables: syllables.map {
            SyllableScorer.Syllable(base: $0.base, tone: $0.tone, costBucket: $0.costBucket, implicit: $0.implicit)
        }, candidates: candidates)
    }
}
if let reranker {
    composer.rerankingCount = reranker.candidateCount
    composer.reranker = { reranker.choose($0, context: recentText) }
    composer.candidateScorer = { before, candidates in
        reranker.candidateScores(context: recentText + before, candidates: candidates)
    }
}

var keyTimes: [Double] = []
var clock = 1000.0
let usageStart = Usage.now(), wallStart = DispatchTime.now().uptimeNanoseconds
for key in keys {
    clock += 0.12
    let start = DispatchTime.now().uptimeNanoseconds
    let result = composer.handle(key, at: clock)
    keyTimes.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
    if let commit = result.commit, !commit.isEmpty {
        recentText += commit
        print("  committed: \(commit)")
    }
}
print("  shown: \(composer.presentation.text)")
if timing {
    print(usageSummary(keyTimes: keyTimes, from: usageStart, to: Usage.now(),
                       wall: Double(DispatchTime.now().uptimeNanoseconds - wallStart) / 1e9), terminator: "")
}
if let pause = ProcessInfo.processInfo.environment["BENCH_PAUSE"], let seconds = UInt32(pause) { sleep(seconds) }
