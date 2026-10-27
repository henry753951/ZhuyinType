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
    /// The open list holds only the character at the cursor (Shift-↓), not every span.
    private var singleCharacterList = false
    /// The open list shows typing corrections and fuzzy matches (Option), not only words for the keys as typed.
    private var tolerantList = false
    /// The open list is for a literal (punctuation), which has no corrections.
    private var literalList = false
    /// The token the open list starts from (its anchor character).
    private var listTarget = 0
    private var selectionAnchor: Int?
    private var notice: String?
    /// When the key being handled went down, if the caller said.
    private var keyTime: TimeInterval?
    /// Keys typed at the end of the composition since anything else happened, split again into syllables on every key
    /// (`KeyDecoder`), and how many tokens at the end they produced.
    private var run: [KeyDecoder.Key] = []
    private var runTokens = 0
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
    /// A character model (the v3 syllable encoder): for each syllable, log P over the candidate characters given.
    /// Nil when it cannot answer.  `characterWeight` is how much its scores count in the lattice.
    public var characterScorer: ((_ syllables: [CharacterModelSyllable], _ candidates: [[Character]]) -> [[Double]]?)? {
        didSet { conversionDirty = true; refresh() }
    }
    public var characterWeight = 1.0
    /// After a pick from the candidate list, keep every other shown word as it is.
    public var freezesOnPick = true
    /// Learning paused for now (secure input: a password field may be focused); the setting itself is unchanged.
    public var learningSuspended = false
    private var learns: Bool { options.learningEnabled && !learningSuspended }
    /// Also order the candidate list by the character model's scores at each position.
    public var characterScoresInList = true
    /// The character model's scores for the current tokens, recomputed when they change.
    private var characterScores: CharacterScores?
    /// How many characters per syllable the character model is asked about.
    static let characterCandidates = 64

    /// How many of a span's best-scoring candidates the scorer reorders.
    public var candidateScoringCount = 24

    /// Beam-bounded alternatives for completed tokens, excluding the unfinished syllable buffer.
    public func nBest(_ k: Int = 8) -> [ScoredSentence] {
        engine.sentences(tokens, fixed: fixed, typo: options.typoTolerance, k: k, start: leadingHistory,
                         characters: characterScores)
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

    /// `time`: when the key went down, in seconds (`NSEvent.timestamp`); the decoder reads how close keys were.
    public func handle(_ key: KeyInput, at time: TimeInterval) -> KeyResult {
        keyTime = time
        defer { keyTime = nil }
        return handle(key)
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
        if !continuesRun(key) { settleRun() }
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
            case .char(let character) where decodesRun && (ZhuyinKeyboard.symbols[character] != nil || ZhuyinKeyboard.tones[character] != nil):
                result = typeIntoRun(character)
            case .space where decodesRun && hasBuffer:
                result = typeIntoRun(" ")
            case .backspace where decodesRun && hasBuffer && !run.isEmpty:
                result = eraseFromRun()
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
            case .down, .shiftDown, .shiftUp:
                result = hasComposition ? .consumed : .passThrough
                if hasComposition {
                    finishBuffer(); refresh()
                    openCandidates(singleCharacter: key == .shiftDown, tolerant: key == .shiftUp && correctionsAvailable)
                }
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
            if learns, let annotation = choice.annotation {
                // A picked correction: the keys typed against the reading meant teach this user's slips.
                let meant = annotation.split(separator: " ").map { part in
                    part.first == "˙" ? String(part.dropFirst()) + "˙" : String(part)
                }
                let typed = tokens[choice.range].compactMap { token -> String? in
                    if case .syllable(_, let keys, _) = token { return keys }
                    return nil
                }
                if typed.count == meant.count { engine.recordTypos(Array(zip(typed, meant))) }
            }
            if learns {
                // What was shown there loses some of the user's evidence in its context.
                for word in conversion where word.range.overlaps(choice.range) && word.text != choice.text && !word.reading.isEmpty {
                    engine.weaken(text: word.text, reading: word.reading, wordID: word.wordID, history: word.history)
                }
            }
            fix(choice)
            if freezesOnPick {
                // The rest of what is shown stays as it is: picking one word must not change words the user has
                // already read (as the system's own Zhuyin input does).
                for word in conversion where !word.reading.isEmpty && !word.range.overlaps(choice.range)
                    && !fixed.contains(where: { $0.range.overlaps(word.range) }) {
                    fixed.append(FixedWord(range: word.range, text: word.text, reading: word.reading,
                                           wordID: word.wordID, frozen: true))
                }
            }
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
        if learns, !text.isEmpty {
            var committed = conversion.map { word in
                CommittedWord(range: word.range, text: word.text, reading: word.reading, wordID: word.wordID,
                              picked: fixed.contains { $0.range == word.range && $0.text == word.text && !$0.frozen })
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

    /// The text before the insertion point, read from the app: the next composition continues its last words (or
    /// starts a sentence after punctuation, a space or a line break).  Ignored while composing.
    public func setPrecedingText(_ text: String) {
        guard !hasComposition else { return }
        engine.adaptDomain(to: text)
        let words = engine.trailingWords(of: text)
        leadingWords = words
        leadingHistory = words.last?.history ?? .init()
        conversionDirty = true
    }

    /// The next composition starts a sentence: call when the text before the insertion point is unknown.
    public func breakContext() {
        forgetContext()
        if !tokens.isEmpty { conversionDirty = true; refresh() }
    }

    public func reset() {
        settleRun()
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

    /// Typing at the end of the composition, with a decoder that splits key runs.
    private var decodesRun: Bool {
        options.keyDecoder != .slots && cursor == tokens.count && selectionAnchor == nil && choices.isEmpty
    }

    /// Keys that extend the run: Zhuyin and tone keys, Space finishing a syllable, Backspace inside the unfinished one.
    private func continuesRun(_ key: KeyInput) -> Bool {
        switch key {
        case .char(let c): return ZhuyinKeyboard.symbols[c] != nil || ZhuyinKeyboard.tones[c] != nil
        case .space, .backspace: return hasBuffer
        default: return false
        }
    }

    /// The run's tokens stay as they are; typing on starts a new run from the unfinished syllable.
    private func settleRun() {
        run = []
        runTokens = 0
    }

    /// With typing correction off, only where syllables end is decided; keys are taken as typed.
    private var keyDecoder: KeyDecoder {
        let method = options.typoTolerance == .off && [.reorder, .timed].contains(options.keyDecoder) ? .boundaries
            : options.keyDecoder
        return KeyDecoder(method: method) { [engine, options] reading, keys in
            if engine.isValidSyllable(reading) { return 0 }
            guard options.typoTolerance != .off, let cheapest = engine.alternatives(for: keys).first else { return nil }
            return cheapest.cost * options.typoTolerance.costMultiplier
        }
    }

    private func typeIntoRun(_ character: Character) -> KeyResult {
        if run.isEmpty {
            run = bufferKeys.map { KeyDecoder.Key(character: $0, time: nil) }
            runTokens = 0
        }
        run.append(KeyDecoder.Key(character: character, time: keyTime))
        if !applyRun() { run.removeLast() }
        return .consumed
    }

    /// Asks the character model about every syllable: the characters any lattice word puts there, the most frequent
    /// `characterCandidates` of them.
    private func scoreCharacters() -> CharacterScores? {
        guard let characterScorer, tokens.contains(where: { $0.reading != nil }) else { return nil }
        var candidates = Array(repeating: [Character: Double](), count: tokens.count)
        for start in tokens.indices where tokens[start].reading != nil {
            for match in engine.matches(tokens, start: start, typo: options.typoTolerance) {
                for (offset, character) in match.entry.word.enumerated() where start + offset < tokens.count {
                    let prior = match.entry.logp + match.entry.readingPrior - match.entry.penalty
                    candidates[start + offset][character] = max(candidates[start + offset][character] ?? -.infinity, prior)
                }
            }
        }
        for word in fixed {
            for (offset, character) in word.text.enumerated() where word.range.lowerBound + offset < tokens.count {
                candidates[word.range.lowerBound + offset][character] = 0
            }
        }
        let lists = candidates.map { $0.sorted { $0.value > $1.value }.prefix(Self.characterCandidates).map(\.key) }
        let syllables = tokens.map { token -> CharacterModelSyllable in
            guard case .syllable(let reading, let keys, let implicit) = token else {
                return CharacterModelSyllable(base: nil, tone: 0, costBucket: 0)
            }
            let toneMarks: [Character: Int] = ["ˊ": 2, "ˇ": 3, "ˋ": 4, "˙": 5]
            let tone = reading.last.flatMap { toneMarks[$0] } ?? 1
            let base = String(reading.filter { toneMarks[$0] == nil })
            var bucket = 0
            if !engine.isValidSyllable(reading) {
                let cost = engine.alternatives(for: keys).first?.cost ?? .infinity
                bucket = cost <= 6 ? 1 : cost <= 11 ? 2 : 3
            }
            return CharacterModelSyllable(base: base, tone: tone, costBucket: bucket, implicit: implicit)
        }
        guard let scores = characterScorer(syllables, lists) else { return nil }
        var positions = Array(repeating: [Character: Double](), count: tokens.count)
        for (index, list) in lists.enumerated() where index < scores.count {
            for (column, character) in list.enumerated() where column < scores[index].count {
                positions[index][character] = scores[index][column]
            }
        }
        return CharacterScores(positions: positions, weight: characterWeight)
    }

    /// The run's split: the decoder's cheapest, unless another split of the same shape (as many syllables, the same
    /// unfinished part) close in cost reads better to the language model, as when two keys pressed together leave
    /// it open which one was meant.
    private func bestSplit() -> KeyDecoder.Result? {
        let splits = keyDecoder.decode(run, limit: 6)
        guard let cheapest = splits.first, cheapest.cost.isFinite else { return nil }
        let rivals = splits.filter {
            $0.cost.isFinite && $0.cost <= cheapest.cost + Self.splitMargin
                && $0.syllables.count == cheapest.syllables.count && $0.bufferKeys.isEmpty == cheapest.bufferKeys.isEmpty
        }
        guard rivals.count > 1, options.keyDecoder != .slots else { return cheapest }
        let settled = Array(tokens.dropLast(runTokens))
        return rivals.max { a, b in score(a, after: settled) < score(b, after: settled) }
    }

    private func score(_ split: KeyDecoder.Result, after settled: [CompositionToken]) -> Double {
        let candidate = settled + split.syllables.map { CompositionToken.syllable($0.reading, keys: $0.keys, implicit: $0.implicit) }
        guard let sentence = engine.sentences(candidate, fixed: fixed, typo: options.typoTolerance, k: 1,
                                              start: leadingHistory).first else { return -.infinity }
        return sentence.score - split.cost
    }

    /// How much costlier than the cheapest a split may be and still be judged by the language model.
    static let splitMargin = 6.0

    private func eraseFromRun() -> KeyResult {
        run.removeLast()
        if !applyRun() { settleRun() }
        return .consumed
    }

    /// Splits the run again and replaces the tokens it produced.  False when no split exists or it would make the
    /// composition too long, and nothing changes.
    @discardableResult
    private func applyRun() -> Bool {
        guard let decoded = bestSplit() else { return false }
        let settled = tokens.dropLast(runTokens).filter { $0.reading != nil }.count
        let unfinished = decoded.bufferKeys.isEmpty ? 0 : 1
        guard decoded.cost.isFinite, settled + decoded.syllables.count + unfinished <= options.maxSyllables else { return false }
        if runTokens > 0 { delete((tokens.count - runTokens)..<tokens.count) }
        for syllable in decoded.syllables { insert(.syllable(syllable.reading, keys: syllable.keys, implicit: syllable.implicit)) }
        runTokens = decoded.syllables.count
        buffer = decoded.buffer
        bufferKeys = decoded.bufferKeys
        // A long run is settled except for its unfinished syllable, which keeps the search short.
        if run.count > 48 {
            run = bufferKeys.map { KeyDecoder.Key(character: $0, time: nil) }
            runTokens = 0
        }
        return true
    }

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
        case .shiftDown: openCandidates(singleCharacter: !singleCharacterList, tolerant: tolerantList)
        case .shiftUp:
            guard !literalList, correctionsAvailable else { return .consumed }
            openCandidates(singleCharacter: singleCharacterList, tolerant: !tolerantList)
        case .escape, .backspace: closeCandidates()
        // Along the list moves the highlight; across it moves to the neighbouring character (vertical) or turns
        // the page (horizontal).
        case .left where options.verticalList: moveListTarget(by: -1)
        case .right where options.verticalList: moveListTarget(by: 1)
        case .up where !options.verticalList, .pageUp, .shiftTab: candidateSelection = max(0, page - 1) * pageSize
        case .down where !options.verticalList, .pageDown, .tab: candidateSelection = min(pageCount - 1, page + 1) * pageSize
        case .up, .left: candidateSelection = max(0, candidateSelection - 1)
        case .down, .right: candidateSelection = min(choices.count - 1, candidateSelection + 1)
        case .space: candidateSelection = ((page + 1) % pageCount) * pageSize
        default:
            closeCandidates()
            return nil
        }
        refresh()
        return .consumed
    }

    /// The open list moves to the previous or next character, in the same mode.
    private func moveListTarget(by delta: Int) {
        let target = min(tokens.count - 1, max(0, listTarget + delta))
        guard target != listTarget else { return }
        cursor = options.candidateAnchor == .afterCursor ? target : target + 1
        openCandidates(singleCharacter: singleCharacterList, tolerant: tolerantList)
    }

    /// Text at token boundaries, including partial words and multi-character literal tokens.
    /// The converted text over `range`, in the lexicon's own forms.
    private func converted(in range: Range<Int>) -> String {
        conversion.filter { $0.range.overlaps(range) }.map { word in
            if range.lowerBound <= word.range.lowerBound && range.upperBound >= word.range.upperBound { return word.text }
            return String(word.text.dropFirst(max(0, range.lowerBound - word.range.lowerBound))
                .prefix(min(word.range.upperBound, range.upperBound) - max(word.range.lowerBound, range.lowerBound)))
        }.joined()
    }

    /// The text over `range` as shown, in the preferred character forms.
    private func shown(in range: Range<Int>) -> String { options.characterForms.apply(converted(in: range)) }

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

    /// Switches the open candidate window between words for the keys as typed and typing corrections.
    public func toggleCorrections() {
        guard !choices.isEmpty, !literalList, correctionsAvailable else { return }
        openCandidates(singleCharacter: singleCharacterList, tolerant: !tolerantList)
        refresh()
    }

    private var correctionsAvailable: Bool { options.typoTolerance != .off || !engine.fuzzySounds.isEmpty }

    /// The list for the span at the cursor.  Words for the keys as typed, longest spans first, each span ranked in
    /// context; only dictionary words and single characters, never pieces of the conversion joined together.
    /// `singleCharacter`: only the character at the cursor.  `tolerant`: typing corrections and fuzzy matches first,
    /// then the rest.
    private func openCandidates(singleCharacter: Bool = false, tolerant: Bool = false) {
        guard !tokens.isEmpty, !hasBuffer else { return }
        singleCharacterList = singleCharacter
        tolerantList = tolerant
        literalList = false
        let forward = cursor == 0 || (options.candidateAnchor == .afterCursor && cursor < tokens.count)
        let target = forward ? cursor : cursor - 1
        listTarget = target
        if case .literal(let text) = tokens[target] {
            literalList = true
            choices = ZhuyinKeyboard.alternatives(for: text).map {
                FixedWord(range: target..<(target + 1), text: $0, reading: "")
            }
            candidateSelection = 0
            return
        }
        var ranges: [Range<Int>] = []
        for length in 1...(singleCharacter ? 1 : min(6, forward ? tokens.count - cursor : cursor)) {
            let range = forward ? cursor..<(cursor + length) : (cursor - length)..<cursor
            guard tokens[range].allSatisfy({ $0.reading != nil }) else { break }
            ranges.append(range)
        }
        choices.removeAll(keepingCapacity: true)
        var head: FixedWord?
        var exact: [FixedWord] = []
        var corrected: [FixedWord] = []
        let longest = ranges.last
        for range in ranges.reversed() {
            let reading = tokens[range].compactMap(\.reading).joined(separator: " ")
            var history = leadingHistory
            for word in conversion where word.range.upperBound <= range.lowerBound {
                history = word.reading.isEmpty ? .init()
                    : (word.history ?? history.appending(word.wordID, order: engine.languageModel.order))
            }
            let allMatches = engine.matches(tokens, start: range.lowerBound, typo: options.typoTolerance)
                .filter { $0.range == range }
            let matchesInRange = allMatches.filter { tolerant || $0.annotation == nil }
            var ranked = matchesInRange.map { (match: $0, score: engine.score($0.entry, reading: reading, history: history)) }
            let order: ((match: Engine.Match, score: Double), (match: Engine.Match, score: Double)) -> Bool = { a, b in
                a.score == b.score ? a.match.entry.word < b.match.entry.word : a.score > b.score
            }
            if characterScoresInList, let characterScores {
                for index in ranked.indices {
                    ranked[index].score += characterScores.score(ranked[index].match.entry.word, at: range.lowerBound)
                }
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
            // What is shown there: first for the whole span; for a shorter span only when it is one word or a pick.
            let current = converted(in: range)
            if let existing = fixed.first(where: { $0.range == range && $0.text == current }) {
                exact.append(existing)
            } else if range == longest || conversion.contains(where: { $0.range == range }) {
                var word = choice(range: range, text: current, reading: reading)
                word.annotation = allMatches.first(where: { $0.entry.word == current && $0.annotation == nil })
                    .map { _ in nil } ?? allMatches.first(where: { $0.entry.word == current })?.annotation
                if range == longest { head = word } else { exact.append(word) }
            }
            for match in ranked.map(\.match) {
                let word = FixedWord(range: range, text: match.entry.word, reading: reading, wordID: match.entry.wordID,
                                     annotation: match.annotation)
                if match.annotation == nil { exact.append(word) } else { corrected.append(word) }
            }
        }
        // Two forms of one word (臺北, 台北) are shown once, in the preferred form.
        let forms = options.characterForms
        for word in [head].compactMap({ $0 }) + (tolerant ? corrected + exact : exact) where choices.count < 60
            && !choices.contains(where: { $0.range == word.range && forms.apply($0.text) == forms.apply(word.text) }) {
            choices.append(word)
        }
        candidateSelection = 0
    }

    private func refresh() {
        let revision = engine.conversionRevision
        if conversionDirty || conversionRevision != revision {
            characterScores = scoreCharacters()
            conversion = engine.convert(tokens, fixed: fixed, typo: options.typoTolerance, cache: &conversionCache,
                                        start: leadingHistory, characters: characterScores)
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
                let shown = options.characterForms.apply(word.text)
                if word.range.count == 1 { text += shown }
                else { text += String(shown.dropFirst(token - word.range.lowerBound).prefix(1)) }
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
        // With the cursor moved inside the composition, the character ↓ would change.
        var targetRange: Range<Int>?
        if focusRange == nil, !hasBuffer, cursor < tokens.count {
            let forward = cursor == 0 || options.candidateAnchor == .afterCursor
            let target = forward ? cursor : cursor - 1
            let range = offsets[target]..<offsets[target + 1]
            if !range.isEmpty {
                targetRange = range
                boundaries.insert(range.lowerBound)
                boundaries.insert(range.upperBound)
            }
        }
        let sorted = boundaries.sorted()
        let clauses = zip(sorted, sorted.dropFirst()).compactMap { start, end in start < end ? start..<end : nil }
        if let range = selection, notice == nil { notice = "Enter 加入自訂詞：" + shown(in: range) }
        let candidates: CandidateList? = choices.isEmpty ? nil : CandidateList(
            items: choices.map { Candidate(text: options.characterForms.apply($0.text), annotation: $0.annotation) },
            selection: candidateSelection,
            pageSize: max(1, options.selectionKeys.count),
            tolerant: literalList || !correctionsAvailable ? nil : tolerantList,
            singleCharacter: singleCharacterList)
        let hint = choices.isEmpty ? nil : tokens[choices[candidateSelection].range].map(\.reading)
        let zhuyin = hint.flatMap { parts in
            parts.allSatisfy { $0 != nil } ? ZhuyinKeyboard.plain(parts.compactMap { $0 }.joined(separator: " ")) : nil
        }
        presentation = Presentation(text: text, cursor: caret, clauses: clauses,
                                    focusedClause: focus.flatMap { clauses.firstIndex(of: $0) },
                                    targetClause: targetRange.flatMap { clauses.firstIndex(of: $0) }, candidates: candidates,
                                    notice: notice, zhuyinHint: zhuyin)
    }
}
