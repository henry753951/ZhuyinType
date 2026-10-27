import Foundation

/// ZTLX/1: immutable tables and UTF-8 stay mapped; only requested entries become Swift values.
struct MappedLexicon: Sendable {
    private let data: Data
    let wordCount: Int
    private let readingCount: Int
    private let entryCount: Int
    private let baseCount: Int
    private let prefixCount: Int
    private let syllableCount: Int
    private let sections: [Int]

    init(url: URL) throws {
        let data = try Data(contentsOf: url, options: .alwaysMapped)
        func invalid() -> NSError {
            NSError(domain: "ZhuyinCore.ZTLX", code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Invalid or truncated ZTLX/1 lexicon"])
        }
        guard data.count >= 128 else { throw invalid() }
        let valid = data.withUnsafeBytes { raw -> Bool in
            guard Self.uint(raw, 0) == 0x584c545a, Self.uint(raw, 4) == 1 else { return false }
            return true
        }
        guard valid else { throw invalid() }
        let counts = data.withUnsafeBytes { raw in (2..<8).map { Int(Self.uint(raw, $0 * 4)) } }
        let offsets = data.withUnsafeBytes { raw in
            (0..<12).map { UInt64(littleEndian: raw.loadUnaligned(fromByteOffset: 32 + $0 * 8, as: UInt64.self)) }
        }
        guard offsets[9] == UInt64(data.count), offsets[10] == 0, offsets[11] == 0,
              offsets[0] == 128, offsets.prefix(9).allSatisfy({ $0 <= UInt64(data.count) && $0 % 8 == 0 }),
              zip(offsets.prefix(9), offsets.dropFirst().prefix(9)).allSatisfy({ $0 <= $1 }) else { throw invalid() }
        let s = offsets.prefix(10).map(Int.init)
        let lengths = [counts[0] * 24, counts[1] * 16, counts[2] * 24, counts[2] * 4,
                       counts[3] * 16, counts[1] * 4, counts[4] * 8, counts[5] * 8]
        guard (0..<8).allSatisfy({ lengths[$0] <= s[$0 + 1] - s[$0] }) else { throw invalid() }
        let tablesValid = data.withUnsafeBytes { raw -> Bool in
            func u(_ offset: Int) -> Int { Int(Self.uint(raw, offset)) }
            for (table, count, stride) in [(0, counts[0], 24), (1, counts[1], 16), (4, counts[3], 16),
                                            (6, counts[4], 8), (7, counts[5], 8)] {
                var previous: Range<Int>?
                for i in 0..<count {
                    let p = s[table] + i * stride, start = u(p), length = u(p + 4)
                    guard length > 0, start <= data.count - s[8], length <= data.count - s[8] - start else { return false }
                    let range = (s[8] + start)..<(s[8] + start + length)
                    guard String(bytes: raw[range], encoding: .utf8) != nil else { return false }
                    if let previous, !raw[previous].lexicographicallyPrecedes(raw[range]) { return false }
                    previous = range
                    if table == 0 || table == 1 || table == 4 {
                        let limit = table == 4 ? counts[1] : counts[2]
                        guard u(p + 8) <= limit, u(p + 12) <= limit - u(p + 8) else { return false }
                    }
                    if table == 0 && u(p + 16) >= counts[2] { return false }
                }
            }
            for i in 0..<counts[2] {
                let p = s[2] + i * 24
                guard u(p) < counts[0], u(p + 4) < counts[1],
                      Self.double(raw, p + 8).isFinite, Self.double(raw, p + 16).isFinite,
                      u(s[3] + i * 4) < counts[2] else { return false }
            }
            for i in 0..<counts[1] where u(s[5] + i * 4) >= counts[1] { return false }
            return true
        }
        guard tablesValid else { throw invalid() }
        self.data = data; sections = s
        wordCount = counts[0]; readingCount = counts[1]; entryCount = counts[2]
        baseCount = counts[3]; prefixCount = counts[4]; syllableCount = counts[5]
    }

    private static func uint(_ raw: UnsafeRawBufferPointer, _ offset: Int) -> UInt32 {
        UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: offset, as: UInt32.self))
    }
    private static func double(_ raw: UnsafeRawBufferPointer, _ offset: Int) -> Double {
        Double(bitPattern: UInt64(littleEndian: raw.loadUnaligned(fromByteOffset: offset, as: UInt64.self)))
    }
    private func uint(_ offset: Int) -> Int { data.withUnsafeBytes { Int(Self.uint($0, offset)) } }
    private func spelling(_ record: Int) -> String {
        data.withUnsafeBytes { raw in
            let start = sections[8] + Int(Self.uint(raw, record)), length = Int(Self.uint(raw, record + 4))
            return String(decoding: raw[start..<(start + length)], as: UTF8.self)
        }
    }
    private func find(_ text: String, table: Int, count: Int, stride: Int) -> Int? {
        func search(_ bytes: UnsafeBufferPointer<UInt8>) -> Int? {
            data.withUnsafeBytes { raw in
                var low = 0, high = count
                while low < high {
                    let mid = (low + high) / 2, p = sections[table] + mid * stride
                    let start = sections[8] + Int(Self.uint(raw, p)), length = Int(Self.uint(raw, p + 4))
                    let n = min(length, bytes.count)
                    let cmp = n == 0 ? 0 : memcmp(raw.baseAddress! + start, bytes.baseAddress!, n)
                    if cmp < 0 || (cmp == 0 && length < bytes.count) { low = mid + 1 }
                    else if cmp > 0 || length > bytes.count { high = mid }
                    else { return mid }
                }
                return nil
            }
        }
        if let result = text.utf8.withContiguousStorageIfAvailable({ search($0) }) { return result }
        return Array(text.utf8).withUnsafeBufferPointer { search($0) }
    }
    func wordIndex(_ word: String) -> Int? { find(word, table: 0, count: wordCount, stride: 24) }
    func containsPrefix(_ base: String) -> Bool { find(base, table: 6, count: prefixCount, stride: 8) != nil }
    var syllables: [String] { (0..<syllableCount).map { spelling(sections[7] + $0 * 8) } }
    func readings(forBase base: String) -> [String] {
        guard let i = find(base, table: 4, count: baseCount, stride: 16) else { return [] }
        let p = sections[4] + i * 16, start = uint(p + 8), count = uint(p + 12)
        return (start..<(start + count)).map { spelling(sections[1] + uint(sections[5] + $0 * 4) * 16) }
    }
    func entries(for reading: String) -> [WordEntry] {
        guard let i = find(reading, table: 1, count: readingCount, stride: 16) else { return [] }
        let p = sections[1] + i * 16, start = uint(p + 8), count = uint(p + 12)
        return (start..<(start + count)).map { i in
            let p = sections[2] + i * 24
            return data.withUnsafeBytes { raw in
                WordEntry(word: spelling(sections[0] + Int(Self.uint(raw, p)) * 24),
                          logp: Self.double(raw, p + 8), readingPrior: Self.double(raw, p + 16))
            }
        }
    }
    private func readingEntry(_ index: Int) -> (String, Double) {
        let p = sections[2] + index * 24
        return data.withUnsafeBytes { raw in
            (spelling(sections[1] + Int(Self.uint(raw, p + 4)) * 16),
             Self.double(raw, p + 8) + Self.double(raw, p + 16))
        }
    }
    func readings(of word: String) -> [(String, Double)] {
        guard let i = wordIndex(word) else { return [] }
        let p = sections[0] + i * 24, start = uint(p + 8), count = uint(p + 12)
        return (start..<(start + count)).map { readingEntry(uint(sections[3] + $0 * 4)) }
    }
    func bestReading(of word: String) -> (String, Double)? {
        guard let i = wordIndex(word) else { return nil }
        return readingEntry(uint(sections[0] + i * 24 + 16))
    }
}
