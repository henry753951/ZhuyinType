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
    var wordID: UInt32? = nil
}

struct FixedWord {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32? = nil
}

private final class PathNode {
    let score: Double
    let word: ConvertedWord?
    let previous: PathNode?
    let history: LanguageModelHistory
    let beamScore: Double
    let lastWord: String
    init(score: Double, word: ConvertedWord?, previous: PathNode?, history: LanguageModelHistory, beamScore: Double? = nil) {
        self.score = score; self.word = word; self.previous = previous; self.history = history
        self.beamScore = beamScore ?? score
        lastWord = word?.reading.isEmpty == false ? word!.text : "<s>"
    }
    func precedes(_ other: PathNode) -> Bool {
        if beamScore != other.beamScore { return beamScore < other.beamScore }
        if lastWord != other.lastWord { return lastWord > other.lastWord }
        return (history.older ?? 0) > (other.history.older ?? 0)
    }
}

extension Engine {
    func convert(_ tokens: [CompositionToken], fixed: [FixedWord]) -> [ConvertedWord] {
        guard !tokens.isEmpty else { return [] }
        var states = Array(repeating: [PathNode](), count: tokens.count + 1)
        states[0] = [PathNode(score: 0, word: nil, previous: nil, history: .init())]
        for start in tokens.indices where !states[start].isEmpty {
            let beam = states[start]
            states[start].removeAll(keepingCapacity: true)
            var edges: [(ConvertedWord, WordEntry?)] = []
            switch tokens[start] {
            case .literal(let text):
                edges.append((ConvertedWord(range: start..<(start + 1), text: text, reading: ""), nil))
            case .syllable:
                if let word = fixed.first(where: { $0.range.lowerBound == start }) {
                    let entry = entries(for: word.reading).first(where: { $0.word == word.text })
                        ?? WordEntry(word: word.text, logp: -7, wordID: word.wordID)
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
            for (var word, entry) in edges {
                guard fixed.allSatisfy({ !$0.range.overlaps(word.range) || ($0.range == word.range && $0.text == word.text) }) else { continue }
                word.wordID = entry?.wordID
                let values = entry.map { scoring($0, reading: word.reading) }
                for previous in beam {
                    let value = values.map {
                        languageModel.score(wordID: entry?.wordID, history: previous.history, unigram: $0.unigram) + $0.adjustment
                    } ?? languageModel.endScore(history: previous.history)
                    let stateKey = entry == nil ? LanguageModelHistory()
                        : previous.history.appending(entry?.wordID, order: languageModel.order)
                    let total = previous.score + value
                    let end = word.range.upperBound
                    let rank = total + (end == tokens.count ? languageModel.endScore(history: stateKey) : 0)
                    if states[end].count == Self.decodingBeamWidth, let worst = states[end].last {
                        let last = entry == nil ? "<s>" : word.text
                        if rank < worst.beamScore || (rank == worst.beamScore &&
                            (last > worst.lastWord || (last == worst.lastWord &&
                                (stateKey.older ?? 0) > (worst.history.older ?? 0)))) { continue }
                    }
                    if let index = states[end].firstIndex(where: { $0.history == stateKey }) {
                        let old = states[end][index]
                        guard total > old.score || (total == old.score && previous.lastWord < (old.previous?.lastWord ?? "")) else { continue }
                        states[end].remove(at: index)
                    } else if total == -.infinity { continue }
                    let node = PathNode(score: total, word: word, previous: previous, history: stateKey, beamScore: rank)
                    let index = states[end].firstIndex(where: { $0.precedes(node) }) ?? states[end].count
                    states[end].insert(node, at: index)
                    if states[end].count > Self.decodingBeamWidth { states[end].removeLast() }
                }
            }
        }
        var node = states[tokens.count].first
        var result: [ConvertedWord] = []
        while let current = node, let word = current.word {
            result.append(word)
            node = current.previous
        }
        return result.reversed()
    }
}
