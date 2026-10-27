// Phase 2 sweeps: Swift's number formatting, conversion (Viterbi, N-best, incremental cache, fixed words, groups,
// character scores, the text before the cursor) on dev-set key scripts, and a learning session with its personal
// data.  The twin is crates/zhuyin-core/tests/golden_engine.rs.
import Foundation

/// A 64-bit LCG shared with the Rust twin.
struct Generator {
    var state: UInt64
    mutating func next() -> UInt64 {
        state = state &* 6364136223846793005 &+ 1442695040888963407
        return state
    }
}

func formatSweep() -> [(String, [String])] {
    var generator = Generator(state: 7)
    var lines: [String] = []
    for i in 0..<60000 {
        let bits = generator.next()
        var value: Double
        switch i % 6 {
        case 0: value = Double(bitPattern: bits)
        case 1: value = Double(bits >> 11) / 1000
        case 2: value = 1_700_000_000 + Double(bits >> 40) / 1000
        // exp2 and parsed powers of ten: pow's results depend on how the optimizer rewrites it.
        case 3: value = Double(bits >> 30) * exp2(-Double(bits % 64))
        case 4: value = Double(bits % 100_000) / 100_000 * Double("1e\(Int(bits >> 58) - 20)")!
        default: value = 0.05 * Double(bits % 40) * exp2(-(Double(bits >> 54) / 3600))
        }
        guard value.isFinite else { continue }
        lines.append("\(hex(value))\t\(value)")
    }
    return [("format.doubles", lines)]
}

/// The dev-set Han runs (4–20 characters, as zhuyintype_ml.core.zhuyin.sample_runs finds them) with the text before them.
func devRuns(_ path: String, limit: Int) -> [(run: String, before: String)] {
    guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return [] }
    var result: [(String, String)] = []
    for line in text.split(separator: "\n") {
        let scalars = Array(line.unicodeScalars)
        var i = 0
        while i < scalars.count {
            guard (0x4E00...0x9FFF).contains(scalars[i].value) else { i += 1; continue }
            var end = i
            while end < scalars.count, end - i < 20, (0x4E00...0x9FFF).contains(scalars[end].value) { end += 1 }
            if end - i >= 4 {
                var run = String.UnicodeScalarView(), before = String.UnicodeScalarView()
                run.append(contentsOf: scalars[i..<end])
                before.append(contentsOf: scalars[max(0, i - 32)..<i])
                result.append((String(run), String(before)))
                if result.count >= limit { return result }
            }
            i = end
        }
    }
    return result
}

/// Each word's first most likely reading (zhuyintype_ml.core.zhuyin.Lexicon).
func bestReadings(_ tsv: Data) -> [String: (String, Double)] {
    var best: [String: (String, Double)] = [:]
    scanTSV(tsv) { fields in
        guard fields.count == 3, let score = Double(fields[2]), score.isFinite, !fields[0].isEmpty, !fields[1].isEmpty else { return }
        if let old = best[fields[1]], old.1 >= score { return }
        best[fields[1]] = (fields[0], score)
    }
    return best
}

func runTokens(_ run: String, _ best: [String: (String, Double)]) -> [CompositionToken]? {
    let scalars = Array(run.unicodeScalars)
    var readings: [String] = []
    var i = 0
    outer: while i < scalars.count {
        for length in stride(from: min(6, scalars.count - i), through: 1, by: -1) {
            var piece = String.UnicodeScalarView()
            piece.append(contentsOf: scalars[i..<(i + length)])
            if let reading = best[String(piece)] {
                readings += reading.0.split(separator: " ").map(String.init)
                i += length
                continue outer
            }
        }
        return nil
    }
    var tokens: [CompositionToken] = []
    for reading in readings {
        guard let keys = KeyChannel.keys(for: reading) else { return nil }
        tokens.append(.syllable(reading, keys: keys))
    }
    return tokens
}

/// The middle syllable typed with its first key replaced by that key's first neighbour.
func typoTokens(_ tokens: [CompositionToken]) -> [CompositionToken]? {
    let middle = tokens.count / 2
    guard case .syllable(_, let keys, _) = tokens[middle], let first = keys.first,
          let neighbour = KeyChannel.neighbours(of: first).first else { return nil }
    let typed = String(neighbour) + keys.dropFirst()
    var slots: [Character?] = [nil, nil, nil]
    var tone = ""
    for key in typed {
        if let symbol = ZhuyinKeyboard.symbols[key], let slot = ZhuyinKeyboard.slot(of: symbol) { slots[slot] = symbol }
        else if let mark = ZhuyinKeyboard.tones[key] { tone = mark }
    }
    guard slots.contains(where: { $0 != nil }) else { return nil }
    var result = tokens
    result[middle] = .syllable(String(slots.compactMap { $0 }) + tone, keys: typed)
    return result
}

func historyText(_ history: LanguageModelHistory?) -> String {
    guard let history else { return "-" }
    return "\(history.older.map(String.init) ?? "-"),\(history.previous)"
}

func wordsLine(_ words: [ConvertedWord]) -> String {
    words.map { "\($0.text)/\($0.reading)/\($0.range.lowerBound)-\($0.range.upperBound)/\($0.wordID.map(String.init) ?? "-")/\(historyText($0.history))/\($0.corrected ? 1 : 0)" }
        .joined(separator: " ")
}

func sentencesLine(_ sentences: [ScoredSentence]) -> String {
    sentences.map { sentence in
        "\(sentence.text),\(hex(sentence.score)),\(sentence.corrected ? 1 : 0)," + sentence.words.map { "\($0.range.lowerBound)-\($0.range.upperBound)" }.joined(separator: ".")
    }.joined(separator: "\t")
}

/// Synthetic character scores from the N best: rank r's characters score −0.25 − r/4 where nothing earlier did.
func syntheticCharacters(_ sentences: [ScoredSentence], count: Int) -> CharacterScores {
    var positions = Array(repeating: [Character: Double](), count: count)
    for (rank, sentence) in sentences.enumerated() {
        for word in sentence.words where !word.reading.isEmpty {
            for (offset, character) in word.text.enumerated() where word.range.lowerBound + offset < count {
                if positions[word.range.lowerBound + offset][character] == nil {
                    positions[word.range.lowerBound + offset][character] = -0.25 - Double(rank) / 4
                }
            }
        }
    }
    return CharacterScores(positions: positions, weight: 0.5)
}

func appEngine(_ data: URL) -> Engine {
    Engine.chatModelDirectory = data.appendingPathComponent("lm_chat")
    Engine.chatPrior = 0.3
    defer { Engine.chatModelDirectory = nil }
    return try! Engine(lexiconURL: data.appendingPathComponent("lexicon.tsv"), userDataDirectory: nil,
                       languageModelDirectory: data.appendingPathComponent("lm"))
}

/// Every dev file's runs: (file index, run, text before, tokens).
func devScripts(_ inputs: Inputs, files: [String], limit: Int) -> [(Int, String, String, [CompositionToken])] {
    let best = bestReadings(inputs.tsv)
    var result: [(Int, String, String, [CompositionToken])] = []
    for (index, file) in files.enumerated() {
        for (run, before) in devRuns(file, limit: limit * 2) {
            guard let tokens = runTokens(run, best) else { continue }
            result.append((index, run, before, tokens))
            if result.filter({ $0.0 == index }).count >= limit { break }
        }
    }
    return result
}

func conversionSweeps(_ inputs: Inputs, files: [String]) -> [(String, [String])] {
    let engine = appEngine(inputs.data)
    var plain: [String] = [], nbest: [String] = [], incremental: [String] = [], fixedLines: [String] = []
    var characterLines: [String] = []
    for (index, (file, run, before, tokens)) in devScripts(inputs, files: files, limit: 150).enumerated() {
        // The text before the run counts for the encyclopedic set (`zt eval conversion --context line`).
        let context = file == 1 ? before : ""
        let weight = engine.adaptDomain(to: context)
        let start = engine.trailingWords(of: context).last?.history ?? LanguageModelHistory()
        var variants = [("plain", tokens)]
        if index % 3 == 1, let typo = typoTokens(tokens) { variants.append(("typo", typo)) }
        for (label, tokens) in variants {
            let head = "\(file)\t\(run)\t\(label)\t\(weight.map(hex) ?? "-")\t"
            var cache: ConversionCache?
            let best = engine.convert(tokens, fixed: [], typo: .standard, cache: &cache, start: start)
            plain.append(head + wordsLine(best))
            let sentences = engine.sentences(tokens, fixed: [], typo: .standard, k: 16, start: start)
            nbest.append(head + sentencesLine(sentences))
            var steps: [String] = []
            var growing: ConversionCache?
            for count in 1...tokens.count {
                steps.append(engine.convert(Array(tokens.prefix(count)), fixed: [], typo: .standard, cache: &growing, start: start)
                    .map(\.text).joined())
            }
            // Edit in the middle and type on: the cache keeps only the unchanged prefix.
            if tokens.count > 3 {
                var edited = tokens
                edited.remove(at: 1)
                steps.append(engine.convert(edited, fixed: [], typo: .standard, cache: &growing, start: start).map(\.text).joined())
                steps.append(engine.convert(tokens, fixed: [], typo: .standard, cache: &growing, start: start).map(\.text).joined())
            }
            incremental.append(head + steps.joined(separator: "|"))
            if best.count >= 2 {
                let word = best[1]
                let fixed = [FixedWord(range: word.range, text: word.text, reading: word.reading, wordID: word.wordID)]
                var fixedCache: ConversionCache?
                var line = head + wordsLine(engine.convert(tokens, fixed: fixed, typo: .standard, cache: &fixedCache, start: start))
                if tokens.count > 2 {
                    line += "\t" + wordsLine(engine.convert(tokens, fixed: fixed, typo: .standard, cache: &fixedCache,
                                                            group: 1..<(tokens.count - 1), start: start))
                }
                line += "\t" + sentencesLine(engine.sentences(tokens, fixed: fixed, typo: .standard, k: 8, start: start))
                fixedLines.append(line)
            }
            let characters = syntheticCharacters(sentences, count: tokens.count)
            var characterCache: ConversionCache?
            characterLines.append(head + wordsLine(engine.convert(tokens, fixed: [], typo: .standard, cache: &characterCache,
                                                                  start: start, characters: characters))
                + "\t" + sentencesLine(engine.sentences(tokens, fixed: [], typo: .standard, k: 8, start: start, characters: characters)))
        }
    }
    // Fuzzy sounds and the other correction strengths.
    var optionLines: [String] = []
    engine.adaptDomain(to: "")
    for (index, (_, run, _, tokens)) in devScripts(inputs, files: files, limit: 150).enumerated() where index % 5 == 2 {
        var line = run
        for (fuzzy, typo) in [(FuzzySounds([.zZh, .cCh, .sSh, .enEng, .anAng]), TypoTolerance.standard), ([], .off), ([], .aggressive),
                              ([.zZh, .enEng], .aggressive)] as [(FuzzySounds, TypoTolerance)] {
            engine.fuzzySounds = fuzzy
            var cache: ConversionCache?
            line += "\t" + wordsLine(engine.convert(tokens, fixed: [], typo: typo, cache: &cache)) + "\t"
                + sentencesLine(engine.sentences(tokens, fixed: [], typo: typo, k: 4))
        }
        engine.fuzzySounds = []
        optionLines.append(line)
    }
    // The TSV lexicon path without a language model (personal IDs, the reading index, reverse lookups).
    let text = try! Engine(lexiconText: String(decoding: inputs.tsv, as: UTF8.self))
    var textLines: [String] = []
    for (_, run, _, tokens) in devScripts(inputs, files: files, limit: 40) {
        var cache: ConversionCache?
        textLines.append("\(run)\t" + wordsLine(text.convert(tokens, fixed: [], typo: .standard, cache: &cache)) + "\t"
            + sentencesLine(text.sentences(tokens, fixed: [], typo: .standard, k: 8)) + "\t"
            + text.readings(of: String(run.prefix(2))).joined(separator: ","))
    }
    return [("conversion.options", optionLines), ("conversion.text", textLines), ("conversion.best", plain), ("conversion.nbest", nbest), ("conversion.incremental", incremental),
            ("conversion.fixed", fixedLines), ("conversion.characters", characterLines)]
}

/// A learning session on a fixed clock: commits with corrections, weakening, user phrases and typing slips, then the
/// personal data written, exported and imported into a fresh engine.
func learningSweeps(_ inputs: Inputs, files: [String]) -> [(String, [String])] {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("zhuyin-parity-\(getpid())")
    try? FileManager.default.removeItem(at: directory)
    Engine.chatModelDirectory = inputs.data.appendingPathComponent("lm_chat")
    let engine = try! Engine(lexiconURL: inputs.data.appendingPathComponent("lexicon.tsv"), userDataDirectory: directory,
                             languageModelDirectory: inputs.data.appendingPathComponent("lm"))
    Engine.chatModelDirectory = nil
    var step = 0.0
    engine.clock = { 1_800_000_000 + step * 5 * 3600 }
    var lines: [String] = []
    var context: [CommittedWord] = []
    let scripts = devScripts(inputs, files: files, limit: 60)
    for (index, (_, run, _, tokens)) in scripts.enumerated() {
        step = Double(index)
        var cache: ConversionCache?
        let before = engine.convert(tokens, fixed: [], typo: .standard, cache: &cache)
        let sentences = engine.sentences(tokens, fixed: [], typo: .standard, k: 2)
        // Every fourth run the user picks the second sentence: its words that differ are picks.
        let chosen: [(range: Range<Int>, text: String, reading: String, picked: Bool)]
        if index % 4 == 0, sentences.count > 1 {
            chosen = sentences[1].words.map { word in
                (word.range, word.text, word.reading, !before.contains { $0.range == word.range && $0.text == word.text })
            }
        } else {
            chosen = before.map { ($0.range, $0.text, $0.reading, false) }
        }
        let committed = chosen.map { word in
            CommittedWord(range: word.range, text: word.text, reading: word.reading,
                          wordID: before.first { $0.range == word.range && $0.text == word.text }?.wordID, picked: word.picked)
        }
        engine.learn(commit: committed, syllableCount: tokens.count, context: context)
        context = Array(committed.suffix(2)).map { CommittedWord(range: 0..<0, text: $0.text, reading: $0.reading, wordID: $0.wordID, picked: false) }
        if index % 7 == 3, let word = before.first {
            engine.weaken(text: word.text, reading: word.reading, wordID: word.wordID, history: LanguageModelHistory())
        }
        if index % 11 == 5, tokens.count >= 2 {
            let reading = tokens.prefix(2).compactMap(\.reading).joined(separator: " ")
            try? engine.addUserPhrase(word: String(run.prefix(2)), reading: reading)
        }
        if index % 13 == 6, case .syllable(let reading, let keys, _) = tokens[0], let slip = typoTokens([tokens[0]]),
           case .syllable(_, let typed, _) = slip[0] {
            _ = keys
            engine.recordTypos([(typed, reading)])
        }
        var after: ConversionCache?
        let converted = engine.convert(tokens, fixed: [], typo: .standard, cache: &after)
        var history = LanguageModelHistory()
        let remembered = converted.map { word -> String in
            defer { history = word.history ?? history }
            return engine.remembers(word, before: history) ? "1" : "0"
        }.joined()
        lines.append("\(run)\t\(wordsLine(converted))\t\(remembered)\t\(engine.learnedEntries().count)")
        if let typo = typoTokens(tokens) {
            lines.append("typo\t\(engine.typoAlternatives(for: typo[tokens.count / 2].keysText).map { "\($0.reading),\(hex($0.cost))" }.joined(separator: " "))")
        }
    }
    lines += engine.learnedEntries().map { "\($0.reading)\t\($0.word)\t\(hex($0.count))" }
    lines += engine.readings(of: "什麼") + engine.readings(of: "不存在字") + engine.userPhrases().map { "\($0.reading)\t\($0.word)" }
    engine.flushPersonalData()
    var files: [String] = []
    for name in ["user_phrases.tsv", "learned.tsv", "excluded.tsv", "typos.tsv", "personal_ngrams.tsv"] {
        files.append("== \(name)")
        files += (try? String(contentsOf: directory.appendingPathComponent(name), encoding: .utf8))?.split(separator: "\n", omittingEmptySubsequences: false).map(String.init) ?? ["(missing)"]
    }
    let export = String(decoding: engine.exportPersonalData(), as: UTF8.self)
    let fresh = appEngine(inputs.data)
    fresh.clock = { 1_800_000_000 + 100 * 5 * 3600 }
    let rows = (try? fresh.importPersonalData(engine.exportPersonalData())) ?? -1
    let reimport = String(decoding: fresh.exportPersonalData(), as: UTF8.self)
    // A reloaded engine reads the files back.
    let reloaded = try! Engine(lexiconURL: inputs.data.appendingPathComponent("lexicon.tsv"), userDataDirectory: directory,
                               languageModelDirectory: inputs.data.appendingPathComponent("lm"))
    reloaded.clock = { 1_800_000_000 + 100 * 5 * 3600 }
    let reloadedLines = reloaded.learnedEntries().map { "\($0.reading)\t\($0.word)\t\(hex($0.count))" }
    try? FileManager.default.removeItem(at: directory)
    return [("learning.session", lines), ("learning.files", files),
            ("learning.export", export.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)),
            ("learning.import", ["\(rows)"] + reimport.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)),
            ("learning.reload", reloadedLines)]
}

extension CompositionToken {
    var keysText: String { if case .syllable(_, let keys, _) = self { return keys }; return "" }
}
