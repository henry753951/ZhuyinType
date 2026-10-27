import Foundation

enum CompositionToken: Equatable {
    /// `implicit`: the syllable ended without a tone key or Space, before the next initial (first tone).
    case syllable(String, keys: String, implicit: Bool = false)
    case literal(String)
    var reading: String? { if case .syllable(let value, _, _) = self { return value }; return nil }
}

struct ConvertedWord: Equatable {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32? = nil
    var history: LanguageModelHistory? = nil
    /// Matched through a typing correction or a fuzzy sound, not the keys as typed.
    var corrected = false
}

struct FixedWord: Equatable {
    var range: Range<Int>
    var text: String
    var reading: String
    var wordID: UInt32? = nil
    var annotation: String? = nil
    /// Internal words of a fixed phrase, retained for LM history even though it is displayed as one clause.
    var components: [ConvertedWord] = []
    /// Kept as shown because the user picked another word in the composition (not a pick of its own).
    var frozen = false
}

fileprivate final class PathNode {
    let score: Double
    let word: ConvertedWord?
    let previous: PathNode?
    let history: LanguageModelHistory
    let beamScore: Double
    let lastWord: String
    let text: String
    init(score: Double, word: ConvertedWord?, previous: PathNode?, history: LanguageModelHistory, beamScore: Double? = nil,
         distinct: Bool = false) {
        self.score = score; self.word = word; self.previous = previous; self.history = history
        self.beamScore = beamScore ?? score
        lastWord = word?.reading.isEmpty == false ? word!.text : "<s>"
        text = distinct ? (previous?.text ?? "") + (word?.text ?? "") : ""
    }
    func precedes(_ other: PathNode) -> Bool {
        if beamScore != other.beamScore { return beamScore < other.beamScore }
        if lastWord != other.lastWord { return lastWord > other.lastWord }
        return (history.older ?? 0) > (other.history.older ?? 0)
    }
}

fileprivate final class DecodingEdge {
    let word: ConvertedWord
    let entry: WordEntry?
    let values: (unigram: Double, adjustment: Double)?
    let components: [(entry: WordEntry, unigram: Double, adjustment: Double)]
    /// The character model's weighted score for this word here (0 without one).
    let characters: Double
    private var transitions: [LanguageModelHistory: (Double, LanguageModelHistory)] = [:]

    init(word: ConvertedWord, entry: WordEntry?, values: (unigram: Double, adjustment: Double)?,
         components: [(entry: WordEntry, unigram: Double, adjustment: Double)], characters: Double = 0) {
        self.word = word; self.entry = entry; self.values = values; self.components = components
        self.characters = characters
    }

    func transition(after history: LanguageModelHistory, model: any LanguageModel) -> (Double, LanguageModelHistory) {
        if let cached = transitions[history] { return cached }
        var value = values.map {
            (entry?.recalled == true ? 0 : model.score(wordID: entry?.wordID, history: history, unigram: $0.unigram)) + $0.adjustment
        } ?? model.endScore(history: history)
        var next = entry == nil ? LanguageModelHistory() : history.appending(entry?.wordID, order: model.order)
        if !components.isEmpty {
            value = 0; next = history
            for part in components {
                value += (part.entry.recalled ? 0 : model.score(wordID: part.entry.wordID,
                    history: next, unigram: part.unigram)) + part.adjustment
                next = next.appending(part.entry.wordID, order: model.order)
            }
        }
        value += characters
        transitions[history] = (value, next)
        return (value, next)
    }
}

fileprivate final class DecodingPosition {
    let edges: [DecodingEdge]
    private var best: [LanguageModelHistory: [DecodingEdge]] = [:]
    init(_ edges: [DecodingEdge]) { self.edges = edges }

    func expansions(after history: LanguageModelHistory, model: any LanguageModel, end: Int) -> [DecodingEdge] {
        if let cached = best[history] { return cached }
        typealias Ranked = (index: Int, rank: Double, history: LanguageModelHistory)
        var groups = Array(repeating: [Ranked](), count: 7)
        func better(_ a: Ranked, than b: Ranked) -> Bool {
            if a.rank != b.rank { return a.rank > b.rank }
            let aWord = edges[a.index].word.text, bWord = edges[b.index].word.text
            if aWord != bWord { return aWord < bWord }
            return (a.history.older ?? 0) < (b.history.older ?? 0)
        }
        for (index, edge) in edges.enumerated() {
            let (value, next) = edge.transition(after: history, model: model)
            let target = edge.word.range.upperBound
            let rank = value + (target == end ? model.endScore(history: next) : 0)
            let length = edge.word.range.count
            let candidate: Ranked = (index, rank, next)
            if groups[length].count == Engine.decodingBeamWidth, let worst = groups[length].last,
               better(worst, than: candidate) { continue }
            if let old = groups[length].firstIndex(where: { $0.history == next }) {
                guard rank > groups[length][old].rank else { continue }
                groups[length].remove(at: old)
            }
            let insertion = groups[length].firstIndex { better(candidate, than: $0) } ?? groups[length].count
            groups[length].insert(candidate, at: insertion)
            if groups[length].count > Engine.decodingBeamWidth { groups[length].removeLast() }
        }
        // A single predecessor cannot contribute more than one beam's worth of distinct histories per end.
        let indices = groups.flatMap { $0.map(\.index) }.sorted()
        let result = indices.map { edges[$0] }
        best[history] = result
        return result
    }
}

/// Per syllable position, log P(character) from a character model (the v3 syllable encoder), with the weight they
/// enter the lattice at.  A character the model was not asked about gets `floor`.
public struct CharacterScores: Equatable {
    public var positions: [[Character: Double]]
    public var weight: Double
    public var floor = -12.0
    public init(positions: [[Character: Double]], weight: Double) { self.positions = positions; self.weight = weight }

    /// The weighted sum over a word's characters, which start at token `start`.
    func score(_ text: String, at start: Int) -> Double {
        var total = 0.0
        for (offset, character) in text.enumerated() {
            let index = start + offset
            guard positions.indices.contains(index), !positions[index].isEmpty else { continue }
            total += positions[index][character] ?? floor
        }
        return weight * total
    }
}

struct ConversionCache {
    var tokens: [CompositionToken]
    var fixed: [FixedWord]
    var typo: TypoTolerance
    var revision: UInt64
    var group: Range<Int>?
    var start: LanguageModelHistory
    fileprivate var states: [[PathNode]]
    fileprivate var edges: [DecodingPosition?]
}

extension Engine {
    /// `start` is the LM history before the first token: the sentence start, or the words committed just before.
    func convert(_ tokens: [CompositionToken], fixed: [FixedWord], typo: TypoTolerance, cache: inout ConversionCache?,
                 group: Range<Int>? = nil, start: LanguageModelHistory = .init(), characters: CharacterScores? = nil) -> [ConvertedWord] {
        path(decode(tokens, fixed: fixed, typo: typo, cache: &cache, group: group, start: start, characters: characters).first)
    }

    func sentences(_ tokens: [CompositionToken], fixed: [FixedWord], typo: TypoTolerance, k: Int,
                   start: LanguageModelHistory = .init(), characters: CharacterScores? = nil) -> [ScoredSentence] {
        guard k > 0 else { return [] }
        var cache: ConversionCache?
        let nodes = decode(tokens, fixed: fixed, typo: typo, cache: &cache, start: start, characters: characters, k: k)
        var seen = Set<String>()
        return nodes.compactMap { node in
            let words = path(node)
            guard seen.insert(words.map(\.text).joined()).inserted else { return nil }
            return ScoredSentence(words: words.map { SentenceWord(range: $0.range, text: $0.text, reading: $0.reading,
                                                                 corrected: $0.corrected) },
                                  score: node.beamScore)
        }.prefix(k).map { $0 }
    }

    private func decode(_ tokens: [CompositionToken], fixed: [FixedWord], typo: TypoTolerance,
                        cache: inout ConversionCache?, group: Range<Int>? = nil, start: LanguageModelHistory = .init(),
                        characters: CharacterScores? = nil, k: Int = 1) -> [PathNode] {
        guard !tokens.isEmpty else { return [] }
        // Character scores depend on every syllable (the model reads both directions): nothing carries over.
        if characters != nil { cache = nil }
        let distinct = k > 1
        let width = max(Self.decodingBeamWidth, k)
        let revision = conversionRevision
        var reused = 0
        if !distinct, let old = cache, old.fixed == fixed, old.typo == typo, old.revision == revision, old.start == start,
           old.group == group || (old.group == nil && group != nil) {
            let common = zip(old.tokens, tokens).prefix { $0 == $1 }.count
            // Final-position beams include EOS scores; only reuse strictly interior positions.
            reused = max(0, min(common, old.tokens.count - 1, tokens.count - 1))
            if old.group != group { reused = min(reused, group!.lowerBound) }
        }
        var states = Array(repeating: [PathNode](), count: tokens.count + 1)
        var edgeCache = Array<DecodingPosition?>(repeating: nil, count: tokens.count)
        if let old = cache, old.fixed == fixed, old.typo == typo, old.revision == revision {
            if old.tokens == tokens { edgeCache = old.edges }
            else {
                let common = zip(old.tokens, tokens).prefix { $0 == $1 }.count
                let interior = min(common, old.tokens.count - 1, tokens.count - 1)
                for start in 0..<max(0, interior - 5) { edgeCache[start] = old.edges[start] }
            }
        }
        if reused > 0, let old = cache {
            states.replaceSubrange(0...reused, with: old.states[0...reused])
        } else { states[0] = [PathNode(score: 0, word: nil, previous: nil, history: start)] }
        // Only words of at most six syllables can cross the reusable prefix boundary.
        for start in max(0, reused - 5)..<tokens.count where !states[start].isEmpty {
            let beam = states[start]
            if edgeCache[start] == nil {
                var edges: [(ConvertedWord, WordEntry?, [WordEntry])] = []
                switch tokens[start] {
                case .literal(let text):
                    edges.append((ConvertedWord(range: start..<(start + 1), text: text, reading: ""), nil, []))
                case .syllable:
                    if let word = fixed.first(where: { $0.range.lowerBound == start }) {
                        let entry = entries(for: word.reading).first(where: { $0.word == word.text })
                            ?? WordEntry(word: word.text, logp: -7, wordID: word.wordID)
                        let components = word.components.map { part in
                            entries(for: part.reading).first(where: { $0.word == part.text })
                                ?? WordEntry(word: part.text, logp: -7, wordID: part.wordID)
                        }
                        edges.append((ConvertedWord(range: word.range, text: word.text, reading: word.reading), entry, components))
                    }
                    if !fixed.contains(where: { $0.range.lowerBound == start }) {
                        for match in matches(tokens, start: start, typo: typo) {
                            edges.append((ConvertedWord(range: match.range, text: match.entry.word,
                                reading: match.typedReading, corrected: match.annotation != nil), match.entry, []))
                        }
                    }
                    // Preserve completed tokens if a user removes a phrase that supplied their only dictionary entry.
                    if !edges.contains(where: { $0.0.range.count == 1 }), let reading = tokens[start].reading {
                        edges.append((ConvertedWord(range: start..<(start + 1), text: reading, reading: reading),
                                      WordEntry(word: reading, logp: -100), []))
                    }
                }
                edgeCache[start] = DecodingPosition(edges.filter { word, _, _ in
                    fixed.allSatisfy { !$0.range.overlaps(word.range) || ($0.range == word.range && $0.text == word.text) }
                }.map { word, entry, parts in
                    DecodingEdge(word: word, entry: entry, values: entry.map { scoring($0, reading: word.reading) },
                                 components: parts.map {
                        let values = scoring($0, reading: $0.reading ?? "")
                        return ($0, values.unigram, values.adjustment)
                    }, characters: word.reading.isEmpty ? 0 : (characters?.score(word.text, at: word.range.lowerBound) ?? 0))
                })
            }
            let position = edgeCache[start]!
            for previous in beam {
                let expansions = distinct ? position.edges
                    : position.expansions(after: previous.history, model: languageModel, end: tokens.count)
                for edge in expansions {
                    var word = edge.word
                    let entry = edge.entry
                    if let group, [group.lowerBound, group.upperBound].contains(where: {
                        word.range.lowerBound < $0 && $0 < word.range.upperBound
                    }) { continue }
                    word.wordID = edge.components.last?.entry.wordID ?? entry?.wordID
                    let end = word.range.upperBound
                    guard end > reused else { continue }
                    let (value, stateKey) = edge.transition(after: previous.history, model: languageModel)
                    let total = previous.score + value
                    let rank = total + (end == tokens.count ? languageModel.endScore(history: stateKey) : 0)
                    if states[end].count == width, let worst = states[end].last {
                        let last = entry == nil ? "<s>" : word.text
                        if rank < worst.beamScore || (rank == worst.beamScore &&
                            (last > worst.lastWord || (last == worst.lastWord &&
                                (stateKey.older ?? 0) > (worst.history.older ?? 0)))) { continue }
                    }
                    let text = distinct ? previous.text + word.text : ""
                    let finalDistinct = distinct && end == tokens.count
                    if let index = states[end].firstIndex(where: {
                        finalDistinct ? $0.text == text : ($0.history == stateKey && (!distinct || $0.text == text))
                    }) {
                        let old = states[end][index]
                        let value = finalDistinct ? rank : total
                        let oldValue = finalDistinct ? old.beamScore : old.score
                        guard value > oldValue || (value == oldValue && previous.lastWord < (old.previous?.lastWord ?? "")) else { continue }
                        states[end].remove(at: index)
                    } else if total == -.infinity { continue }
                    let node = PathNode(score: total, word: word, previous: previous, history: stateKey, beamScore: rank,
                                        distinct: distinct)
                    let index = states[end].firstIndex(where: { $0.precedes(node) }) ?? states[end].count
                    states[end].insert(node, at: index)
                    if states[end].count > width { states[end].removeLast() }
                }
            }
        }
        cache = ConversionCache(tokens: tokens, fixed: fixed, typo: typo, revision: revision, group: group, start: start,
                                states: states, edges: edgeCache)
        return states[tokens.count]
    }

    private func path(_ end: PathNode?) -> [ConvertedWord] {
        var node = end
        var result: [ConvertedWord] = []
        while let current = node, var word = current.word {
            word.history = current.history
            result.append(word)
            node = current.previous
        }
        return result.reversed()
    }
}
