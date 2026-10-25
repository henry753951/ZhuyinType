import Foundation

/// Scores use natural logarithms. The decoder retains the previous word as its state.
public protocol LanguageModel: Sendable {
    func score(word: String, after previousWord: String, unigram: Double) -> Double
    func endScore(after previousWord: String) -> Double
}

public struct UnigramLanguageModel: LanguageModel {
    public init() {}
    public func score(word: String, after previousWord: String, unigram: Double) -> Double { unigram }
    public func endScore(after previousWord: String) -> Double { 0 }
}

public struct BigramLanguageModel: LanguageModel {
    private var unigrams: [String: Double] = [:]
    private var bigrams: [String: [String: Double]] = [:]
    private var backoffs: [String: Double] = [:]

    public init(directory: URL) throws {
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("unigram.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 2 else { return }
            if let score = Double(fields[1]), score.isFinite { unigrams[fields[0]] = score }
        }
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("bigram.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 3 else { return }
            if let score = Double(fields[2]), score.isFinite { bigrams[fields[0], default: [:]][fields[1]] = score }
        }
        scanTSV(try Data(contentsOf: directory.appendingPathComponent("backoff.tsv")), separatingSpaces: true) { fields in
            guard fields.count == 2 else { return }
            if let score = Double(fields[1]), score.isFinite { backoffs[fields[0]] = score }
        }
    }

    public func score(word: String, after previousWord: String, unigram: Double) -> Double {
        bigrams[previousWord]?[word] ?? ((backoffs[previousWord] ?? 0) + (unigrams[word] ?? unigram))
    }
    public func endScore(after previousWord: String) -> Double {
        score(word: "</s>", after: previousWord, unigram: 0)
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
