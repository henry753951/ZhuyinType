// Reference values and timings of the Swift engine for the Rust port.  Every sweep here has a twin in
// crates/zhuyin-core/tests/golden.rs (and the timings in benches/data_layer.rs) that builds the same queries in the same
// order; the digests (FNV-1a over the result lines) must match.  Floats are printed as their bit patterns.
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

func hex(_ value: Double) -> String { String(format: "%016llx", value.bitPattern) }

func field<T>(_ object: Any, _ name: String) -> T {
    guard let child = Mirror(reflecting: object).children.first(where: { $0.label == name }) else { fail("no field \(name)") }
    return child.value as! T
}

func byteSorted<S: Sequence>(_ strings: S) -> [String] where S.Element == String {
    Array(Set(strings)).sorted { $0.utf8.lexicographicallyPrecedes($1.utf8) }
}

/// The shared inputs: the TSV lexicon's readings and words, and word sequences of the text.
struct Inputs {
    let data: URL
    let tsv: Data
    let readings: [String]
    let words: [String]
    let sequences: [[String]]

    init(data: URL, text: URL?) {
        self.data = data
        guard let tsv = try? Data(contentsOf: data.appendingPathComponent("lexicon.tsv")) else { fail("no lexicon.tsv") }
        self.tsv = tsv
        var readings: [String] = [], words: [String] = []
        scanTSV(tsv) { fields in
            guard fields.count == 3, let score = Double(fields[2]), score.isFinite, !fields[0].isEmpty, !fields[1].isEmpty
            else { return }
            readings.append(fields[0]); words.append(fields[1])
        }
        self.readings = byteSorted(readings)
        self.words = byteSorted(words)
        var sequences: [[String]] = []
        if let text, let content = try? String(contentsOf: text, encoding: .utf8) {
            let known = Set(self.words)
            for line in content.split(separator: "\n").prefix(2000) {
                let scalars = Array(line.unicodeScalars)
                var sequence: [String] = []
                var i = 0
                while i < scalars.count {
                    for length in stride(from: min(6, scalars.count - i), through: 1, by: -1) {
                        var piece = String.UnicodeScalarView()
                        piece.append(contentsOf: scalars[i..<(i + length)])
                        let word = String(piece)
                        if length == 1 || known.contains(word) { sequence.append(word); i += length; break }
                    }
                }
                if !sequence.isEmpty { sequences.append(sequence) }
            }
        }
        self.sequences = sequences
    }
}

/// A trigram model to exercise the trigram branches (the shipped models are bigrams): `$ZHUYIN_TRIGRAM_LM`.
let trigramDirectory = URL(fileURLWithPath: ProcessInfo.processInfo.environment["ZHUYIN_TRIGRAM_LM"] ?? "/nonexistent")

/// The dev sets for the conversion sweeps (colloquial, encyclopedic, chat): `$ZHUYIN_PARITY_DEV`, colon-separated.
let devFiles = (ProcessInfo.processInfo.environment["ZHUYIN_PARITY_DEV"] ?? "").split(separator: ":").map(String.init)

let extras = ["", "ㄅ", "不存在的讀音", "x", "ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ"]

func withoutTones(_ reading: String) -> String {
    String(String.UnicodeScalarView(reading.unicodeScalars.filter { $0 != "ˊ" && $0 != "ˇ" && $0 != "ˋ" && $0 != "˙" }))
}

func lexiconSweeps(_ inputs: Inputs) -> [(String, [String])] {
    guard let lexicon = try? MappedLexicon(url: inputs.data.appendingPathComponent("lexicon.bin")) else { fail("lexicon.bin") }
    var result: [(String, [String])] = []
    result.append(("lexicon.entries", (inputs.readings + extras).map { reading in
        reading + lexicon.entries(for: reading).map { "\t\($0.word),\(hex($0.logp)),\(hex($0.readingPrior))" }.joined()
    }))
    result.append(("lexicon.words", (inputs.words + extras).map { word in
        let index = lexicon.wordIndex(word).map(String.init) ?? "-"
        let readings = lexicon.readings(of: word).map { "\($0.0),\(hex($0.1))" }.joined(separator: " ")
        let best = lexicon.bestReading(of: word).map { "\($0.0),\(hex($0.1))" } ?? "-"
        return "\(word)\t\(index)\t\(readings)\t\(best)"
    }))
    let bases = byteSorted(inputs.readings.map(withoutTones)) + extras
    result.append(("lexicon.bases", bases.map { "\($0)\t" + lexicon.readings(forBase: $0).joined(separator: ",") }))
    var prefixes: [String] = []
    for base in bases {
        var prefix = ""
        for part in base.split(separator: " ") {
            prefix += (prefix.isEmpty ? "" : " ") + part
            prefixes.append(prefix)
        }
        prefixes.append(base + " ㄅ"); prefixes.append(base + "ㄅ")
    }
    result.append(("lexicon.prefixes", byteSorted(prefixes).map { "\($0)\t\(lexicon.containsPrefix($0) ? 1 : 0)" }))
    result.append(("lexicon.syllables", lexicon.syllables))
    // The TSV path of Engine: its private tables, read by reflection.
    let engine = try! Engine(lexiconText: String(decoding: inputs.tsv, as: UTF8.self))
    let table: [String: [WordEntry]] = field(engine, "lexicon")
    let reverse: [String: [(String, Double)]] = field(engine, "reverse")
    result.append(("textlexicon.entries", inputs.readings.map { reading in
        reading + (table[reading] ?? []).map {
            "\t\($0.word),\(hex($0.logp)),\(hex($0.readingPrior)),\($0.wordID.map(String.init) ?? "-")"
        }.joined()
    }))
    result.append(("textlexicon.reverse", inputs.words.map { word in
        word + (reverse[word] ?? []).map { "\t\($0.0),\(hex($0.1))" }.joined()
    }))
    return result
}

/// Scores along the text's word sequences (with sentence ends), over random pairs, and special IDs.
func modelSweep(_ name: String, _ model: any LanguageModel, _ inputs: Inputs, ids: Bool = true) -> [(String, [String])] {
    var result: [(String, [String])] = []
    if ids {
        result.append(("\(name).ids", (inputs.words + ["</s>", "<s>", "<unk>", ""]).map {
            "\($0)\t" + (model.wordID(for: $0).map(String.init) ?? "-")
        }))
    }
    var lines: [String] = []
    for sequence in inputs.sequences {
        let wordIDs = sequence.map { model.wordID(for: $0) }
        var history = LanguageModelHistory(older: nil, previous: 1)
        for (j, word) in sequence.enumerated() {
            history = LanguageModelHistory(older: j == 0 ? nil : j == 1 ? 1 : (wordIDs[j - 2] ?? 2),
                                           previous: j == 0 ? 1 : (wordIDs[j - 1] ?? 2))
            lines.append("\(word)\t" + hex(model.score(wordID: wordIDs[j], history: history, unigram: -9.25)))
        }
        let end = LanguageModelHistory(older: sequence.count > 1 ? (wordIDs[sequence.count - 2] ?? 2) : 1,
                                       previous: wordIDs.last! ?? 2)
        Engine.sentenceEndShare = 1
        let full = model.endScore(history: end)
        Engine.sentenceEndShare = 0.7
        lines.append("</s>\t\(hex(full))\t\(hex(model.endScore(history: end)))")
        Engine.sentenceEndShare = 1
    }
    let n = inputs.words.count
    for i in stride(from: 0, to: n, by: 7) {
        let word = inputs.words[i], previous = inputs.words[(i * 7919 + 13) % n], older = inputs.words[(i * 104729 + 7) % n]
        let history = LanguageModelHistory(older: model.wordID(for: older), previous: model.wordID(for: previous) ?? 2)
        lines.append("\(word)\t\(previous)\t\(older)\t" + hex(model.score(wordID: model.wordID(for: word), history: history, unigram: -9.25)))
    }
    let max = UInt32.max
    for (word, older, previous) in [(nil, nil, 1), (max, nil, max), (3, nil, max), (3, nil, 2), (0, nil, 1), (2, nil, 2),
                                    (1, nil, 0), (5, 1, 1), (4, 3, 5), (nil, 4, 3)] as [(UInt32?, UInt32?, UInt32)] {
        lines.append(hex(model.score(wordID: word, history: LanguageModelHistory(older: older, previous: previous), unigram: -9.25)))
    }
    result.append(("\(name).scores", lines))
    return result
}

func languageModelSweeps(_ inputs: Inputs) -> [(String, [String])] {
    var result: [(String, [String])] = []
    let directory = inputs.data
    guard let lm = try? NGramLanguageModel(directory: directory.appendingPathComponent("lm")) else { fail("lm/lm.bin") }
    result += modelSweep("lm", lm, inputs)
    if let chat = try? NGramLanguageModel(directory: directory.appendingPathComponent("lm_chat")) {
        result += modelSweep("lm_chat", chat, inputs)
        let mixture = MixtureLanguageModel(chat: chat, encyclopedia: lm, chatWeight: 0.3)
        result += modelSweep("mixture", mixture, inputs)
        var parts: [String] = []
        for sequence in inputs.sequences {
            var previous: UInt32 = 1
            for word in sequence {
                let id = mixture.wordID(for: word)
                let (c, e) = mixture.scores(wordID: id, previous: previous, unigram: -14)
                parts.append("\(word)\t\(hex(c))\t\(hex(e))")
                previous = id ?? 2
            }
        }
        result.append(("mixture.parts", parts))
        mixture.chatWeight = 0.7
        result += modelSweep("mixture07", mixture, inputs, ids: false)
    }
    result += modelSweep("boundaryfree", BoundaryFreeLanguageModel(base: lm), inputs, ids: false)
    if let lm2 = try? NGramLanguageModel(directory: trigramDirectory) {
        result += modelSweep("lm2", lm2, inputs)
    }
    if let tsv = try? BigramLanguageModel(directory: directory.appendingPathComponent("lm")) {
        result += modelSweep("lmtsv", tsv, inputs)
    }
    return result
}

/// A mapped-lexicon engine without a language model: its syllables and channel codes.
func mappedEngine(_ inputs: Inputs) -> Engine {
    try! Engine(lexiconURL: inputs.data.appendingPathComponent("lexicon.tsv"), userDataDirectory: nil, languageModelDirectory: nil)
}

/// Key strings near every syllable: as typed, one key deleted, two swapped, one replaced by a neighbour, one added.
func channelKeys(_ syllables: [String]) -> [String] {
    var result: [String] = []
    for syllable in syllables {
        guard let keys = KeyChannel.keys(for: syllable) else { continue }
        let k = Array(keys)
        result.append(keys)
        let middle = k.count / 2
        var deleted = k; deleted.remove(at: middle); result.append(String(deleted))
        if k.count > 1 { var swapped = k; swapped.swapAt(0, 1); result.append(String(swapped)) }
        if let neighbour = KeyChannel.neighbours(of: k[0]).first { var replaced = k; replaced[0] = neighbour; result.append(String(replaced)) }
        result.append("u" + keys)
    }
    return result
}

func channelSweeps(_ inputs: Inputs) -> [(String, [String])] {
    let engine = mappedEngine(inputs)
    let validCodes: [Int: String] = field(engine, "validCodes")
    var result: [(String, [String])] = []
    result.append(("channel.codes", validCodes.keys.sorted().map { "\($0)\t\(validCodes[$0]!)" }))
    let syllables = byteSorted(field(engine, "syllables") as Set<String>)
    let keys = channelKeys(syllables)
    func line(_ keys: String, _ alternatives: [ReadingAlternative]) -> String {
        keys + alternatives.map { "\t\($0.reading),\(hex($0.cost))" }.joined()
    }
    result.append(("channel.alternatives", keys.map { line($0, KeyChannel.alternatives(for: $0) { validCodes[$0] }) }))
    let discount: (UInt8, UInt8) -> Double = { typed, intended in
        (Int(typed) + Int(intended)) % 5 == 0 ? 1.5 * log(1 + Double(typed % 7)) : 0
    }
    let subset = keys.enumerated().filter { $0.offset % 10 == 0 }.map(\.element)
    result.append(("channel.discount", subset.map { line($0, KeyChannel.alternatives(for: $0, discount: discount) { validCodes[$0] }) }))
    KeyChannel.hotspot = true
    result.append(("channel.hotspot", subset.map { line($0, KeyChannel.alternatives(for: $0) { validCodes[$0] }) }))
    KeyChannel.hotspot = false
    return result
}

/// Key runs typing lexicon phrases, as typed and with typical slips.
func decoderRuns(_ inputs: Inputs) -> [(String, [KeyDecoder.Key])] {
    let phrases = inputs.readings.filter { $0.contains(" ") }.enumerated().filter { $0.offset % 50 == 0 }.map(\.element)
    var runs: [(String, [KeyDecoder.Key])] = []
    func timed(_ keys: [Character], gapAt: Int? = nil) -> [KeyDecoder.Key] {
        var time = 100.0
        return keys.enumerated().map { index, key in
            time += index == gapAt ? 0.006 : 0.12
            return KeyDecoder.Key(character: key, time: time)
        }
    }
    for phrase in phrases {
        let parts = phrase.split(separator: " ").compactMap { KeyChannel.keys(for: String($0)) }
        let keys = Array(parts.joined())
        runs.append(("exact", timed(keys)))
        // First tones without Space, except the last.
        let implicit = Array(parts.enumerated().map { $0.offset < parts.count - 1 && $0.element.hasSuffix(" ") ? String($0.element.dropLast()) : $0.element }.joined())
        runs.append(("implicit", timed(implicit)))
        let middle = keys.count / 2
        if middle + 1 < keys.count {
            var swapped = keys; swapped.swapAt(middle, middle + 1)
            runs.append(("swapped", timed(swapped)))
            runs.append(("together", timed(swapped, gapAt: middle + 1)))
        }
        if keys.count > 1, let neighbour = KeyChannel.neighbours(of: keys[1]).first {
            var extra = keys; extra.insert(neighbour, at: 2)
            runs.append(("extra", timed(extra)))
            runs.append(("extra-together", timed(extra, gapAt: 2)))
        }
    }
    return runs
}

func decodeLine(_ results: [KeyDecoder.Result]) -> String {
    results.map { result in
        result.syllables.map { "\($0.reading)/\($0.keys)/\($0.implicit ? 1 : 0)" }.joined(separator: " ") + "|"
            + String(result.buffer.map { $0 ?? "_" }) + "|" + result.bufferKeys + "|" + hex(result.cost)
    }.joined(separator: "\t")
}

func decoderSweeps(_ inputs: Inputs) -> [(String, [String])] {
    let engine = mappedEngine(inputs)
    let decoders = KeyDecoderMethod.allCases.map { method in
        KeyDecoder(method: method) { reading, keys in
            if engine.isValidSyllable(reading) { return 0 }
            return engine.alternatives(for: keys).first?.cost
        }
    }
    var lines: [String] = []
    for (label, keys) in decoderRuns(inputs) {
        for (index, method) in KeyDecoderMethod.allCases.enumerated() {
            lines.append("\(label)\t\(String(keys.map(\.character)))\t\(method.rawValue)\t" + decodeLine(decoders[index].decode(keys, limit: 6)))
        }
    }
    return [("decoder", lines)]
}

func fnv(_ lines: [String]) -> UInt64 {
    var hash: UInt64 = 0xcbf2_9ce4_8422_2325
    for line in lines {
        for byte in line.utf8 { hash ^= UInt64(byte); hash = hash &* 0x100_0000_01b3 }
        hash ^= 10; hash = hash &* 0x100_0000_01b3
    }
    return hash
}

// MARK: Timings

/// Median seconds per call of `body`, over `samples` timed batches of `batch` calls.
func measure(_ label: String, samples: Int = 40, batch: Int = 1, _ body: () -> Void) {
    for _ in 0..<3 { body() }
    var times: [Double] = []
    for _ in 0..<samples {
        let start = DispatchTime.now().uptimeNanoseconds
        for _ in 0..<batch { body() }
        times.append(Double(DispatchTime.now().uptimeNanoseconds - start) / Double(batch))
    }
    times.sort()
    print(String(format: "%@\tmedian %.3f µs\tp10 %.3f µs\tp90 %.3f µs", label, times[times.count / 2] / 1000,
                 times[times.count / 10] / 1000, times[times.count * 9 / 10] / 1000))
}

@inline(never) func blackHole<T>(_ value: T) { withExtendedLifetime(value) {} }

/// Runs of `count` syllables typing the text's words (each word's most common reading).
func syllableRuns(_ inputs: Inputs, lexicon: MappedLexicon, count: Int, runs: Int) -> [[KeyDecoder.Key]] {
    var syllables: [String] = []
    for sequence in inputs.sequences {
        for word in sequence {
            guard let reading = lexicon.bestReading(of: word)?.0 else { continue }
            syllables += reading.split(separator: " ").map(String.init)
        }
        if syllables.count > count * runs * 2 { break }
    }
    return (0..<runs).map { run in
        var time = 100.0
        return syllables[(run * count)..<(run * count + count)].flatMap { KeyChannel.keys(for: $0).map(Array.init) ?? [] }.map {
            time += 0.12
            return KeyDecoder.Key(character: $0, time: time)
        }
    }
}

func bench(_ inputs: Inputs) {
    guard let lexicon = try? MappedLexicon(url: inputs.data.appendingPathComponent("lexicon.bin")) else { fail("lexicon.bin") }
    let readings = inputs.readings.enumerated().filter { $0.offset % 50 == 0 }.map(\.element)
    let words = inputs.words.enumerated().filter { $0.offset % 50 == 0 }.map(\.element)
    print("lexicon queries: \(readings.count) readings, \(words.count) words")
    measure("lexicon.entries (batch of \(readings.count))") { for r in readings { blackHole(lexicon.entries(for: r)) } }
    measure("lexicon.word_index (batch of \(words.count))") { for w in words { blackHole(lexicon.wordIndex(w)) } }
    measure("lexicon.contains_prefix (batch of \(readings.count))") { for r in readings { blackHole(lexicon.containsPrefix(withoutTones(r))) } }
    measure("lexicon.open") { blackHole(try! MappedLexicon(url: inputs.data.appendingPathComponent("lexicon.bin"))) }

    let lm = try! NGramLanguageModel(directory: inputs.data.appendingPathComponent("lm"))
    let chat = try! NGramLanguageModel(directory: inputs.data.appendingPathComponent("lm_chat"))
    let mixture = MixtureLanguageModel(chat: chat, encyclopedia: lm, chatWeight: 0.3)
    let lm2 = try? NGramLanguageModel(directory: trigramDirectory)
    func scoringBatch(_ model: any LanguageModel) -> [(UInt32?, LanguageModelHistory)] {
        var batch: [(UInt32?, LanguageModelHistory)] = []
        for sequence in inputs.sequences.prefix(200) {
            let ids = sequence.map { model.wordID(for: $0) }
            for j in ids.indices {
                batch.append((ids[j], LanguageModelHistory(older: j == 0 ? nil : j == 1 ? 1 : (ids[j - 2] ?? 2),
                                                           previous: j == 0 ? 1 : (ids[j - 1] ?? 2))))
            }
        }
        return batch
    }
    for (name, model) in [("lm", lm as any LanguageModel), ("mixture", mixture)] + (lm2.map { [("lm2-trigram", $0 as any LanguageModel)] } ?? []) {
        let batch = scoringBatch(model)
        measure("score \(name) (batch of \(batch.count))") {
            var total = 0.0
            for (id, history) in batch { total += model.score(wordID: id, history: history, unigram: -9.25) }
            blackHole(total)
        }
    }
    let idWords = Array(inputs.sequences.prefix(200).joined())
    measure("lm.word_id (batch of \(idWords.count))") { for w in idWords { blackHole(lm.wordID(for: w)) } }
    measure("lm.open") { blackHole(try! NGramLanguageModel(directory: inputs.data.appendingPathComponent("lm"))) }
    measure("mixture.new") { blackHole(MixtureLanguageModel(chat: chat, encyclopedia: lm, chatWeight: 0.3)) }

    let engine = mappedEngine(inputs)
    let validCodes: [Int: String] = field(engine, "validCodes")
    let keys = channelKeys(byteSorted(field(engine, "syllables") as Set<String>)).enumerated().filter { $0.offset % 20 == 0 }.map(\.element)
    measure("channel.alternatives (batch of \(keys.count))", samples: 20) {
        for k in keys { blackHole(KeyChannel.alternatives(for: k) { validCodes[$0] }) }
    }
    let decoder = KeyDecoder(method: .timed) { reading, keys in
        if engine.isValidSyllable(reading) { return 0 }
        return engine.alternatives(for: keys).first?.cost
    }
    let app = appEngine(inputs.data)
    for count in [10, 25, 40] {
        let runs = syllableRuns(inputs, lexicon: lexicon, count: count, runs: 10).map { run -> [CompositionToken] in
            var tokens: [CompositionToken] = [], keys = ""
            for key in run.map(\.character) {
                keys.append(key)
                if ZhuyinKeyboard.tones[key] != nil || key == " " {
                    var slots: [Character?] = [nil, nil, nil]
                    for k in keys { if let s = ZhuyinKeyboard.symbols[k], let slot = ZhuyinKeyboard.slot(of: s) { slots[slot] = s } }
                    tokens.append(.syllable(String(slots.compactMap { $0 }) + (ZhuyinKeyboard.tones[key] ?? ""), keys: keys))
                    keys = ""
                }
            }
            return tokens
        }
        measure("convert per syllable typed, \(count) syllables (batch of \(runs.count) runs)", samples: 20) {
            for tokens in runs {
                var cache: ConversionCache?
                for end in 1...tokens.count { blackHole(app.convert(Array(tokens.prefix(end)), fixed: [], typo: .standard, cache: &cache)) }
            }
        }
        measure("16-best, \(count) syllables (batch of \(runs.count) runs)", samples: 20) {
            for tokens in runs { blackHole(app.sentences(tokens, fixed: [], typo: .standard, k: 16)) }
        }
        // The whole composer path, key by key, as the app types (timed key decoder, candidate-free).
        let keyRuns = syllableRuns(inputs, lexicon: lexicon, count: count, runs: 10)
        let composer = Composer(engine: app, options: ComposerOptions())
        measure("composer typing, \(count) syllables (batch of \(keyRuns.count) runs, \(keyRuns.map(\.count).reduce(0, +)) keys)", samples: 20) {
            for run in keyRuns {
                composer.reset()
                for key in run { blackHole(composer.handle(key.character == " " ? .space : .char(key.character), at: key.time!)) }
            }
        }
    }
    for count in [10, 25, 40] {
        let runs = syllableRuns(inputs, lexicon: lexicon, count: count, runs: 10)
        measure("decode timed \(count) syllables (batch of \(runs.count) runs, \(runs.map(\.count).reduce(0, +)) keys)", samples: 20) {
            for run in runs { blackHole(decoder.decode(run, limit: 6)) }
        }
    }
}

let arguments = CommandLine.arguments
guard arguments.count >= 3 else { fail("usage: SwiftParity golden DATA TEXT OUT | bench DATA TEXT") }
let inputs = Inputs(data: URL(fileURLWithPath: arguments[2]), text: arguments.count > 3 ? URL(fileURLWithPath: arguments[3]) : nil)
switch arguments[1] {
case "golden":
    guard arguments.count == 5 else { fail("golden DATA TEXT OUT") }
    let out = URL(fileURLWithPath: arguments[4])
    try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
    let only = ProcessInfo.processInfo.environment["SWEEPS"]?.split(separator: ",").map(String.init)
    var sweeps: [(String, () -> [(String, [String])])] = [
        ("lexicon", { lexiconSweeps(inputs) }), ("lm", { languageModelSweeps(inputs) }),
        ("channel", { channelSweeps(inputs) }), ("decoder", { decoderSweeps(inputs) }),
        ("format", { formatSweep() }), ("conversion", { conversionSweeps(inputs, files: devFiles) }),
        ("learning", { learningSweeps(inputs, files: devFiles) }), ("composer", { composerSweeps(inputs, files: devFiles) }),
    ]
    if let only { sweeps = sweeps.filter { only.contains($0.0) } }
    for (_, run) in sweeps {
        for (name, lines) in run() {
            try! Data((lines.joined(separator: "\n") + "\n").utf8).write(to: out.appendingPathComponent("\(name).txt"))
            print("\(name)\t\(lines.count)\t\(String(format: "%016llx", fnv(lines)))")
        }
    }
case "bench":
    bench(inputs)
default:
    fail("unknown command \(arguments[1])")
}
