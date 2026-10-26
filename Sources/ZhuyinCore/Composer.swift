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
    /// LM history of the words committed just before this composition; the sentence start after punctuation, a
    /// passed-through key or `breakContext()`.
    private var leadingHistory = LanguageModelHistory()
    /// The words behind `leadingHistory` (at most two, after the last literal), for learning across commits.
    private var leadingWords: [ConvertedWord] = []
    /// The last commit, followed while Backspace keys passed to the app delete it from its end.
    private var lastCommit: (words: [ConvertedWord], leading: LanguageModelHistory, leadingWords: [ConvertedWord],
                             time: Double)?
    private var deletedCharacters = 0
    /// Whole words of the last commit deleted that way.  Retyping their readings as other words is a correction.
    private var deletedWords: [ConvertedWord] = []
    /// Seconds after a commit in which deleting and retyping it counts as a correction.
    static let correctionWindow: Double = 30
    /// Return an index into the score-ordered N-best list. Invalid indices fall back to the decoder's first choice.
    /// Called synchronously only when conversion changes; keep the hook within the typing latency budget.
    public var reranker: (([ScoredSentence]) -> Int)? {
        didSet { conversionDirty = true; closeCandidates(); refresh() }
    }
    public var rerankingCount: Int = 8 {
        didSet { conversionDirty = true; closeCandidates(); refresh() }
    }
    /// Scores to add to the best candidates for a span, given the composed text before it; nil leaves the order as is.
    /// Called once per span length when the candidate window opens.
    public var candidateScorer: ((_ before: String, _ candidates: [String]) -> [Double]?)?
    /// How many of a span's best-scoring candidates the scorer reorders.
    public var candidateScoringCount = 24

    /// Beam-bounded alternatives for completed tokens, excluding the unfinished syllable buffer.
    public func nBest(_ k: Int = 8) -> [ScoredSentence] {
        engine.sentences(tokens, fixed: fixed, typo: options.typoTolerance, k: k, start: leadingHistory)
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
        let result = process(key)
        // A key the app receives may move the insertion point or edit the text before it.
        if !result.handled && !(key == .backspace && followDeletion()) { forgetContext() }
        return result
    }

    private func forgetContext() {
        leadingHistory = .init()
        leadingWords = []
        lastCommit = nil
        deletedWords = []
    }

    /// Backspace passed to the app with nothing composed deletes the last commit from its end: the context becomes
    /// what is left of it.  False when it is not following a recent commit.
    private func followDeletion() -> Bool {
        guard !hasComposition, let last = lastCommit, engine.clock() - last.time < Self.correctionWindow else { return false }
        deletedCharacters += 1
        var remaining = deletedCharacters, kept = last.words.count
        while kept > 0, remaining >= last.words[kept - 1].text.count {
            remaining -= last.words[kept - 1].text.count
            kept -= 1
        }
        guard kept > 0 || remaining == 0 else { return false }
        deletedWords = last.words[kept...].filter { !$0.reading.isEmpty }
        if remaining > 0 {
            leadingHistory = .init(); leadingWords = []
        } else if kept == 0 {
            leadingHistory = last.leading; leadingWords = last.leadingWords
        } else {
            let before = Array(last.words[..<kept])
            leadingHistory = before.last?.history ?? .init()
            leadingWords = Self.trailingWords(before, after: last.leadingWords)
        }
        return true
    }

    /// The last two syllable words of `words` after its last literal, continuing `earlier` when there is no literal.
    private static func trailingWords(_ words: [ConvertedWord], after earlier: [ConvertedWord]) -> [ConvertedWord] {
        let tail = words.reversed().prefix { !$0.reading.isEmpty }.reversed()
        return Array((tail.count == words.count ? earlier + Array(tail) : Array(tail)).suffix(2))
    }

    private func process(_ key: KeyInput) -> KeyResult {
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
            case .shiftedChar, .printed, .shiftLeft, .shiftRight: result = .passThrough
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
            if options.learningEnabled {
                // What was shown there loses some of the user's evidence in its context.
                for word in conversion where word.range.overlaps(choice.range) && word.text != choice.text && !word.reading.isEmpty {
                    engine.weaken(text: word.text, reading: word.reading, wordID: word.wordID, history: word.history)
                }
            }
            fix(choice)
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
        if options.learningEnabled, !text.isEmpty {
            var committed = conversion.map { word in
                CommittedWord(range: word.range, text: word.text, reading: word.reading, wordID: word.wordID,
                              picked: fixed.contains { $0.range == word.range && $0.text == word.text })
            }
            let corrected = correctedPrefix()
            if corrected > 0 {
                // Deleted right after it was committed and typed again as other words: learn it like a pick, and
                // weaken what was deleted where it stood.
                for index in committed.indices where committed[index].range.upperBound <= corrected {
                    committed[index].picked = true
                }
                for word in deletedWords where !conversion.contains(where: { $0.text == word.text && $0.reading == word.reading }) {
                    engine.weaken(text: word.text, reading: word.reading, wordID: word.wordID, history: word.history)
                }
            }
            let context = leadingWords.map {
                CommittedWord(range: 0..<0, text: $0.text, reading: $0.reading, wordID: $0.wordID, picked: false)
            }
            engine.learn(commit: committed, syllableCount: tokens.filter { $0.reading != nil }.count, context: context)
        }
        if !text.isEmpty {
            // A literal at the end (punctuation, symbols as typed) leaves the sentence-start history.
            lastCommit = (conversion, leadingHistory, leadingWords, engine.clock())
            leadingWords = Self.trailingWords(conversion, after: leadingWords)
            leadingHistory = conversion.last?.history ?? .init()
            deletedCharacters = 0
            deletedWords = []
        }
        reset()
        return text
    }

    /// The number of leading tokens that retype the deleted words' readings as different words; 0 when they do not.
    private func correctedPrefix() -> Int {
        guard !deletedWords.isEmpty else { return 0 }
        let readings = deletedWords.flatMap { $0.reading.split(separator: " ").map(String.init) }
        guard tokens.count >= readings.count,
              zip(tokens, readings).allSatisfy({ $0.reading == $1 }),
              conversion.contains(where: { $0.range.upperBound == readings.count }) else { return 0 }
        let retyped = conversion.filter { $0.range.upperBound <= readings.count }.map(\.text).joined()
        return retyped == deletedWords.map(\.text).joined() ? 0 : readings.count
    }

    /// The next composition starts a sentence: call when the text before the insertion point is unknown.
    public func breakContext() {
        forgetContext()
        if !tokens.isEmpty { conversionDirty = true; refresh() }
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
        // The displayed conversion, which the reranker may have chosen over the decoder's best.
        let decoded = conversion
        for range in ranges.reversed() {
            let reading = tokens[range].compactMap(\.reading).joined(separator: " ")
            var history = leadingHistory
            for word in conversion where word.range.upperBound <= range.lowerBound {
                history = word.reading.isEmpty ? .init()
                    : (word.history ?? history.appending(word.wordID, order: engine.languageModel.order))
            }
            let matchesInRange = engine.matches(tokens, start: range.lowerBound, typo: options.typoTolerance)
                .filter { $0.range == range }
            var ranked = matchesInRange.map { (match: $0, score: engine.score($0.entry, reading: reading, history: history)) }
            let order: ((match: Engine.Match, score: Double), (match: Engine.Match, score: Double)) -> Bool = { a, b in
                a.score == b.score ? a.match.entry.word < b.match.entry.word : a.score > b.score
            }
            ranked.sort(by: order)
            if let candidateScorer, ranked.count > 1 {
                let head = min(ranked.count, candidateScoringCount)
                let before = conversion.filter { $0.range.upperBound <= range.lowerBound }.map(\.text).joined()
                if let extra = candidateScorer(before, ranked[..<head].map(\.match.entry.word)), extra.count == head {
                    for index in 0..<head { ranked[index].score += extra[index] }
                    ranked[..<head].sort(by: order)
                }
            }
            let matches = ranked.map(\.match)
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
                                       typo: options.typoTolerance, cache: &cache, group: range, start: leadingHistory)
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
            conversion = engine.convert(tokens, fixed: fixed, typo: options.typoTolerance, cache: &conversionCache,
                                        start: leadingHistory)
            if let reranker, !tokens.isEmpty {
                let sentences = nBest(max(1, rerankingCount))
                let index = reranker(sentences)
                if sentences.indices.contains(index) {
                    var history = leadingHistory
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
