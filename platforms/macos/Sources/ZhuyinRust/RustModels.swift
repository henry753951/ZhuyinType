// The neural models for the Rust engine: their decisions (gate, margins, tokenization, caching) run in Rust
// (zhuyin-models), their inference here with Core ML, called back from Rust.  The Core ML calls are the ones
// ZhuyinML makes, so the scores are the same.
import CoreML
import CZhuyin
import Foundation
import ZhuyinCore

private func compiledModel(_ directory: URL, _ name: String, _ configuration: MLModelConfiguration) throws -> MLModel {
    let compiled = directory.appendingPathComponent(name + ".mlmodelc")
    let url = FileManager.default.fileExists(atPath: compiled.path) ? compiled
        : try MLModel.compileModel(at: directory.appendingPathComponent(name + ".mlpackage"))
    return try MLModel(contentsOf: url, configuration: configuration)
}

private func int32Array(_ values: UnsafePointer<Int32>, _ shape: [Int]) throws -> MLMultiArray {
    let array = try MLMultiArray(shape: shape.map(NSNumber.init(value:)), dataType: .int32)
    let count = shape.reduce(1, *)
    array.dataPointer.bindMemory(to: Int32.self, capacity: count).update(from: values, count: count)
    return array
}

/// Core ML inference of the reranker: the batched model and, when present, the prefix-tree model.
private final class CoreMLReranker {
    let batch: MLModel
    let tree: MLModel?

    init(directory: URL, computeUnits: MLComputeUnits) throws {
        let configuration = MLModelConfiguration()
        configuration.computeUnits = computeUnits
        batch = try compiledModel(directory, "Reranker", configuration)
        let treeURL = directory.appendingPathComponent("RerankerTree.mlmodelc")
        tree = FileManager.default.fileExists(atPath: treeURL.path)
            ? try? MLModel(contentsOf: treeURL, configuration: configuration) : nil
    }

    func logprob(ids: UnsafePointer<Int32>, targets: UnsafePointer<Int32>, rows: Int, length: Int,
                 out: UnsafeMutablePointer<Double>) -> Bool {
        do {
            let input = try MLDictionaryFeatureProvider(dictionary: [
                "ids": try int32Array(ids, [rows, length]), "targets": try int32Array(targets, [rows, length])])
            guard let values = try batch.prediction(from: input).featureValue(for: "logprob")?.multiArrayValue,
                  values.count == rows * length else { return false }
            for i in 0..<values.count { out[i] = values[i].doubleValue }
            return true
        } catch { return false }
    }

    func treeLogprob(_ packed: ZtPackedTree, out: UnsafeMutablePointer<Double>) -> Bool {
        guard let tree else { return false }
        do {
            let n = packed.node_count, t = packed.target_count
            let input = try MLDictionaryFeatureProvider(dictionary: [
                "ids": try int32Array(packed.ids, [1, n]), "positions": try int32Array(packed.positions, [1, n]),
                "ancestors": try int32Array(packed.ancestors, [n, n]), "parents": try int32Array(packed.parents, [t]),
                "targets": try int32Array(packed.targets, [t])])
            guard let values = try tree.prediction(from: input).featureValue(for: "logprob")?.multiArrayValue,
                  values.count == t else { return false }
            for i in 0..<values.count { out[i] = values[i].doubleValue }
            return true
        } catch { return false }
    }
}

/// Core ML inference of the character model: the context encoder's memory is kept for the last context.
private final class CoreMLSyllableModel {
    let context: MLModel
    let syllable: MLModel
    private var cached: (ids: [Int32], k: MLMultiArray, v: MLMultiArray)?

    init(directory: URL, computeUnits: MLComputeUnits) throws {
        let configuration = MLModelConfiguration()
        configuration.computeUnits = computeUnits
        context = try compiledModel(directory, "ContextEncoder", configuration)
        syllable = try compiledModel(directory, "SyllableEncoder", configuration)
    }

    func predict(_ input: ZtSyllableInput, out: UnsafeMutablePointer<Double>) -> Bool {
        do {
            let ids = Array(UnsafeBufferPointer(start: input.context_ids, count: input.context_count))
            if cached?.ids != ids {
                let output = try context.prediction(from: MLDictionaryFeatureProvider(dictionary: [
                    "context_ids": try int32Array(input.context_ids, [1, ids.count])]))
                guard let k = output.featureValue(for: "memory_k")?.multiArrayValue,
                      let v = output.featureValue(for: "memory_v")?.multiArrayValue else { return false }
                cached = (ids, k, v)
            }
            let count = input.count, width = input.width
            let output = try syllable.prediction(from: MLDictionaryFeatureProvider(dictionary: [
                "syllable_ids": try int32Array(input.syllable_ids, [1, count]),
                "tone_ids": try int32Array(input.tones, [1, count]),
                "cost_bucket": try int32Array(input.costs, [1, count]),
                "boundary": try int32Array(input.boundaries, [1, count]),
                "candidate_ids": try int32Array(input.candidates, [1, count, width]),
                "memory_k": cached!.k, "memory_v": cached!.v,
            ]))
            guard let logprob = output.featureValue(for: "logprob")?.multiArrayValue,
                  logprob.count >= count * width else { return false }
            for i in 0..<(count * width) { out[i] = logprob[i].doubleValue }
            return true
        } catch { return false }
    }
}

private let releaseObject: @convention(c) (UnsafeMutableRawPointer?) -> Void = {
    Unmanaged<AnyObject>.fromOpaque($0!).release()
}

/// The sentence reranker: `NeuralReranker`'s decisions in Rust, Core ML inference here.
public final class RustReranker: @unchecked Sendable {
    let handle: OpaquePointer

    /// The reranker as the app loads it: weight and sentence count from the directory's config.json.
    public static func configured(directory: URL, computeUnits: MLComputeUnits = .cpuOnly) throws -> RustReranker {
        let model = try CoreMLReranker(directory: directory, computeUnits: computeUnits)
        let host = ZtRerankerModel(
            context: Unmanaged.passRetained(model as AnyObject).toOpaque(),
            logprob: { context, ids, targets, rows, length, out in
                let model = Unmanaged<AnyObject>.fromOpaque(context!).takeUnretainedValue() as! CoreMLReranker
                return model.logprob(ids: ids!, targets: targets!, rows: rows, length: length, out: out!)
            },
            tree_logprob: model.tree == nil ? nil : { context, tree, out in
                let model = Unmanaged<AnyObject>.fromOpaque(context!).takeUnretainedValue() as! CoreMLReranker
                return model.treeLogprob(tree!.pointee, out: out!)
            },
            release: releaseObject)
        return RustReranker(handle: try checked { error in zt_reranker_configured(directory.path, host, error) })
    }

    private init(handle: OpaquePointer) { self.handle = handle }
    deinit { zt_reranker_free(handle) }

    /// How many of the engine's best sentences to choose among.
    public var candidateCount: Int { zt_reranker_candidate_count(handle) }

    /// The index of the sentence to display.
    public func choose(_ sentences: RustSentences, context: String) -> Int {
        zt_reranker_choose(handle, sentences.pointer, context)
    }

    /// `weight` × log P(candidate | context) for candidate-list order, written into the hook's buffer.
    public func candidateScores(context: String, candidates: RustCandidates) -> RustScores? {
        zt_reranker_candidate_scores_view(handle, context, candidates.pointer, candidates.scores) ? RustScores() : nil
    }

    /// log P(candidate | context) for each candidate; nil when the model cannot score them.
    public func scores(context: String, candidates: [String]) -> [Double]? {
        var scores = [Double](repeating: 0, count: candidates.count)
        let strings = candidates.map { strdup($0) }
        defer { strings.forEach { free($0) } }
        let pointers: [UnsafePointer<CChar>?] = strings.map { $0.map { UnsafePointer($0) } }
        let ok = pointers.withUnsafeBufferPointer { list in
            zt_reranker_scores(handle, context, list.baseAddress, candidates.count, &scores)
        }
        return ok ? scores : nil
    }
}

/// The v3 character model: `SyllableScorer`'s decisions in Rust, Core ML inference here.
public final class RustSyllableScorer: @unchecked Sendable {
    /// How much the model's scores count in the lattice in the app.
    public static let appWeight = 0.5

    /// One syllable of the composition as the model reads it.
    public struct Syllable: Equatable {
        public var base: String?
        public var tone: Int
        public var costBucket: Int
        public var implicit: Bool
        public init(base: String?, tone: Int, costBucket: Int, implicit: Bool) {
            self.base = base; self.tone = tone; self.costBucket = costBucket; self.implicit = implicit
        }
    }

    let handle: OpaquePointer

    public init(directory: URL, computeUnits: MLComputeUnits = .cpuOnly) throws {
        let model = try CoreMLSyllableModel(directory: directory, computeUnits: computeUnits)
        let host = ZtSyllableModel(
            context: Unmanaged.passRetained(model as AnyObject).toOpaque(),
            predict: { context, input, out in
                let model = Unmanaged<AnyObject>.fromOpaque(context!).takeUnretainedValue() as! CoreMLSyllableModel
                return model.predict(input!.pointee, out: out!)
            },
            release: releaseObject)
        handle = try checked { error in zt_syllable_scorer_new(directory.path, host, error) }
    }

    deinit { zt_syllable_scorer_free(handle) }

    /// Checks the model against `directory/test_vectors.json` (the v3 golden outputs), as `zhuyin-cli --check-v3`.
    public func checkVectors(directory: URL) throws -> (passed: Int, total: Int, worst: Double) {
        var result = ZtVectorCheck()
        _ = try checked { error in zt_syllable_scorer_check_vectors(handle, directory.path, &result, error) ? true : nil }
        return (result.passed, result.total, result.worst)
    }

    /// For each syllable, log P over its candidate characters, written into the hook's buffers.
    public func scores(context: String, syllables: [Syllable], candidates: RustCandidateRows) -> RustScores? {
        let bases = syllables.map { $0.base.flatMap { strdup($0) } }
        defer { bases.forEach { free($0) } }
        let views = zip(syllables, bases).map { syllable, base in
            ZtSyllable(base: base.map { UnsafePointer($0) }, tone: Int32(syllable.tone),
                       cost_bucket: Int32(syllable.costBucket), implicit: syllable.implicit)
        }
        let ok = views.withUnsafeBufferPointer { list in
            zt_syllable_scorer_scores_rows(handle, context, list.baseAddress, list.count, candidates.pointer,
                                           candidates.scores, candidates.lengths)
        }
        return ok ? RustScores() : nil
    }
}
