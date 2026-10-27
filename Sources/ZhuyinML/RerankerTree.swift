import Foundation

/// Node identity is (parent, token), never just the token: siblings cannot share attention.
struct RerankerTree {
    var ids: [Int32]
    var targets: [Int32]
    var positions: [Int32]
    var parents: [Int32]
    var ancestors: [Int32] = []
    var paths: [[Int]] = []

    init(prefix: [Int32], candidates: [[Int32]]) {
        ids = prefix
        targets = []
        positions = prefix.indices.map(Int32.init)
        parents = prefix.indices.map { Int32(max(0, $0 - 1)) }
        var children: [Int: [Int32: Int]] = [:]
        for candidate in candidates {
            var parent = prefix.count - 1
            var path: [Int] = []
            for token in candidate {
                let node: Int
                if let existing = children[parent]?[token] { node = existing }
                else {
                    node = ids.count
                    children[parent, default: [:]][token] = node
                    ids.append(token)
                    positions.append(positions[parent] + 1)
                    parents.append(Int32(parent))
                }
                path.append(node)
                parent = node
            }
            paths.append(path)
        }
        // A terminal leaf needs its parent's logits, but never its own hidden state.
        targets = ids
        let stateNodes = Set(parents.map(Int.init)).sorted()
        let stateIndex = Dictionary(uniqueKeysWithValues: stateNodes.enumerated().map { ($1, $0) })
        let originalParents = parents
        ids = stateNodes.map { targets[$0] }
        positions = stateNodes.map { positions[$0] }
        parents = originalParents.map { Int32(stateIndex[Int($0)]!) }
        let count = ids.count
        ancestors = [Int32](repeating: 0, count: count * count)
        for (row, node) in stateNodes.enumerated() {
            var ancestor = node
            while true {
                ancestors[row * count + stateIndex[ancestor]!] = 1
                if ancestor == 0 { break }
                ancestor = Int(originalParents[ancestor])
            }
        }
    }

    func scores(_ logprob: [Double], unknownPenalty: Double) -> [Double]? {
        guard logprob.count == targets.count, logprob.allSatisfy(\.isFinite) else { return nil }
        return paths.map { path in
            path.reduce(0.0) { $0 + logprob[$1] - (targets[$1] == 2 ? unknownPenalty : 0) }
        }
    }
}
