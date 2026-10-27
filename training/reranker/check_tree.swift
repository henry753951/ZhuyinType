// Compile alongside Sources/ZhuyinML/*.swift, linking the existing ZhuyinCore and ONNXBridge objects.
// Linux only: replay captured evaluator inputs through the real Composer and compare every model call.
import Foundation
import ZhuyinCore

@main
struct CheckTree {
    static func main() throws {
        let test = RerankerTree(prefix: [1, 3], candidates: [[4, 5], [4, 6], [4], [], [4, 5], [2]])
        precondition(test.ids == [1, 3, 4])
        precondition(test.targets == [1, 3, 4, 5, 6, 2])
        precondition(test.positions == [0, 1, 2])
        precondition(test.parents == [0, 0, 1, 2, 2, 1])
        precondition(test.paths == [[2, 3], [2, 4], [2], [], [2, 3], [5]])
        precondition(test.ancestors == [1, 0, 0, 1, 1, 0, 1, 1, 1])
        let branches = RerankerTree(prefix: [1], candidates: [[3, 4, 5], [3, 6, 7]])
        precondition(Array(branches.ancestors.suffix(4)) == [1, 1, 0, 1])
        precondition(test.scores(Array(repeating: -1, count: 6), unknownPenalty: 10) == [-2, -2, -1, 0, -2, -11])
        precondition(test.scores([.nan], unknownPenalty: 0) == nil)
        precondition(test.scores(Array(repeating: .nan, count: 6), unknownPenalty: 0) == nil)
        let limit = RerankerTree(prefix: [1], candidates: (0..<32).map { [Int32($0 + 3)] + Array(repeating: 3, count: 62) })
        precondition(limit.targets.count == 2017 && limit.positions.max() == 62)
        if CommandLine.arguments.count == 1 { print("tree structure self-check passed"); return }

        let inputs = try JSONDecoder().decode([String].self, from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))
        let output = CommandLine.arguments[2]
        let directory = URL(fileURLWithPath: "data/build/reranker")
        let baseline = try NeuralReranker(directory: directory, weight: 1, prediction: ONNXModel.reranker(directory))
        let tree = try NeuralReranker(directory: directory, weight: 1)
        guard let predict = tree.treePrediction else { fatalError("tree model did not load") }
        var predictions = 0
        tree.treePrediction = { packed in
            let result = try predict(packed)
            precondition(result.count == packed.targets.count && result.allSatisfy(\.isFinite), "invalid tree output")
            predictions += 1
            return result
        }
        Engine.chatModelDirectory = URL(fileURLWithPath: "data/build/lm_chat")
        Engine.chatPrior = 0.5
        let engine = try Engine(lexiconURL: URL(fileURLWithPath: "data/build/lexicon.tsv"), userDataDirectory: nil)
        let v3 = try SyllableScorer(directory: URL(fileURLWithPath: "data/build/v3"))
        let composer = Composer(engine: engine)
        composer.rerankingCount = 16
        composer.characterWeight = 0.5
        var context = "", committed = ""
        var records: [[String: Any]] = []
        var nodes = 0, edges = 0
        let countedPrediction = tree.treePrediction!
        tree.treePrediction = { packed in
            nodes = packed.ids.count; edges = packed.targets.count
            return try countedPrediction(packed)
        }
        func argmax(_ scores: [Double]) -> Int { scores.indices.max { scores[$0] < scores[$1] }! }
        func choice(_ scores: [Double], _ sentences: [ScoredSentence]) -> Int {
            let best = argmax(scores)
            let exact = scores.indices.filter { !sentences[$0].corrected }.max { scores[$0] < scores[$1] }
            if sentences[best].corrected, let exact, scores[best] - scores[exact] < 4 { return exact }
            return best
        }
        func compare(_ candidates: [String], context: String, sentences: [ScoredSentence]? = nil) -> Int {
            let a = baseline.scores(context: context, candidates: candidates)
            let before = predictions
            let b = tree.scores(context: context, candidates: candidates)
            if a == nil || candidates.isEmpty { precondition(a == b); return 0 }
            precondition(predictions == before + 1, "tree fell back to batched scoring")
            guard let a, let b else { fatalError("missing scores") }
            let base = sentences?.map(\.score) ?? Array(repeating: 0, count: a.count)
            let ta = zip(base, a).map(+), tb = zip(base, b).map(+)
            let ca = sentences.map { choice(ta, $0) } ?? argmax(a)
            let cb = sentences.map { choice(tb, $0) } ?? argmax(b)
            records.append(["context": context, "candidates": candidates, "engine": base,
                            "corrected": sentences?.map(\.corrected) ?? [], "current": a, "tree": b,
                            "currentArgmax": argmax(a), "treeArgmax": argmax(b),
                            "currentTotalArgmax": argmax(ta), "treeTotalArgmax": argmax(tb),
                            "currentChoice": ca, "treeChoice": cb, "nodes": nodes, "edges": edges,
                            "kind": sentences == nil ? "candidates" : "sentences"])
            if records.count % 100 == 0, let sentences {
                precondition(baseline.choose(sentences, context: context) == ca)
                precondition(tree.choose(sentences, context: context) == cb)
            }
            return cb
        }
        composer.characterScorer = { syllables, candidates in
            v3.scores(context: context + committed, syllables: syllables.map {
                SyllableScorer.Syllable(base: $0.base, tone: $0.tone, costBucket: $0.costBucket, implicit: false)
            }, candidates: candidates)
        }
        composer.reranker = { sentences in compare(sentences.map(\.text), context: context + committed, sentences: sentences) }
        composer.candidateScorer = { before, candidates in
            _ = compare(candidates, context: context + committed + before)
            return tree.candidateScores(context: context + committed + before, candidates: candidates)
        }
        var time = 1000.0
        if CommandLine.arguments.last == "--edges" {
            for context in ["", String(repeating: "今天晚上", count: 30)] {
                _ = compare(["今"], context: context)
                _ = compare(["今天", "今天晚上", "今天", "明天", "", "𠀀？"], context: context)
                _ = compare([String(repeating: "今", count: 63), "今"], context: context)
            }
            let vocabulary = try JSONDecoder().decode([String].self, from: Data(contentsOf: directory.appendingPathComponent("vocab.json")))
            let distinct = vocabulary.dropFirst(3).filter { $0.count == 1 }.prefix(32)
            _ = compare(distinct.map { $0 + String(repeating: "今", count: 62) }, context: "")
            precondition(tree.scores(context: "", candidates: [String(repeating: "今", count: 64)]) == nil)
            precondition(tree.scores(context: "", candidates: []) == [])
            tree.treePrediction = { _ in [] }
            precondition(tree.scores(context: "", candidates: ["今天"]) == baseline.scores(context: "", candidates: ["今天"]))
            tree.treePrediction = { _ in throw NSError(domain: "test", code: 1) }
            precondition(tree.scores(context: "", candidates: ["今天"]) == baseline.scores(context: "", candidates: ["今天"]))
            print("length, duplicate, empty, OOV, context truncation, max nodes, malformed/throwing fallback passed")
        }
        for line in inputs {
            composer.reset(); composer.breakContext(); engine.forgetAllLearned()
            committed = ""; context = ""
            var script = line
            if let tab = line.firstIndex(of: "\t") {
                context = String(line[..<tab]); script = String(line[line.index(after: tab)...])
            }
            composer.setPrecedingText(context)
            // Also check the candidate-list call site, before each commit.
            script = script.replacingOccurrences(of: "<enter>", with: "<down><esc><enter>")
            var i = script.startIndex
            let specials: [String: KeyInput] = ["space": .space, "enter": .enter, "left": .left, "down": .down, "esc": .escape]
            while i < script.endIndex {
                let key: KeyInput
                if script[i] == "<", let end = script[i...].firstIndex(of: ">") {
                    let name = String(script[script.index(after: i)..<end])
                    guard let special = specials[name] else { fatalError("unknown special \(name)") }
                    key = special; i = script.index(after: end)
                } else { key = script[i] == " " ? .space : .char(script[i]); i = script.index(after: i) }
                time += 0.12
                committed += composer.handle(key, at: time).commit ?? ""
            }
        }
        try JSONSerialization.data(withJSONObject: records, options: [.sortedKeys]).write(to: URL(fileURLWithPath: output))
        print("\(inputs.count) lines, \(records.count) calls, \(predictions) tree predictions; no fallback")
    }
}
