import Foundation
import XCTest
@testable import ZhuyinCore
@testable import ZhuyinML

final class BackendTests: XCTestCase {
    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        for (name, tokens) in [("vocab", ["<pad>", "<bos>", "<unk>", "甲", "乙"]),
                               ("chars", ["<pad>", "<bos>", "<unk>", "<sep>", "甲", "乙"]),
                               ("syllables", ["<pad>", "<literal>", "ㄐㄧㄚ"]) ] {
            try JSONEncoder().encode(tokens).write(to: url.appendingPathComponent(name + ".json"))
        }
        addTeardownBlock { try FileManager.default.removeItem(at: url) }
        return url
    }

    func testRerankerInputsPenaltyBatchingAndMargin() throws {
        var shapes: [(Int, Int)] = []
        let model = try NeuralReranker(directory: directory(), weight: 2) { ids, targets, batch, length in
            shapes.append((batch, length))
            XCTAssertEqual(ids.count, batch * length)
            XCTAssertEqual(targets.count, ids.count)
            XCTAssertEqual(ids[0], 1)
            return [Double](repeating: -1, count: ids.count)
        }
        XCTAssertEqual(model.scores(context: "", candidates: ["甲"]), [-1])
        XCTAssertEqual(shapes.last!.1, 2)
        XCTAssertEqual(model.scores(context: "", candidates: ["丙"])![0], -1 - log(5465), accuracy: 1e-10)
        _ = model.scores(context: String(repeating: "甲", count: 100), candidates: ["甲乙"])
        XCTAssertEqual(shapes.last!.1, 26) // Last 24 context characters, BOS predicts the first.
        XCTAssertEqual(model.candidateScores(context: "", candidates: Array(repeating: "甲", count: 65)), Array(repeating: -2, count: 65))
        XCTAssertEqual(shapes.suffix(3).map(\.0), [32, 32, 1])
        func sentence(_ text: String, _ score: Double, corrected: Bool) -> ScoredSentence {
            ScoredSentence(words: [SentenceWord(range: 0..<1, text: text, reading: "", corrected: corrected)], score: score)
        }
        XCTAssertEqual(model.choose([sentence("甲", 0, corrected: false), sentence("乙", 3.9, corrected: true)], context: ""), 0)
        XCTAssertEqual(model.choose([sentence("甲", 0, corrected: false), sentence("乙", 4, corrected: true)], context: ""), 1)
        XCTAssertNil(model.scores(context: "", candidates: [String(repeating: "甲", count: 64)]))
    }

    func testSyllableMappingLimitsAndOutputShape() throws {
        let model = try SyllableScorer(directory: directory()) { context, ids, tones, costs, boundary, candidates in
            XCTAssertEqual(context.count, 256)
            XCTAssertEqual(context.last, 3)
            XCTAssertEqual(ids, [2, 1])
            XCTAssertEqual(tones, [1, 0])
            XCTAssertEqual(costs, [2, 0])
            XCTAssertEqual(boundary, [1, 0])
            XCTAssertEqual(candidates[0].count, 64)
            XCTAssertEqual(candidates[1][0], 2)
            XCTAssertEqual(candidates[1][1], 0)
            return candidates.map { $0.map { _ in -0.5 } }
        }
        let input = [SyllableScorer.Syllable(base: "ㄐㄧㄚ", tone: 1, costBucket: 2, implicit: true),
                     SyllableScorer.Syllable(base: nil, tone: 0, costBucket: 0, implicit: false)]
        let result = model.scores(context: String(repeating: "甲", count: 300) + "\n", syllables: input,
                                  candidates: [Array(repeating: "甲", count: 70), []])
        XCTAssertEqual(result?.map(\.count), [64, 0])
        XCTAssertNil(model.scores(context: "", syllables: [], candidates: []))
        let bad = try SyllableScorer(directory: directory()) { _, _, _, _, _, _ in [[.nan]] }
        XCTAssertNil(bad.scores(context: "", syllables: [input[0]], candidates: [["甲"]]))
    }
}
