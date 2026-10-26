import Foundation

enum CompositionToken: Equatable {
    case syllable(String, keys: String)
    case literal(String)
    var reading: String? { if case .syllable(let value, _) = self { return value }; return nil }
}

struct ConvertedWord {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32? = nil
}

struct FixedWord: Equatable {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32? = nil
    var annotation: String? = nil
}

fileprivate final class PathNode {
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

struct ConversionCache {
    var tokens: [CompositionToken]
    var fixed: [FixedWord]
    var typo: TypoTolerance
    var revision: UInt64
    fileprivate var states: [[PathNode]]
}

extension Engine {
    func convert(_ tokens: [CompositionToken], fixed: [FixedWord], typo: TypoTolerance, cache: inout ConversionCache?) -> [ConvertedWord] {
        guard !tokens.isEmpty else { return [] }
        let revision = conversionRevision
        var reused = 0
        if let old = cache, old.fixed == fixed, old.typo == typo, old.revision == revision {
            let common = zip(old.tokens, tokens).prefix { $0 == $1 }.count
            // Final-position beams include EOS scores; only reuse strictly interior positions.
            reused = max(0, min(common, old.tokens.count - 1, tokens.count - 1))
        }
        var states = Array(repeating: [PathNode](), count: tokens.count + 1)
        if reused > 0, let old = cache {
            states.replaceSubrange(0...reused, with: old.states[0...reused])
        } else { states[0] = [PathNode(score: 0, word: nil, previous: nil, history: .init())] }
        // Only words of at most six syllables can cross the reusable prefix boundary.
        for start in max(0, reused - 5)..<tokens.count where !states[start].isEmpty {
            let beam = states[start]
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
                for match in matches(tokens, start: start, typo: typo) {
                    edges.append((ConvertedWord(range: match.range, text: match.entry.word,
                        reading: match.typedReading), match.entry))
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
                let end = word.range.upperBound
                guard end > reused else { continue }
                let values = entry.map { scoring($0, reading: word.reading) }
                for previous in beam {
                    let value = values.map {
                        (entry?.recalled == true ? 0 : languageModel.score(wordID: entry?.wordID, history: previous.history, unigram: $0.unigram)) + $0.adjustment
                    } ?? languageModel.endScore(history: previous.history)
                    let stateKey = entry == nil ? LanguageModelHistory()
                        : previous.history.appending(entry?.wordID, order: languageModel.order)
                    let total = previous.score + value
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
        cache = ConversionCache(tokens: tokens, fixed: fixed, typo: typo, revision: revision, states: states)
        var node = states[tokens.count].first
        var result: [ConvertedWord] = []
        while let current = node, let word = current.word {
            result.append(word)
            node = current.previous
        }
        return result.reversed()
    }
}
