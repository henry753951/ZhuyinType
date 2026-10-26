import Foundation

/// ZTLM/1 sections stay mapped; only the header and array bounds are inspected at startup.
public struct NGramLanguageModel: LanguageModel {
    private let data: Data
    public let order: Int
    public let unknownWordShift: Double
    private let vocabularyCount: Int
    private let bigramCount: Int
    private let trigramCount: Int
    private let vocabulary: Int
    private let blob: Int
    private let unigrams: Int
    private let bigrams: Int
    private let trigrams: Int

    public init(directory: URL) throws {
        let mapped = try Data(contentsOf: directory.appendingPathComponent("lm.bin"), options: .alwaysMapped)
        func invalid() -> NSError {
            NSError(domain: "ZhuyinCore.ZTLM", code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Invalid or truncated ZTLM/1 language model"])
        }
        guard mapped.count >= 64 else { throw invalid() }
        let header = mapped.withUnsafeBytes { raw in
            (0..<8).map { UInt64(littleEndian: raw.loadUnaligned(fromByteOffset: $0 * 8, as: UInt64.self)) }
        }
        guard header[0] & 0xffffffff == 0x4d4c545a, header[0] >> 32 == 1,
              header[7] == UInt64(mapped.count) else { throw invalid() }
        let order = Int(header[1] & 0xffffffff), v = Int(header[1] >> 32)
        let b = Int(header[2] & 0xffffffff), t = Int(header[2] >> 32)
        guard (order == 2 || order == 3), v >= 3,
              order == 3 || (t == 0 && header[6] == 0) else { throw invalid() }
        let sectionOffsets = order == 3 ? Array(header[3...6]) : Array(header[3...5])
        guard sectionOffsets.allSatisfy({ $0 >= 64 && $0 <= UInt64(mapped.count) && $0 % 8 == 0 }),
              zip(sectionOffsets, sectionOffsets.dropFirst()).allSatisfy({ $0 <= $1 }) else { throw invalid() }
        let vocab = Int(header[3]), uni = Int(header[4]), bi = Int(header[5])
        let tri = order == 3 ? Int(header[6]) : mapped.count
        guard (v + 1) * 4 <= uni - vocab, v * 8 <= bi - uni,
              (v + 1) * 4 + b * 12 <= tri - bi,
              order == 2 || (b + 1) * 4 + t * 8 <= mapped.count - tri else { throw invalid() }
        let blob = vocab + (v + 1) * 4
        let valid = mapped.withUnsafeBytes { raw -> Bool in
            func uint(_ offset: Int) -> Int { Int(Self.uint(raw, offset)) }
            func bounds(_ offset: Int, _ count: Int, _ limit: Int) -> Bool {
                guard uint(offset) == 0, uint(offset + count * 4) <= limit else { return false }
                var previous = 0
                for i in 0..<count {
                    let current = uint(offset + (i + 1) * 4)
                    guard current >= previous, current <= limit else { return false }
                    previous = current
                }
                return true
            }
            guard bounds(vocab, v, uni - blob), bounds(bi, v, b), uint(bi + v * 4) == b,
                  order == 2 || (bounds(tri, b, t) && uint(tri + b * 4) == t) else { return false }
            for (i, spelling) in ["</s>", "<s>", "<unk>"].enumerated() {
                let start = uint(vocab + i * 4), end = uint(vocab + (i + 1) * 4)
                guard raw[blob + start..<blob + end].elementsEqual(spelling.utf8) else { return false }
            }
            return true
        }
        guard valid else { throw invalid() }
        data = mapped
        self.order = order; vocabularyCount = v; bigramCount = b; trigramCount = t
        vocabulary = vocab; self.blob = blob; unigrams = uni; bigrams = bi; trigrams = tri
        struct Metadata: Decodable { var unknown_word_shift: Double? }
        let metadata = try? JSONDecoder().decode(Metadata.self,
            from: Data(contentsOf: directory.appendingPathComponent("lm.json")))
        let shift = metadata?.unknown_word_shift ?? 0
        unknownWordShift = shift.isFinite ? shift : 0
    }

    private static func uint(_ raw: UnsafeRawBufferPointer, _ offset: Int) -> UInt32 {
        UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: offset, as: UInt32.self))
    }

    private static func float(_ raw: UnsafeRawBufferPointer, _ offset: Int) -> Double {
        let value = Double(Float(bitPattern: uint(raw, offset)))
        return value.isFinite ? value : -.infinity
    }

    public func wordID(for word: String) -> UInt32? {
        func search(_ bytes: UnsafeBufferPointer<UInt8>) -> UInt32? {
            data.withUnsafeBytes { raw in
                var low = 0, high = vocabularyCount
                while low < high {
                    let mid = (low + high) / 2
                    let start = blob + Int(Self.uint(raw, vocabulary + mid * 4))
                    let end = blob + Int(Self.uint(raw, vocabulary + (mid + 1) * 4))
                    let length = min(end - start, bytes.count)
                    let comparison = length == 0 ? 0 : memcmp(raw.baseAddress! + start, bytes.baseAddress!, length)
                    if comparison < 0 || (comparison == 0 && end - start < bytes.count) { low = mid + 1 }
                    else if comparison > 0 || end - start > bytes.count { high = mid }
                    else { return UInt32(mid) }
                }
                return nil
            }
        }
        if let result = word.utf8.withContiguousStorageIfAvailable({ search($0) }) { return result }
        return Array(word.utf8).withUnsafeBufferPointer { search($0) }
    }

    public func score(wordID: UInt32?, history: LanguageModelHistory, unigram: Double) -> Double {
        guard let wordID, wordID < vocabularyCount else { return unigram + unknownWordShift }
        return data.withUnsafeBytes { raw in
            func find(_ section: Int, _ groups: Int, _ context: UInt32, _ word: UInt32) -> Int? {
                guard context < groups else { return nil }
                var low = Int(Self.uint(raw, section + Int(context) * 4))
                let end = Int(Self.uint(raw, section + (Int(context) + 1) * 4))
                var high = end
                let next = section + (groups + 1) * 4
                while low < high {
                    let mid = (low + high) / 2
                    if Self.uint(raw, next + mid * 4) < word { low = mid + 1 }
                    else { high = mid }
                }
                return low < end && Self.uint(raw, next + low * 4) == word ? low : nil
            }
            if history.previous == LanguageModelHistory.noContext {
                return Self.float(raw, unigrams + Int(wordID) * 4)
            }
            var backoff = 0.0
            if order == 3, let older = history.older,
               let context = find(bigrams, vocabularyCount, older, history.previous) {
                if let index = find(trigrams, bigramCount, UInt32(context), wordID) {
                    return Self.float(raw, trigrams + (bigramCount + 1 + trigramCount + index) * 4)
                }
                backoff = Self.float(raw, bigrams + (vocabularyCount + 1 + bigramCount * 2 + context) * 4)
            }
            if let index = find(bigrams, vocabularyCount, history.previous, wordID) {
                return backoff + Self.float(raw, bigrams + (vocabularyCount + 1 + bigramCount + index) * 4)
            }
            let previous = history.previous < vocabularyCount ? history.previous : 2
            return backoff + Self.float(raw, unigrams + (vocabularyCount + Int(previous)) * 4)
                + Self.float(raw, unigrams + Int(wordID) * 4)
        }
    }
}
