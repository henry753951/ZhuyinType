import CoreML
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
var rerankWeight = 0.5
var rerankCount = 8
var exactMargin: Double?
var nbest = 0
var context = ""
/// Text committed so far by the line being typed (batch mode) or by the keys (single run).
var committed = ""
var timing = false
var fuzzy: FuzzySounds = []
var options = ComposerOptions()
let usage = "Usage: zhuyin-cli [--lexicon PATH] [--typo off|standard|aggressive] [--anchor after|before] [--nbest K] [--sentence-boundaries on|off] [--fuzzy zh,ch,sh,en,an] [--reranker DIR [--rerank-weight W] [--exact-margin M] [--context TEXT] [--timing]] [--batch [--keep-memory] | \"<keys>\"]"
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
    case "--reranker":
        guard arguments.count >= 2 else { fail(usage) }
        rerankerPath = arguments[1]
        arguments.removeFirst(2)
    case "--rerank-count":
        guard arguments.count >= 2, let count = Int(arguments[1]), count > 0 else { fail(usage) }
        rerankCount = count
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
    let engine = try Engine(lexiconURL: URL(fileURLWithPath: lexiconPath), userDataDirectory: nil)
    engine.fuzzySounds = fuzzy
    // ZHUYIN_UNITS=all|gpu|ane compares Core ML compute units with --timing.
    let units: MLComputeUnits = ["all": .all, "gpu": .cpuAndGPU, "ane": .cpuAndNeuralEngine][ProcessInfo.processInfo.environment["ZHUYIN_UNITS"] ?? ""] ?? .cpuOnly
    let reranker = try rerankerPath.map { try NeuralReranker(directory: URL(fileURLWithPath: $0), weight: rerankWeight, computeUnits: units) }
    func attach(_ composer: Composer) {
        guard let reranker else { return }
        composer.rerankingCount = rerankCount
        if let exactMargin { reranker.exactMargin = exactMargin }
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
            for (_, key) in try parse(String(keys)) { committed += composer.handle(key).commit ?? "" }
            print(committed)
        }
        exit(0)
    }
    let composer = Composer(engine: engine, options: options)
    attach(composer)
    composer.setPrecedingText(context)
    for (label, key) in try parse(arguments[0]) {
        let result = composer.handle(key)
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
