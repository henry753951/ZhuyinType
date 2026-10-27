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
    /// log P(this reading | word) against the word's most common reading (0 for it, below 0 for rarer ones), so the
    /// language model's word probability is not spent on a reading the word rarely has: 說 for ㄕㄨㄟˋ, 睡 is likelier.
    var readingPrior: Double = 0
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
    private let mappedLexicon: MappedLexicon?
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
    /// Words the user asked never to remember.
    private var excluded = Set<PhraseKey>()
    /// Personal hit areas: how often this user typed one key (first character) for another (second), learned from
    /// picked corrections.
    private var typoCounts: [String: Double] = [:]
    /// The most a personal slip is discounted, and how fast: 1.5 × ln(1 + times seen).
    static let typoDiscountCap = 4.0
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
    private var fuzzy: FuzzySounds = []
    /// Cost of one fuzzy-sound substitution, a little under a 3↔4 tone slip: whoever turns a pair on merges those
    /// sounds, so a clearly likelier word wins, while an equally likely exact match stays first.
    static let fuzzyCost = 3.0
    /// Sound pairs that match each other; none by default.
    public var fuzzySounds: FuzzySounds {
        get { synchronized { fuzzy } }
        set {
            synchronized {
                guard newValue != fuzzy else { return }
                fuzzy = newValue
                matchCache.removeAll(keepingCapacity: true)
                revision &+= 1
            }
        }
    }
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
        let binaryURL = lexiconURL.deletingPathExtension().appendingPathExtension("bin")
        if FileManager.default.fileExists(atPath: binaryURL.path) {
            let mapped = try MappedLexicon(url: binaryURL)
            self.init(data: Data(), mappedLexicon: mapped, userDataDirectory: userDataDirectory,
                      languageModelDirectory: languageModelDirectory)
        } else {
            self.init(data: try Data(contentsOf: lexiconURL), userDataDirectory: userDataDirectory,
                      languageModelDirectory: languageModelDirectory)
        }
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

    /// The chat-domain bigram next to the encyclopedic one (`lm_chat/` beside `lm/`), mixed by the text before the
    /// cursor; nil keeps the encyclopedic model alone.  Read when an Engine is created.
    nonisolated(unsafe) public static var chatModelDirectory: URL?
    /// How often the end of a composition is the end of a sentence (see `LanguageModel.endScore`).
    nonisolated(unsafe) public static var sentenceEndShare = 1.0
    /// The chat share with no text before the cursor, and how fast the text before it moves it (see `chatWeight`).
    nonisolated(unsafe) public static var chatPrior = 0.3
    nonisolated(unsafe) public static var domainEvidenceScale = 1.0
    nonisolated(unsafe) public static var domainEvidenceCentre = -2.5

    /// The mixture, when a chat model is loaded.
    private let mixture: MixtureLanguageModel?

    private init(data: Data, mappedLexicon: MappedLexicon? = nil, userDataDirectory: URL?, languageModelDirectory: URL?) {
        directory = userDataDirectory
        self.mappedLexicon = mappedLexicon
        var model: any LanguageModel
        var mixture: MixtureLanguageModel?
        func load(_ url: URL) -> (any VocabularyLanguageModel)? {
            if let ngram = try? NGramLanguageModel(directory: url) { return ngram }
            return try? BigramLanguageModel(directory: url)
        }
        if let url = languageModelDirectory, let encyclopedia = load(url) {
            model = encyclopedia
            if encyclopedia.order == 2, let chatURL = Engine.chatModelDirectory, let chat = load(chatURL), chat.order == 2 {
                let mixed = MixtureLanguageModel(chat: chat, encyclopedia: encyclopedia, chatWeight: Engine.chatPrior)
                mixture = mixed
                model = mixed
            }
        } else {
            model = UnigramLanguageModel()
        }
        self.mixture = mixture
        let base = Engine.sentenceBoundaries || model.order == 1 ? model : BoundaryFreeLanguageModel(base: model)
        languageModel = PersonalizedLanguageModel(base: base, personal: personal)
        if let mappedLexicon {
            nextPersonalID += UInt32(mappedLexicon.wordCount)
        } else {
            lexicon.reserveCapacity(50000)
            reverse.reserveCapacity(125000)
            scanTSV(data) { fields in
                guard fields.count == 3, let score = Double(fields[2]), score.isFinite,
                      !fields[0].isEmpty, !fields[1].isEmpty else { return }
                lexicon[fields[0], default: []].append(WordEntry(word: fields[1], logp: score, wordID: resolveID(fields[1])))
                reverse[fields[1], default: []].append((fields[0], score))
            }
            // The lexicon gives log P(word, reading); split it into the word's probability and the reading's share.
            var best: [String: Double] = [:]
            best.reserveCapacity(reverse.count)
            for (word, readings) in reverse where readings.count > 1 { best[word] = readings.map(\.1).max() }
            for index in lexicon.indices {
                for i in lexicon.values[index].indices {
                    guard let top = best[lexicon.values[index][i].word] else { continue }
                    lexicon.values[index][i].readingPrior = lexicon.values[index][i].logp - top
                    lexicon.values[index][i].logp = top
                }
            }
        }
        loadPersonalData()
        rebuildReadingIndex()
    }

    /// Every word gets an ID: the language model's, or a personal one above its vocabulary, so the user's statistics
    /// can follow words the model does not know.
    private func resolveID(_ word: String) -> UInt32? {
        if let index = mappedLexicon?.wordIndex(word) {
            return languageModel.wordID(for: word) ?? (0x4000_0000 + UInt32(index))
        }
        if let id = wordIDs[word] { return id }
        let id = languageModel.wordID(for: word) ?? {
            defer { nextPersonalID += 1 }
            return nextPersonalID
        }()
        wordIDs[word] = id
        return id
    }

    private func lexicalEntries(_ reading: String) -> [WordEntry] {
        guard let mappedLexicon else { return lexicon[reading] ?? [] }
        return mappedLexicon.entries(for: reading).map {
            var entry = $0
            entry.wordID = resolveID(entry.word)
            return entry
        }
    }

    private func knownReadings(_ word: String) -> [(String, Double)] {
        mappedLexicon?.readings(of: word) ?? reverse[word] ?? []
    }

    private func bestReading(_ word: String) -> (String, Double)? {
        if let mappedLexicon { return mappedLexicon.bestReading(of: word) }
        return reverse[word]?.max(by: { $0.1 < $1.1 })
    }

    private func knowsWord(_ word: String) -> Bool {
        if let mappedLexicon { return mappedLexicon.wordIndex(word) != nil }
        return reverse[word] != nil
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
            var position: [String: Int] = [:]
            let base = Self.withoutTones(reading)
            let alternatives = Set((mappedLexicon?.readings(forBase: base) ?? []) + (readingIndex[base] ?? []))
            let readings = [reading] + alternatives.filter { $0 != reading }.sorted()
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
                var entries = lexicalEntries(source)
                // By word: the Dictionary's order would change the entries' order from run to run.
                for (word, phrase) in (phraseIndex[source] ?? [:]).sorted(by: { $0.key.utf8.lexicographicallyPrecedes($1.key.utf8) }) {
                    if let index = entries.firstIndex(where: { $0.word == word }) {
                        entries[index].logp = phrase.logp
                        entries[index].readingPrior = 0
                        entries[index].recalled = source == reading
                    } else {
                        var entry = phrase
                        entry.recalled = source == reading
                        entries.append(entry)
                    }
                }
                // One entry per word: the reading that costs least, so a particle typed without its neutral tone
                // (吧 as ㄅㄚ) uses ㄅㄚ˙ rather than the rare first-tone reading.
                for var entry in entries {
                    entry.reading = source
                    entry.penalty = penalty
                    if let index = position[entry.word] {
                        let old = result[index]
                        guard !old.recalled, entry.readingPrior - penalty > old.readingPrior - old.penalty else { continue }
                        result[index] = entry
                    } else {
                        position[entry.word] = result.count
                        result.append(entry)
                    }
                }
            }
            let now = personal.clock()
            for word in (learnedWords[reading] ?? []).sorted(by: { $0.utf8.lexicographicallyPrecedes($1.utf8) }) {
                guard let value = learned[PhraseKey(reading: reading, word: word)], value.isActive(at: now) else { continue }
                let source = bestReading(word)
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
        syllables.formUnion(mappedLexicon?.syllables ?? [])
        for syllable in syllables {
            let base = Self.withoutTones(syllable)
            syllableBases.insert(base)
            if syllable.last == "˙" { neutralBases.insert(base) }
        }
        rebuildValidCodes()
    }

    /// Three steps, each overriding the codes of the one before.  Within a step, two readings with one code (never
    /// for canonical readings) resolve to the greater in UTF-8 order, whatever order the sets are visited in.
    private func rebuildValidCodes() {
        validCodes.removeAll(keepingCapacity: true)
        func assign<S: Sequence>(_ readings: S) where S.Element == String {
            var step: [Int: String] = [:]
            for reading in readings {
                let code = KeyChannel.code(for: reading)
                if let old = step[code], !old.utf8.lexicographicallyPrecedes(reading.utf8) { continue }
                step[code] = reading
            }
            validCodes.merge(step) { $1 }
        }
        assign(syllables.union(learnedSyllables))
        assign(syllableBases.lazy.map { $0 + "˙" })
        assign(neutralBases.lazy.flatMap { base in ["", "ˊ", "ˇ", "ˋ", "˙"].lazy.map { base + $0 } })
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
            let counts = typoCounts
            let result = KeyChannel.alternatives(for: keys, discount: { typed, intended in
                guard let n = counts[String(Character(UnicodeScalar(typed))) + String(Character(UnicodeScalar(intended)))] else { return 0 }
                return min(Self.typoDiscountCap, 1.5 * log(1 + n))
            }) { validCodes[$0] }
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
        var key = typo.rawValue + "\(fuzzySounds.rawValue)"
        for token in tokens[start..<min(tokens.count, start + 6)] {
            guard case .syllable(_, let keys, _) = token else { break }
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
        let fuzzy = fuzzySounds
        var options: [[ReadingAlternative]] = []
        var typed: [String] = []
        for token in tokens[start..<min(tokens.count, start + 6)] {
            guard case .syllable(let reading, let keys, _) = token else { break }
            typed.append(reading)
            var cheapest: [String: ReadingAlternative] = [reading: ReadingAlternative(reading: reading, cost: 0)]
            for option in Self.fuzzyAlternatives(for: reading, fuzzy) + (typo == .off ? [] : alternatives(for: keys))
            where option.cost < cheapest[option.reading]?.cost ?? .infinity {
                cheapest[option.reading] = option
            }
            options.append(cheapest.values.sorted { $0.cost == $1.cost ? $0.reading < $1.reading : $0.cost < $1.cost })
        }
        var result: [Match] = []
        func visit(_ index: Int, _ reading: String, _ base: String, _ cost: Double, _ edits: Int) {
            guard index < options.count else { return }
            for option in options[index] {
                let count = edits + (option.cost > 0 && !option.fuzzy ? 1 : 0)
                guard count <= 2 else { continue }
                let next = reading + (reading.isEmpty ? "" : " ") + option.reading
                let nextBase = base + (base.isEmpty ? "" : " ") + Self.withoutTones(option.reading)
                guard synchronized({ (readingPrefixes.contains(nextBase) || mappedLexicon?.containsPrefix(nextBase) == true) }) else { continue }
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
        // A word is kept once per span and recall kind, so these keys order every match.
        return best.values.map { $0.0 }.sorted {
            if $0.range.count != $1.range.count { return $0.range.count < $1.range.count }
            if $0.entry.penalty != $1.entry.penalty { return $0.entry.penalty < $1.entry.penalty }
            if $0.entry.word != $1.entry.word { return $0.entry.word < $1.entry.word }
            return !$0.entry.wholeOnly && $1.entry.wholeOnly
        }
    }

    /// The readings `reading` also matches under `fuzzy`; invalid ones are pruned by the reading prefixes.
    static func fuzzyAlternatives(for reading: String, _ fuzzy: FuzzySounds) -> [ReadingAlternative] {
        guard !fuzzy.isEmpty else { return [] }
        var variants: [([Character], Int)] = [(Array(reading), 0)]
        for pair in FuzzySounds.pairs where fuzzy.contains(pair.set) {
            for (symbols, swaps) in variants {
                let index = pair.initial ? symbols.indices.first
                    : symbols.lastIndex { !"ˊˇˋ˙".contains($0) }
                guard let index, symbols[index] == pair.a || symbols[index] == pair.b else { continue }
                var swapped = symbols
                swapped[index] = symbols[index] == pair.a ? pair.b : pair.a
                variants.append((swapped, swaps + 1))
            }
        }
        return variants.dropFirst().map {
            ReadingAlternative(reading: String($0.0), cost: Double($0.1) * fuzzyCost, fuzzy: true)
        }
    }

    func scoring(_ entry: WordEntry, reading: String) -> (unigram: Double, adjustment: Double) {
        synchronized {
            let source = entry.reading ?? reading
            let phrase = phrases[PhraseKey(reading: source, word: entry.word)]
            let original = phrase == nil ? entry.logp
                : (lexicalEntries(source).first(where: { $0.word == entry.word })?.logp ?? -7)
            return (original, (phrase.map { $0 - original } ?? 0) - entry.penalty + entry.readingPrior)
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
            addPhrase(word: word, reading: reading)
            revision &+= 1
            rebuildReadingIndex()
            scheduleWrite()
        }
    }

    // Called under lock.
    private func addPhrase(word: String, reading: String) {
        let key = PhraseKey(reading: reading, word: word)
        phrases[key] = max(lexicalEntries(reading).first(where: { $0.word == word })?.logp ?? -7, -7) + 1
        phraseIndex[reading, default: [:]][word] = WordEntry(word: word, logp: phrases[key]!, wordID: resolveID(word))
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
            let known = knownReadings(word).sorted { $0.1 > $1.1 }.map(\.0)
            let personal = phrases.keys.filter { $0.word == word }.map(\.reading).sorted()
            var seen = Set<String>()
            let result = (known + personal).filter { seen.insert($0).inserted }
            if !result.isEmpty { return result }
            var parts: [String] = []
            for character in word {
                guard let best = bestReading(String(character)) else { return [] }
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

    /// Sets the chat share of the language model mixture from the text before the cursor: the prior, moved by how much
    /// likelier the chat model finds that text than the encyclopedic one, per word,
    ///     logit λ = logit(prior) + scale × (mean log P_chat(w) − log P_encyclopedia(w) − centre).
    /// No text keeps the prior.  Returns the share, or nil without a mixture.
    @discardableResult
    public func adaptDomain(to text: String) -> Double? {
        guard let mixture else { return nil }
        let words = segmentWords(text)
        var weight = Engine.chatPrior
        if !words.isEmpty {
            var total = 0.0
            var previous: UInt32 = 1
            for word in words {
                let id = mixture.wordID(for: word)
                let (c, e) = mixture.scores(wordID: id, previous: previous, unigram: -14)
                total += c - e
                previous = id ?? 2
            }
            let evidence = Engine.domainEvidenceScale * (total / Double(words.count) - Engine.domainEvidenceCentre)
            let prior = log(Engine.chatPrior / (1 - Engine.chatPrior))
            weight = 1 / (1 + exp(-(prior + evidence * min(1, Double(words.count) / 4))))
        }
        if abs(mixture.chatWeight - weight) > 0.01 {
            mixture.chatWeight = weight
            synchronized {
                entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
                revision &+= 1
            }
        }
        return weight
    }

    /// Han runs of `text` split into dictionary words by longest match (for scoring, not display).
    private func segmentWords(_ text: String) -> [String] {
        var words: [String] = []
        let characters = Array(text)
        var index = 0
        while index < characters.count {
            guard Self.isHan(characters[index]) else { index += 1; continue }
            var length = min(6, characters.count - index)
            while length > 1 && (characters[index..<(index + length)].contains { !Self.isHan($0) }
                                 || synchronized({ !knowsWord(String(characters[index..<(index + length)])) })) {
                length -= 1
            }
            words.append(String(characters[index..<(index + length)]))
            index += length
        }
        return words
    }

    /// The last words of `text` (at most two, within its final run of Han characters) by longest dictionary match from
    /// the end, each with its most common reading and the LM history after it.
    func trailingWords(of text: String) -> [ConvertedWord] {
        let run = Array(text.reversed().prefix(while: Self.isHan).reversed().suffix(12))
        guard !run.isEmpty else { return [] }
        return synchronized {
            var words: [String] = []
            var end = run.count
            while end > 0 && words.count < 2 {
                var length = min(6, end)
                while length > 1 && !knowsWord(String(run[(end - length)..<end])) { length -= 1 }
                words.insert(String(run[(end - length)..<end]), at: 0)
                end -= length
            }
            var history = LanguageModelHistory()
            return words.map { word in
                let id = resolveID(word)
                history = history.appending(id, order: languageModel.order)
                let reading = bestReading(word)?.0 ?? ""
                return ConvertedWord(range: 0..<0, text: word, reading: reading, wordID: id, history: history)
            }
        }
    }

    private static func isHan(_ character: Character) -> Bool {
        character.unicodeScalars.allSatisfy {
            (0x3400...0x9FFF).contains($0.value) || (0xF900...0xFAFF).contains($0.value) || (0x20000...0x3134F).contains($0.value)
        }
    }

    /// A correction was picked: the keys typed for each syllable against the reading meant.  Each key typed for
    /// another makes that slip cheaper next time.
    public func recordTypos(_ syllables: [(typedKeys: String, intendedReading: String)]) {
        synchronized {
            var changed = false
            for syllable in syllables {
                guard let intended = KeyChannel.keys(for: syllable.intendedReading) else { continue }
                for pair in KeyChannel.substitutions(typed: syllable.typedKeys, intended: intended) {
                    typoCounts[String(pair.typed) + String(pair.intended), default: 0] += 1
                    changed = true
                }
            }
            guard changed else { return }
            alternativeCache.removeAll(keepingCapacity: true)
            entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
            revision &+= 1
            scheduleWrite()
        }
    }

    /// Forgets `word` and never learns it again, until `includeInLearning`.
    public func excludeFromLearning(word: String, reading: String) {
        _ = synchronized { excluded.insert(PhraseKey(reading: reading, word: word)) }
        forgetLearned(word: word, reading: reading)
    }

    public func includeInLearning(word: String, reading: String) {
        synchronized {
            excluded.remove(PhraseKey(reading: reading, word: word))
            scheduleWrite()
        }
    }

    public func excludedEntries() -> [(reading: String, word: String)] {
        synchronized { excluded.map { ($0.reading, $0.word) }.sorted { ($0.0, $0.1) < ($1.0, $1.1) } }
    }

    /// Everything personal in one text file: user phrases, what was learned, and the words never to learn.
    public func exportPersonalData() -> Data {
        synchronized {
            var text = "# ZhuyinType personal data 1\n[phrases]\n" + Self.phraseText(phrases.keys)
            text += "[learned]\n" + Self.learnedText(learned)
            text += "[ngrams]\n" + personal.serialized()
            text += "[excluded]\n" + Self.phraseText(excluded)
            text += "[typos]\n" + Self.typoText(typoCounts)
            return Data(text.utf8)
        }
    }

    /// Adds an export to what is here; for anything both have, the stronger memory stays.  Returns the rows read.
    @discardableResult
    public func importPersonalData(_ data: Data) throws -> Int {
        guard let text = String(data: data, encoding: .utf8), text.hasPrefix("# ZhuyinType personal data 1") else {
            throw NSError(domain: "ZhuyinCore", code: 2,
                          userInfo: [NSLocalizedDescriptionKey: "This file is not a ZhuyinType personal data export."])
        }
        var sections: [String: String] = [:]
        var current = ""
        for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
            if line.hasPrefix("[") && line.hasSuffix("]") { current = String(line.dropFirst().dropLast()); continue }
            if line.hasPrefix("#") || line.isEmpty { continue }
            sections[current, default: ""] += line + "\n"
        }
        return synchronized {
            var rows = 0
            scanTSV(Data((sections["phrases"] ?? "").utf8)) { fields in
                guard fields.count == 2, Self.validPhrase(word: fields[1], reading: fields[0]) else { return }
                addPhrase(word: fields[1], reading: fields[0])
                rows += 1
            }
            scanTSV(Data((sections["learned"] ?? "").utf8)) { fields in
                guard let (key, value) = Self.learnedRow(fields) else { return }
                let now = personal.clock()
                if let old = learned[key], old.value(at: now) >= value.value(at: now) { return }
                learned[key] = value
                learnedWords[key.reading, default: []].insert(key.word)
                rows += 1
            }
            rows += personal.load(Data((sections["ngrams"] ?? "").utf8), merging: true) { resolveID($0)! }
            scanTSV(Data((sections["typos"] ?? "").utf8)) { fields in
                guard fields.count == 3, fields[0].count == 1, fields[1].count == 1, let n = Double(fields[2]), n > 0 else { return }
                typoCounts[fields[0] + fields[1]] = max(typoCounts[fields[0] + fields[1]] ?? 0, n)
                rows += 1
            }
            alternativeCache.removeAll(keepingCapacity: true)
            scanTSV(Data((sections["excluded"] ?? "").utf8)) { fields in
                guard fields.count == 2 else { return }
                excluded.insert(PhraseKey(reading: fields[0], word: fields[1]))
                rows += 1
            }
            revision &+= 1
            rebuildReadingIndex()
            trimLearned(now: personal.clock())
            learnedChanged()
            return rows
        }
    }

    public func forgetAllLearned() {
        synchronized {
            learned.removeAll(); learnedWords.removeAll()
            typoCounts.removeAll()
            alternativeCache.removeAll(keepingCapacity: true)
            personal.forgetAll()
            learnedChanged()
        }
    }

    /// Learns from one committed composition.  Literal words split it into stretches for the personal n-grams;
    /// corrections also become recalls of their exact typed readings.
    /// `context` holds up to two words committed just before, which the personal n-grams continue from.
    func learn(commit words: [CommittedWord], syllableCount: Int, context: [CommittedWord] = []) {
        synchronized {
            let now = personal.clock()
            var stretch: [PersonalLanguageModel.Word] = context.compactMap { word in
                resolveID(word.text).map { .init(id: word.wordID ?? $0, text: word.text, reading: word.reading, picked: false) }
            }
            var leading = stretch.count
            var syllables: [(reading: String, text: String, picked: Bool)] = []
            var wholeIsPlain = true
            func flush() {
                personal.learn(stretch, context: leading)
                stretch.removeAll()
                leading = 0
            }
            for word in words {
                guard !word.reading.isEmpty, !excluded.contains(PhraseKey(reading: word.reading, word: word.text)) else {
                    flush(); wholeIsPlain = false
                    syllables.append(("", word.text, false))
                    continue
                }
                let id = (word.picked ? resolveID(word.text) : (word.wordID ?? resolveID(word.text)))!
                stretch.append(.init(id: id, text: word.text, reading: word.reading, picked: word.picked))
                let parts = word.reading.split(separator: " ").map(String.init)
                for (part, character) in zip(parts, word.text) { syllables.append((part, String(character), word.picked)) }
                guard word.picked else { continue }
                let lexical = lexicalEntries(word.reading).contains(where: { $0.word == word.text })
                    || phrases[PhraseKey(reading: word.reading, word: word.text)] != nil
                if !lexical {
                    let typo = knowsWord(word.text)
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

    /// The user picked this word in this place before (after the same word, or a phrase anywhere): the neural
    /// reranker must not replace it.
    func remembers(_ word: ConvertedWord, before history: LanguageModelHistory) -> Bool {
        synchronized {
            let phrase = word.text.count > 1
            if phrase, (learned[PhraseKey(reading: word.reading, word: word.text)]?.picks ?? 0) > 0 { return true }
            let id = word.wordID ?? languageModel.wordID(for: word.text) ?? wordIDs[word.text]
                ?? mappedLexicon?.wordIndex(word.text).map { 0x4000_0000 + UInt32($0) }
            guard let id else { return false }
            return personal.remembers(id, before: history, phrase: phrase)
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
                addPhrase(word: fields[1], reading: fields[0])
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent("typos.tsv")) {
            scanTSV(data) { fields in
                guard fields.count == 3, fields[0].count == 1, fields[1].count == 1, let n = Double(fields[2]), n > 0 else { return }
                typoCounts[fields[0] + fields[1]] = n
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent("excluded.tsv")) {
            scanTSV(data) { fields in
                if fields.count == 2 { excluded.insert(PhraseKey(reading: fields[0], word: fields[1])) }
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
                guard let (key, value) = Self.learnedRow(fields) else { return }
                learned[key] = value
                learnedWords[key.reading, default: []].insert(key.word)
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent(PersonalLanguageModel.fileName)) {
            personal.load(data) { resolveID($0)! }
        }
        learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
        trimLearned(now: personal.clock())
    }

    private static func phraseText<S: Sequence>(_ keys: S) -> String where S.Element == PhraseKey {
        keys.sorted { ($0.reading, $0.word) < ($1.reading, $1.word) }.map { "\($0.reading)\t\($0.word)\n" }.joined()
    }

    private static func typoText(_ counts: [String: Double]) -> String {
        counts.sorted { $0.key < $1.key }.map { key, n in
            "\(key.first!)\t\(key.last!)\t\(n)\n"
        }.joined()
    }

    private static func learnedText(_ learned: [PhraseKey: RecallValue]) -> String {
        learned.sorted { ($0.key.reading, $0.key.word) < ($1.key.reading, $1.key.word) }.map { key, value in
            [value.kind.rawValue, key.reading, key.word, "\(value.short.value)", "\(value.short.time)",
             "\(value.long.value)", "\(value.long.time)", "\(value.picks)"].joined(separator: "\t") + "\n"
        }.joined()
    }

    /// One row of learned.tsv: kind, reading, word, short value and time, long value and time, picks.
    private static func learnedRow(_ fields: [String]) -> (PhraseKey, RecallValue)? {
        guard fields.count == 8, let kind = RecallKind(rawValue: fields[0]),
              validPhrase(word: fields[2], reading: fields[1]),
              let shortValue = Double(fields[3]), let shortTime = Double(fields[4]),
              let longValue = Double(fields[5]), let longTime = Double(fields[6]), let picks = Int(fields[7]),
              [shortValue, shortTime, longValue, longTime].allSatisfy({ $0.isFinite && $0 >= 0 }) else { return nil }
        var value = RecallValue(kind: kind)
        value.short = DecayingCount(value: shortValue, time: shortTime)
        value.long = DecayingCount(value: longValue, time: longTime)
        value.picks = max(0, picks)
        return (PhraseKey(reading: fields[1], word: fields[2]), value)
    }

    /// Writes pending personal data now: before another Engine reads the same directory, or before the process exits.
    public func flushPersonalData() {
        guard let work = synchronized({ pendingWrite }) else { return }
        // The scheduled copy may run as well; it writes the same snapshot.
        writer.sync { if !work.isCancelled { work.perform() } }
    }

    // Called under lock. Snapshots survive Engine teardown and are serialized before atomically replacing each file.
    private func scheduleWrite() {
        guard let directory else { return }
        pendingWrite?.cancel()
        let phraseText = Self.phraseText(phrases.keys)
        let learnedText = Self.learnedText(learned)
        let excludedText = Self.phraseText(excluded)
        let typoText = Self.typoText(typoCounts)
        let personalText = personal.serialized()
        let work = DispatchWorkItem {
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                try Data(phraseText.utf8).write(to: directory.appendingPathComponent("user_phrases.tsv"), options: .atomic)
                try Data(learnedText.utf8).write(to: directory.appendingPathComponent("learned.tsv"), options: .atomic)
                try Data(excludedText.utf8).write(to: directory.appendingPathComponent("excluded.tsv"), options: .atomic)
                try Data(typoText.utf8).write(to: directory.appendingPathComponent("typos.tsv"), options: .atomic)
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
