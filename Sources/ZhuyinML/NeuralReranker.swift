import CoreML
import Foundation
import ZhuyinCore

/// Chooses the displayed conversion among the engine's N best with a character language model.
///
/// Each candidate sentence is scored as log P(candidate | text before it) by one batched Core ML call; the choice is
/// the candidate with the highest engine score + `weight` × that log probability.  The model directory holds
/// `Reranker.mlmodelc` (or `Reranker.mlpackage`, compiled on first use) and `vocab.json`.
public final class NeuralReranker: @unchecked Sendable {
    public var weight: Double
    /// How many of the engine's best conversions to choose among.
    public var candidateCount = 16
    /// Characters of preceding text given to the model.
    public var contextLength = 24
    private let model: MLModel
    private let index: [Character: Int32]
    private let maxLength: Int
    private let lock = NSLock()

    /// `computeUnits` defaults to the CPU: for batches this small it answers in 2–11 ms, while the GPU path costs
    /// about 30 ms per call in dispatch overhead and the first call pays a 130 ms warm-up.
    public init(directory: URL, weight: Double = 0.5, computeUnits: MLComputeUnits = .cpuOnly) throws {
        self.weight = weight
        let vocabulary = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("vocab.json")))
        var index: [Character: Int32] = [:]
        for (id, token) in vocabulary.enumerated() where id > 2 && token.count == 1 { index[token.first!] = Int32(id) }
        self.index = index
        var modelURL = directory.appendingPathComponent("Reranker.mlmodelc")
        if !FileManager.default.fileExists(atPath: modelURL.path) {
            modelURL = try MLModel.compileModel(at: directory.appendingPathComponent("Reranker.mlpackage"))
        }
        let configuration = MLModelConfiguration()
        configuration.computeUnits = computeUnits
        model = try MLModel(contentsOf: modelURL, configuration: configuration)
        maxLength = 64
    }

    private func encode(_ text: String) -> [Int32] { text.map { index[$0] ?? 2 } }

    /// log P(candidate | context) for each candidate; nil when the model cannot score them.
    public func scores(context: String, candidates: [String]) -> [Double]? {
        guard !candidates.isEmpty else { return [] }
        let encoded = candidates.map(encode)
        let longest = encoded.map(\.count).max() ?? 0
        guard longest > 0, longest < maxLength else { return nil }
        let prefix = [Int32(1)] + encode(String(context.suffix(min(contextLength, maxLength - 1 - longest))))
        // The model accepts sequences of 2 or more; a lone character after <bos> is padded.
        let length = max(2, prefix.count - 1 + longest)
        guard let ids = try? MLMultiArray(shape: [NSNumber(value: candidates.count), NSNumber(value: length)], dataType: .int32),
              let targets = try? MLMultiArray(shape: ids.shape, dataType: .int32) else { return nil }
        for (row, candidate) in encoded.enumerated() {
            let tokens = prefix + candidate
            for column in 0..<length {
                let input = column < tokens.count - 1 ? tokens[column] : 0
                let target = column + 1 < tokens.count ? tokens[column + 1] : 0
                ids[row * length + column] = NSNumber(value: input)
                targets[row * length + column] = NSNumber(value: target)
            }
        }
        let output: MLFeatureProvider
        do {
            let input = try MLDictionaryFeatureProvider(dictionary: ["ids": ids, "targets": targets])
            output = try lock.withLock { try model.prediction(from: input) }
        } catch { return nil }
        guard let logprob = output.featureValue(for: "logprob")?.multiArrayValue else { return nil }
        let start = prefix.count - 1
        return encoded.enumerated().map { row, candidate in
            (start..<(start + candidate.count)).reduce(0.0) { $0 + logprob[row * length + $1].doubleValue }
        }
    }

    /// `weight` × log P(candidate | context) for candidate-list ordering, scored in batches the model accepts.
    public func candidateScores(context: String, candidates: [String]) -> [Double]? {
        var result: [Double] = []
        for start in stride(from: 0, to: candidates.count, by: 32) {
            guard let part = scores(context: context, candidates: Array(candidates[start..<min(candidates.count, start + 32)]))
            else { return nil }
            result += part.map { weight * $0 }
        }
        return result
    }

    /// How much better a corrected sentence must score than the best one matching the keys as typed to be shown.
    public var exactMargin = 2.0

    /// The index of the sentence to display.  The keys as typed win unless a correction scores clearly better.
    public func choose(_ sentences: [ScoredSentence], context: String) -> Int {
        guard sentences.count > 1, let neural = scores(context: context, candidates: sentences.map(\.text)) else { return 0 }
        let totals = sentences.indices.map { sentences[$0].score + weight * neural[$0] }
        let best = totals.indices.max { totals[$0] < totals[$1] }!
        guard sentences[best].corrected,
              let exact = totals.indices.filter({ !sentences[$0].corrected }).max(by: { totals[$0] < totals[$1] }),
              totals[best] - totals[exact] < exactMargin else { return best }
        return exact
    }
}
