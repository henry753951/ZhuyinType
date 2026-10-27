#if canImport(CoreML)
import CoreML
#endif
import Foundation
import ZhuyinCore

/// Chooses the displayed conversion among the engine's N best with a character language model.
///
/// Each candidate sentence is scored as log P(candidate | text before it) by one Core ML / ONNX call; the choice is
/// the candidate with the highest engine score + `weight` × that log probability.  The model directory holds
/// `Reranker.mlmodelc` (or `Reranker.mlpackage`, compiled on first use) and `vocab.json`.
/// An optional `RerankerTree` model shares candidate prefixes; the batched model remains the fallback.
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
    typealias TreePrediction = (RerankerTree) throws -> [Double]
    var treePrediction: TreePrediction?
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
        let treeURL = directory.appendingPathComponent("RerankerTree.mlmodelc")
        if FileManager.default.fileExists(atPath: treeURL.path),
           let tree = try? MLModel(contentsOf: treeURL, configuration: configuration) {
            treePrediction = { packed in
                func array(_ values: [Int32], _ shape: [Int]) throws -> MLMultiArray {
                    let array = try MLMultiArray(shape: shape.map(NSNumber.init(value:)), dataType: .int32)
                    let pointer = array.dataPointer.bindMemory(to: Int32.self, capacity: values.count)
                    values.withUnsafeBufferPointer { pointer.update(from: $0.baseAddress!, count: values.count) }
                    return array
                }
                let n = packed.ids.count
                let input = try MLDictionaryFeatureProvider(dictionary: [
                    "ids": try array(packed.ids, [1, n]), "positions": try array(packed.positions, [1, n]),
                    "ancestors": try array(packed.ancestors, [n, n]), "parents": try array(packed.parents, [packed.targets.count]),
                    "targets": try array(packed.targets, [packed.targets.count])])
                let output = try tree.prediction(from: input)
                guard let values = output.featureValue(for: "logprob")?.multiArrayValue else { return [] }
                return (0..<values.count).map { values[$0].doubleValue }
            }
        }
        maxLength = 64
    }

    #endif

    /// The reranker as the app loads it: weight and sentence count from the directory's config.json (weight 0.5 and 8
    /// sentences without one).  Core ML on macOS, ONNX Runtime on Linux.
    public static func configured(directory: URL) throws -> NeuralReranker {
        let config = (try? Data(contentsOf: directory.appendingPathComponent("config.json")))
            .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        let reranker = try NeuralReranker(directory: directory, weight: config?["weight"] as? Double ?? 0.5)
        if let count = config?["count"] as? Int, count > 0 { reranker.candidateCount = count }
        return reranker
    }

    #if os(Linux)
    public convenience init(directory: URL, weight: Double = 0.5) throws {
        try self.init(directory: directory, weight: weight, prediction: ONNXModel.reranker(directory))
        if FileManager.default.fileExists(atPath: directory.appendingPathComponent("RerankerTree.onnx").path) {
            treePrediction = try? ONNXModel.rerankerTree(directory)
        }
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
        if let treePrediction, candidates.count <= 32 {
            let packed = RerankerTree(prefix: prefix, candidates: encoded)
            if let logprob = try? lock.withLock({ try treePrediction(packed) }),
               let result = packed.scores(logprob, unknownPenalty: unknownPenalty) { return result }
        }
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
    /// τ: the model is consulted only when the engine's two best sentences are this close.  At 8 the outputs on the
    /// three dev sets are identical to always consulting it (1,200/1,200 runs), and short compositions skip the
    /// model often enough to cut per-key p95 from 24 to 15 ms (docs/engine/MODEL.md §7).
    public var gateMargin = 8.0
    /// δ: the model's choice replaces the engine's first sentence only when it wins by at least this much.  Any
    /// δ > 0 lost accuracy on every dev set (its switches are mostly right), so it stays 0.
    public var switchMargin = 0.0

    /// The index of the sentence to display.  The keys as typed win unless a correction scores clearly better.
    /// The last texts scored for `choose`: a key that adds only an unfinished Zhuyin symbol leaves the sentences as
    /// they were, and is answered without running the model.
    private var lastChoice: (context: String, texts: [String], neural: [Double])?

    public func choose(_ sentences: [ScoredSentence], context: String) -> Int {
        // The engine is sure (its two best are far apart): keep its choice without running the model.
        if sentences.count > 1, sentences[0].score - sentences[1].score >= gateMargin { return 0 }
        let texts = sentences.map(\.text)
        let cached = lock.withLock { lastChoice.flatMap { $0.context == context && $0.texts == texts ? $0.neural : nil } }
        guard sentences.count > 1, let neural = cached ?? scores(context: context, candidates: texts) else { return 0 }
        if cached == nil { lock.withLock { lastChoice = (context, texts, neural) } }
        let totals = sentences.indices.map { sentences[$0].score + weight * neural[$0] }
        var best = totals.indices.max { totals[$0] < totals[$1] }!
        if best != 0, totals[best] - totals[0] < switchMargin { best = 0 }
        guard sentences[best].corrected,
              let exact = totals.indices.filter({ !sentences[$0].corrected }).max(by: { totals[$0] < totals[$1] }),
              totals[best] - totals[exact] < exactMargin else { return best }
        return exact
    }
}
