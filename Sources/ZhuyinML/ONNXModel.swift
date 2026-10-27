#if os(Linux)
import Foundation
import ONNXBridge

private func checked(_ error: UnsafeMutablePointer<CChar>?) throws {
    if let error {
        defer { zt_free_error(error) }
        throw NSError(domain: "ONNXRuntime", code: 1, userInfo: [NSLocalizedDescriptionKey: String(cString: error)])
    }
}

final class ONNXTensor {
    let value: OpaquePointer
    init(_ value: OpaquePointer) { self.value = value }
    convenience init(_ values: [Int32], shape: [Int]) throws {
        guard shape.allSatisfy({ $0 > 0 }), shape.reduce(1, *) == values.count else {
            throw NSError(domain: "ONNXRuntime", code: 2, userInfo: [NSLocalizedDescriptionKey: "Invalid tensor shape"])
        }
        var result: OpaquePointer?
        try checked(zt_ints(values, shape.map(Int64.init), shape.count, &result))
        self.init(result!)
    }
    deinit { zt_release_tensor(value) }
    func doubles() throws -> [Double] {
        var values = [Double](repeating: 0, count: zt_count(value))
        try checked(zt_doubles(value, &values, values.count))
        return values
    }
}

/// Only tensor calls live here. Tokenization, penalties and engine scoring stay in the shared scorers.
final class ONNXModel {
    private let session: OpaquePointer
    init(_ directory: URL, _ name: String) throws {
        var result: OpaquePointer?
        try checked(zt_session(directory.appendingPathComponent(name + ".onnx").path, &result))
        session = result!
    }
    deinit { zt_release_session(session) }
    func run(_ names: [String], _ inputs: [ONNXTensor], outputs: [String]) throws -> [ONNXTensor] {
        let strings = (names + outputs).map { strdup($0)! }
        defer { strings.forEach { free($0) } }
        let pointers = strings.map { UnsafePointer<CChar>($0) as UnsafePointer<CChar>? }
        var result = [OpaquePointer?](repeating: nil, count: outputs.count)
        try pointers.withUnsafeBufferPointer { names in
            try checked(zt_run(session, names.baseAddress, inputs.map { Optional($0.value) }, inputs.count,
                               names.baseAddress! + inputs.count, outputs.count, &result))
        }
        return result.map { ONNXTensor($0!) }
    }

    static func reranker(_ directory: URL) throws -> NeuralReranker.Prediction {
        let model = try ONNXModel(directory, "Reranker")
        return { ids, targets, batch, length in
            let inputs = try [ONNXTensor(ids, shape: [batch, length]), ONNXTensor(targets, shape: [batch, length])]
            return try model.run(["ids", "targets"], inputs, outputs: ["logprob"])[0].doubles()
        }
    }

    static func rerankerTree(_ directory: URL) throws -> NeuralReranker.TreePrediction {
        let model = try ONNXModel(directory, "RerankerTree")
        return { tree in
            let n = tree.ids.count
            let inputs = try [ONNXTensor(tree.ids, shape: [1, n]), ONNXTensor(tree.positions, shape: [1, n]),
                              ONNXTensor(tree.ancestors, shape: [n, n]), ONNXTensor(tree.parents, shape: [tree.targets.count]),
                              ONNXTensor(tree.targets, shape: [tree.targets.count])]
            return try model.run(["ids", "positions", "ancestors", "parents", "targets"], inputs, outputs: ["logprob"])[0].doubles()
        }
    }

    static func syllable(_ directory: URL) throws -> SyllableScorer.Prediction {
        let context = try ONNXModel(directory, "ContextEncoder")
        let syllable = try ONNXModel(directory, "SyllableEncoder")
        var cachedIDs: [Int32] = []
        var memory: [ONNXTensor] = []
        return { ctx, ids, tones, costs, boundaries, candidates in
            let count = ids.count, width = candidates.map(\.count).max() ?? 0
            guard count > 0, count <= 64, width > 0, width <= 64, ctx.count <= 256,
                  tones.count == count, costs.count == count, boundaries.count == count, candidates.count == count else {
                throw NSError(domain: "ONNXRuntime", code: 2, userInfo: [NSLocalizedDescriptionKey: "Invalid syllable shape"])
            }
            if ctx != cachedIDs {
                memory = try context.run(["context_ids"], [ONNXTensor(ctx, shape: [1, ctx.count])], outputs: ["memory_k", "memory_v"])
                cachedIDs = ctx
            }
            let flat = candidates.flatMap { $0 + [Int32](repeating: 0, count: width - $0.count) }
            let inputs = try [ids, tones, costs, boundaries].map { try ONNXTensor($0, shape: [1, count]) }
                + [ONNXTensor(flat, shape: [1, count, width])] + memory
            let output = try syllable.run(["syllable_ids", "tone_ids", "cost_bucket", "boundary", "candidate_ids", "memory_k", "memory_v"],
                                          inputs, outputs: ["logprob"])[0].doubles()
            guard output.count == count * width else {
                throw NSError(domain: "ONNXRuntime", code: 2, userInfo: [NSLocalizedDescriptionKey: "Invalid syllable output"])
            }
            return candidates.indices.map { Array(output[($0 * width)..<($0 * width + candidates[$0].count)]) }
        }
    }
}
#endif
