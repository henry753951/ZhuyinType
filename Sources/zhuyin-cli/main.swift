import Foundation
import ZhuyinCore

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
var arguments = Array(CommandLine.arguments.dropFirst())
var lexiconPath = "data/build/lexicon.tsv"
var batch = false
var nbest = 0
var options = ComposerOptions()
let usage = "Usage: zhuyin-cli [--lexicon PATH] [--typo off|standard|aggressive] [--anchor after|before] [--nbest K] [--sentence-boundaries on|off] [--batch | \"<keys>\"]"
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
    case "--sentence-boundaries":
        guard arguments.count >= 2, ["on", "off"].contains(arguments[1]) else { fail(usage) }
        Engine.sentenceBoundaries = arguments[1] == "on"
        arguments.removeFirst(2)
    case "--anchor":
        guard arguments.count >= 2, ["after", "before"].contains(arguments[1]) else { fail(usage) }
        options.candidateAnchor = arguments[1] == "after" ? .afterCursor : .beforeCursor
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
    "sleft": .shiftLeft, "sright": .shiftRight
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
    if batch {
        let composer = Composer(engine: engine, options: options)
        while let line = readLine() {
            composer.reset()
            engine.forgetAllLearned()
            var committed = ""
            for (_, key) in try parse(line) { committed += composer.handle(key).commit ?? "" }
            print(committed)
        }
        exit(0)
    }
    let composer = Composer(engine: engine, options: options)
    for (label, key) in try parse(arguments[0]) {
        let result = composer.handle(key)
        let p = composer.presentation
        let string = p.text as NSString
        let focus = p.focusedClause.map { p.clauses[$0] }
        var display = ""
        let cuts = Set([0, p.cursor, string.length] + (focus.map { [$0.lowerBound, $0.upperBound] } ?? [])).sorted()
        for (index, offset) in cuts.enumerated() {
            if offset == focus?.upperBound { display += "]" }
            if offset == focus?.lowerBound { display += "[" }
            if offset == p.cursor { display += "|" }
            if index + 1 < cuts.count {
                display += string.substring(with: NSRange(location: offset, length: cuts[index + 1] - offset))
            }
        }
        print("\(label) → \(display)\(result.handled ? "" : " [pass]")")
        if let list = p.candidates {
            let items = list.pageRange.map { index in
                "\(index == list.selection ? ">" : "")\(index - list.page * list.pageSize + 1):\(list.items[index].text)"
            }.joined(separator: "  ")
            print("  candidates \(list.page + 1)/\(list.pageCount): \(items)")
        }
        if let hint = p.zhuyinHint { print("  ` \(hint)") }
        if let notice = p.notice { print("  \(notice)") }
        if let text = result.commit { print("  committed: \(text)") }
    }
    for (index, sentence) in composer.nBest(nbest).enumerated() {
        print(String(format: "  nbest %d: %.6f %@ [%@]", index + 1, sentence.score, sentence.text,
                     sentence.words.map { "\($0.text):\($0.range)" }.joined(separator: ", ")))
    }
} catch { fail(error.localizedDescription) }
