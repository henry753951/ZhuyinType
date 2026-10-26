import Foundation

struct WordEntry {
    var word: String
    var logp: Double
    var reading: String? = nil
    var penalty: Double = 0
    // A user-confirmed correction is an exact typed-reading recall on subsequent input.
    var recalled = false
    var wordID: UInt32? = nil
    /// A recall of a whole earlier composition: offered only when it covers the whole current one.
    var wholeOnly = false
}

struct PhraseKey: Hashable {
    var reading: String
    var word: String
}

/// What a recall entry remembers: exact typed readings mapped to what the user committed for them.
enum RecallKind: String {
    /// A whole composition with a correction in it; offered only for an identical whole composition.
    case whole
    /// A correction together with the syllables around it (3–6 syllables).
    case span
    /// A word picked under a reading it does not have in the lexicon (a typo the user corrected).
    case typo
    /// A multi-word phrase the user picked as one unit.
    case phrase
}

struct RecallValue {
    var kind: RecallKind
    var short = DecayingCount()
    var long = DecayingCount()
    var picks = 0
    static let shortHalfLife: Double = 8 * 3600
    static let longHalfLife: Double = 90 * 86400
    /// Picked this many times, a recall keeps a floor and is never forgotten.
    static let floorPicks = 3
    func value(at now: Double) -> Double {
        let long = self.long.at(now, halfLife: Self.longHalfLife)
        return short.at(now, halfLife: Self.shortHalfLife) + (picks >= Self.floorPicks ? max(long, 1) : long)
    }
    /// One pick stays active for about two months unless used again.
    func isActive(at now: Double) -> Bool { value(at: now) >= 0.6 }
}

/// One word of a committed composition, as the composer hands it to the engine for learning.
struct CommittedWord {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32?
    var picked: Bool
}

/// Lexicon and personal data are shared safely by composers. Disk writes never run on the calling thread.
public final class Engine: @unchecked Sendable {
    private var lexicon: [String: [WordEntry]] = [:]
    private var reverse: [String: [(String, Double)]] = [:]
    private var phrases: [PhraseKey: Double] = [:]
    private var phraseIndex: [String: [String: WordEntry]] = [:]
    private var wordIDs: [String: UInt32] = [:]
    private var readingIndex: [String: [String]] = [:]
    private var syllables = Set<String>()
    private var syllableBases = Set<String>()
    private var neutralBases = Set<String>()
    private var matchCache: [String: [Match]] = [:]
    private var readingPrefixes = Set<String>()
    private var alternativeCache: [String: [ReadingAlternative]] = [:]
    private var entryCache: [String: [WordEntry]] = [:]
    private var learned: [PhraseKey: RecallValue] = [:]
    let personal = PersonalLanguageModel()
    /// The time the personal memory decays by (seconds since 1970); replaceable for tests.
    public var clock: () -> Double {
        get { personal.clock }
        set { personal.clock = newValue }
    }
    private var nextPersonalID: UInt32 = 0x4000_0000
    private var cacheDay = Int.min
    private var validCodes: [Int: String] = [:]
    private var learnedSyllables = Set<String>()
    private var learnedWords: [String: Set<String>] = [:]
    private let lock = NSLock()
    private let directory: URL?
    private let writer = DispatchQueue(label: "ZhuyinType.personal-data", qos: .utility)
    private var pendingWrite: DispatchWorkItem?
    private var revision: UInt64 = 0
    var conversionRevision: UInt64 { synchronized { revision } }
    let languageModel: any LanguageModel
    /// Default beam width; N-best decoding widens it when K is larger.
    public static let decodingBeamWidth = 12

    public convenience init(lexiconURL: URL, userDataDirectory: URL?) throws {
        try self.init(lexiconURL: lexiconURL, userDataDirectory: userDataDirectory,
                      languageModelDirectory: lexiconURL.deletingLastPathComponent().appendingPathComponent("lm"))
    }

    /// An explicit model directory overrides discovery of the lexicon's adjacent `lm/` directory.
    public convenience init(lexiconURL: URL, userDataDirectory: URL?, languageModelDirectory: URL?) throws {
        try self.init(data: Data(contentsOf: lexiconURL), userDataDirectory: userDataDirectory,
                      languageModelDirectory: languageModelDirectory)
    }

    public convenience init(lexiconText: String, userDataDirectory: URL? = nil,
                            languageModelDirectory: URL? = nil) throws {
        self.init(data: Data(lexiconText.utf8), userDataDirectory: userDataDirectory,
                  languageModelDirectory: languageModelDirectory)
    }

    /// Score a composition as a whole sentence (`<s>` … `</s>`).  On by default: measured on held-out Wikipedia and
    /// colloquial text, dropping either boundary costs whole-run accuracy (most compositions are a clause typed from
    /// its start and committed at its end).  Read when an Engine is created.
    public static var sentenceBoundaries = true

    private init(data: Data, userDataDirectory: URL?, languageModelDirectory: URL?) {
        directory = userDataDirectory
        let model: any LanguageModel
        if let url = languageModelDirectory, let ngram = try? NGramLanguageModel(directory: url) {
            model = ngram
        } else if let url = languageModelDirectory, let bigram = try? BigramLanguageModel(directory: url) {
            model = bigram
        } else {
            model = UnigramLanguageModel()
        }
        let base = Engine.sentenceBoundaries || model.order == 1 ? model : BoundaryFreeLanguageModel(base: model)
        languageModel = PersonalizedLanguageModel(base: base, personal: personal)
        lexicon.reserveCapacity(50000)
        reverse.reserveCapacity(125000)
        scanTSV(data) { fields in
            guard fields.count == 3, let score = Double(fields[2]), score.isFinite,
                  !fields[0].isEmpty, !fields[1].isEmpty else { return }
            lexicon[fields[0], default: []].append(WordEntry(word: fields[1], logp: score, wordID: resolveID(fields[1])))
            reverse[fields[1], default: []].append((fields[0], score))
        }
        loadPersonalData()
        rebuildReadingIndex()
    }

    /// Every word gets an ID: the language model's, or a personal one above its vocabulary, so the user's statistics
    /// can follow words the model does not know.
    private func resolveID(_ word: String) -> UInt32? {
        if let id = wordIDs[word] { return id }
        let id = languageModel.wordID(for: word) ?? {
            defer { nextPersonalID += 1 }
            return nextPersonalID
        }()
        wordIDs[word] = id
        return id
    }

    private func synchronized<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }

    func entries(for reading: String) -> [WordEntry] {
        synchronized {
            expireCachesOnNewDay()
            if let cached = entryCache[reading] { return cached }
            let typed = reading.split(separator: " ")
            var result: [WordEntry] = []
            var exactWords = Set<String>()
            let readings = [reading] + (readingIndex[Self.withoutTones(reading)] ?? []).filter { $0 != reading }
            for source in readings {
                var penalty = 0.0
                if source != reading {
                    for (t, l) in zip(typed, source.split(separator: " ")) where t != l {
                        if l.last == "˙" { penalty += 0.4 }
                        else if t.last == "˙" { penalty += 1.2 }
                        else { penalty = .infinity; break }
                    }
                    if !penalty.isFinite { continue }
                }
                var entries = lexicon[source] ?? []
                for (word, phrase) in phraseIndex[source] ?? [:] {
                    if let index = entries.firstIndex(where: { $0.word == word }) {
                        entries[index].logp = phrase.logp
                        entries[index].recalled = source == reading
                    } else {
                        var entry = phrase
                        entry.recalled = source == reading
                        entries.append(entry)
                    }
                }
                for var entry in entries {
                    if source == reading { exactWords.insert(entry.word) }
                    else if exactWords.contains(entry.word) { continue }
                    entry.reading = source
                    entry.penalty = penalty
                    result.append(entry)
                }
            }
            let now = personal.clock()
            for word in learnedWords[reading] ?? [] {
                guard let value = learned[PhraseKey(reading: reading, word: word)], value.isActive(at: now) else { continue }
                let source = reverse[word]?.max(by: { $0.1 < $1.1 })
                result.append(WordEntry(word: word, logp: source?.1 ?? -7, recalled: true, wordID: resolveID(word),
                                        wholeOnly: value.kind == .whole))
            }
            if entryCache.count >= 4096 { entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true) }
            entryCache[reading] = result
            return result
        }
    }

    /// Memories decay, so cached lookups live for at most a day.  Called under lock.
    private func expireCachesOnNewDay() {
        let today = Int(personal.clock() / 86400)
        guard today != cacheDay else { return }
        cacheDay = today
        entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
        revision &+= 1
    }

    private static func withoutTones(_ reading: String) -> String {
        String(String.UnicodeScalarView(reading.unicodeScalars.filter {
            $0 != "ˊ" && $0 != "ˇ" && $0 != "ˋ" && $0 != "˙"
        }))
    }

    // Rebuilt when personal phrases change; completed tokens remain safe through the decoder's fallback.
    private func rebuildReadingIndex() {
        readingPrefixes.removeAll(keepingCapacity: true)
        alternativeCache.removeAll(keepingCapacity: true)
        readingIndex.removeAll(keepingCapacity: true)
        syllables.removeAll(keepingCapacity: true)
        syllableBases.removeAll(keepingCapacity: true)
        neutralBases.removeAll(keepingCapacity: true)
        entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
        readingIndex.reserveCapacity(lexicon.count)
        for reading in Set(lexicon.keys).union(phraseIndex.filter { !$0.value.isEmpty }.keys) {
            readingIndex[Self.withoutTones(reading), default: []].append(reading)
            for part in reading.split(separator: " ") {
                syllables.insert(String(part))
            }
        }
        for base in readingIndex.keys { addBasePrefixes(base) }
        for reading in learnedWords.keys { addPrefixes(reading) }
        for syllable in syllables {
            let base = Self.withoutTones(syllable)
            syllableBases.insert(base)
            if syllable.last == "˙" { neutralBases.insert(base) }
        }
        rebuildValidCodes()
    }

    private func rebuildValidCodes() {
        validCodes.removeAll(keepingCapacity: true)
        for reading in syllables.union(learnedSyllables) { validCodes[KeyChannel.code(for: reading)] = reading }
        for base in syllableBases { validCodes[KeyChannel.code(for: base + "˙")] = base + "˙" }
        for base in neutralBases {
            for tone in ["", "ˊ", "ˇ", "ˋ", "˙"] { validCodes[KeyChannel.code(for: base + tone)] = base + tone }
        }
    }

    private func validSyllable(_ reading: String) -> Bool {
        if syllables.contains(reading) || learnedSyllables.contains(reading) { return true }
        let base = Self.withoutTones(reading)
        return neutralBases.contains(base) || (reading.last == "˙" && syllableBases.contains(base))
    }

    func isValidSyllable(_ reading: String) -> Bool { synchronized { validSyllable(reading) } }

    /// The eight cheapest valid readings of actual typed keys, before strength scaling.
    public func typoAlternatives(for keys: String) -> [(reading: String, cost: Double)] {
        alternatives(for: keys).map { ($0.reading, $0.cost) }
    }

    func alternatives(for keys: String) -> [ReadingAlternative] {
        synchronized {
            if let cached = alternativeCache[keys] { return cached }
            let result = KeyChannel.alternatives(for: keys) { validCodes[$0] }
            if alternativeCache.count >= 4096 { alternativeCache.removeAll(keepingCapacity: true) }
            alternativeCache[keys] = result
            return result
        }
    }

    private func addPrefixes(_ reading: String) { addBasePrefixes(Self.withoutTones(reading)) }

    private func addBasePrefixes(_ base: String) {
        var prefix = ""
        for part in base.split(separator: " ") {
            prefix += (prefix.isEmpty ? "" : " ") + part
            readingPrefixes.insert(prefix)
        }
    }

    struct Match {
        var range: Range<Int>
        var typedReading: String
        var entry: WordEntry
        var annotation: String?
    }

    func matches(_ tokens: [CompositionToken], start: Int, typo: TypoTolerance) -> [Match] {
        var span: [CompositionToken] = []
        var key = typo.rawValue
        for token in tokens[start..<min(tokens.count, start + 6)] {
            guard case .syllable(_, let keys) = token else { break }
            span.append(token)
            key += "\t" + keys
        }
        let (cached, cacheRevision) = synchronized { () -> ([Match]?, UInt64) in
            expireCachesOnNewDay()
            return (matchCache[key], revision)
        }
        let matches: [Match]
        if let cached { matches = cached }
        else {
            matches = buildMatches(span, typo: typo)
            synchronized {
                guard cacheRevision == revision else { return }
                if matchCache.count >= 4096 { matchCache.removeAll(keepingCapacity: true) }
                matchCache[key] = matches
            }
        }
        return matches.compactMap {
            var match = $0
            match.range = (match.range.lowerBound + start)..<(match.range.upperBound + start)
            if match.entry.wholeOnly && match.range != 0..<tokens.count { return nil }
            return match
        }
    }

    private func buildMatches(_ tokens: [CompositionToken], typo: TypoTolerance) -> [Match] {
        let start = 0
        var options: [[ReadingAlternative]] = []
        var typed: [String] = []
        for token in tokens[start..<min(tokens.count, start + 6)] {
            guard case .syllable(let reading, let keys) = token else { break }
            typed.append(reading)
            options.append([ReadingAlternative(reading: reading, cost: 0)]
                + (typo == .off ? [] : alternatives(for: keys)))
        }
        var result: [Match] = []
        func visit(_ index: Int, _ reading: String, _ base: String, _ cost: Double, _ edits: Int) {
            guard index < options.count else { return }
            for option in options[index] {
                let count = edits + (option.cost > 0 ? 1 : 0)
                guard count <= 2 else { continue }
                let next = reading + (reading.isEmpty ? "" : " ") + option.reading
                let nextBase = base + (base.isEmpty ? "" : " ") + Self.withoutTones(option.reading)
                guard synchronized({ readingPrefixes.contains(nextBase) }) else { continue }
                let total = cost + option.cost * typo.costMultiplier
                if count <= (index < 3 ? 1 : 2) {
                    let typedReading = typed.prefix(index + 1).joined(separator: " ")
                    for var entry in entries(for: next) {
                        let annotation = total > 0 ? ZhuyinKeyboard.plain(entry.reading ?? next) : nil
                        entry.recalled = entry.recalled && typedReading == next
                        entry.penalty += total
                        result.append(Match(range: start..<(start + index + 1), typedReading: typedReading,
                                            entry: entry, annotation: annotation))
                    }
                }
                visit(index + 1, next, nextBase, total, count)
            }
        }
        visit(0, "", "", 0, 0)
        // The LM sees the same word ID on duplicate paths; retain its best channel/lexicon score once.
        var best: [PhraseKey: (Match, Double)] = [:]
        for match in result {
            let key = PhraseKey(reading: match.typedReading + (match.entry.wholeOnly ? "\u{1}" : ""), word: match.entry.word)
            let values = scoring(match.entry, reading: match.typedReading)
            let value = values.adjustment + (!match.entry.recalled && (languageModel.order == 1 || match.entry.wordID == nil) ? values.unigram : 0)
            if let old = best[key], old.1 > value || (old.1 == value && old.0.annotation == nil) { continue }
            best[key] = (match, value)
        }
        return best.values.map { $0.0 }.sorted {
            if $0.range.count != $1.range.count { return $0.range.count < $1.range.count }
            if $0.entry.penalty != $1.entry.penalty { return $0.entry.penalty < $1.entry.penalty }
            return $0.entry.word < $1.entry.word
        }
    }

    func scoring(_ entry: WordEntry, reading: String) -> (unigram: Double, adjustment: Double) {
        synchronized {
            let source = entry.reading ?? reading
            let phrase = phrases[PhraseKey(reading: source, word: entry.word)]
            let original = phrase == nil ? entry.logp
                : (lexicon[source]?.first(where: { $0.word == entry.word })?.logp ?? -7)
            return (original, (phrase.map { $0 - original } ?? 0) - entry.penalty)
        }
    }

    func score(_ entry: WordEntry, reading: String, history: LanguageModelHistory) -> Double {
        let values = scoring(entry, reading: reading)
        return (entry.recalled ? 0 : languageModel.score(wordID: entry.wordID, history: history, unigram: values.unigram)) + values.adjustment
    }

    public func userPhrases() -> [(reading: String, word: String)] {
        synchronized { phrases.keys.map { ($0.reading, $0.word) }.sorted { ($0.0, $0.1) < ($1.0, $1.1) } }
    }

    public func addUserPhrase(word: String, reading: String) throws {
        guard Self.validPhrase(word: word, reading: reading) else {
            throw NSError(domain: "ZhuyinCore", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "A phrase needs 1–6 characters and one valid Zhuyin reading per character."])
        }
        synchronized {
            let key = PhraseKey(reading: reading, word: word)
            phrases[key] = max(lexicon[reading]?.first(where: { $0.word == word })?.logp ?? -7, -7) + 1
            phraseIndex[reading, default: [:]][word] = WordEntry(word: word, logp: phrases[key]!, wordID: resolveID(word))
            revision &+= 1
            rebuildReadingIndex()
            scheduleWrite()
        }
    }

    public func removeUserPhrase(word: String, reading: String) {
        synchronized {
            phrases.removeValue(forKey: PhraseKey(reading: reading, word: word))
            phraseIndex[reading]?.removeValue(forKey: word)
            revision &+= 1
            rebuildReadingIndex()
            scheduleWrite()
        }
    }

    public func readings(of word: String) -> [String] {
        synchronized {
            let known = (reverse[word] ?? []).sorted { $0.1 > $1.1 }.map(\.0)
            let personal = phrases.keys.filter { $0.word == word }.map(\.reading).sorted()
            var seen = Set<String>()
            let result = (known + personal).filter { seen.insert($0).inserted }
            if !result.isEmpty { return result }
            var parts: [String] = []
            for character in word {
                guard let best = reverse[String(character)]?.max(by: { $0.1 < $1.1 }) else { return [] }
                parts.append(best.0)
            }
            return parts.isEmpty ? [] : [parts.joined(separator: " ")]
        }
    }

    /// What the engine remembers, one row per word or phrase: recalls and picked words of the personal model.
    public func learnedEntries() -> [(reading: String, word: String, count: Double)] {
        synchronized {
            let now = personal.clock()
            var rows: [PhraseKey: Double] = [:]
            for (key, value) in learned where value.isActive(at: now) { rows[key] = value.value(at: now) }
            for row in personal.pickedWords() {
                let key = PhraseKey(reading: row.reading, word: row.word)
                rows[key] = max(rows[key] ?? 0, row.count)
            }
            return rows.map { ($0.key.reading, $0.key.word, ($0.value * 10).rounded() / 10) }
                .sorted { ($0.0, $0.1) < ($1.0, $1.1) }
        }
    }

    public func forgetLearned(word: String, reading: String) {
        synchronized {
            learned.removeValue(forKey: PhraseKey(reading: reading, word: word))
            learnedWords[reading]?.remove(word)
            if let id = resolveID(word) { personal.forget(id) }
            learnedChanged()
        }
    }

    public func forgetAllLearned() {
        synchronized {
            learned.removeAll(); learnedWords.removeAll()
            personal.forgetAll()
            learnedChanged()
        }
    }

    /// Learns from one committed composition.  Literal words split it into stretches for the personal n-grams;
    /// corrections also become recalls of their exact typed readings.
    func learn(commit words: [CommittedWord], syllableCount: Int) {
        synchronized {
            let now = personal.clock()
            var stretch: [PersonalLanguageModel.Word] = []
            var syllables: [(reading: String, text: String, picked: Bool)] = []
            var wholeIsPlain = true
            func flush() {
                personal.learn(stretch)
                stretch.removeAll()
            }
            for word in words {
                guard !word.reading.isEmpty else {
                    flush(); wholeIsPlain = false
                    syllables.append(("", word.text, false))
                    continue
                }
                let id = (word.picked ? resolveID(word.text) : (word.wordID ?? resolveID(word.text)))!
                stretch.append(.init(id: id, text: word.text, reading: word.reading, picked: word.picked))
                let parts = word.reading.split(separator: " ").map(String.init)
                for (part, character) in zip(parts, word.text) { syllables.append((part, String(character), word.picked)) }
                guard word.picked else { continue }
                let lexical = lexicon[word.reading]?.contains(where: { $0.word == word.text }) == true
                    || phrases[PhraseKey(reading: word.reading, word: word.text)] != nil
                if !lexical {
                    let typo = reverse[word.text] != nil
                    remember(.init(reading: word.reading, word: word.text), kind: typo ? .typo : .phrase, at: now)
                }
            }
            flush()
            let picked = syllables.indices.filter { syllables[$0].picked }
            guard !picked.isEmpty else { return persistLearned() }
            // The whole composition, when it is short enough to be looked up as one unit.
            if wholeIsPlain, (1...6).contains(syllableCount), syllables.count == syllableCount {
                remember(.init(reading: syllables.map(\.reading).joined(separator: " "),
                               word: syllables.map(\.text).joined()), kind: .whole, at: now)
            }
            // Each correction with up to two syllables of context on its left (its right at the start).
            for index in picked {
                var lower = index, upper = index + 1
                while lower > 0, index - lower < 2, !syllables[lower - 1].reading.isEmpty { lower -= 1 }
                if lower == index {
                    while upper < syllables.count, upper - index < 3, !syllables[upper].reading.isEmpty { upper += 1 }
                }
                guard upper - lower >= 3, upper - lower < syllables.count || !wholeIsPlain || syllableCount > 6 else { continue }
                let window = syllables[lower..<upper]
                remember(.init(reading: window.map(\.reading).joined(separator: " "), word: window.map(\.text).joined()),
                         kind: .span, at: now)
            }
            persistLearned()
        }
    }

    /// The user picked something else where `text` was shown: weaken what made it the first choice.
    func weaken(text: String, reading: String, wordID: UInt32?, history: LanguageModelHistory?) {
        synchronized {
            let now = personal.clock()
            if var value = learned[PhraseKey(reading: reading, word: text)] {
                value.short.scale(0.5, at: now, halfLife: RecallValue.shortHalfLife)
                value.long.scale(0.5, at: now, halfLife: RecallValue.longHalfLife)
                value.picks = 0
                learned[PhraseKey(reading: reading, word: text)] = value
            }
            if let id = wordID ?? resolveID(text), let history { personal.weaken(word: id, history: history) }
            learnedChanged()
        }
    }

    // Called under lock.
    private func remember(_ key: PhraseKey, kind: RecallKind, at now: Double) {
        var value = learned[key] ?? RecallValue(kind: kind)
        value.short.add(1, at: now, halfLife: RecallValue.shortHalfLife)
        value.long.add(1, at: now, halfLife: RecallValue.longHalfLife)
        value.picks += 1
        learned[key] = value
        learnedWords[key.reading, default: []].insert(key.word)
        for part in key.reading.split(separator: " ") where !syllables.contains(String(part)) {
            learnedSyllables.insert(String(part))
        }
        addPrefixes(key.reading)
    }

    // Called under lock.
    private func persistLearned() {
        trimLearned(now: personal.clock())
        learnedChanged()
    }

    // Called under lock.
    private func learnedChanged() {
        learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
        rebuildValidCodes()
        alternativeCache.removeAll(keepingCapacity: true)
        entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
        revision &+= 1
        scheduleWrite()
    }

    private func trimLearned(now: Double) {
        guard learned.count > 20000 else { return }
        let keep = learned.map { (key: $0.key, value: $0.value, score: $0.value.value(at: now)) }.sorted {
            return $0.score == $1.score ? ($0.key.reading, $0.key.word) < ($1.key.reading, $1.key.word) : $0.score > $1.score
        }.prefix(20000)
        learned = Dictionary(uniqueKeysWithValues: keep.map { ($0.key, $0.value) })
        learnedWords.removeAll(keepingCapacity: true)
        for key in learned.keys { learnedWords[key.reading, default: []].insert(key.word) }
    }

    private static func validPhrase(word: String, reading: String) -> Bool {
        guard !word.contains(where: { $0.isWhitespace || $0.isNewline }), !word.isEmpty else { return false }
        let syllables = reading.components(separatedBy: " ")
        return (1...6).contains(syllables.count) && word.count == syllables.count
            && syllables.allSatisfy { syllable in
                var slots = Set<Int>()
                var lastSlot = -1
                var sawTone = false
                for symbol in syllable {
                    if "ˊˇˋ˙".contains(symbol) {
                        guard !slots.isEmpty, !sawTone, symbol == syllable.last else { return false }
                        sawTone = true
                    } else {
                        guard !sawTone, let slot = ZhuyinKeyboard.slot(of: symbol), slot > lastSlot,
                              slots.insert(slot).inserted else { return false }
                        lastSlot = slot
                    }
                }
                return !slots.isEmpty
            }
    }

    private func loadPersonalData() {
        guard let directory else { return }
        if let data = try? Data(contentsOf: directory.appendingPathComponent("user_phrases.tsv")) {
            scanTSV(data) { fields in
                guard fields.count == 2, Self.validPhrase(word: fields[1], reading: fields[0]) else { return }
                let base = lexicon[fields[0]]?.first(where: { $0.word == fields[1] })?.logp ?? -7
                phrases[PhraseKey(reading: fields[0], word: fields[1])] = max(base, -7) + 1
                phraseIndex[fields[0], default: [:]][fields[1]] = WordEntry(
                    word: fields[1], logp: max(base, -7) + 1, wordID: resolveID(fields[1]))
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent("learned.tsv")) {
            scanTSV(data) { fields in
                if fields.count == 4 {
                    // Earlier format: reading, word, count, last used.  Kept as whole-composition recalls.
                    guard Self.validPhrase(word: fields[1], reading: fields[0]),
                          let count = Double(fields[2]), count.isFinite, count > 0,
                          let date = Double(fields[3]), date.isFinite, date >= 0 else { return }
                    var value = RecallValue(kind: .whole)
                    value.long = DecayingCount(value: count, time: date)
                    value.picks = Int(count.rounded(.down))
                    learned[PhraseKey(reading: fields[0], word: fields[1])] = value
                    learnedWords[fields[0], default: []].insert(fields[1])
                    return
                }
                guard fields.count == 8, let kind = RecallKind(rawValue: fields[0]),
                      Self.validPhrase(word: fields[2], reading: fields[1]),
                      let shortValue = Double(fields[3]), let shortTime = Double(fields[4]),
                      let longValue = Double(fields[5]), let longTime = Double(fields[6]), let picks = Int(fields[7]),
                      [shortValue, shortTime, longValue, longTime].allSatisfy({ $0.isFinite && $0 >= 0 }) else { return }
                var value = RecallValue(kind: kind)
                value.short = DecayingCount(value: shortValue, time: shortTime)
                value.long = DecayingCount(value: longValue, time: longTime)
                value.picks = max(0, picks)
                learned[PhraseKey(reading: fields[1], word: fields[2])] = value
                learnedWords[fields[1], default: []].insert(fields[2])
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent(PersonalLanguageModel.fileName)) {
            personal.load(data) { resolveID($0)! }
        }
        learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
        trimLearned(now: personal.clock())
    }

    // Called under lock. Snapshots survive Engine teardown and are serialized before atomically replacing each file.
    private func scheduleWrite() {
        guard let directory else { return }
        pendingWrite?.cancel()
        let phraseSnapshot = phrases
        let learnedSnapshot = learned
        let personalText = personal.serialized()
        let work = DispatchWorkItem {
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let phraseText = phraseSnapshot.keys.sorted { ($0.reading, $0.word) < ($1.reading, $1.word) }
                    .map { "\($0.reading)\t\($0.word)\n" }.joined()
                let learnedText = learnedSnapshot.sorted { ($0.key.reading, $0.key.word) < ($1.key.reading, $1.key.word) }
                    .map { key, value in
                        [value.kind.rawValue, key.reading, key.word, "\(value.short.value)", "\(value.short.time)",
                         "\(value.long.value)", "\(value.long.time)", "\(value.picks)"].joined(separator: "\t") + "\n"
                    }.joined()
                try Data(phraseText.utf8).write(to: directory.appendingPathComponent("user_phrases.tsv"), options: .atomic)
                try Data(learnedText.utf8).write(to: directory.appendingPathComponent("learned.tsv"), options: .atomic)
                try Data(personalText.utf8).write(to: directory.appendingPathComponent(PersonalLanguageModel.fileName),
                                                  options: .atomic)
            } catch {
                NSLog("ZhuyinCore: could not save personal data: %@", error.localizedDescription)
            }
        }
        pendingWrite = work
        writer.asyncAfter(deadline: .now() + .milliseconds(150), execute: work)
    }
}
