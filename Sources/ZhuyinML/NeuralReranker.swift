#if canImport(CoreML)
import CoreML
#endif
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
    /// log of how many characters share the model's <unk> token (5,465 lexicon characters are outside its 12,000):
    /// each gets its share of <unk>'s probability, not all of it, so rare characters do not outrank common ones.
    public var unknownPenalty = log(5465.0)
    public typealias Prediction = ([Int32], [Int32], Int, Int) throws -> [Double]
    private var remotePrediction: Prediction?
    #if canImport(CoreML)
    private var model: MLModel?
    #endif
    private let index: [Character: Int32]
    private let maxLength: Int
    private let lock = NSLock()

    public init(directory: URL, weight: Double = 0.5, prediction: @escaping Prediction) throws {
        self.weight = weight
        remotePrediction = prediction
        let vocabulary = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("vocab.json")))
        var index: [Character: Int32] = [:]
        for (id, token) in vocabulary.enumerated() where id > 2 && token.count == 1 { index[token.first!] = Int32(id) }
        self.index = index
        maxLength = 64
    }

    #if canImport(CoreML)
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

    #endif

    #if os(Linux)
    public convenience init(directory: URL, weight: Double = 0.5) throws {
        try self.init(directory: directory, weight: weight, prediction: ONNXModel.reranker(directory))
    }
    #endif

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
        var ids = [Int32](repeating: 0, count: candidates.count * length)
        var targets = ids
        for (row, candidate) in encoded.enumerated() {
            let tokens = prefix + candidate
            for column in 0..<length {
                ids[row * length + column] = column < tokens.count - 1 ? tokens[column] : 0
                targets[row * length + column] = column + 1 < tokens.count ? tokens[column + 1] : 0
            }
        }
        let logprob: [Double]
        do {
            if let remotePrediction {
                logprob = try lock.withLock { try remotePrediction(ids, targets, candidates.count, length) }
            } else {
                #if canImport(CoreML)
                let shape = [NSNumber(value: candidates.count), NSNumber(value: length)]
                let inputs = try MLMultiArray(shape: shape, dataType: .int32)
                let labels = try MLMultiArray(shape: shape, dataType: .int32)
                for i in ids.indices { inputs[i] = NSNumber(value: ids[i]); labels[i] = NSNumber(value: targets[i]) }
                let input = try MLDictionaryFeatureProvider(dictionary: ["ids": inputs, "targets": labels])
                let output = try lock.withLock { try model!.prediction(from: input) }
                guard let values = output.featureValue(for: "logprob")?.multiArrayValue else { return nil }
                logprob = (0..<values.count).map { values[$0].doubleValue }
                #else
                return nil
                #endif
            }
        } catch { return nil }
        guard logprob.count == ids.count, logprob.allSatisfy({ $0.isFinite }) else { return nil }
        let start = prefix.count - 1
        return encoded.enumerated().map { row, candidate in
            (start..<(start + candidate.count)).reduce(0.0) { $0 + logprob[row * length + $1] }
                - unknownPenalty * Double(candidate.filter { $0 == 2 }.count)
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
    public var exactMargin = 4.0

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
