import Foundation

/// IDs 0, 1 and 2 are </s>, <s> and <unk>. A nil word ID uses the lexicon probability.
public struct LanguageModelHistory: Hashable, Sendable {
    /// No context: the word is scored by its unigram probability.
    public static let noContext: UInt32 = .max
    public var older: UInt32?
    public var previous: UInt32
    public init(older: UInt32? = nil, previous: UInt32 = 1) {
        self.older = older; self.previous = previous
    }
    public func appending(_ wordID: UInt32?, order: Int) -> Self {
        Self(older: order == 3 ? previous : nil, previous: order == 1 ? 1 : (wordID ?? 2))
    }
}

/// Scores use natural logarithms; resolve strings once, before decoding.
public protocol LanguageModel: Sendable {
    var order: Int { get }
    func wordID(for word: String) -> UInt32?
    func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double
}

extension LanguageModel {
    /// Where a composition stops: the sentence ends there with probability `Engine.sentenceEndShare`, otherwise the
    /// user goes on typing (a continuation every next word shares, so it costs nothing):
    ///     log(share × P(</s> | history) + (1 − share)).
    public func endScore(history: LanguageModelHistory) -> Double {
        let end = score(wordID: 0, history: history, unigram: 0)
        let share = Engine.sentenceEndShare
        guard share < 1 else { return end }
        return log(share * exp(end) + (1 - share))
    }
    public func score(word: String, after previousWord: String, unigram: Double) -> Double {
        score(wordID: wordID(for: word), history: .init(previous: wordID(for: previousWord) ?? 2), unigram: unigram)
    }
    public func endScore(after previousWord: String) -> Double {
        endScore(history: .init(previous: wordID(for: previousWord) ?? 2))
    }
}

public struct UnigramLanguageModel: LanguageModel {
    public init() {}
    public let order = 1
    public func wordID(for word: String) -> UInt32? { nil }
    public func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double { unigram }
}

public struct BigramLanguageModel: LanguageModel {
    public let order = 2
    private var ids: [String: UInt32] = ["</s>": 0, "<s>": 1, "<unk>": 2]
    private var unigrams: [UInt32: Double] = [:]
    private var bigrams: [UInt32: [UInt32: Double]] = [:]
    private var backoffs: [UInt32: Double] = [:]

    private mutating func intern(_ word: String) -> UInt32 {
        if let id = ids[word] { return id }
        let id = UInt32(ids.count)
        ids[word] = id
        return id
    }

    public init(directory: URL) throws {
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("unigram.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 2 else { return }
            if let score = Double(fields[1]), score.isFinite { unigrams[intern(fields[0])] = score }
        }
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("bigram.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 3 else { return }
            if let score = Double(fields[2]), score.isFinite {
                let first = intern(fields[0]), second = intern(fields[1])
                bigrams[first, default: [:]][second] = score
            }
        }
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("backoff.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 2 else { return }
            if let score = Double(fields[1]), score.isFinite { backoffs[intern(fields[0])] = score }
        }
    }

    public func wordID(for word: String) -> UInt32? { ids[word] }
    /// Every word the model knows, with its ID.
    public var vocabulary: [String: UInt32] { ids }
    public func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        let word = wordID ?? UInt32.max
        return bigrams[history.previous]?[word]
            ?? ((backoffs[history.previous] ?? 0) + (unigrams[word] ?? unigram))
    }
}

/// Two bigram models of different domains, mixed: P = λ·P(chat) + (1 − λ)·P(encyclopedia).  λ follows what the user
/// is writing (`chatWeight`), so typing starts out conversational and turns encyclopedic when the text before the
/// cursor is.  Word IDs are the encyclopedic model's, then new ones for words only the chat model knows.
public final class MixtureLanguageModel: LanguageModel, @unchecked Sendable {
    public let order = 2
    private let chat: BigramLanguageModel
    private let encyclopedia: BigramLanguageModel
    private var ids: [String: UInt32]
    /// Per mixture ID, the chat model's ID (UInt32.max when it lacks the word).
    private var chatIDs: [UInt32]
    private var encyclopediaCount: UInt32
    private let lock = NSLock()
    private var weight: Double

    public init(chat: BigramLanguageModel, encyclopedia: BigramLanguageModel, chatWeight: Double) {
        self.chat = chat
        self.encyclopedia = encyclopedia
        weight = chatWeight
        ids = encyclopedia.vocabulary
        encyclopediaCount = (ids.values.max() ?? 2) + 1
        var next = encyclopediaCount
        for word in chat.vocabulary.keys where ids[word] == nil {
            ids[word] = next
            next += 1
        }
        chatIDs = Array(repeating: .max, count: Int(next))
        for (word, id) in chat.vocabulary { chatIDs[Int(ids[word]!)] = id }
    }

    /// The chat model's share, between 0 and 1.
    public var chatWeight: Double {
        get { lock.withLock { weight } }
        set { lock.withLock { weight = min(1, max(0, newValue)) } }
    }

    public func wordID(for word: String) -> UInt32? { ids[word] }

    private func chatID(_ id: UInt32?) -> UInt32? {
        guard let id else { return nil }
        if id == LanguageModelHistory.noContext { return id }
        guard Int(id) < chatIDs.count, chatIDs[Int(id)] != .max else { return nil }
        return chatIDs[Int(id)]
    }

    private func encyclopediaID(_ id: UInt32?) -> UInt32? {
        guard let id else { return nil }
        return id < encyclopediaCount || id == LanguageModelHistory.noContext ? id : nil
    }

    /// log P under each model, for the domain posterior.
    func scores(wordID: UInt32?, previous: UInt32, unigram: Double) -> (chat: Double, encyclopedia: Double) {
        let c = chat.score(wordID: chatID(wordID), history: .init(previous: chatID(previous) ?? 2), unigram: unigram)
        let e = encyclopedia.score(wordID: encyclopediaID(wordID), history: .init(previous: encyclopediaID(previous) ?? 2),
                                   unigram: unigram)
        return (c, e)
    }

    public func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        let (c, e) = scores(wordID: wordID, previous: history.previous, unigram: unigram)
        let w = chatWeight
        if w <= 0 { return e }
        if w >= 1 { return c }
        let a = log(w) + c, b = log(1 - w) + e
        let top = max(a, b)
        return top + log(exp(a - top) + exp(b - top))
    }
}

/// Treats a composition as a stretch of a sentence rather than a whole one: what would be scored after `<s>` is
/// scored with no context, and nothing is added for `</s>`.  A composition starts and stops wherever the user starts
/// and stops typing, which is rarely a sentence boundary.
struct BoundaryFreeLanguageModel: LanguageModel {
    let base: any LanguageModel
    var order: Int { base.order }
    func wordID(for word: String) -> UInt32? { base.wordID(for: word) }
    func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        if wordID == 0 { return 0 }
        var history = history
        if history.previous == 1 { history.previous = LanguageModelHistory.noContext; history.older = nil }
        if history.older == 1 { history.older = nil }
        return base.score(wordID: wordID, history: history, unigram: unigram)
    }
}

// Scan UTF-8 once: constructing an array of lines and splitting every String dominates lexicon startup.
func scanTSV(_ data: Data, separatingSpaces: Bool = false, _ row: ([String]) -> Void) {
    data.withUnsafeBytes { raw in
        let bytes = raw.bindMemory(to: UInt8.self)
        var start = 0
        var fields: [String] = []
        fields.reserveCapacity(4)
        var comment = false
        for i in 0...bytes.count {
            let byte: UInt8 = i == bytes.count ? 10 : bytes[i]
            if i == start && fields.isEmpty && byte == 35 { comment = true }
            if byte == 9 || byte == 10 || (separatingSpaces && byte == 32) {
                if !comment {
                    let end = byte == 10 && i > start && bytes[i - 1] == 13 ? i - 1 : i
                    if end > start || !separatingSpaces {
                        fields.append(String(decoding: bytes[start..<end], as: UTF8.self))
                    }
                }
                start = i + 1
                if byte == 10 {
                    if !comment && fields.count > 1 { row(fields) }
                    fields.removeAll(keepingCapacity: true)
                    comment = false
                }
            }
        }
    }
}
