import Foundation

public final class Composer {
    public var options: ComposerOptions
    private let engine: Engine
    private var tokens: [CompositionToken] = []
    private var cursor = 0
    private var buffer: [Character?] = [nil, nil, nil]
    private var fixed: [FixedWord] = []
    private var conversion: [ConvertedWord] = []
    private var conversionDirty = true
    private var conversionRevision: UInt64 = 0
    private var choices: [FixedWord] = []
    private var candidateSelection = 0

    public init(engine: Engine, options: ComposerOptions = ComposerOptions()) {
        self.engine = engine
        self.options = options
    }

    public private(set) var presentation: Presentation = .empty
    public var isComposing: Bool { !presentation.isEmpty }
    private var bufferText: String { String(buffer.compactMap { $0 }) }
    private var hasBuffer: Bool { buffer.contains { $0 != nil } }
    private var hasComposition: Bool { !tokens.isEmpty || hasBuffer }

    public func handle(_ key: KeyInput) -> KeyResult {
        if !choices.isEmpty, let result = handleCandidateKey(key) { return result }
        let result: KeyResult
        if let literal = ZhuyinKeyboard.literal(for: key) {
            finishBuffer(droppingInvalid: true)
            insert(.literal(literal))
            result = .consumed
        } else {
            switch key {
            case .char(let character):
                if let symbol = ZhuyinKeyboard.symbols[character], let slot = ZhuyinKeyboard.slot(of: symbol) {
                    let syllableCount = tokens.reduce(0) { $0 + ($1.reading == nil ? 0 : 1) }
                    if syllableCount < options.maxSyllables { buffer[slot] = symbol }
                    result = .consumed
                } else if let tone = ZhuyinKeyboard.tones[character] {
                    finishBuffer(tone: tone)
                    result = .consumed
                } else { result = .passThrough }
            case .space:
                if hasBuffer { finishBuffer(); result = .consumed }
                else if hasComposition {
                    if options.spaceOpensCandidates { openCandidates(); result = .consumed }
                    else { return KeyResult(handled: true, commit: commitAll()) }
                } else { result = .passThrough }
            case .shiftSpace:
                if hasComposition {
                    finishBuffer(droppingInvalid: true)
                    insert(.literal(" "))
                    result = .consumed
                } else { result = .passThrough }
            case .enter, .shiftEnter:
                guard hasComposition else { return .passThrough }
                return KeyResult(handled: key == .enter, commit: commitAll())
            case .backspace:
                if hasBuffer {
                    if let last = buffer.lastIndex(where: { $0 != nil }) { buffer[last] = nil }
                    result = .consumed
                } else if cursor > 0 { delete((cursor - 1)..<cursor); result = .consumed }
                else { result = hasComposition ? .consumed : .passThrough }
            case .optionBackspace:
                if hasBuffer { clearBuffer(); result = .consumed }
                else if cursor > 0 {
                    let range = conversion.first(where: { $0.range.contains(cursor - 1) })?.range ?? ((cursor - 1)..<cursor)
                    delete(range)
                    result = .consumed
                } else { result = hasComposition ? .consumed : .passThrough }
            case .forwardDelete:
                result = hasComposition ? .consumed : .passThrough
                if !hasBuffer && cursor < tokens.count { delete(cursor..<(cursor + 1)) }
            case .left, .right:
                result = hasComposition ? .consumed : .passThrough
                if hasComposition && !hasBuffer { cursor = min(tokens.count, max(0, cursor + (key == .left ? -1 : 1))) }
            case .home, .end:
                result = hasComposition ? .consumed : .passThrough
                if hasComposition { cursor = key == .home ? 0 : tokens.count }
            case .down:
                result = hasComposition ? .consumed : .passThrough
                if hasComposition { finishBuffer(); refresh(); openCandidates() }
            case .up, .pageUp, .pageDown, .tab, .shiftTab:
                result = hasComposition ? .consumed : .passThrough
            case .escape:
                if hasBuffer { clearBuffer(); result = .consumed }
                else if hasComposition {
                    if options.escapeClearsComposition { reset() }
                    result = .consumed
                } else { result = .passThrough }
            case .shiftedChar: result = .passThrough
            }
        }
        refresh()
        return result
    }

    public func selectCandidate(at index: Int) -> KeyResult {
        guard choices.indices.contains(index) else { return .consumed }
        let choice = choices[index]
        fixed.removeAll { $0.range.overlaps(choice.range) }
        if choice.reading.isEmpty {
            tokens[choice.range.lowerBound] = .literal(choice.text)
        } else {
            fixed.append(choice)
            if options.learningEnabled { engine.learn(reading: choice.reading, word: choice.text, amount: 1) }
        }
        conversionDirty = true
        closeCandidates()
        refresh()
        return .consumed
    }

    public func commitAll() -> String {
        finishBuffer(droppingInvalid: true)
        refresh()
        let text = presentation.text
        if options.learningEnabled {
            for word in fixed { engine.learn(reading: word.reading, word: word.text, amount: 0.5) }
        }
        reset()
        return text
    }

    public func reset() {
        tokens.removeAll()
        cursor = 0
        clearBuffer()
        fixed.removeAll()
        conversion.removeAll()
        conversionDirty = true
        closeCandidates()
        presentation = .empty
    }

    private func clearBuffer() { buffer = [nil, nil, nil] }

    @discardableResult
    private func finishBuffer(tone: String = "", droppingInvalid: Bool = false) -> Bool {
        guard hasBuffer else { return false }
        let reading = bufferText + tone
        if engine.isValidSyllable(reading) {
            insert(.syllable(reading))
            clearBuffer()
            return true
        }
        if droppingInvalid { clearBuffer() }
        return false
    }

    private func insert(_ token: CompositionToken) {
        conversionDirty = true
        fixed = fixed.compactMap { word in
            if word.range.lowerBound < cursor && cursor < word.range.upperBound { return nil }
            var word = word
            if word.range.lowerBound >= cursor { word.range = (word.range.lowerBound + 1)..<(word.range.upperBound + 1) }
            return word
        }
        tokens.insert(token, at: cursor)
        cursor += 1
    }

    private func delete(_ range: Range<Int>) {
        conversionDirty = true
        fixed = fixed.compactMap { word in
            if word.range.overlaps(range) { return nil }
            var word = word
            if word.range.lowerBound >= range.upperBound {
                word.range = (word.range.lowerBound - range.count)..<(word.range.upperBound - range.count)
            }
            return word
        }
        tokens.removeSubrange(range)
        if cursor >= range.upperBound { cursor -= range.count }
        else if cursor > range.lowerBound { cursor = range.lowerBound }
    }

    private func closeCandidates() { choices.removeAll(); candidateSelection = 0 }

    private func handleCandidateKey(_ key: KeyInput) -> KeyResult? {
        let pageSize = max(1, options.selectionKeys.count)
        let page = candidateSelection / pageSize
        let pageCount = (choices.count + pageSize - 1) / pageSize
        if case .char(let character) = key, let number = options.selectionKeys.firstIndex(of: character) {
            return selectCandidate(at: page * pageSize + number)
        }
        switch key {
        case .enter: return selectCandidate(at: candidateSelection)
        case .escape, .backspace: closeCandidates()
        case .up: candidateSelection = max(0, candidateSelection - 1)
        case .down: candidateSelection = min(choices.count - 1, candidateSelection + 1)
        case .left, .pageUp, .shiftTab: candidateSelection = max(0, page - 1) * pageSize
        case .right, .pageDown, .tab: candidateSelection = min(pageCount - 1, page + 1) * pageSize
        case .space: candidateSelection = ((page + 1) % pageCount) * pageSize
        default:
            closeCandidates()
            return nil
        }
        refresh()
        return .consumed
    }

    private func openCandidates() {
        guard !tokens.isEmpty else { return }
        var ranked: [(FixedWord, Double)] = []
        let target = cursor > 0 ? cursor - 1 : 0
        if case .literal(let text) = tokens[target] {
            choices = ZhuyinKeyboard.alternatives(for: text).map {
                FixedWord(range: target..<(target + 1), text: $0, reading: "")
            }
            candidateSelection = 0
            return
        }
        var currentWord = conversion.first { cursor > 0 ? $0.range.upperBound == cursor : $0.range.lowerBound == 0 }
        for length in 1...min(6, cursor > 0 ? cursor : tokens.count) {
            let start = cursor > 0 ? cursor - length : 0
            let end = start + length
            let parts = tokens[start..<end].compactMap(\.reading)
            guard parts.count == length else { continue }
            let reading = parts.joined(separator: " ")
            var history = LanguageModelHistory()
            for word in conversion where word.range.upperBound <= start {
                history = word.reading.isEmpty ? .init()
                    : history.appending(word.wordID, order: engine.languageModel.order)
            }
            for entry in engine.entries(for: reading) {
                ranked.append((FixedWord(range: start..<end, text: entry.word, reading: reading, wordID: entry.wordID),
                               engine.score(entry, reading: reading, history: history)))
            }
        }
        if currentWord == nil {
            let matching = ranked.filter { choice, _ in
                let shown = conversion.filter { $0.range.overlaps(choice.range) }.map { word in
                    String(word.text.dropFirst(max(0, choice.range.lowerBound - word.range.lowerBound))
                        .prefix(min(word.range.upperBound, choice.range.upperBound)
                                - max(word.range.lowerBound, choice.range.lowerBound)))
                }.joined()
                return shown == choice.text
            }
            if let best = matching.max(by: { $0.0.range.count < $1.0.range.count })?.0 {
                currentWord = ConvertedWord(range: best.range, text: best.text, reading: best.reading)
            }
        }
        ranked.sort {
            let aCurrent = $0.0.range == currentWord?.range && $0.0.text == currentWord?.text
            let bCurrent = $1.0.range == currentWord?.range && $1.0.text == currentWord?.text
            if aCurrent != bCurrent { return aCurrent }
            if $0.0.range.count != $1.0.range.count { return $0.0.range.count > $1.0.range.count }
            if $0.1 != $1.1 { return $0.1 > $1.1 }
            return $0.0.text < $1.0.text
        }
        var seen = Set<String>()
        choices = ranked.compactMap { seen.insert($0.0.text).inserted ? $0.0 : nil }
        candidateSelection = 0
    }

    private func refresh() {
        let revision = engine.conversionRevision
        if conversionDirty || conversionRevision != revision {
            conversion = engine.convert(tokens, fixed: fixed)
            conversionDirty = false
            conversionRevision = revision
        }
        var text = ""
        var clauses: [Range<Int>] = []
        var caret = 0
        var insertedBuffer = false
        var focused: Int?
        let focusToken = !choices.isEmpty ? (cursor > 0 ? cursor - 1 : 0) : nil
        func append(_ value: String, focus: Bool = false) {
            guard !value.isEmpty else { return }
            let start = text.utf16.count
            text += value
            if focus { focused = clauses.count }
            clauses.append(start..<text.utf16.count)
        }
        func appendBuffer() {
            guard !insertedBuffer else { return }
            append(bufferText)
            caret = text.utf16.count
            insertedBuffer = true
        }
        for word in conversion {
            if cursor == word.range.lowerBound { appendBuffer() }
            let isFocus = focusToken.map { word.range.contains($0) } ?? false
            let cursorInside = cursor > word.range.lowerBound && cursor < word.range.upperBound
            if cursorInside && hasBuffer {
                let split = word.text.index(word.text.startIndex, offsetBy: min(word.text.count, cursor - word.range.lowerBound))
                append(String(word.text[..<split]), focus: isFocus && (focusToken ?? 0) < cursor)
                appendBuffer()
                append(String(word.text[split...]), focus: isFocus && (focusToken ?? 0) >= cursor)
            } else {
                let start = text.utf16.count
                append(word.text, focus: isFocus)
                if cursorInside {
                    caret = start + word.text.prefix(cursor - word.range.lowerBound).utf16.count
                    insertedBuffer = true
                }
            }
            if cursor == word.range.upperBound { appendBuffer() }
        }
        if !insertedBuffer { appendBuffer() }
        let candidates: CandidateList? = choices.isEmpty ? nil : CandidateList(
            items: choices.map { Candidate(text: $0.text) }, selection: candidateSelection,
            pageSize: max(1, options.selectionKeys.count))
        presentation = Presentation(text: text, cursor: caret, clauses: clauses, focusedClause: focused, candidates: candidates)
    }
}
