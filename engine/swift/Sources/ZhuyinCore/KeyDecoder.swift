import Foundation

/// How a run of typed keys is split into syllables and whether key order mistakes are repaired.
public enum KeyDecoderMethod: String, Sendable, CaseIterable {
    /// Every key goes into the current syllable's slots; only a tone key or Space ends it (the original behaviour).
    case slots
    /// Also ends a first-tone syllable when the next key starts another one (Space left out).
    case boundaries
    /// Boundaries, plus two neighbouring keys taken in the other order, and an extra key skipped, at fixed costs.
    case reorder
    /// Like `reorder`, but keys pressed almost together cost little to reorder or skip: fast typing presses keys out
    /// of order or two at once, while keys typed apart are meant as typed.
    case timed
}

/// A run of keys typed at the end of the composition, split into syllables by a Viterbi search over key positions.
///
/// A syllable is built in the composer's three slots (initial, medial, final), so the order of keys inside one
/// syllable never matters.  What the search adds is where syllables end: after a tone key or Space (explicit), or
/// before the next initial when the syllable is complete as a first tone (implicit); and, for `reorder` and `timed`,
/// repairs across syllables: two adjacent keys swapped (`ji3ap7` typed `jia3p7`) and a key pressed by mistake.
/// Closing a syllable costs nothing when it is valid and the keyboard channel's cheapest correction otherwise, so
/// the per-syllable typo correction stays where it was.
public struct KeyDecoder {
    public struct Key: Equatable {
        public var character: Character
        /// Key-down time in seconds, when known.
        public var time: TimeInterval?
        public init(character: Character, time: TimeInterval?) { self.character = character; self.time = time }
    }

    public struct Syllable: Equatable {
        /// Zhuyin symbols in slot order plus the tone mark: the token's reading.
        public var reading: String
        /// The keys behind it, in the order taken, ending with the tone key or Space.
        public var keys: String
        public var implicit: Bool
    }

    public struct Result: Equatable {
        public var syllables: [Syllable]
        /// The unfinished last syllable: its slots and keys.
        public var buffer: [Character?]
        public var bufferKeys: String
        public var cost: Double
    }

    /// Costs on the keyboard channel's scale (KeyChannel: transposition 9, extra or missing key 12.5, neighbour 11).
    public struct Costs: Sendable {
        /// First tone without Space before the next initial.
        // Chosen with ml/src/zhuyintype_ml/eval/typing_errors.py on both dev sets (docs/engine/MODEL.md §3).
        public var implicitBoundary = 4.0
        /// A filled slot typed again (the slot keeps the later key).
        public var overwrite = 10.0
        public var swap = 10.0
        public var skip = 13.0
        /// For `timed`: keys pressed within `togetherGap` of each other.
        public var swapTogether = 2.0
        public var skipTogether = 4.0
        /// For `timed`: keys typed apart.
        public var swapApart = 12.0
        public var skipApart = 15.0
        public var togetherGap: TimeInterval = 0.035
        public init() {}
    }
    /// Shared by every composer; zhuyin-cli can set them for calibration.
    nonisolated(unsafe) public static var costs = Costs()

    let method: KeyDecoderMethod
    /// Cost of closing a syllable from these slots and keys: 0 when valid, a correction cost, or nil when impossible.
    let closingCost: (_ reading: String, _ keys: String) -> Double?

    /// A decoder; `closingCost(reading, keys)` prices closing a syllable (0 valid, a correction, nil impossible).
    public init(method: KeyDecoderMethod, closingCost: @escaping (_ reading: String, _ keys: String) -> Double?) {
        self.method = method
        self.closingCost = closingCost
    }

    private struct State {
        var slots: [Character?] = [nil, nil, nil]
        var keys = ""
        var syllables: [Syllable] = []
        var cost = 0.0
        /// States agreeing on the unfinished syllable and the last two syllables compete; earlier differences are
        /// settled, so the search stays a beam while close alternatives survive for the language model.
        var signature: String {
            String(slots.map { $0 ?? "_" }) + "|" + keys + "|" + syllables.suffix(2).map(\.reading).joined(separator: " ")
        }
    }

    /// The best state per signature at one key position, in the order the signatures were first reached.  States of
    /// equal cost rank in that order, so every run (and the Rust engine) decodes alike; a Dictionary's order changes
    /// with each process's hash seed.
    private struct Position {
        var states: [State] = []
        var index: [String: Int] = [:]

        /// Keeps `state` unless its signature already has a state that costs no more.
        mutating func offer(_ state: State) {
            let signature = state.signature
            if let i = index[signature] {
                if states[i].cost <= state.cost { return }
                states[i] = state
            } else {
                index[signature] = states.count
                states.append(state)
            }
        }

        /// The states by cost; equal costs in the order their signatures were first reached.
        func ranked() -> [State] {
            states.indices.sorted { a, b in
                states[a].cost == states[b].cost ? a < b : states[a].cost < states[b].cost
            }.map { states[$0] }
        }
    }

    private static let beamWidth = 24

    /// The `limit` cheapest splits, cheapest first (each a different unfinished syllable or a different history);
    /// equal costs in the order the search first reached their states.
    public func decode(_ keys: [Key], limit: Int) -> [Result] {
        // states[i]: the best states after consuming the first i typed keys.
        var states = Array(repeating: Position(), count: keys.count + 1)
        states[0].offer(State())
        func offer(_ state: State, at index: Int) { states[index].offer(state) }
        for index in 0..<keys.count {
            let beam = states[index].ranked().prefix(Self.beamWidth)
            for state in beam {
                for next in take(state, keys[index], pressedWithPrevious: together(keys, index, index - 1)) {
                    offer(next, at: index + 1)
                }
                guard method == .reorder || method == .timed else { continue }
                // Skip a Zhuyin key pressed by mistake.  A tone key or Space that fits nothing is ignored by the
                // composer instead, as it always was.
                if ZhuyinKeyboard.symbols[keys[index].character] != nil {
                    var skipped = state
                    skipped.cost += skipCost(keys, index)
                    offer(skipped, at: index + 1)
                }
                // Take the next two keys in the other order.
                if index + 1 < keys.count, keys[index].character != keys[index + 1].character {
                    for first in take(state, keys[index + 1]) {
                        for second in take(first, keys[index]) {
                            var swapped = second
                            swapped.cost += swapCost(keys, index)
                            offer(swapped, at: index + 2)
                        }
                    }
                }
            }
        }
        return Array(states[keys.count].ranked().prefix(limit).map {
            Result(syllables: $0.syllables, buffer: $0.slots, bufferKeys: $0.keys, cost: $0.cost)
        })
    }

    func decode(_ keys: [Key]) -> Result {
        decode(keys, limit: 1).first ?? Result(syllables: [], buffer: [nil, nil, nil], bufferKeys: "", cost: .infinity)
    }

    /// The states reached by taking one more key.
    /// `pressedWithPrevious`: this key went down almost together with the one before it (`timed` then never ends a
    /// syllable between them: nobody finishes one syllable and starts the next within a few milliseconds).
    private func take(_ state: State, _ key: Key, pressedWithPrevious: Bool = false) -> [State] {
        let character = key.character
        if let tone = ZhuyinKeyboard.tones[character] ?? (character == " " ? "" : nil) {
            return close(state, tone: tone, toneKey: character, implicit: false).map { [$0] } ?? []
        }
        guard let symbol = ZhuyinKeyboard.symbols[character], let slot = ZhuyinKeyboard.slot(of: symbol) else { return [] }
        var results: [State] = []
        var placed = state
        // A filled slot takes the later key, and every key stays in `keys` for the channel's correction, as the
        // composer has always done; replacing a key costs.  An initial after the medial or final is a key out of
        // order: cheap when pressed together with the key before it, otherwise priced like a swap.
        if placed.slots[slot] != nil {
            placed.cost += Self.costs.overwrite
        } else if slot == 0, state.slots[1] != nil || state.slots[2] != nil {
            placed.cost += method == .timed ? (pressedWithPrevious ? Self.costs.swapTogether : Self.costs.swapApart)
                : method == .slots ? 0 : Self.costs.swap
        }
        placed.slots[slot] = symbol
        placed.keys.append(character)
        results.append(placed)
        // The syllable so far is a whole first-tone syllable and this key starts the next one.
        if method != .slots, slot == 0, state.slots.contains(where: { $0 != nil }), !(method == .timed && pressedWithPrevious),
           var closed = close(state, tone: "", toneKey: " ", implicit: true) {
            closed.slots[0] = symbol
            closed.keys = String(character)
            results.append(closed)
        }
        return results
    }

    private func close(_ state: State, tone: String, toneKey: Character, implicit: Bool) -> State? {
        guard state.slots.contains(where: { $0 != nil }) else { return nil }
        let reading = String(state.slots.compactMap { $0 }) + tone
        let keys = state.keys + String(toneKey)
        guard let cost = closingCost(reading, keys) else { return nil }
        var next = state
        next.syllables.append(Syllable(reading: reading, keys: keys, implicit: implicit))
        next.slots = [nil, nil, nil]
        next.keys = ""
        next.cost += cost + (implicit ? Self.costs.implicitBoundary : 0)
        return next
    }

    private func together(_ keys: [Key], _ index: Int, _ other: Int) -> Bool {
        guard keys.indices.contains(other), let a = keys[index].time, let b = keys[other].time else { return false }
        return abs(a - b) < Self.costs.togetherGap
    }

    private func swapCost(_ keys: [Key], _ index: Int) -> Double {
        method == .timed ? (together(keys, index, index + 1) ? Self.costs.swapTogether : Self.costs.swapApart) : Self.costs.swap
    }

    private func skipCost(_ keys: [Key], _ index: Int) -> Double {
        guard method == .timed else { return Self.costs.skip }
        let near = (together(keys, index, index - 1) && KeyChannel.neighbours(of: keys[index - 1].character).contains(keys[index].character))
            || (together(keys, index, index + 1) && KeyChannel.neighbours(of: keys[index + 1].character).contains(keys[index].character))
        return near ? Self.costs.skipTogether : Self.costs.skipApart
    }
}
