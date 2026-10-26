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
struct KeyDecoder {
    struct Key: Equatable {
        var character: Character
        /// Key-down time in seconds, when known.
        var time: TimeInterval?
    }

    struct Syllable: Equatable {
        /// Zhuyin symbols in slot order plus the tone mark: the token's reading.
        var reading: String
        /// The keys behind it, in the order taken, ending with the tone key or Space.
        var keys: String
        var implicit: Bool
    }

    struct Result: Equatable {
        var syllables: [Syllable]
        /// The unfinished last syllable: its slots and keys.
        var buffer: [Character?]
        var bufferKeys: String
        var cost: Double
    }

    /// First tone without Space before the next initial.
    static let implicitBoundaryCost = 2.0
    /// A filled slot replaced by another key of the same kind (the slot keeps the later key, as the composer does).
    static let overwriteCost = 7.0
    static let swapCost = 6.0
    static let skipCost = 9.0
    /// Gap under which two keys count as pressed together.
    static let togetherGap: TimeInterval = 0.035
    static let swapTogetherCost = 1.0
    static let skipTogetherCost = 3.5

    let method: KeyDecoderMethod
    /// Cost of closing a syllable from these slots and keys: 0 when valid, a correction cost, or nil when impossible.
    let closingCost: (_ reading: String, _ keys: String) -> Double?

    private struct State {
        var slots: [Character?] = [nil, nil, nil]
        var keys = ""
        var syllables: [Syllable] = []
        var cost = 0.0
        var signature: String { String(slots.map { $0 ?? "_" }) + "|" + keys }
    }

    private static let beamWidth = 24

    func decode(_ keys: [Key]) -> Result {
        // states[i]: the best states after consuming the first i typed keys.
        var states = Array(repeating: [String: State](), count: keys.count + 1)
        states[0][""] = State()
        func offer(_ state: State, at index: Int) {
            // What follows depends only on the unfinished syllable, so states that agree on it compete.
            let signature = state.signature
            if let old = states[index][signature], old.cost <= state.cost { return }
            states[index][signature] = state
        }
        for index in 0..<keys.count {
            let beam = states[index].values.sorted { $0.cost < $1.cost }.prefix(Self.beamWidth)
            for state in beam {
                for next in take(state, keys[index]) { offer(next, at: index + 1) }
                guard method == .reorder || method == .timed else { continue }
                // Skip a key pressed by mistake.
                var skipped = state
                skipped.cost += skipCost(keys, index)
                offer(skipped, at: index + 1)
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
        guard let best = states[keys.count].values.min(by: { $0.cost < $1.cost }) else {
            return Result(syllables: [], buffer: [nil, nil, nil], bufferKeys: "", cost: .infinity)
        }
        return Result(syllables: best.syllables, buffer: best.slots, bufferKeys: best.keys, cost: best.cost)
    }

    /// The states reached by taking one more key.
    private func take(_ state: State, _ key: Key) -> [State] {
        let character = key.character
        if let tone = ZhuyinKeyboard.tones[character] ?? (character == " " ? "" : nil) {
            return close(state, tone: tone, toneKey: character, implicit: false).map { [$0] } ?? []
        }
        guard let symbol = ZhuyinKeyboard.symbols[character], let slot = ZhuyinKeyboard.slot(of: symbol) else { return [] }
        var results: [State] = []
        var placed = state
        if placed.slots[slot] != nil {
            placed.cost += Self.overwriteCost
            placed.keys.removeAll { ZhuyinKeyboard.symbols[$0].flatMap(ZhuyinKeyboard.slot) == slot }
        }
        placed.slots[slot] = symbol
        placed.keys.append(character)
        results.append(placed)
        // The syllable so far is a whole first-tone syllable and this key starts the next one.
        if method != .slots, slot == 0, state.slots.contains(where: { $0 != nil }),
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
        next.cost += cost + (implicit ? Self.implicitBoundaryCost : 0)
        return next
    }

    private func together(_ keys: [Key], _ index: Int, _ other: Int) -> Bool {
        guard keys.indices.contains(other), let a = keys[index].time, let b = keys[other].time else { return false }
        return abs(a - b) < Self.togetherGap
    }

    private func swapCost(_ keys: [Key], _ index: Int) -> Double {
        method == .timed ? (together(keys, index, index + 1) ? Self.swapTogetherCost : Self.swapCost * 1.5) : Self.swapCost
    }

    private func skipCost(_ keys: [Key], _ index: Int) -> Double {
        guard method == .timed else { return Self.skipCost }
        let near = (together(keys, index, index - 1) && KeyChannel.neighbours(of: keys[index - 1].character).contains(keys[index].character))
            || (together(keys, index, index + 1) && KeyChannel.neighbours(of: keys[index + 1].character).contains(keys[index].character))
        return near ? Self.skipTogetherCost : Self.skipCost * 1.5
    }
}
