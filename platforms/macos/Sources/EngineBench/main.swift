// The app's engine path outside the app, for measuring it: the engine and models loaded as EngineHost loads them, a
// composer set up as ControllerCore sets it up, keys typed one at a time, with Core ML as in the app.  Accepts the zhuyin-cli options `zt eval latency` passes, so
//   ZT_CLI=.build/release/EngineBench tools/zt eval latency
// measures it the same way:
//   EngineBench --models DIR [--timing] [--early-commit N] [--max-syllables N] "<keys>"
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
var earlyCommit = 0
var maxSyllables: Int?
while let option = arguments.first, option.hasPrefix("--") {
    switch option {
    case "--early-commit", "--max-syllables":
        // As zhuyin-cli: the early commit (0: off) and the longest composition.
        guard arguments.count >= 2, let value = Int(arguments[1]), value >= 0 else { fail("\(option) needs a number") }
        if option == "--early-commit" { earlyCommit = value } else { maxSyllables = value }
        arguments.removeFirst(2)
    case "--models":
        guard arguments.count >= 2 else { fail("--models needs a directory") }
        models = URL(fileURLWithPath: arguments[1])
        arguments.removeFirst(2)
    case "--timing":
        timing = true
        arguments.removeFirst()
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
    default: fail("Usage: EngineBench --models DIR [--timing] [--early-commit N] [--max-syllables N] \"<keys>\"")
    }
}
guard let models, arguments.count == 1 else { fail("Usage: EngineBench --models DIR [--timing] [--early-commit N] [--max-syllables N] \"<keys>\"") }

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
        if name.hasPrefix("C-"), name.count == 3, let character = name.last {
            keys.append(.controlChar(character))
        } else {
            guard let key = specials[name] else { fail("Unknown key escape <\(name)>") }
            keys.append(key)
        }
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
options.earlyCommitSyllables = earlyCommit
if let maxSyllables { options.maxSyllables = maxSyllables }
let composer = Composer(engine: engine, options: options)
// A full composition's head is text before the rest while the rest is converted again (ControllerCore does the same).
var sentDuringKey = ""
composer.commitListener = { text in
    recentText += text
    sentDuringKey += text
}
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
        recentText += String(commit.dropFirst(sentDuringKey.count))
        print("  committed: \(commit)")
    }
    sentDuringKey = ""
}
print("  shown: \(composer.presentation.text)")
if timing {
    print(usageSummary(keyTimes: keyTimes, from: usageStart, to: Usage.now(),
                       wall: Double(DispatchTime.now().uptimeNanoseconds - wallStart) / 1e9), terminator: "")
}
if let pause = ProcessInfo.processInfo.environment["BENCH_PAUSE"], let seconds = UInt32(pause) { sleep(seconds) }
