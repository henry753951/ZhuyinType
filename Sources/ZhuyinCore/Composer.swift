import Foundation

public final class Composer {
    public var options: ComposerOptions {
        didSet {
            if options.typoTolerance != oldValue.typoTolerance { conversionDirty = true }
            if options != oldValue { closeCandidates(); refresh() }
        }
    }
    private let engine: Engine
    private var tokens: [CompositionToken] = []
    private var cursor = 0
    private var buffer: [Character?] = [nil, nil, nil]
    private var bufferKeys = ""
    private var fixed: [FixedWord] = []
    private var conversion: [ConvertedWord] = []
    private var conversionCache: ConversionCache?
    private var conversionDirty = true
    private var conversionRevision: UInt64 = 0
    private var choices: [FixedWord] = []
    private var candidateSelection = 0
    private var selectionAnchor: Int?
    private var notice: String?
    /// Return an index into the score-ordered N-best list. Invalid indices fall back to the decoder's first choice.
    /// Called synchronously only when conversion changes; keep the hook within the typing latency budget.
    public var reranker: (([ScoredSentence]) -> Int)? {
        didSet { conversionDirty = true; closeCandidates(); refresh() }
    }
    public var rerankingCount: Int = 8 {
        didSet { conversionDirty = true; closeCandidates(); refresh() }
    }

    /// Beam-bounded alternatives for completed tokens, excluding the unfinished syllable buffer.
    public func nBest(_ k: Int = 8) -> [ScoredSentence] {
        engine.sentences(tokens, fixed: fixed, typo: options.typoTolerance, k: k)
    }

    public init(engine: Engine, options: ComposerOptions = ComposerOptions()) {
        self.engine = engine
        self.options = options
    }

    public private(set) var presentation: Presentation = .empty
    public var isComposing: Bool { !presentation.isEmpty }
    private var bufferText: String { String(buffer.compactMap { $0 }) }
    private var hasBuffer: Bool { buffer.contains { $0 != nil } }
    private var hasComposition: Bool { !tokens.isEmpty || hasBuffer }
    private var selection: Range<Int>? {
        guard let anchor = selectionAnchor, anchor != cursor else { return nil }
        return min(anchor, cursor)..<max(anchor, cursor)
    }

    public func handle(_ key: KeyInput) -> KeyResult {
        notice = nil
        if key == .shiftLeft || key == .shiftRight {
            guard hasComposition else { return .passThrough }
            finishBuffer()
            closeCandidates()
            if !hasBuffer {
                if selectionAnchor == nil { selectionAnchor = cursor }
                cursor = min(tokens.count, max(0, cursor + (key == .shiftLeft ? -1 : 1)))
            }
            refresh()
            return .consumed
        }
        if let range = selection, key == .enter {
            let parts = tokens[range].compactMap(\.reading)
            let text = shown(in: range)
            do {
                guard (1...6).contains(range.count), parts.count == range.count else {
                    throw NSError(domain: "ZhuyinCore", code: 1)
                }
                try engine.addUserPhrase(word: text, reading: parts.joined(separator: " "))
                fix(choice(range: range, text: text, reading: parts.joined(separator: " ")))
                selectionAnchor = nil
                notice = "已加入自訂詞：" + text
            } catch { notice = "自訂詞需為 1 到 6 個字" }
            refresh()
            return .consumed
        }
        if selectionAnchor != nil {
            selectionAnchor = nil
            if key == .escape { refresh(); return .consumed }
        }
        if !choices.isEmpty, let result = handleCandidateKey(key) { return result }
        let result: KeyResult
        if key == .char("`") {
            if hasBuffer {
                let text = bufferText
                clearBuffer()
                insert(.literal(text))
                result = .consumed
            } else if cursor > 0, let reading = tokens[cursor - 1].reading {
                let target = cursor - 1
                delete(target..<cursor)
                insert(.literal(ZhuyinKeyboard.plain(reading)))
                result = .consumed
            } else { result = hasComposition ? .consumed : .passThrough }
        } else if let literal = ZhuyinKeyboard.literal(for: key) {
            finishBuffer(droppingInvalid: true)
            insert(.literal(literal))
            result = .consumed
        } else {
            switch key {
            case .char(let character):
                if let symbol = ZhuyinKeyboard.symbols[character], let slot = ZhuyinKeyboard.slot(of: symbol) {
                    let syllableCount = tokens.reduce(0) { $0 + ($1.reading == nil ? 0 : 1) }
                    if syllableCount < options.maxSyllables { buffer[slot] = symbol; bufferKeys.append(character) }
                    result = .consumed
                } else if let tone = ZhuyinKeyboard.tones[character] {
                    finishBuffer(tone: tone, toneKey: character)
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
                    if let last = buffer.lastIndex(where: { $0 != nil }) {
                        buffer[last] = nil
                        bufferKeys.removeAll { ZhuyinKeyboard.symbols[$0].flatMap(ZhuyinKeyboard.slot) == last }
                    }
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
            case .shiftedChar, .shiftLeft, .shiftRight: result = .passThrough
            }
        }
        refresh()
        return result
    }

    public func selectCandidate(at index: Int) -> KeyResult {
        guard choices.indices.contains(index) else { return .consumed }
        let choice = choices[index]
        if choice.reading.isEmpty {
            fixed.removeAll { $0.range.overlaps(choice.range) }
            tokens[choice.range.lowerBound] = .literal(choice.text)
        } else {
            fix(choice)
            if options.learningEnabled { engine.learn(reading: choice.reading, word: choice.text, amount: 1) }
        }
        conversionDirty = true
        closeCandidates()
        refresh()
        return .consumed
    }

    private func fix(_ choice: FixedWord) {
        fixed.removeAll { $0.range.overlaps(choice.range) }
        fixed.append(choice)
        conversionDirty = true
    }

    public func commitAll() -> String {
        // An unfinished syllable is sent as the symbols on screen: committing never guesses a tone or a correction.
        if hasBuffer {
            insert(.literal(bufferText))
            clearBuffer()
        }
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
        conversionCache = nil
        selectionAnchor = nil
        notice = nil
        conversionDirty = true
        closeCandidates()
        presentation = .empty
    }

    private func clearBuffer() { buffer = [nil, nil, nil]; bufferKeys = "" }

    @discardableResult
    private func finishBuffer(tone: String = "", toneKey: Character = " ", droppingInvalid: Bool = false,
                              allowingTypo: Bool = true) -> Bool {
        guard hasBuffer else { return false }
        let reading = bufferText + tone
        let keys = bufferKeys + String(toneKey)
        if engine.isValidSyllable(reading) || (allowingTypo && options.typoTolerance != .off && !engine.alternatives(for: keys).isEmpty) {
            insert(.syllable(reading, keys: keys))
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
        if case .char(let character) = key, character != "`", let number = options.selectionKeys.firstIndex(of: character) {
            return selectCandidate(at: page * pageSize + number)
        }
        switch key {
        case .char("`"):
            let range = choices[candidateSelection].range
            guard tokens[range].allSatisfy({ $0.reading != nil }) else { closeCandidates(); refresh(); return .consumed }
            let text = ZhuyinKeyboard.plain(tokens[range].compactMap(\.reading).joined(separator: " "))
            delete(range)
            cursor = range.lowerBound
            insert(.literal(text))
            closeCandidates()
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

    /// Text at token boundaries, including partial words and multi-character literal tokens.
    private func shown(in range: Range<Int>) -> String {
        conversion.filter { $0.range.overlaps(range) }.map { word in
            if range.lowerBound <= word.range.lowerBound && range.upperBound >= word.range.upperBound { return word.text }
            return String(word.text.dropFirst(max(0, range.lowerBound - word.range.lowerBound))
                .prefix(min(word.range.upperBound, range.upperBound) - max(word.range.lowerBound, range.lowerBound)))
        }.joined()
    }

    private func choice(range: Range<Int>, text: String, reading: String,
                        words: [ConvertedWord]? = nil) -> FixedWord {
        let source = (words ?? conversion).flatMap { word -> [ConvertedWord] in
            guard let existing = fixed.first(where: { $0.range == word.range && $0.text == word.text }),
                  let first = existing.components.first else { return [word] }
            let delta = word.range.lowerBound - first.range.lowerBound
            return existing.components.map {
                var part = $0
                part.range = (part.range.lowerBound + delta)..<(part.range.upperBound + delta)
                return part
            }
        }
        let parts = source.filter { $0.range.overlaps(range) }.map { word -> ConvertedWord in
            let overlap = max(word.range.lowerBound, range.lowerBound)..<min(word.range.upperBound, range.upperBound)
            if overlap == word.range { return word }
            let value = String(word.text.dropFirst(overlap.lowerBound - word.range.lowerBound).prefix(overlap.count))
            return ConvertedWord(range: overlap, text: value,
                                 reading: tokens[overlap].compactMap(\.reading).joined(separator: " "),
                                 wordID: engine.languageModel.wordID(for: value))
        }
        return FixedWord(range: range, text: text, reading: reading, wordID: parts.last?.wordID, components: parts)
    }

    private func openCandidates() {
        guard !tokens.isEmpty, !hasBuffer else { return }
        let forward = cursor == 0 || (options.candidateAnchor == .afterCursor && cursor < tokens.count)
        let target = forward ? cursor : cursor - 1
        if case .literal(let text) = tokens[target] {
            choices = ZhuyinKeyboard.alternatives(for: text).map {
                FixedWord(range: target..<(target + 1), text: $0, reading: "")
            }
            candidateSelection = 0
            return
        }
        var ranges: [Range<Int>] = []
        for length in 1...min(6, forward ? tokens.count - cursor : cursor) {
            let range = forward ? cursor..<(cursor + length) : (cursor - length)..<cursor
            guard tokens[range].allSatisfy({ $0.reading != nil }) else { break }
            ranges.append(range)
        }
        choices.removeAll(keepingCapacity: true)
        let decoded = engine.cachedWords(conversionCache)
        for range in ranges.reversed() {
            let reading = tokens[range].compactMap(\.reading).joined(separator: " ")
            var history = LanguageModelHistory()
            for word in conversion where word.range.upperBound <= range.lowerBound {
                history = word.reading.isEmpty ? .init()
                    : (word.history ?? history.appending(word.wordID, order: engine.languageModel.order))
            }
            let matchesInRange = engine.matches(tokens, start: range.lowerBound, typo: options.typoTolerance)
                .filter { $0.range == range }
            let ranked = matchesInRange.map { (match: $0, score: engine.score($0.entry, reading: reading, history: history)) }
            let matches = ranked.sorted {
                $0.score == $1.score ? $0.match.entry.word < $1.match.entry.word : $0.score > $1.score
            }.map(\.match)
            var seen = Set<String>()
            func add(_ word: FixedWord) {
                if choices.count < 60 && seen.insert(word.text).inserted { choices.append(word) }
            }
            let current = shown(in: range)
            if let existing = fixed.first(where: { $0.range == range && $0.text == current }) {
                add(existing)
            } else {
                var word = choice(range: range, text: current, reading: reading)
                word.annotation = matches.first(where: { $0.entry.word == current })?.annotation
                add(word)
            }
            // Boundaries make the entire span one selectable unit, while allowing internal word segmentation.
            var cache = conversionCache
            let words: [ConvertedWord]
            if !decoded.contains(where: { word in
                (word.range.lowerBound < range.lowerBound && range.lowerBound < word.range.upperBound)
                    || (word.range.lowerBound < range.upperBound && range.upperBound < word.range.upperBound)
            }) {
                words = decoded
            } else {
                let constraints = fixed.filter { word in
                    !word.range.overlaps(range) || (range.lowerBound <= word.range.lowerBound && word.range.upperBound <= range.upperBound)
                }
                words = engine.convert(tokens, fixed: constraints,
                                       typo: options.typoTolerance, cache: &cache, group: range)
            }
            let parts = words.filter { range.overlaps($0.range) }
            let text = parts.map(\.text).joined()
            if !text.isEmpty { add(choice(range: range, text: text, reading: reading, words: parts)) }
            for match in matches {
                add(FixedWord(range: range, text: match.entry.word, reading: reading, wordID: match.entry.wordID,
                              annotation: match.annotation))
            }
            if choices.count == 60 { break }
        }
        candidateSelection = 0
    }

    private func refresh() {
        let revision = engine.conversionRevision
        if conversionDirty || conversionRevision != revision {
            conversion = engine.convert(tokens, fixed: fixed, typo: options.typoTolerance, cache: &conversionCache)
            if let reranker, !tokens.isEmpty {
                let sentences = nBest(max(1, rerankingCount))
                let index = reranker(sentences)
                if sentences.indices.contains(index) {
                    var history = LanguageModelHistory()
                    conversion = sentences[index].words.map { word in
                        let constraint = fixed.first { $0.range == word.range && $0.text == word.text }
                        let wordID = constraint?.wordID ?? engine.languageModel.wordID(for: word.text)
                        if word.reading.isEmpty { history = .init() }
                        else if let parts = constraint?.components, !parts.isEmpty {
                            for part in parts { history = history.appending(part.wordID, order: engine.languageModel.order) }
                        } else { history = history.appending(wordID, order: engine.languageModel.order) }
                        return ConvertedWord(range: word.range, text: word.text, reading: word.reading,
                                             wordID: wordID, history: history)
                    }
                }
            }
            conversionDirty = false
            conversionRevision = revision
        }
        let focusRange = selection ?? (choices.isEmpty ? nil : choices[candidateSelection].range)
        var text = ""
        var offsets = Array(repeating: 0, count: tokens.count + 1)
        var boundaries = Set<Int>()
        var caret = 0
        var insertedBuffer = false
        func appendBuffer() {
            guard !insertedBuffer else { return }
            if hasBuffer { boundaries.insert(text.utf16.count); text += bufferText; boundaries.insert(text.utf16.count) }
            caret = text.utf16.count
            insertedBuffer = true
        }
        for word in conversion {
            boundaries.insert(text.utf16.count)
            for token in word.range {
                if cursor == token { appendBuffer() }
                offsets[token] = text.utf16.count
                if word.range.count == 1 { text += word.text }
                else { text += String(word.text.dropFirst(token - word.range.lowerBound).prefix(1)) }
            }
            offsets[word.range.upperBound] = text.utf16.count
            boundaries.insert(text.utf16.count)
            if cursor == word.range.upperBound { appendBuffer() }
        }
        if !insertedBuffer { appendBuffer() }
        var focus: Range<Int>?
        if let range = focusRange {
            let focused = offsets[range.lowerBound]..<offsets[range.upperBound]
            focus = focused
            boundaries = boundaries.filter { !focused.contains($0) }
            boundaries.insert(focused.lowerBound)
            boundaries.insert(focused.upperBound)
        }
        let sorted = boundaries.sorted()
        let clauses = zip(sorted, sorted.dropFirst()).compactMap { start, end in start < end ? start..<end : nil }
        if let range = selection, notice == nil { notice = "Enter 加入自訂詞：" + shown(in: range) }
        let candidates: CandidateList? = choices.isEmpty ? nil : CandidateList(
            items: choices.map { Candidate(text: $0.text, annotation: $0.annotation) }, selection: candidateSelection,
            pageSize: max(1, options.selectionKeys.count))
        let hint = choices.isEmpty ? nil : tokens[choices[candidateSelection].range].map(\.reading)
        let zhuyin = hint.flatMap { parts in
            parts.allSatisfy { $0 != nil } ? ZhuyinKeyboard.plain(parts.compactMap { $0 }.joined(separator: " ")) : nil
        }
        presentation = Presentation(text: text, cursor: caret, clauses: clauses,
                                    focusedClause: focus.flatMap { clauses.firstIndex(of: $0) }, candidates: candidates,
                                    notice: notice, zhuyinHint: zhuyin)
    }
}
