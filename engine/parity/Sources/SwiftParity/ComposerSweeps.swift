// Phase 3 sweep: the composer key by key.  Every key's result and the whole Presentation after it, for dev-set key
// scripts with picks, corrections, deletions, punctuation and selections, and for targeted scripts.  The twin is
// crates/zhuyin-core/tests/golden_composer.rs.
import Foundation

/// zhuyin-cli's key syntax: characters, `<name>` escapes, `<S-x>`, `<O-x>`, `<C-x>`, `<dt:MS>` (a delay).
func parseKeys(_ input: String) -> [(String, KeyInput?, Double?)] {
    let specials: [String: KeyInput] = [
        "space": .space, "bs": .backspace, "obs": .optionBackspace, "del": .forwardDelete,
        "left": .left, "right": .right, "home": .home, "end": .end, "up": .up, "down": .down,
        "enter": .enter, "senter": .shiftEnter, "esc": .escape, "tab": .tab,
        "stab": .shiftTab, "sspace": .shiftSpace, "pgup": .pageUp, "pgdown": .pageDown,
        "sleft": .shiftLeft, "sright": .shiftRight, "sdown": .shiftDown, "sup": .shiftUp,
    ]
    var result: [(String, KeyInput?, Double?)] = []
    var index = input.startIndex
    while index < input.endIndex {
        let character = input[index]
        if character == "<", let end = input[index...].firstIndex(of: ">") {
            let name = String(input[input.index(after: index)..<end])
            if let key = specials[name] { result.append(("<\(name)>", key, nil)) }
            else if name.hasPrefix("S-"), name.dropFirst(2).count == 1 { result.append(("<\(name)>", .shiftedChar(name.last!), nil)) }
            else if name.hasPrefix("O-"), name.dropFirst(2).count == 1 { result.append(("<\(name)>", .printed(name.last!), nil)) }
            else if name.hasPrefix("C-"), name.dropFirst(2).count == 1 { result.append(("<\(name)>", .controlChar(name.last!), nil)) }
            else if name.hasPrefix("dt:"), let ms = Double(name.dropFirst(3)) { result.append(("dt", nil, ms / 1000)) }
            else { fail("Unknown key escape <\(name)>") }
            index = input.index(after: end)
        } else {
            result.append((character == " " ? "<space>" : String(character), character == " " ? .space : .char(character), nil))
            index = input.index(after: index)
        }
    }
    return result
}

func presentationLine(_ p: Presentation) -> String {
    var parts = [p.text, "\(p.cursor)", p.clauses.map { "\($0.lowerBound)-\($0.upperBound)" }.joined(separator: ","),
                 p.focusedClause.map(String.init) ?? "-", p.targetClause.map(String.init) ?? "-"]
    if let list = p.candidates {
        let tolerant = list.tolerant.map { $0 ? "1" : "0" } ?? "-"
        parts.append("\(list.selection)/\(list.pageSize)/\(tolerant)/\(list.singleCharacter ? 1 : 0)/"
            + list.items.map { $0.text + ($0.annotation.map { "*" + $0 } ?? "") }.joined(separator: ","))
    } else { parts.append("-") }
    parts.append(p.notice ?? "-")
    parts.append(p.zhuyinHint ?? "-")
    return parts.joined(separator: "|")
}

/// Key scripts: each dev-set run typed and finished in one of several ways, then targeted scripts.
func composerScripts(_ inputs: Inputs, files: [String]) -> [(context: String, keys: String)] {
    let endings = ["<enter>", "<home><down><down><enter><enter>", "<down><sup><down><enter><enter>", "<enter><bs><bs>{keys}<enter>",
                   "<S-,><C-.>18 <enter>", "<sleft><sleft><enter><enter>", "<left><down><right><down><enter><esc><esc>",
                   "<home><sdown><down><enter><end><bs><obs><enter>", "<down><space><tab><stab><pgdown><pgup><up>`<enter>"]
    var scripts: [(String, String)] = []
    for (index, (file, _, before, tokens)) in devScripts(inputs, files: files, limit: 60).enumerated() {
        let keys = tokens.map(\.keysText).joined()
        let context = file == 1 ? before : ""
        scripts.append((context, keys + endings[index % endings.count].replacingOccurrences(of: "{keys}", with: keys)))
        if index % 3 == 0, let typo = typoTokens(tokens) {
            scripts.append((context, typo.map(\.keysText).joined() + "<down><sup><enter>"))
        }
        // Fast typing: keys pressed almost together, out of order.
        if index % 4 == 1, keys.count > 3 {
            var swapped = Array(keys)
            swapped.swapAt(1, 2)
            scripts.append((context, String(swapped.prefix(2)) + "<dt:8>" + String(swapped.dropFirst(2)) + "<enter>"))
        }
    }
    let targeted = [
        "18 q8 a8 <left><left><bs>18 <home><del><end><bs><S-,>18 ek4`<bs><enter>",
        "su3cl38 <sleft><sleft><enter><end>18 <enter>", "su3cl38 <left><sleft><sright><sright><esc><enter>",
        "rup wu0 wu0 fu4cp3cl3<home><down><down><down><enter><right><down><sdown><down><enter><enter>",
        "ji3ap7g4<enter><bs>g4<enter><bs><bs><bs>ji3ap7<enter>", "jia3p7g4<enter>", "suc3<dt:6>l3<enter>",
        "1q8iujm<bs><bs><obs>18 <down>`<enter>", "18 q8<home><end><del>a8<sspace><O-.><C-?><C-[>",
        "<S-a><S-1>18 <S-->q8 <down><down><enter><enter>", "vu,4vu,7<down><sup><sup><sdown><sdown><enter><enter>",
        "rupwu0 <left><down><left><left><right><enter><enter>", "18 <tab><senter>", "<C-,>18 <C-[>",
        "ek4wk3<down><sup><down><down><enter><enter>", "hl4su3a8 <down><down><enter><home><down><down><enter><enter>",
        String(repeating: "wu0 ", count: 45) + "<enter>", String(repeating: "vu,4", count: 30) + "<down><enter>",
    ]
    return scripts + targeted.map { ("", $0) }
}

func composerSweeps(_ inputs: Inputs, files: [String]) -> [(String, [String])] {
    var result: [(String, [String])] = []
    for (name, options) in composerConfigurations() {
        let engine = appEngine(inputs.data)
        var step = 0.0
        engine.clock = { 1_800_000_000 + step }
        let composer = Composer(engine: engine, options: options)
        var lines: [String] = []
        var time = 1000.0
        for (context, keys) in composerScripts(inputs, files: files) {
            composer.reset()
            composer.breakContext()
            composer.setPrecedingText(context)
            lines.append("== \(context)\t\(keys)")
            var gap = 0.12
            for (label, key, delay) in parseKeys(keys) {
                if let delay { gap = delay; continue }
                time += gap; gap = 0.12; step += 1
                let result = composer.handle(key!, at: time)
                lines.append("\(label)\t\(result.handled ? 1 : 0)\t\(result.commit ?? "-")\t" + presentationLine(composer.presentation))
            }
            lines.append("nbest\t" + composer.nBest(4).map { "\($0.text),\(hex($0.score))" }.joined(separator: " "))
        }
        lines.append("learned\t" + engine.learnedEntries().map { "\($0.reading)/\($0.word)/\(hex($0.count))" }.joined(separator: " "))
        result.append(("composer.\(name)", lines))
    }
    return result
}

func composerConfigurations() -> [(String, ComposerOptions)] {
    var standard = ComposerOptions()
    standard.characterForms = .standard
    var legacy = ComposerOptions(); legacy.typoTolerance = .off; legacy.punctuationKeys = .mac
    var other = ComposerOptions()
    other.candidateAnchor = .beforeCursor; other.verticalList = false; other.selectionKeys = Array("asdfghjkl")
    other.spaceAction = .commitAndSpace; other.escapeClearsComposition = false; other.keyDecoder = .reorder
    other.typoTolerance = .aggressive
    var commit = ComposerOptions(); commit.spaceAction = .commit; commit.keyDecoder = .slots; commit.maxSyllables = 12
    return [("app", ComposerOptions()), ("standard-forms", standard), ("legacy", legacy), ("other", other), ("commit", commit)]
}
