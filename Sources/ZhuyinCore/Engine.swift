import Foundation

struct WordEntry {
    var word: String
    var logp: Double
    var reading: String? = nil
    var penalty: Double = 0
    // A user-confirmed correction is an exact typed-reading recall on subsequent input.
    var recalled = false
    var wordID: UInt32? = nil
}

struct PhraseKey: Hashable {
    var reading: String
    var word: String
}

struct LearnedValue {
    var count: Double
    var lastUsed: Double
    func boost(now: Double) -> Double {
        2 * log1p(count * pow(0.5, max(0, now - lastUsed) / (90 * 86400)))
    }
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
    private var learned: [PhraseKey: LearnedValue] = [:]
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

    private init(data: Data, userDataDirectory: URL?, languageModelDirectory: URL?) {
        directory = userDataDirectory
        if let url = languageModelDirectory, let model = try? NGramLanguageModel(directory: url) {
            languageModel = model
        } else if let url = languageModelDirectory, let model = try? BigramLanguageModel(directory: url) {
            languageModel = model
        } else {
            languageModel = UnigramLanguageModel()
        }
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

    private func resolveID(_ word: String) -> UInt32? {
        guard languageModel.order > 1 else { return nil }
        if let id = wordIDs[word] { return id == UInt32.max ? nil : id }
        let id = languageModel.wordID(for: word)
        wordIDs[word] = id ?? UInt32.max
        return id
    }

    private func synchronized<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }

    func entries(for reading: String) -> [WordEntry] {
        synchronized {
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
            for word in learnedWords[reading] ?? [] where !result.contains(where: { $0.word == word }) {
                let source = reverse[word]?.max(by: { $0.1 < $1.1 })
                result.append(WordEntry(word: word, logp: source?.1 ?? -7, recalled: true, wordID: resolveID(word)))
            }
            if entryCache.count >= 4096 { entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true) }
            entryCache[reading] = result
            return result
        }
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
        let (cached, cacheRevision) = synchronized { (matchCache[key], revision) }
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
        return matches.map {
            var match = $0
            match.range = (match.range.lowerBound + start)..<(match.range.upperBound + start)
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
            let key = PhraseKey(reading: match.typedReading, word: match.entry.word)
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
            let key = PhraseKey(reading: reading, word: entry.word)
            let source = entry.reading ?? reading
            let phrase = phrases[PhraseKey(reading: source, word: entry.word)]
            let original = phrase == nil ? entry.logp
                : (lexicon[source]?.first(where: { $0.word == entry.word })?.logp ?? -7)
            return (original, (phrase.map { $0 - original } ?? 0) - entry.penalty
                + (learned[key]?.boost(now: Date().timeIntervalSince1970) ?? 0))
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

    public func learnedEntries() -> [(reading: String, word: String, count: Double)] {
        synchronized {
            learned.map { ($0.key.reading, $0.key.word, $0.value.count) }.sorted { ($0.0, $0.1) < ($1.0, $1.1) }
        }
    }

    public func forgetLearned(word: String, reading: String) {
        synchronized {
            learned.removeValue(forKey: PhraseKey(reading: reading, word: word))
            learnedWords[reading]?.remove(word)
            learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
            rebuildValidCodes()
            alternativeCache.removeAll(keepingCapacity: true)
            entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
            revision &+= 1
            scheduleWrite()
        }
    }

    public func forgetAllLearned() {
        synchronized {
            guard !learned.isEmpty else { return }
            learned.removeAll(); learnedWords.removeAll(); learnedSyllables.removeAll()
            rebuildValidCodes()
            entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
            alternativeCache.removeAll(keepingCapacity: true)
            revision &+= 1
            scheduleWrite()
        }
    }

    func learn(reading: String, word: String, amount: Double) {
        synchronized {
            let now = Date().timeIntervalSince1970
            let key = PhraseKey(reading: reading, word: word)
            learned[key] = LearnedValue(count: (learned[key]?.count ?? 0) + amount, lastUsed: now)
            learnedWords[reading, default: []].insert(word)
            for part in reading.split(separator: " ") {
                let part = String(part)
                learnedSyllables.insert(part)
                validCodes[KeyChannel.code(for: part)] = part
            }
            alternativeCache.removeAll(keepingCapacity: true)
            entryCache.removeAll(keepingCapacity: true); matchCache.removeAll(keepingCapacity: true)
            addPrefixes(reading)
            revision &+= 1
            trimLearned(now: now)
            scheduleWrite()
        }
    }

    private func trimLearned(now: Double) {
        guard learned.count > 20000 else { return }
        let keep = learned.map { (key: $0.key, value: $0.value, boost: $0.value.boost(now: now)) }.sorted {
            return $0.boost == $1.boost ? ($0.key.reading, $0.key.word) < ($1.key.reading, $1.key.word) : $0.boost > $1.boost
        }.prefix(20000)
        learned = Dictionary(uniqueKeysWithValues: keep.map { ($0.key, $0.value) })
        learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
        learnedWords.removeAll(keepingCapacity: true)
        for key in learned.keys { learnedWords[key.reading, default: []].insert(key.word) }
        rebuildValidCodes()
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
                guard fields.count == 4, Self.validPhrase(word: fields[1], reading: fields[0]),
                      let count = Double(fields[2]), count.isFinite, count > 0,
                      let date = Double(fields[3]), date.isFinite, date >= 0 else { return }
                learned[PhraseKey(reading: fields[0], word: fields[1])] = LearnedValue(count: count, lastUsed: date)
                learnedWords[fields[0], default: []].insert(fields[1])
            }
        }
        learnedSyllables = Set(learned.keys.flatMap { $0.reading.split(separator: " ").map(String.init) })
        trimLearned(now: Date().timeIntervalSince1970)
    }

    // Called under lock. Snapshots survive Engine teardown and are serialized before atomically replacing each file.
    private func scheduleWrite() {
        guard let directory else { return }
        pendingWrite?.cancel()
        let phraseSnapshot = phrases
        let learnedSnapshot = learned
        let work = DispatchWorkItem {
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let phraseText = phraseSnapshot.keys.sorted { ($0.reading, $0.word) < ($1.reading, $1.word) }
                    .map { "\($0.reading)\t\($0.word)\n" }.joined()
                let learnedText = learnedSnapshot.sorted { ($0.key.reading, $0.key.word) < ($1.key.reading, $1.key.word) }
                    .map { "\($0.key.reading)\t\($0.key.word)\t\($0.value.count)\t\($0.value.lastUsed)\n" }.joined()
                try Data(phraseText.utf8).write(to: directory.appendingPathComponent("user_phrases.tsv"), options: .atomic)
                try Data(learnedText.utf8).write(to: directory.appendingPathComponent("learned.tsv"), options: .atomic)
            } catch {
                NSLog("ZhuyinCore: could not save personal data: %@", error.localizedDescription)
            }
        }
        pendingWrite = work
        writer.asyncAfter(deadline: .now() + .milliseconds(150), execute: work)
    }
}
