import Foundation

/// A count that halves every `halfLife` seconds.
struct DecayingCount: Equatable {
    var value: Double = 0
    var time: Double = 0

    func at(_ now: Double, halfLife: Double) -> Double {
        value == 0 ? 0 : value * pow(0.5, max(0, now - time) / halfLife)
    }

    mutating func add(_ amount: Double, at now: Double, halfLife: Double) {
        value = at(now, halfLife: halfLife) + amount
        time = now
    }

    mutating func scale(_ factor: Double, at now: Double, halfLife: Double) {
        value = at(now, halfLife: halfLife) * factor
        time = now
    }
}

/// The user's own word statistics, learned from what they commit, used to adapt the base language model.
///
/// Every commit adds its word sequence to unigram, bigram and trigram counts in two layers: a short-term layer that
/// forgets within a day (what is being written about these days) and a long-term layer that forgets over months (habits;
/// it grows at most once per day per n-gram unless the user picked the words, and n-grams picked three times keep a
/// floor).  Scoring shrinks the base probability towards the personal estimate one order at a time:
///
///     p ← (c(h, w) + κₙ · p) / (C(h) + κₙ)        for n = 1, 2, 3
///
/// so the personal evidence counts in proportion to how much of it exists for that exact history.  κ is large for short
/// histories and small for long ones, and a pick weighs most on the longest n-gram: one correction settles its own
/// context at once but barely moves the same words elsewhere, while habits seen in many contexts generalise.
final class PersonalLanguageModel: @unchecked Sendable {
    struct Options {
        /// Weight of a picked word on the unigram, bigram and trigram that end in or contain it.
        var pickWeights: [Double] = [0.1, 0.3, 1.0]
        /// Weight of text accepted without picking, on every order.
        var passiveWeight = 0.05
        /// Prior strength per order (unigram, bigram, trigram).
        var kappa: [Double] = [1000, 100, 1]
        var shortHalfLife: Double = 8 * 3600
        var longHalfLife: Double = 90 * 86400
        var floorPicks = 3
        var floorValue = 1.0
        var capacity = 50_000
    }

    struct Key: Hashable {
        var words: [UInt32]
    }

    struct Entry {
        var texts: [String]
        var readings: [String]
        var short = DecayingCount()
        var long = DecayingCount()
        var picks = 0
        var firstDay = Int.max
        var longDay = -1
        var longDayAmount = 0.0
    }

    var options = Options()
    var clock: () -> Double = { Date().timeIntervalSince1970 }
    private(set) var entries: [Key: Entry] = [:]
    private var totals: [Key: (short: DecayingCount, long: DecayingCount)] = [:]
    private let lock = NSLock()

    var isEmpty: Bool { lock.withLock { entries.isEmpty } }
    /// Whether anything has been learned; read without the lock on every decoding step (a stale read only delays the
    /// switch to two-word histories by one keystroke).
    private(set) var hasData = false

    private func day(_ time: Double) -> Int { Int((time / 86400).rounded(.down)) }

    private func value(_ entry: Entry, at now: Double) -> Double {
        let long = entry.long.at(now, halfLife: options.longHalfLife)
        return entry.short.at(now, halfLife: options.shortHalfLife)
            + (entry.picks >= options.floorPicks ? max(long, options.floorValue) : long)
    }

    private func total(_ history: Key, at now: Double) -> Double {
        guard let total = totals[history] else { return 0 }
        return total.short.at(now, halfLife: options.shortHalfLife) + total.long.at(now, halfLife: options.longHalfLife)
    }

    // MARK: Scoring

    /// The base log probability of `word` after `history`, adapted to the user.
    func adapt(_ base: Double, word: UInt32?, history: LanguageModelHistory) -> Double {
        guard let word, word > 2 else { return base }
        return lock.withLock {
            guard !entries.isEmpty else { return base }
            let now = clock()
            var p = exp(base)
            var changed = false
            var contexts: [[UInt32]] = [[]]
            let usable: (UInt32) -> Bool = { $0 > 2 && $0 != LanguageModelHistory.noContext }
            if usable(history.previous) {
                contexts.append([history.previous])
                if let older = history.older, usable(older) { contexts.append([older, history.previous]) }
            }
            for (order, context) in contexts.enumerated() {
                let count = total(Key(words: context), at: now)
                guard count > 0 else { continue }
                let observed = entries[Key(words: context + [word])].map { value($0, at: now) } ?? 0
                let kappa = options.kappa[order]
                p = (min(observed, count) + kappa * p) / (count + kappa)
                changed = true
            }
            return changed ? log(max(p, .leastNonzeroMagnitude)) : base
        }
    }

    // MARK: Learning

    struct Word {
        var id: UInt32
        var text: String
        var reading: String
        var picked: Bool
    }

    /// Learns one committed stretch of words (no literals inside).
    func learn(_ words: [Word]) {
        guard !words.isEmpty else { return }
        lock.withLock {
            let now = clock()
            for end in words.indices {
                for order in 1...3 where end - order + 1 >= 0 {
                    let gram = Array(words[(end - order + 1)...end])
                    let picked = gram.contains(where: \.picked)
                    add(gram, weight: picked ? options.pickWeights[order - 1] : options.passiveWeight,
                        picked: picked && gram.last!.picked, at: now)
                }
            }
            trim(now: now)
            hasData = !entries.isEmpty
        }
    }

    private func add(_ gram: [Word], weight: Double, picked: Bool, at now: Double) {
        let key = Key(words: gram.map(\.id))
        var entry = entries[key] ?? Entry(texts: gram.map(\.text), readings: gram.map(\.reading))
        let today = day(now)
        entry.short.add(weight, at: now, halfLife: options.shortHalfLife)
        var longAmount = 0.0
        if picked || weight > options.passiveWeight {
            longAmount = weight
            if picked { entry.picks += 1 }
        } else if entry.firstDay < today {
            // Habits: text accepted on an earlier day as well, at most one unit per day.
            if entry.longDay != today { entry.longDay = today; entry.longDayAmount = 0 }
            longAmount = min(weight, 1 - entry.longDayAmount)
            entry.longDayAmount += longAmount
        }
        if longAmount > 0 { entry.long.add(longAmount, at: now, halfLife: options.longHalfLife) }
        entry.firstDay = min(entry.firstDay, today)
        entries[key] = entry
        let history = Key(words: Array(key.words.dropLast()))
        var total = totals[history] ?? (DecayingCount(), DecayingCount())
        total.short.add(weight, at: now, halfLife: options.shortHalfLife)
        if longAmount > 0 { total.long.add(longAmount, at: now, halfLife: options.longHalfLife) }
        totals[history] = total
    }

    /// A pick replaced `word` after `history`: halve the user's evidence for it there.
    func weaken(word: UInt32, history: LanguageModelHistory) {
        lock.withLock {
            let now = clock()
            var keys = [Key(words: [history.previous, word])]
            if let older = history.older { keys.append(Key(words: [older, history.previous, word])) }
            for key in keys {
                guard var entry = entries[key] else { continue }
                entry.short.scale(0.5, at: now, halfLife: options.shortHalfLife)
                entry.long.scale(0.5, at: now, halfLife: options.longHalfLife)
                entry.picks = 0
                entries[key] = entry
            }
            rebuildTotals()
        }
    }

    /// Removes every n-gram containing `id`.
    func forget(_ id: UInt32) {
        lock.withLock {
            entries = entries.filter { !$0.key.words.contains(id) }
            rebuildTotals()
            hasData = !entries.isEmpty
        }
    }

    func forgetAll() {
        lock.withLock { entries.removeAll(); totals.removeAll(); hasData = false }
    }

    /// Picked single words, for the 記住的字詞 list.
    func pickedWords() -> [(reading: String, word: String, count: Double)] {
        lock.withLock {
            let now = clock()
            return entries.filter { $0.key.words.count == 1 && $0.value.picks > 0 }
                .map { ($0.value.readings[0], $0.value.texts[0], value($0.value, at: now)) }
        }
    }

    private func trim(now: Double) {
        guard entries.count > options.capacity else { return }
        let keep = entries.sorted { value($0.value, at: now) > value($1.value, at: now) }.prefix(options.capacity)
        entries = Dictionary(uniqueKeysWithValues: keep.map { ($0.key, $0.value) })
        rebuildTotals()
    }

    private func rebuildTotals() {
        let now = clock()
        totals.removeAll(keepingCapacity: true)
        for (key, entry) in entries {
            let history = Key(words: Array(key.words.dropLast()))
            var total = totals[history] ?? (DecayingCount(), DecayingCount())
            total.short.add(entry.short.at(now, halfLife: options.shortHalfLife), at: now, halfLife: options.shortHalfLife)
            total.long.add(entry.long.at(now, halfLife: options.longHalfLife), at: now, halfLife: options.longHalfLife)
            totals[history] = total
        }
    }

    // MARK: Persistence

    static let fileName = "personal_ngrams.tsv"

    func serialized() -> String {
        lock.withLock {
            entries.values.sorted { $0.texts.lexicographicallyPrecedes($1.texts) }.map { entry in
                [entry.texts.joined(separator: "\u{1F}"), entry.readings.joined(separator: "\u{1F}"),
                 "\(entry.short.value)", "\(entry.short.time)", "\(entry.long.value)", "\(entry.long.time)",
                 "\(entry.picks)", "\(entry.firstDay == Int.max ? -1 : entry.firstDay)", "\(entry.longDay)",
                 "\(entry.longDayAmount)"].joined(separator: "\t") + "\n"
            }.joined()
        }
    }

    /// Loads rows written by `serialized()`, resolving words to IDs.
    func load(_ data: Data, resolve: (String) -> UInt32) {
        lock.withLock {
            scanTSV(data) { fields in
                guard fields.count == 10 else { return }
                let texts = fields[0].components(separatedBy: "\u{1F}")
                let readings = fields[1].components(separatedBy: "\u{1F}")
                guard (1...3).contains(texts.count), texts.count == readings.count, !texts.contains(""),
                      let shortValue = Double(fields[2]), let shortTime = Double(fields[3]),
                      let longValue = Double(fields[4]), let longTime = Double(fields[5]),
                      let picks = Int(fields[6]), let firstDay = Int(fields[7]), let longDay = Int(fields[8]),
                      let longDayAmount = Double(fields[9]),
                      [shortValue, shortTime, longValue, longTime, longDayAmount].allSatisfy({ $0.isFinite && $0 >= 0 })
                else { return }
                var entry = Entry(texts: texts, readings: readings)
                entry.short = DecayingCount(value: shortValue, time: shortTime)
                entry.long = DecayingCount(value: longValue, time: longTime)
                entry.picks = max(0, picks)
                entry.firstDay = firstDay < 0 ? Int.max : firstDay
                entry.longDay = longDay
                entry.longDayAmount = longDayAmount
                entries[Key(words: texts.map(resolve))] = entry
            }
            rebuildTotals()
            hasData = !entries.isEmpty
        }
    }
}

/// The base model adapted by the user's statistics.  Once anything is learned it keeps two words of history, so
/// personal trigrams apply even over a unigram or bigram base; with nothing learned it is exactly the base model.
struct PersonalizedLanguageModel: LanguageModel {
    let base: any LanguageModel
    let personal: PersonalLanguageModel
    var order: Int { personal.hasData ? 3 : base.order }
    func wordID(for word: String) -> UInt32? { base.wordID(for: word) }
    func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        personal.adapt(base.score(wordID: wordID, history: history, unigram: unigram), word: wordID, history: history)
    }
}
