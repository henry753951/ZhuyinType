import Foundation
import ZhuyinCore

/// Independent writer for a three-entry lexicon: neutral tone, two readings, and a shared word.
func lexiconFixture() -> Data {
    var data = Data(repeating: 0, count: 128)
    func u32(_ value: UInt32) { var v = value.littleEndian; withUnsafeBytes(of: &v) { data.append(contentsOf: $0) } }
    func u64(_ value: UInt64) { var v = value.littleEndian; withUnsafeBytes(of: &v) { data.append(contentsOf: $0) } }
    var blob = Data()
    func string(_ text: String) -> [UInt32] {
        let bytes = Data(text.utf8), result = [UInt32(blob.count), UInt32(bytes.count)]
        blob.append(bytes); return result
    }
    let words = [string("八"), string("吧")]
    let readings = [string("ㄅㄚ"), string("ㄅㄚ˙")]
    let base = string("ㄅㄚ")
    var offsets: [UInt64] = []
    func section() { while data.count % 8 != 0 { data.append(0) }; offsets.append(UInt64(data.count)) }
    section()
    for row in [words[0] + [0, 1, 0, 0], words[1] + [1, 2, 2, 0]] { row.forEach(u32) }
    section()
    for row in [readings[0] + [0, 2], readings[1] + [2, 1]] { row.forEach(u32) }
    section()
    for (word, reading, logp, prior): (UInt32, UInt32, Double, Double) in [(0, 0, -3, 0), (1, 0, -2, -8), (1, 1, -2, 0)] {
        u32(word); u32(reading); u64(logp.bitPattern); u64(prior.bitPattern)
    }
    section(); [0, 1, 2].forEach(u32)
    section(); (base + [0, 2]).forEach(u32)
    section(); [0, 1].forEach(u32)
    section(); base.forEach(u32)
    section(); readings.flatMap { $0 }.forEach(u32)
    section(); data.append(blob)
    let size = UInt64(data.count), body = data.dropFirst(128)
    data = Data()
    [0x584c545a, 1, 2, 2, 3, 1, 1, 2].forEach(u32)
    (offsets + [size, 0, 0]).forEach(u64)
    data.append(body)
    return data
}

func checkMappedLexicon(in temp: URL) throws {
    let dir = temp.appendingPathComponent("mapped-lexicon")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let url = dir.appendingPathComponent("lexicon.bin"), tsv = dir.appendingPathComponent("lexicon.tsv")
    let bytes = lexiconFixture()
    try bytes.write(to: url)
    let engine = try Engine(lexiconURL: tsv, userDataDirectory: nil, languageModelDirectory: nil)
    check(composer(engine, "18 ").presentation.text == "吧", "mapped reading prior and neutral-tone lookup")
    check(engine.readings(of: "吧") == ["ㄅㄚ˙", "ㄅㄚ"], "mapped reverse index ordered by original score")
    check(engine.readings(of: "八吧") == ["ㄅㄚ ㄅㄚ˙"], "mapped best-reading fallback")
    try engine.addUserPhrase(word: "八", reading: "ㄅㄚ")
    check(composer(engine, "18 ").presentation.text == "八", "personal phrase overlays mapped entry")
    engine.removeUserPhrase(word: "八", reading: "ㄅㄚ")
    check(composer(engine, "18 ").presentation.text == "吧", "removing overlay preserves mapped entries")
    try engine.addUserPhrase(word: "謝謝", reading: "ㄒㄧㄝˋ ㄒㄧㄝ˙")
    check(composer(engine, "vu,4vu,4").presentation.text == "謝謝", "mapped lexicon accepts new personal syllables")
    engine.removeUserPhrase(word: "謝謝", reading: "ㄒㄧㄝˋ ㄒㄧㄝ˙")
    check(composer(engine, "vu,4").presentation.text == "ㄒㄧㄝ", "removed personal syllables leave mapped index intact")
    check(composer(engine, "18 ").commitAll() == "吧", "mapped word IDs support learning")
    check(engine.exportPersonalData().count > 0, "mapped engine exports personal data")
    for length in 0..<bytes.count {
        try bytes.prefix(length).write(to: url, options: .atomic)
        check((try? Engine(lexiconURL: tsv, userDataDirectory: nil)) == nil, "reject truncated lexicon \(length)")
    }
    var corrupt = bytes
    corrupt.replaceSubrange(128..<132, with: [255, 255, 255, 255])
    try corrupt.write(to: url, options: .atomic)
    check((try? Engine(lexiconURL: tsv, userDataDirectory: nil)) == nil, "reject lexicon string outside mapping")
    try FileManager.default.removeItem(at: url)
    try Data("ㄅㄚ\t八\t-3\n".utf8).write(to: tsv)
    let fallback = try Engine(lexiconURL: tsv, userDataDirectory: nil)
    check(composer(fallback, "18 ").presentation.text == "八", "absent binary falls back to TSV")
}
