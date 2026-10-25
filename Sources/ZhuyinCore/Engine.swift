import Foundation

struct WordEntry {
    var word: String
    var logp: Double
    var reading: String? = nil
    var penalty: Double = 0
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
    private var phraseIndex: [String: [String: Double]] = [:]
    private var readingIndex: [String: [String]] = [:]
    private var syllables = Set<String>()
    private var syllableBases = Set<String>()
    private var neutralBases = Set<String>()
    private var entryCache: [String: [WordEntry]] = [:]
    private var learned: [PhraseKey: LearnedValue] = [:]
    private let lock = NSLock()
    private let directory: URL?
    private let writer = DispatchQueue(label: "ZhuyinType.personal-data", qos: .utility)
    private var pendingWrite: DispatchWorkItem?
    let languageModel: any LanguageModel
    let usesBigram: Bool

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
        if let url = languageModelDirectory, let model = try? BigramLanguageModel(directory: url) {
            languageModel = model
            usesBigram = true
        } else {
            languageModel = UnigramLanguageModel()
            usesBigram = false
        }
        lexicon.reserveCapacity(50000)
        reverse.reserveCapacity(125000)
        scanTSV(data) { fields in
            guard fields.count == 3, let score = Double(fields[2]), score.isFinite,
                  !fields[0].isEmpty, !fields[1].isEmpty else { return }
            lexicon[fields[0], default: []].append(WordEntry(word: fields[1], logp: score))
            reverse[fields[1], default: []].append((fields[0], score))
        }
        loadPersonalData()
        rebuildReadingIndex()
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
                for (word, score) in phraseIndex[source] ?? [:] {
                    if let index = entries.firstIndex(where: { $0.word == word }) {
                        entries[index].logp = score
                    } else {
                        entries.append(WordEntry(word: word, logp: score))
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
            if entryCache.count >= 4096 { entryCache.removeAll(keepingCapacity: true) }
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
        readingIndex.removeAll(keepingCapacity: true)
        syllables.removeAll(keepingCapacity: true)
        syllableBases.removeAll(keepingCapacity: true)
        neutralBases.removeAll(keepingCapacity: true)
        entryCache.removeAll(keepingCapacity: true)
        readingIndex.reserveCapacity(lexicon.count)
        for reading in Set(lexicon.keys).union(phraseIndex.keys) {
            guard lexicon[reading] != nil || phraseIndex[reading]?.isEmpty == false else { continue }
            readingIndex[Self.withoutTones(reading), default: []].append(reading)
            for part in reading.split(separator: " ") {
                syllables.insert(String(part))
            }
        }
        for syllable in syllables {
            let base = Self.withoutTones(syllable)
            syllableBases.insert(base)
            if syllable.last == "˙" { neutralBases.insert(base) }
        }
    }

    func isValidSyllable(_ reading: String) -> Bool {
        synchronized {
            if syllables.contains(reading) { return true }
            let base = Self.withoutTones(reading)
            return neutralBases.contains(base) || (reading.last == "˙" && syllableBases.contains(base))
        }
    }

    func score(_ entry: WordEntry, reading: String, after previous: String) -> Double {
        synchronized {
            let key = PhraseKey(reading: reading, word: entry.word)
            let source = entry.reading ?? reading
            let phrase = phrases[PhraseKey(reading: source, word: entry.word)]
            let original = phrase == nil ? entry.logp
                : (lexicon[source]?.first(where: { $0.word == entry.word })?.logp ?? -7)
            let base = languageModel.score(word: entry.word, after: previous, unigram: original)
            return base + (phrase.map { $0 - original } ?? 0) - entry.penalty
                + (learned[key]?.boost(now: Date().timeIntervalSince1970) ?? 0)
        }
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
            phraseIndex[reading, default: [:]][word] = phrases[key]
            rebuildReadingIndex()
            scheduleWrite()
        }
    }

    public func removeUserPhrase(word: String, reading: String) {
        synchronized {
            phrases.removeValue(forKey: PhraseKey(reading: reading, word: word))
            phraseIndex[reading]?.removeValue(forKey: word)
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
            scheduleWrite()
        }
    }

    public func forgetAllLearned() {
        synchronized { learned.removeAll(); scheduleWrite() }
    }

    func learn(reading: String, word: String, amount: Double) {
        synchronized {
            let now = Date().timeIntervalSince1970
            let key = PhraseKey(reading: reading, word: word)
            learned[key] = LearnedValue(count: (learned[key]?.count ?? 0) + amount, lastUsed: now)
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
                phraseIndex[fields[0], default: [:]][fields[1]] = max(base, -7) + 1
            }
        }
        if let data = try? Data(contentsOf: directory.appendingPathComponent("learned.tsv")) {
            scanTSV(data) { fields in
                guard fields.count == 4, Self.validPhrase(word: fields[1], reading: fields[0]),
                      let count = Double(fields[2]), count.isFinite, count > 0,
                      let date = Double(fields[3]), date.isFinite, date >= 0 else { return }
                learned[PhraseKey(reading: fields[0], word: fields[1])] = LearnedValue(count: count, lastUsed: date)
            }
        }
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
