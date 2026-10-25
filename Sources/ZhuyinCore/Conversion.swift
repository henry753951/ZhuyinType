import Foundation

enum CompositionToken {
    case syllable(String)
    case literal(String)
    var reading: String? { if case .syllable(let value) = self { return value }; return nil }
}

struct ConvertedWord {
    var range: Range<Int>
    var text: String
    var reading: String
}

struct FixedWord {
    var range: Range<Int>
    var text: String
    var reading: String
}

private final class PathNode {
    let score: Double
    let word: ConvertedWord?
    let previous: PathNode?
    let lastWord: String
    init(score: Double, word: ConvertedWord?, previous: PathNode?, lastWord: String) {
        self.score = score; self.word = word; self.previous = previous; self.lastWord = lastWord
    }
}

extension Engine {
    func convert(_ tokens: [CompositionToken], fixed: [FixedWord]) -> [ConvertedWord] {
        guard !tokens.isEmpty else { return [] }
        var states = Array(repeating: [String: PathNode](), count: tokens.count + 1)
        states[0][""] = PathNode(score: 0, word: nil, previous: nil, lastWord: "<s>")
        for start in tokens.indices where !states[start].isEmpty {
            var edges: [(ConvertedWord, WordEntry?)] = []
            switch tokens[start] {
            case .literal(let text):
                edges.append((ConvertedWord(range: start..<(start + 1), text: text, reading: ""), nil))
            case .syllable:
                if let word = fixed.first(where: { $0.range.lowerBound == start }) {
                    let entry = entries(for: word.reading).first(where: { $0.word == word.text })
                        ?? WordEntry(word: word.text, logp: -7)
                    edges.append((ConvertedWord(range: word.range, text: word.text, reading: word.reading), entry))
                }
                var reading = ""
                for end in start..<min(tokens.count, start + 6) {
                    guard let part = tokens[end].reading else { break }
                    reading += (reading.isEmpty ? "" : " ") + part
                    let span = start..<(end + 1)
                    for entry in entries(for: reading) {
                        edges.append((ConvertedWord(range: span, text: entry.word, reading: reading), entry))
                    }
                }
                // Preserve completed tokens if a user removes a phrase that supplied their only dictionary entry.
                if !edges.contains(where: { $0.0.range.count == 1 }), let reading = tokens[start].reading {
                    edges.append((ConvertedWord(range: start..<(start + 1), text: reading, reading: reading),
                                  WordEntry(word: reading, logp: -100)))
                }
            }
            for (word, entry) in edges {
                guard fixed.allSatisfy({ !$0.range.overlaps(word.range) || ($0.range == word.range && $0.text == word.text) }) else { continue }
                for previous in states[start].values {
                    let value = entry.map { score($0, reading: word.reading, after: previous.lastWord) } ?? 0
                    let last = entry == nil ? "<s>" : word.text
                    let stateKey = usesBigram ? last : ""
                    let total = previous.score + value + (entry == nil ? languageModel.endScore(after: previous.lastWord) : 0)
                    let end = word.range.upperBound
                    let existing = states[end][stateKey]
                    if total > (existing?.score ?? -.infinity)
                        || (total == existing?.score && previous.lastWord < (existing?.previous?.lastWord ?? "")) {
                        states[end][stateKey] = PathNode(score: total, word: word, previous: previous, lastWord: last)
                    }
                }
            }
        }
        var node = states[tokens.count].values.max {
            let a = $0.score + languageModel.endScore(after: $0.lastWord)
            let b = $1.score + languageModel.endScore(after: $1.lastWord)
            return a == b ? $0.lastWord > $1.lastWord : a < b
        }
        var result: [ConvertedWord] = []
        while let current = node, let word = current.word {
            result.append(word)
            node = current.previous
        }
        return result.reversed()
    }
}
