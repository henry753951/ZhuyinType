#if canImport(CoreML)
import CoreML
#endif
import Foundation

/// The v3 character model: a causal context encoder run once per composition, and a bidirectional syllable encoder
/// that, for every syllable of the composition, gives log P(character | all syllables, the text before) over the
/// characters it may be.  The engine adds these scores to its word lattice.  Model files and vocabularies come from
/// nlp/training/v3 (data/build/v3/config.json).
public final class SyllableScorer: @unchecked Sendable {
    /// How much the model's scores count in the lattice in the app (docs/MODEL.md §6).
    public static let appWeight = 0.5
    /// One syllable of the composition as the model reads it.
    public struct Syllable: Equatable {
        /// Toneless reading, e.g. "ㄓㄨㄥ"; nil for a literal (punctuation, symbols).
        public var base: String?
        /// 0 none (literal), 1 first, 2 ˊ, 3 ˇ, 4 ˋ, 5 ˙.
        public var tone: Int
        /// How far the reading is from the keys typed: 0 exact, 1 small, 2 medium, 3 large correction.
        public var costBucket: Int
        /// The syllable ended without a tone key or Space.
        public var implicit: Bool
        public init(base: String?, tone: Int, costBucket: Int, implicit: Bool) {
            self.base = base; self.tone = tone; self.costBucket = costBucket; self.implicit = implicit
        }
    }

    public static let maxCandidates = 64
    public static let maxContext = 255

    public typealias Prediction = ([Int32], [Int32], [Int32], [Int32], [Int32], [[Int32]]) throws -> [[Double]]
    private var remotePrediction: Prediction?
    #if canImport(CoreML)
    private var context: MLModel!
    private var syllable: MLModel!
    private var cachedContext: (text: String, k: MLMultiArray, v: MLMultiArray)?
    #endif
    private let characterIDs: [Character: Int32]
    private let syllableIDs: [String: Int32]
    private let lock = NSLock()

    #if canImport(CoreML)
    public init(directory: URL, computeUnits: MLComputeUnits = .cpuOnly) throws {
        let configuration = MLModelConfiguration()
        configuration.computeUnits = computeUnits
        func load(_ name: String) throws -> MLModel {
            let compiled = directory.appendingPathComponent(name + ".mlmodelc")
            let url = FileManager.default.fileExists(atPath: compiled.path) ? compiled
                : try MLModel.compileModel(at: directory.appendingPathComponent(name + ".mlpackage"))
            return try MLModel(contentsOf: url, configuration: configuration)
        }
        context = try load("ContextEncoder")
        syllable = try load("SyllableEncoder")
        let characters = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("chars.json")))
        var ids: [Character: Int32] = [:]
        for (id, token) in characters.enumerated() where id > 3 && token.count == 1 { ids[token.first!] = Int32(id) }
        characterIDs = ids
        let syllables = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("syllables.json")))
        syllableIDs = Dictionary(uniqueKeysWithValues: syllables.enumerated().dropFirst(2).map { ($1, Int32($0)) })
    }

    #endif

    #if os(Linux)
    public convenience init(directory: URL) throws {
        try self.init(directory: directory, prediction: ONNXModel.syllable(directory))
    }
    #endif

    public init(directory: URL, prediction: @escaping Prediction) throws {
        remotePrediction = prediction
        let characters = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("chars.json")))
        var ids: [Character: Int32] = [:]
        for (id, token) in characters.enumerated() where id > 3 && token.count == 1 { ids[token.first!] = Int32(id) }
        characterIDs = ids
        let syllables = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("syllables.json")))
        syllableIDs = Dictionary(uniqueKeysWithValues: syllables.enumerated().dropFirst(2).map { ($1, Int32($0)) })
    }

    public func characterID(_ character: Character) -> Int32 { characterIDs[character] ?? 2 }

    #if canImport(CoreML)
    private func array(_ shape: [Int], _ values: [Int32]) throws -> MLMultiArray {
        let array = try MLMultiArray(shape: shape.map { NSNumber(value: $0) }, dataType: .int32)
        let pointer = array.dataPointer.bindMemory(to: Int32.self, capacity: values.count)
        for (index, value) in values.enumerated() { pointer[index] = value }
        return array
    }

    /// The context memory for `text` (the text before the composition), computed once per distinct text.
    private func memory(for text: String) throws -> (MLMultiArray, MLMultiArray) {
        if let cached = cachedContext, cached.text == text { return (cached.k, cached.v) }
        let tail = text.suffix(Self.maxContext)
        let ids: [Int32] = [1] + tail.map { $0 == "\n" ? 3 : characterID($0) }
        let input = try MLDictionaryFeatureProvider(dictionary: ["context_ids": try array([1, ids.count], ids)])
        let output = try context.prediction(from: input)
        guard let k = output.featureValue(for: "memory_k")?.multiArrayValue,
              let v = output.featureValue(for: "memory_v")?.multiArrayValue else {
            throw NSError(domain: "ZhuyinML", code: 1, userInfo: [NSLocalizedDescriptionKey: "context encoder output"])
        }
        cachedContext = (text, k, v)
        return (k, v)
    }

    #endif

    /// For each syllable, log P over its candidate characters (same order; at most `maxCandidates` each, the caller
    /// prunes).  Nil when the model cannot run or the input is empty or too long.
    /// The last request and its answer: a key that leaves the syllables and candidates as they were is answered
    /// without running the model.
    private var lastRequest: (context: String, syllables: [Syllable], candidates: [[Character]], scores: [[Double]])?

    public func scores(context text: String, syllables: [Syllable], candidates: [[Character]]) -> [[Double]]? {
        guard !syllables.isEmpty, syllables.count == candidates.count, syllables.count <= 64 else { return nil }
        if let last = lock.withLock({ lastRequest }), last.context == text, last.syllables == syllables,
           last.candidates == candidates {
            return last.scores
        }
        let result = computeScores(context: text, syllables: syllables, candidates: candidates)
        if let result { lock.withLock { lastRequest = (text, syllables, candidates, result) } }
        return result
    }

    private func computeScores(context text: String, syllables: [Syllable], candidates: [[Character]]) -> [[Double]]? {
        let width = max(1, min(Self.maxCandidates, candidates.map(\.count).max() ?? 1))
        let count = syllables.count
        var candidateIDs = [Int32](repeating: 0, count: count * width)
        for (row, list) in candidates.enumerated() {
            let ids = list.isEmpty ? [Int32(2)] : list.prefix(width).map(characterID)
            for (column, id) in ids.enumerated() { candidateIDs[row * width + column] = id }
        }
        let bases = syllables.map { $0.base.flatMap { syllableIDs[$0] } ?? 1 }
        let tones = syllables.map { Int32($0.tone) }
        let costs = syllables.map { Int32($0.costBucket) }
        let boundaries: [Int32] = syllables.map { $0.implicit ? 1 : 0 }
        if let remotePrediction {
            let ids = (0..<count).map { Array(candidateIDs[($0 * width)..<(($0 + 1) * width)]) }
            let output = try? lock.withLock { try remotePrediction([1] + text.suffix(Self.maxContext).map { $0 == "\n" ? 3 : characterID($0) },
                bases, tones, costs, boundaries, ids) }
            guard let output, output.count == candidates.count,
                  output.allSatisfy({ $0.count == width && $0.allSatisfy { $0.isFinite } }) else { return nil }
            return candidates.enumerated().map { row, list in Array(output[row].prefix(min(list.count, width))) }
        }
        #if canImport(CoreML)
        return lock.withLock {
            do {
                let (k, v) = try memory(for: text)
                let inputs: [String: Any] = [
                    "syllable_ids": try array([1, count], bases),
                    "tone_ids": try array([1, count], tones),
                    "cost_bucket": try array([1, count], costs),
                    "boundary": try array([1, count], boundaries),
                    "candidate_ids": try array([1, count, width], candidateIDs),
                    "memory_k": k, "memory_v": v,
                ]
                let output = try syllable.prediction(from: MLDictionaryFeatureProvider(dictionary: inputs))
                guard let logprob = output.featureValue(for: "logprob")?.multiArrayValue else { return nil }
                return candidates.enumerated().map { row, list in
                    (0..<min(list.count, width)).map { logprob[row * width + $0].doubleValue }
                }
            } catch { return nil }
        }
        #else
        return nil
        #endif
    }

    /// Runs the model on raw ids (for checking against nlp/training/v3 test vectors).
    public func rawScores(contextIDs: [Int32], syllableIDs: [Int32], tones: [Int32], costs: [Int32], boundaries: [Int32],
                          candidates: [[Int32]]) throws -> [[Double]] {
        if let remotePrediction {
            return try lock.withLock { try remotePrediction(contextIDs, syllableIDs, tones, costs, boundaries, candidates) }
        }
        #if canImport(CoreML)
        let input = try MLDictionaryFeatureProvider(dictionary: ["context_ids": try array([1, contextIDs.count], contextIDs)])
        let memory = try context.prediction(from: input)
        let count = syllableIDs.count, width = candidates.map(\.count).max() ?? 1
        var flat = [Int32](repeating: 0, count: count * width)
        for (row, list) in candidates.enumerated() { for (column, id) in list.enumerated() { flat[row * width + column] = id } }
        let output = try syllable.prediction(from: MLDictionaryFeatureProvider(dictionary: [
            "syllable_ids": try array([1, count], syllableIDs), "tone_ids": try array([1, count], tones),
            "cost_bucket": try array([1, count], costs), "boundary": try array([1, count], boundaries),
            "candidate_ids": try array([1, count, width], flat),
            "memory_k": memory.featureValue(for: "memory_k")!.multiArrayValue!,
            "memory_v": memory.featureValue(for: "memory_v")!.multiArrayValue!,
        ]))
        let logprob = output.featureValue(for: "logprob")!.multiArrayValue!
        return candidates.enumerated().map { row, list in (0..<list.count).map { logprob[row * width + $0].doubleValue } }
        #else
        throw NSError(domain: "ZhuyinML", code: 1, userInfo: [NSLocalizedDescriptionKey: "Use --model-server on Linux"])
        #endif
    }
}
