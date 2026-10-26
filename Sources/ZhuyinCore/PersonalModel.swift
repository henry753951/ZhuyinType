import Foundation

public struct PersonalModelOptions: Codable, Sendable {
    public var pickWeight: Double = 32
    public var passiveWeight: Double = 0.02
    public var kappa1: Double = 2048
    public var kappa2: Double = 512
    public var kappa3: Double = 1
    public init() {}
    var isValid: Bool {
        [pickWeight, passiveWeight, kappa1, kappa2, kappa3].allSatisfy { $0.isFinite && $0 > 0 }
            && kappa1 >= kappa2 && kappa2 >= kappa3 && pickWeight > passiveWeight
    }
}

struct PersonalWord: Hashable, Codable {
    var text: String
    var reading: String
}

struct PersonalCount: Codable {
    var words: [PersonalWord]
    var count: Double
    var updated: Double
    var firstDay: Int
    var lastDay: Int
    var dailyPassive: Double = 0
    var picks: Int = 0
    var floor: Double = 0
    func value(at now: Double, halfLife: Double) -> Double {
        max(floor, count * pow(0.5, max(0, now - updated) / halfLife))
    }
}

/// The persisted keys retain typed readings; the scoring snapshot aggregates identical text histories.
struct PersonalModel: Codable {
    var version = 1
    var short: [PersonalCount] = []
    var long: [PersonalCount] = []
    static let shortHalfLife = 8.0 * 3600
    static let longHalfLife = 90.0 * 86400

    mutating func observe(_ events: [([PersonalWord], Bool)], now: Double, calendar: Calendar,
                          options: PersonalModelOptions) {
        let day = calendar.dateComponents([.year, .month, .day], from: Date(timeIntervalSince1970: now))
        let today = day.year! * 10000 + day.month! * 100 + day.day!
        var recent = Dictionary(uniqueKeysWithValues: short.map { ($0.words, $0) })
        var durable = Dictionary(uniqueKeysWithValues: long.map { ($0.words, $0) })
        for (words, picked) in events {
            let amount = picked ? options.pickWeight : options.passiveWeight
            if picked {
                // Only alternatives to the same typed target compete; unrelated continuations are retained.
                for (key, var count) in recent where Self.competes(key, words) {
                    count.count = count.value(at: now, halfLife: Self.shortHalfLife) * 0.5
                    count.floor *= 0.5; count.updated = now; recent[key] = count
                }
                for (key, var count) in durable where Self.competes(key, words) {
                    count.count = count.value(at: now, halfLife: Self.longHalfLife) * 0.5
                    count.floor *= 0.5; count.updated = now; durable[key] = count
                }
            }
            var count = recent[words] ?? PersonalCount(words: words, count: 0, updated: now,
                                                       firstDay: today, lastDay: today)
            let previous = count
            count.count = count.value(at: now, halfLife: Self.shortHalfLife) + amount
            count.updated = now; count.lastDay = today
            if picked { count.picks += 1 }
            recent[words] = count
            if durable[words] != nil || previous.firstDay != today || count.picks >= 3 {
                // Three explicit picks establish the requested durable floor even within one day.
                var saved = durable[words] ?? PersonalCount(words: words, count: 0, updated: now,
                    firstDay: previous.firstDay, lastDay: today, picks: previous.picks)
                if saved.lastDay != today { saved.dailyPassive = 0 }
                let increment = picked ? amount : min(amount, max(0, 1 - saved.dailyPassive))
                saved.count = saved.value(at: now, halfLife: Self.longHalfLife) + increment
                saved.updated = now; saved.lastDay = today
                if picked { saved.picks += 1 } else { saved.dailyPassive += increment }
                if saved.picks >= 3 { saved.floor = max(saved.floor, 1) }
                durable[words] = saved
            }
        }
        short = Self.trim(recent.values, cap: 2000, now: now, halfLife: Self.shortHalfLife)
        long = Self.trim(durable.values, cap: 50000, now: now, halfLife: Self.longHalfLife)
    }

    private static func competes(_ a: [PersonalWord], _ b: [PersonalWord]) -> Bool {
        a.count == b.count && a.dropLast() == b.dropLast()
            && a.last?.reading == b.last?.reading && a.last?.text != b.last?.text
    }

    static func trim(_ counts: Dictionary<[PersonalWord], PersonalCount>.Values, cap: Int,
                     now: Double, halfLife: Double) -> [PersonalCount] {
        counts.filter { $0.value(at: now, halfLife: halfLife) >= 1e-12 }.sorted {
            let a = $0.value(at: now, halfLife: halfLife), b = $1.value(at: now, halfLife: halfLife)
            return a == b ? $0.words.map { $0.reading + $0.text }.lexicographicallyPrecedes($1.words.map { $0.reading + $0.text }) : a > b
        }.prefix(cap).map { $0 }
    }

    mutating func forget(word: PersonalWord) {
        short.removeAll { $0.words.contains(word) }; long.removeAll { $0.words.contains(word) }
    }
}

struct PersonalDistribution {
    var counts: [UInt32: Double] = [:]
    var total: Double = 0
    var types: Double { Double(counts.count) }
}

/// Immutable, indexed snapshot: no decays, string lookups, or locks in the decoder's inner loop.
struct AdaptiveLanguageModel: LanguageModel {
    let base: any LanguageModel
    let ids: [String: UInt32]
    let distributions: [LanguageModelHistory: PersonalDistribution]
    let options: PersonalModelOptions
    var order: Int { distributions.isEmpty ? base.order : 3 }
    func wordID(for word: String) -> UInt32? { ids[word] ?? base.wordID(for: word) }
    static func baseID(_ id: UInt32?) -> UInt32? { id.flatMap { $0 >= 0x80000000 ? nil : $0 } }
    private func baseHistory(_ history: LanguageModelHistory) -> LanguageModelHistory {
        .init(older: history.older.map { Self.baseID($0) ?? 2 },
              previous: history.previous == .max ? .max : (Self.baseID(history.previous) ?? 2))
    }
    func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        let baseScore = base.score(wordID: Self.baseID(wordID), history: baseHistory(history), unigram: unigram)
        guard let wordID, !distributions.isEmpty, let uni = distributions[.init()] else { return baseScore }
        var probability = (uni.counts[wordID] ?? 0) / uni.total
        var evidence = uni.total, kappa = options.kappa1
        if history.previous != 1 && history.previous != .max {
            let bigram = distributions[.init(previous: history.previous)]
            if let bigram { probability = ((bigram.counts[wordID] ?? 0) + bigram.types * probability) / (bigram.total + bigram.types) }
            evidence = bigram?.total ?? 0; kappa = options.kappa2
            if let older = history.older, older != 1 && older != .max {
                let trigram = distributions[history]
                if let trigram { probability = ((trigram.counts[wordID] ?? 0) + trigram.types * probability) / (trigram.total + trigram.types) }
                evidence = trigram?.total ?? 0; kappa = options.kappa3
            }
        }
        guard evidence > 0 else { return baseScore }
        let lambda = evidence / (evidence + kappa)
        // Log-sum-exp preserves rare base paths and avoids underflow on old personal counts.
        let a = log1p(-lambda) + baseScore
        let b = probability > 0 ? log(lambda) + log(probability) : -.infinity
        let maximum = max(a, b)
        return maximum.isFinite ? maximum + log1p(exp(min(a, b) - maximum)) : maximum
    }
    func endScore(history: LanguageModelHistory) -> Double { base.endScore(history: baseHistory(history)) }
}
