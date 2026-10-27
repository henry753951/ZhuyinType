import Foundation
import ZhuyinCore

struct LMBigram {
    var first: String
    var second: String
    var logp: Float
    var backoff: Float = 0
}

struct LMTrigram {
    var first: String
    var second: String
    var third: String
    var logp: Float
}

/// Independent Swift writer for the FORMAT.md arrays, including alignment and little-endian fields.
func lmFixture(order: Int, unigrams: [String: Float], backoffs: [String: Float] = [:],
               bigrams: [LMBigram], trigrams: [LMTrigram] = []) -> Data {
    let reserved = ["</s>", "<s>", "<unk>"]
    let words = reserved + Set(unigrams.keys).subtracting(reserved).sorted {
        $0.utf8.lexicographicallyPrecedes($1.utf8)
    }
    precondition(Array(words.prefix(3)) == ["</s>", "<s>", "<unk>"])
    let ids = Dictionary(uniqueKeysWithValues: words.enumerated().map { ($0.element, UInt32($0.offset)) })
    let pairs = bigrams.sorted { (ids[$0.first]!, ids[$0.second]!) < (ids[$1.first]!, ids[$1.second]!) }
    var data = Data(repeating: 0, count: 64)
    func uint(_ value: UInt32) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { data.append(contentsOf: $0) }
    }
    func float(_ value: Float) { uint(value.bitPattern) }
    func align() { while data.count % 8 != 0 { data.append(0) } }
    let vocabulary = data.count
    var size: UInt32 = 0
    uint(0)
    for word in words { size += UInt32(word.utf8.count); uint(size) }
    for word in words { data.append(contentsOf: word.utf8) }
    align()
    let unigram = data.count
    for word in words { float(unigrams[word] ?? -5) }
    for word in words { float(backoffs[word] ?? 0) }
    align()
    let bigram = data.count
    var position: UInt32 = 0
    uint(0)
    for word in words { position += UInt32(pairs.filter { $0.first == word }.count); uint(position) }
    for pair in pairs { uint(ids[pair.second]!) }
    for pair in pairs { float(pair.logp) }
    for pair in pairs { float(pair.backoff) }
    align()
    let trigram = order == 3 ? data.count : 0
    if order == 3 {
        let groups = pairs.map { pair in
            trigrams.filter { $0.first == pair.first && $0.second == pair.second }
                .sorted { ids[$0.third]! < ids[$1.third]! }
        }
        precondition(groups.flatMap { $0 }.count == trigrams.count)
        position = 0
        uint(0)
        for group in groups { position += UInt32(group.count); uint(position) }
        for group in groups { for triple in group { uint(ids[triple.third]!) } }
        for group in groups { for triple in group { float(triple.logp) } }
        align()
    }
    var header = Data("ZTLM".utf8)
    func header32(_ value: Int) {
        var little = UInt32(value).littleEndian
        withUnsafeBytes(of: &little) { header.append(contentsOf: $0) }
    }
    func header64(_ value: Int) {
        var little = UInt64(value).littleEndian
        withUnsafeBytes(of: &little) { header.append(contentsOf: $0) }
    }
    for value in [1, order, words.count, pairs.count, trigrams.count] { header32(value) }
    for value in [vocabulary, unigram, bigram, trigram, data.count] { header64(value) }
    data.replaceSubrange(0..<64, with: header)
    return data
}

/// A 150 MB structural benchmark with the real lexicon vocabulary and dense, sorted trigram groups.
/// Its probabilities are synthetic; it measures loading/decoding, never conversion quality.
func largeLMFixture(words: Set<String>) -> Data {
    let words = words.union(["</s>", "<s>", "<unk>"]).sorted { $0.utf8.lexicographicallyPrecedes($1.utf8) }
    precondition(Array(words.prefix(3)) == ["</s>", "<s>", "<unk>"])
    let v = words.count, b = v * 2
    let strings = words.map { Array($0.utf8) }
    func align(_ n: Int) -> Int { (n + 7) & ~7 }
    let vocabulary = 64, blob = vocabulary + (v + 1) * 4
    let unigram = align(blob + strings.reduce(0) { $0 + $1.count })
    let bigram = align(unigram + v * 8)
    let trigram = align(bigram + (v + 1) * 4 + b * 12)
    let t = max(0, (150_000_000 - trigram - (b + 1) * 4) / 8)
    precondition(t / b + 1 < v)
    var data = Data(repeating: 0, count: align(trigram + (b + 1) * 4 + t * 8))
    let size = data.count
    data.withUnsafeMutableBytes { raw in
        func uint(_ at: Int, _ value: Int) { raw.storeBytes(of: UInt32(value).littleEndian, toByteOffset: at, as: UInt32.self) }
        func uint64(_ at: Int, _ value: Int) { raw.storeBytes(of: UInt64(value).littleEndian, toByteOffset: at, as: UInt64.self) }
        func float(_ at: Int, _ value: Float) { uint(at, Int(value.bitPattern)) }
        uint(0, 0x4d4c545a); uint(4, 1); uint(8, 3); uint(12, v); uint(16, b); uint(20, t)
        for (i, value) in [vocabulary, unigram, bigram, trigram, size].enumerated() { uint64(24 + i * 8, value) }
        var offset = 0
        for (i, bytes) in strings.enumerated() {
            uint(vocabulary + i * 4, offset)
            for byte in bytes { raw[blob + offset] = byte; offset += 1 }
            float(unigram + i * 4, -5)
            uint(bigram + i * 4, i * 2)
        }
        uint(vocabulary + v * 4, offset); uint(bigram + v * 4, b)
        for i in 0..<b {
            uint(bigram + (v + 1 + i) * 4, i % 2)
            float(bigram + (v + 1 + b + i) * 4, -5)
            let start = i * t / b, end = (i + 1) * t / b
            uint(trigram + i * 4, start)
            for j in start..<end {
                uint(trigram + (b + 1 + j) * 4, j - start)
                float(trigram + (b + 1 + t + j) * 4, -5)
            }
        }
        uint(trigram + b * 4, t)
    }
    return data
}

// The old, unpruned last-word Viterbi recurrence, independent of the new decoder's ID/history implementation.
func legacyBigramConversion(_ readings: [String], lexicon: String, model: BigramLanguageModel) -> (String, [Range<Int>]) {
    struct Node {
        var score: Double
        var last: String
        var previous: String
        var words: [String]
    }
    var entries: [String: [(String, Double)]] = [:]
    for line in lexicon.split(separator: "\n") where !line.hasPrefix("#") {
        let fields = line.split(separator: "\t")
        if fields.count == 3, let score = Double(fields[2]) {
            entries[String(fields[0]), default: []].append((String(fields[1]), score))
        }
    }
    var states = Array(repeating: [String: Node](), count: readings.count + 1)
    states[0][""] = Node(score: 0, last: "<s>", previous: "", words: [])
    for start in readings.indices {
        for end in (start + 1)...min(start + 6, readings.count) {
            let reading = readings[start..<end].joined(separator: " ")
            for (word, unigram) in entries[reading] ?? [] {
                for previous in states[start].values {
                    let total = previous.score + model.score(word: word, after: previous.last, unigram: unigram)
                    let old = states[end][word]
                    if total > (old?.score ?? -.infinity) || (total == old?.score && previous.last < old!.previous) {
                        states[end][word] = Node(score: total, last: word, previous: previous.last, words: previous.words + [word])
                    }
                }
            }
        }
    }
    let best = states[readings.count].values.max {
        let a = $0.score + model.endScore(after: $0.last), b = $1.score + model.endScore(after: $1.last)
        return a == b ? $0.last > $1.last : a < b
    }
    var position = 0
    let clauses = (best?.words ?? []).map { word -> Range<Int> in
        defer { position += word.utf16.count }
        return position..<(position + word.utf16.count)
    }
    return (best?.words.joined() ?? "", clauses)
}

func checkTSVBinaryMixture(in temp: URL) throws {
    let root = temp.appendingPathComponent("tsv-binary-mixture")
    let wiki = root.appendingPathComponent("wiki"), chat = root.appendingPathComponent("chat")
    for dir in [wiki, chat] {
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let extra = dir == chat ? "巴" : "八"
        let unigrams: [String: Float] = ["</s>": -1, "<unk>": -8, "!": -2, extra: -3]
        let bigrams: [LMBigram] = [.init(first: "<s>", second: extra, logp: -0.1),
                                  .init(first: extra, second: "!", logp: -0.2)]
        try lmFixture(order: 2, unigrams: unigrams, backoffs: ["<s>": -0.7, extra: -0.4], bigrams: bigrams)
            .write(to: dir.appendingPathComponent("lm.bin"))
        try Data("{\"tsv_compatibility\":true,\"missing_unigrams\":[1]}".utf8).write(to: dir.appendingPathComponent("lm.json"))
        try Data(unigrams.map { "\($0.key)\t\($0.value)\n" }.joined().utf8).write(to: dir.appendingPathComponent("unigram.tsv"))
        try Data(bigrams.map { "\($0.first)\t\($0.second)\t\($0.logp)\n" }.joined().utf8).write(to: dir.appendingPathComponent("bigram.tsv"))
        try Data("<s>\t-0.7\n\(extra)\t-0.4\n".utf8).write(to: dir.appendingPathComponent("backoff.tsv"))
    }
    let tsvWiki = try BigramLanguageModel(directory: wiki), tsvChat = try BigramLanguageModel(directory: chat)
    let binWiki = try NGramLanguageModel(directory: wiki), binChat = try NGramLanguageModel(directory: chat)
    check(binWiki.wordID(for: "!") == 3, "mapped LM supports ASCII before reserved spellings")
    let baseline = MixtureLanguageModel(chat: tsvChat, encyclopedia: tsvWiki, chatWeight: 0.5)
    let variants = [MixtureLanguageModel(chat: binChat, encyclopedia: binWiki, chatWeight: 0.5),
                    MixtureLanguageModel(chat: tsvChat, encyclopedia: binWiki, chatWeight: 0.5),
                    MixtureLanguageModel(chat: binChat, encyclopedia: tsvWiki, chatWeight: 0.5)]
    for word in ["八", "巴", "!", "</s>", "<s>", "<unk>", "不存在"] {
        for previous in ["八", "巴", "<s>", "<unk>", "不存在"] {
            let expected = baseline.score(word: word, after: previous, unigram: -9)
            for model in variants {
                check(abs(model.score(word: word, after: previous, unigram: -9) - expected) < 1e-6,
                      "mixed TSV/binary models preserve backoff for \(previous) → \(word)")
            }
        }
    }
}
