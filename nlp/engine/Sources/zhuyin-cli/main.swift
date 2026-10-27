#if canImport(CoreML)
import CoreML
#endif
import Foundation
import ZhuyinCore
import ZhuyinML

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
var arguments = Array(CommandLine.arguments.dropFirst())
var lexiconPath = "data/build/lexicon.tsv"
var batch = false
var keepMemory = false
var rerankerPath: String?
var modelServerPath: String?
var rerankWeight = 0.5
var rerankCount = 8
var exactMargin: Double?
/// Batch: print the engine's K best sentences (tab-separated) for each line instead of what it commits.
var reportNBest = 0
var v3Path: String?
var v3Weight = 0.5
var v3Boundaries = true
var v3InList = true
/// Batch mode: print the open candidate list (tab-separated) instead of what was committed.
var reportList = false
var rerankGate = Double.infinity
var rerankSwitch = 0.0
var freezeOnPick = true
var nbest = 0
var context = ""
/// Text committed so far by the line being typed (batch mode) or by the keys (single run).
var committed = ""
var timing = false
var fuzzy: FuzzySounds = []
var options = ComposerOptions()
// The engine's own forms by default, so results compare with the evaluation text and earlier runs; the app writes 台.
options.characterForms = .standard
let usage = "Usage: zhuyin-cli [--model-server PATH] [--lexicon PATH] [--typo off|standard|aggressive] [--anchor after|before] [--nbest K] [--sentence-boundaries on|off] [--fuzzy zh,ch,sh,en,an] [--reranker DIR [--rerank-weight W] [--exact-margin M] [--context TEXT] [--timing]] [--batch [--keep-memory] [--report-nbest K] [--decoder slots|boundaries|reorder|timed] [--hotspot on|off] | \"<keys>\"]"
while let option = arguments.first, option.hasPrefix("--") {
    switch option {
    case "--lexicon":
        guard arguments.count >= 2 else { fail(usage) }
        lexiconPath = arguments[1]
        arguments.removeFirst(2)
    case "--typo":
        guard arguments.count >= 2, let typo = TypoTolerance(rawValue: arguments[1]) else { fail(usage) }
        options.typoTolerance = typo
        arguments.removeFirst(2)
    case "--batch": batch = true; arguments.removeFirst()
    case "--keep-memory": keepMemory = true; arguments.removeFirst()
    case "--model-server":
        guard arguments.count >= 2 else { fail(usage) }
        modelServerPath = arguments[1]
        arguments.removeFirst(2)
    case "--reranker":
        guard arguments.count >= 2 else { fail(usage) }
        rerankerPath = arguments[1]
        arguments.removeFirst(2)
    case "--rerank-count":
        guard arguments.count >= 2, let count = Int(arguments[1]), count > 0 else { fail(usage) }
        rerankCount = count
        arguments.removeFirst(2)
    case "--forms":
        guard arguments.count >= 2, let forms = CharacterForms(rawValue: arguments[1]) else {
            fail("--forms needs one of: " + CharacterForms.allCases.map(\.rawValue).joined(separator: ", "))
        }
        options.characterForms = forms
        arguments.removeFirst(2)
    case "--decoder":
        guard arguments.count >= 2, let method = KeyDecoderMethod(rawValue: arguments[1]) else {
            fail("--decoder needs one of: " + KeyDecoderMethod.allCases.map(\.rawValue).joined(separator: ", "))
        }
        options.keyDecoder = method
        arguments.removeFirst(2)
    case "--decoder-costs":
        // implicit,overwrite,swap,skip,swapTogether,skipTogether,swapApart,skipApart
        let values = arguments.count >= 2 ? arguments[1].split(separator: ",").compactMap { Double($0) } : []
        guard values.count == 8 else { fail("--decoder-costs needs 8 comma-separated numbers") }
        KeyDecoder.costs.implicitBoundary = values[0]; KeyDecoder.costs.overwrite = values[1]
        KeyDecoder.costs.swap = values[2]; KeyDecoder.costs.skip = values[3]
        KeyDecoder.costs.swapTogether = values[4]; KeyDecoder.costs.skipTogether = values[5]
        KeyDecoder.costs.swapApart = values[6]; KeyDecoder.costs.skipApart = values[7]
        arguments.removeFirst(2)
    case "--check-v3":
        // Compares the v3 Core ML models with training/v3's golden outputs, then exits.
        guard arguments.count >= 2 else { fail(usage) }
        let directory = URL(fileURLWithPath: arguments[1])
        do {
            let scorer: SyllableScorer
            if let modelServerPath {
                let server = try ModelServer(path: modelServerPath)
                scorer = try SyllableScorer(directory: directory, prediction: server.syllable)
            } else {
                scorer = try SyllableScorer(directory: directory)
            }
            let file = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("test_vectors.json"))) as! [String: Any]
            var passed = 0, worst = 0.0
            let vectors = file["vectors"] as! [[String: Any]]
            for vector in vectors {
                let inputs = vector["inputs"] as! [String: Any]
                func row(_ key: String) -> [Int32] { ((inputs[key] as! [[Int]])[0]).map(Int32.init) }
                let candidates = ((inputs["candidate_ids"] as! [[[Int]]])[0]).map { $0.map(Int32.init) }
                let got = try scorer.rawScores(contextIDs: row("context_ids"), syllableIDs: row("syllable_ids"), tones: row("tone_ids"),
                                               costs: row("cost_bucket"), boundaries: row("boundary"), candidates: candidates)
                let expected = ((vector["expected"] as! [String: Any])["logprob"] as! [[[Double]]])[0]
                var ok = true
                for (r, list) in expected.enumerated() {
                    for (c, value) in list.enumerated() where value > -9000 {
                        let difference = abs(got[r][c] - value)
                        worst = max(worst, difference)
                        if difference > 0.002 + 0.002 * abs(value) { ok = false }
                    }
                }
                if ok { passed += 1 }
            }
            print("v3 test vectors: \(passed)/\(vectors.count) within tolerance; largest difference \(worst)")
            exit(passed == vectors.count ? 0 : 1)
        } catch { fail(error.localizedDescription) }
    case "--v3":
        guard arguments.count >= 2 else { fail(usage) }
        v3Path = arguments[1]
        arguments.removeFirst(2)
    case "--rerank-gate", "--rerank-switch", "--freeze-on-pick":
        // τ and δ of NeuralReranker; freezing the other words after a pick (on/off).
        guard arguments.count >= 2 else { fail("\(arguments[0]) needs a value") }
        if arguments[0] == "--freeze-on-pick" { freezeOnPick = arguments[1] != "off" }
        else if let value = Double(arguments[1]) {
            if arguments[0] == "--rerank-gate" { rerankGate = value } else { rerankSwitch = value }
        } else { fail("\(arguments[0]) needs a number") }
        arguments.removeFirst(2)
    case "--report-list":
        reportList = true
        arguments.removeFirst()
    case "--v3-list":
        guard arguments.count >= 2, ["on", "off"].contains(arguments[1]) else { fail("--v3-list needs on or off") }
        v3InList = arguments[1] == "on"
        arguments.removeFirst(2)
    case "--v3-boundaries":
        // Whether the character model is told which syllables ended without a tone key or Space.
        guard arguments.count >= 2, ["on", "off"].contains(arguments[1]) else { fail("--v3-boundaries needs on or off") }
        v3Boundaries = arguments[1] == "on"
        arguments.removeFirst(2)
    case "--v3-weight":
        guard arguments.count >= 2, let value = Double(arguments[1]) else { fail(usage) }
        v3Weight = value
        arguments.removeFirst(2)
    case "--chat-lm":
        guard arguments.count >= 2 else { fail(usage) }
        Engine.chatModelDirectory = URL(fileURLWithPath: arguments[1])
        arguments.removeFirst(2)
    case "--end-share":
        guard arguments.count >= 2, let value = Double(arguments[1]), value > 0, value <= 1 else { fail(usage) }
        Engine.sentenceEndShare = value
        arguments.removeFirst(2)
    case "--chat-prior":
        guard arguments.count >= 2, let value = Double(arguments[1]), value > 0, value < 1 else { fail(usage) }
        Engine.chatPrior = value
        arguments.removeFirst(2)
    case "--domain-scale":
        guard arguments.count >= 2, let value = Double(arguments[1]) else { fail(usage) }
        Engine.domainEvidenceScale = value
        arguments.removeFirst(2)
    case "--domain-centre":
        guard arguments.count >= 2, let value = Double(arguments[1]) else { fail(usage) }
        Engine.domainEvidenceCentre = value
        arguments.removeFirst(2)
    case "--hotspot":
        guard arguments.count >= 2, ["on", "off"].contains(arguments[1]) else { fail(usage) }
        KeyChannel.hotspot = arguments[1] == "on"
        arguments.removeFirst(2)
    case "--report-nbest":
        guard arguments.count >= 2, let count = Int(arguments[1]), count > 0 else { fail(usage) }
        reportNBest = count
        arguments.removeFirst(2)
    case "--exact-margin":
        guard arguments.count >= 2, let margin = Double(arguments[1]) else { fail(usage) }
        exactMargin = margin
        arguments.removeFirst(2)
    case "--rerank-weight":
        guard arguments.count >= 2, let weight = Double(arguments[1]) else { fail(usage) }
        rerankWeight = weight
        arguments.removeFirst(2)
    case "--sentence-boundaries":
        guard arguments.count >= 2, ["on", "off"].contains(arguments[1]) else { fail(usage) }
        Engine.sentenceBoundaries = arguments[1] == "on"
        arguments.removeFirst(2)
    case "--anchor":
        guard arguments.count >= 2, ["after", "before"].contains(arguments[1]) else { fail(usage) }
        options.candidateAnchor = arguments[1] == "after" ? .afterCursor : .beforeCursor
        arguments.removeFirst(2)
    case "--context":
        guard arguments.count >= 2 else { fail(usage) }
        context = arguments[1]
        arguments.removeFirst(2)
    case "--timing": timing = true; arguments.removeFirst()
    case "--fuzzy":
        let names: [String: FuzzySounds] = ["zh": .zZh, "ch": .cCh, "sh": .sSh, "en": .enEng, "an": .anAng]
        guard arguments.count >= 2 else { fail(usage) }
        for name in arguments[1].split(separator: ",") {
            guard let set = names[String(name)] else { fail(usage) }
            fuzzy.insert(set)
        }
        arguments.removeFirst(2)
    case "--nbest":
        guard arguments.count >= 2, let count = Int(arguments[1]), count > 0 else { fail(usage) }
        nbest = count
        arguments.removeFirst(2)
    default: fail(usage)
    }
}
guard arguments.count == (batch ? 0 : 1) else { fail(usage) }
let specials: [String: KeyInput] = [
    "space": .space, "bs": .backspace, "obs": .optionBackspace, "del": .forwardDelete,
    "left": .left, "right": .right, "home": .home, "end": .end, "up": .up, "down": .down,
    "enter": .enter, "senter": .shiftEnter, "esc": .escape, "tab": .tab,
    "stab": .shiftTab, "sspace": .shiftSpace, "pgup": .pageUp, "pgdown": .pageDown,
    "sleft": .shiftLeft, "sright": .shiftRight, "sdown": .shiftDown, "sup": .shiftUp
]
/// Types parsed keys into a composer with key-down times: 0.12 s apart unless a `<dt:MS>` sets the gap before the next
/// key.  Returns what was committed.
var clock: TimeInterval = 1000
/// With --timing, how long each key took in the composer (all models included), in milliseconds.
var keyTimes: [Double] = []

func typed(_ keys: [(String, KeyInput)], into composer: Composer, each: ((String, KeyResult) -> Void)? = nil) -> String {
    var committed = ""
    var gap = 0.12
    for (label, key) in keys {
        if label.hasPrefix("<dt:") { gap = (Double(label.dropFirst(4).dropLast()) ?? 120) / 1000; continue }
        clock += gap
        gap = 0.12
        let start = DispatchTime.now().uptimeNanoseconds
        let result = composer.handle(key, at: clock)
        if timing { keyTimes.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6) }
        committed += result.commit ?? ""
        each?(label, result)
    }
    return committed
}

func parse(_ input: String) throws -> [(String, KeyInput)] {
    var result: [(String, KeyInput)] = []
    var index = input.startIndex
    while index < input.endIndex {
        let character = input[index]
        if character == "<", let end = input[index...].firstIndex(of: ">") {
            let name = String(input[input.index(after: index)..<end])
            if let key = specials[name] { result.append(("<\(name)>", key)) }
            else if name.hasPrefix("S-"), name.dropFirst(2).count == 1, let key = name.last {
                result.append(("<\(name)>", .shiftedChar(key)))
            } else if name.hasPrefix("O-"), name.dropFirst(2).count == 1, let key = name.last {
                result.append(("<\(name)>", .printed(key)))
            } else if name.hasPrefix("dt:"), let ms = Double(name.dropFirst(3)) {
                result.append(("<dt:\(ms)>", .space))  // a delay, not a key: see `typed(_:into:)`
            } else {
                throw NSError(domain: "zhuyin-cli", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unknown key escape <\(name)>"])
            }
            index = input.index(after: end)
        } else {
            result.append((character == " " ? "<space>" : String(character), character == " " ? .space : .char(character)))
            index = input.index(after: index)
        }
    }
    return result
}
assert((try? parse("a<space><bs><obs><del><left><right><home><end><up><down><enter><senter><esc><tab><S-x><sleft><sright>"))?.map(\.1)
       == [.char("a"), .space, .backspace, .optionBackspace, .forwardDelete, .left, .right,
           .home, .end, .up, .down, .enter, .shiftEnter, .escape, .tab, .shiftedChar("x"), .shiftLeft, .shiftRight])
assert((try? parse("<unknown>")) == nil)
do {
    let engineStart = DispatchTime.now().uptimeNanoseconds
    let engine = try Engine(lexiconURL: URL(fileURLWithPath: lexiconPath), userDataDirectory: nil)
    if timing {
        FileHandle.standardError.write(Data(String(format: "engine load: %.3f ms\n",
            Double(DispatchTime.now().uptimeNanoseconds - engineStart) / 1e6).utf8))
    }
    engine.fuzzySounds = fuzzy
    let reranker: NeuralReranker?
    let characterModel: SyllableScorer?
    if let modelServerPath {
        let server = try ModelServer(path: modelServerPath)
        reranker = try rerankerPath.map { try NeuralReranker(directory: URL(fileURLWithPath: $0), weight: rerankWeight, prediction: server.rerank) }
        characterModel = try v3Path.map { try SyllableScorer(directory: URL(fileURLWithPath: $0), prediction: server.syllable) }
    } else {
        #if canImport(CoreML)
        // ZHUYIN_UNITS=all|gpu|ane compares Core ML compute units with --timing.
        let units: MLComputeUnits = ["all": .all, "gpu": .cpuAndGPU, "ane": .cpuAndNeuralEngine][ProcessInfo.processInfo.environment["ZHUYIN_UNITS"] ?? ""] ?? .cpuOnly
        reranker = try rerankerPath.map { try NeuralReranker(directory: URL(fileURLWithPath: $0), weight: rerankWeight, computeUnits: units) }
        characterModel = try v3Path.map { try SyllableScorer(directory: URL(fileURLWithPath: $0)) }
        #else
        reranker = try rerankerPath.map { try NeuralReranker(directory: URL(fileURLWithPath: $0), weight: rerankWeight) }
        characterModel = try v3Path.map { try SyllableScorer(directory: URL(fileURLWithPath: $0)) }
        #endif
    }
    func attach(_ composer: Composer) {
        composer.freezesOnPick = freezeOnPick
        if let characterModel {
            composer.characterWeight = v3Weight
            composer.characterScoresInList = v3InList
            composer.characterScorer = { syllables, candidates in
                characterModel.scores(context: context + committed, syllables: syllables.map {
                    SyllableScorer.Syllable(base: $0.base, tone: $0.tone, costBucket: $0.costBucket, implicit: v3Boundaries && $0.implicit)
                }, candidates: candidates)
            }
        }
        guard let reranker else { return }
        composer.rerankingCount = rerankCount
        if let exactMargin { reranker.exactMargin = exactMargin }
        reranker.gateMargin = rerankGate
        reranker.switchMargin = rerankSwitch
        func timed<T>(_ label: String, _ body: () -> T) -> T {
            let start = DispatchTime.now().uptimeNanoseconds
            defer {
                if timing { print(String(format: "  [%@ %.1f ms]", label, Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)) }
            }
            return body()
        }
        composer.reranker = { sentences in timed("rerank \(sentences.count)") { reranker.choose(sentences, context: context + committed) } }
        composer.candidateScorer = { before, candidates in
            timed("candidates \(candidates.count)") {
                reranker.candidateScores(context: context + committed + before, candidates: candidates)
            }
        }
    }
    if batch {
        let composer = Composer(engine: engine, options: options)
        attach(composer)
        // One line is one stretch of typing: what it commits is the context of the rest of the line.  A line
        // "TEXT<tab>KEYS" types KEYS after TEXT, as if TEXT were in the document before the insertion point.
        let baseContext = context
        let usageStart = Usage.now(), wallStart = DispatchTime.now().uptimeNanoseconds
        while let line = readLine() {
            composer.reset()
            composer.breakContext()
            if !keepMemory { engine.forgetAllLearned() }
            committed = ""
            var keys = Substring(line)
            context = baseContext
            if let tab = line.firstIndex(of: "\t") {
                context = String(line[..<tab])
                keys = line[line.index(after: tab)...]
            }
            composer.setPrecedingText(context)
            committed = typed(try parse(String(keys)), into: composer)
            if reportList {
                print(composer.presentation.candidates?.items.map(\.text).joined(separator: "\t") ?? "")
                composer.reset()
            } else if reportNBest > 0 {
                print(composer.nBest(reportNBest).map(\.text).joined(separator: "\t"))
                composer.reset()
            } else {
                print(committed)
            }
        }
        // On standard error, so batch output stays one line per input.
        if timing {
            FileHandle.standardError.write(Data(usageSummary(keyTimes: keyTimes, from: usageStart, to: Usage.now(),
                wall: Double(DispatchTime.now().uptimeNanoseconds - wallStart) / 1e9).utf8))
        }
        exit(0)
    }
    let composer = Composer(engine: engine, options: options)
    attach(composer)
    composer.setPrecedingText(context)
    let usageStart = Usage.now(), wallStart = DispatchTime.now().uptimeNanoseconds
    defer {
        if timing {
            print(usageSummary(keyTimes: keyTimes, from: usageStart, to: Usage.now(),
                               wall: Double(DispatchTime.now().uptimeNanoseconds - wallStart) / 1e9), terminator: "")
        }
    }
    _ = typed(try parse(arguments[0]), into: composer) { label, result in
        let p = composer.presentation
        let string = p.text as NSString
        let focus = p.focusedClause.map { p.clauses[$0] }
        let target = p.targetClause.map { p.clauses[$0] }
        var display = ""
        let cuts = Set([0, p.cursor, string.length] + (focus.map { [$0.lowerBound, $0.upperBound] } ?? [])
                       + (target.map { [$0.lowerBound, $0.upperBound] } ?? [])).sorted()
        for (index, offset) in cuts.enumerated() {
            if offset == focus?.upperBound { display += "]" }
            if offset == target?.upperBound { display += "⟩" }
            if offset == focus?.lowerBound { display += "[" }
            if offset == p.cursor { display += "|" }
            if offset == target?.lowerBound { display += "⟨" }
            if index + 1 < cuts.count {
                display += string.substring(with: NSRange(location: offset, length: cuts[index + 1] - offset))
            }
        }
        print("\(label) → \(display)\(result.handled ? "" : " [pass]")")
        if let list = p.candidates {
            let items = list.pageRange.map { index in
                "\(index == list.selection ? ">" : "")\(index - list.page * list.pageSize + 1):\(list.items[index].text)"
                    + (list.items[index].annotation == nil ? "" : "*")
            }.joined(separator: "  ")
            print("  candidates \(list.page + 1)/\(list.pageCount): \(items)")
        }
        if let hint = p.zhuyinHint { print("  ` \(hint)") }
        if let notice = p.notice { print("  \(notice)") }
        if let text = result.commit { committed += text; print("  committed: \(text)") }
    }
    let sentences = composer.nBest(nbest)
    let neural = reranker?.scores(context: context, candidates: sentences.map(\.text))
    for (index, sentence) in sentences.enumerated() {
        let rerank = neural.map { String(format: " neural %.3f", $0[index]) } ?? ""
        print(String(format: "  nbest %d: %.6f%@ %@%@ [%@]", index + 1, sentence.score, rerank, sentence.text, sentence.corrected ? "*" : "",
                     sentence.words.map { "\($0.text):\($0.range)" }.joined(separator: ", ")))
    }
} catch { fail(error.localizedDescription) }
