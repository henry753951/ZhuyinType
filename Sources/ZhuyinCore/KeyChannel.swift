import Foundation

/// Physical US ANSI neighbours and the keyboard error channel (natural-log costs).
public enum KeyChannel {
    // q straddles 1/2, a straddles q/w, and z straddles a/s.
    private static let rows = [Array("1234567890-="), Array("qwertyuiop[]"), Array("asdfghjkl;'"), Array("zxcvbnm,./")]
    private static let table: [Character: [Character]] = {
        var result: [Character: Set<Character>] = [:]
        for (row, keys) in rows.enumerated() {
            for (index, key) in keys.enumerated() {
                func connect(_ other: Character) {
                    result[key, default: []].insert(other)
                    result[other, default: []].insert(key)
                }
                if index + 1 < keys.count { connect(keys[index + 1]) }
                if row > 0 {
                    for above in [index, index + 1] where above < rows[row - 1].count {
                        connect(rows[row - 1][above])
                    }
                }
            }
        }
        return result.mapValues { $0.sorted() }
    }()
    public static func neighbours(of key: Character) -> [Character] { table[key] ?? [] }

    /// Key centres in key widths: rows are staggered as on a Mac keyboard (number row at 0, Q row +0.5, A row +0.75,
    /// Z row +1.25).
    private static let centres: [Character: (x: Double, y: Double)] = {
        let offsets = [0.0, 0.5, 0.75, 1.25]
        var result: [Character: (x: Double, y: Double)] = [:]
        for (row, keys) in rows.enumerated() {
            for (index, key) in keys.enumerated() { result[key] = (Double(index) + offsets[row], Double(row)) }
        }
        return result
    }()

    /// Hit-area substitution costs instead of one cost for every neighbour (a setting for evaluation).
    nonisolated(unsafe) public static var hotspot = false
    static let hotspotBase = 9.0
    static let hotspotSigmaX = 0.5
    static let hotspotSigmaY = 0.4

    /// −log of how likely a press meant for `intended` lands on `typed`, up to a constant: an anisotropic Gaussian
    /// around the intended key's centre, wider across a row than between rows.
    static func hotspotCost(_ intended: Character, _ typed: Character) -> Double {
        guard let a = centres[intended], let b = centres[typed] else { return substitution }
        let dx = b.x - a.x, dy = b.y - a.y
        return hotspotBase + dx * dx / (2 * hotspotSigmaX * hotspotSigmaX) + dy * dy / (2 * hotspotSigmaY * hotspotSigmaY)
    }

    // Calibrated on 400 runs per dev set, seed 7: scales 1/3/4/5/6/10/16; 5 wins the clean-accuracy constraint.
    static let tone34 = 5.0
    static let tone67 = 4.5
    static let substitution = 11.0
    static let transposition = 9.0
    static let deletion = 12.5
    static let insertion = 12.5
    private static let inputKeys = Array("1234567890qwertyuiopasdfghjkl;zxcvbnm,./- ".utf8)
    private static let symbolCodes = Dictionary(uniqueKeysWithValues: ZhuyinKeyboard.symbols.values.sorted().enumerated().map { ($0.element, $0.offset + 1) })
    // Six bits per slot, plus three tone bits: parsing edits needs no Unicode strings or allocations.
    static func code(for reading: String) -> Int {
        var code = 0
        for symbol in reading {
            if let slot = ZhuyinKeyboard.slot(of: symbol), let value = symbolCodes[symbol] {
                code = (code & ~(63 << (slot * 6))) | (value << (slot * 6))
            } else if let tone = Array("ˊˇˋ˙").firstIndex(of: symbol) { code |= (tone + 1) << 18 }
        }
        return code
    }
    private static let keyCodes: [(mask: Int, value: Int)] = (0..<128).map { byte in
        let key = Character(UnicodeScalar(byte)!)
        if let symbol = ZhuyinKeyboard.symbols[key], let slot = ZhuyinKeyboard.slot(of: symbol) {
            return (63 << (slot * 6), symbolCodes[symbol]! << (slot * 6))
        }
        if let tone = ZhuyinKeyboard.tones[key] { return (7 << 18, code(for: tone)) }
        return (0, 0)
    }
    private static let neighbourBytes: [[UInt8]] = (0..<128).map { byte in
        neighbours(of: Character(UnicodeScalar(byte)!)).compactMap { key in
            guard ZhuyinKeyboard.symbols[key] != nil || ZhuyinKeyboard.tones[key] != nil else { return nil }
            return key.asciiValue
        }
    }
    private static func code(for keys: [UInt8]) -> Int? {
        var code = 0, hasSymbols = false
        for (index, key) in keys.enumerated() {
            guard key < 128 else { return nil }
            let item = keyCodes[Int(key)]
            if item.mask == 7 << 18 || key == 32 {
                guard index == keys.count - 1 else { return nil }
            } else if item.mask == 0 { return nil }
            else { hasSymbols = true }
            code = (code & ~item.mask) | item.value
        }
        return hasSymbols ? code : nil
    }

    /// The keys that type `reading` on the standard layout: symbols, then the tone key or Space.
    static func keys(for reading: String) -> String? {
        var keys = ""
        var tone: Character = " "
        for symbol in reading {
            if let key = keyForSymbol[symbol] { keys.append(key) }
            else if let key = keyForTone[symbol] { tone = key }
            else { return nil }
        }
        return keys + String(tone)
    }
    private static let keyForSymbol = Dictionary(uniqueKeysWithValues: ZhuyinKeyboard.symbols.map { ($1, $0) })
    private static let keyForTone: [Character: Character] = ["ˊ": "6", "ˇ": "3", "ˋ": "4", "˙": "7"]

    /// Keys typed in place of others, by a minimal edit alignment of what was typed with what was meant.
    static func substitutions(typed: String, intended: String) -> [(typed: Character, intended: Character)] {
        let a = Array(typed), b = Array(intended)
        var cost = Array(repeating: Array(repeating: 0, count: b.count + 1), count: a.count + 1)
        for i in 0...a.count { cost[i][0] = i }
        for j in 0...b.count { cost[0][j] = j }
        for i in stride(from: 1, through: a.count, by: 1) {
            for j in stride(from: 1, through: b.count, by: 1) {
                cost[i][j] = min(cost[i - 1][j] + 1, cost[i][j - 1] + 1, cost[i - 1][j - 1] + (a[i - 1] == b[j - 1] ? 0 : 1))
            }
        }
        var pairs: [(Character, Character)] = []
        var i = a.count, j = b.count
        while i > 0 && j > 0 {
            if cost[i][j] == cost[i - 1][j - 1] + (a[i - 1] == b[j - 1] ? 0 : 1) {
                if a[i - 1] != b[j - 1] { pairs.append((a[i - 1], b[j - 1])) }
                i -= 1; j -= 1
            } else if cost[i][j] == cost[i - 1][j] + 1 { i -= 1 } else { j -= 1 }
        }
        return pairs.reversed()
    }

    private static func edits(_ keys: [UInt8], discount: (UInt8, UInt8) -> Double, _ visit: ([UInt8], Double) -> Void) {
        var edited = keys
        for index in keys.indices {
            let old = keys[index]
            if old < 128 {
                for key in neighbourBytes[Int(old)] {
                    edited[index] = key
                    let cost = (old == 51 && key == 52) || (old == 52 && key == 51) ? tone34
                        : (old == 54 && key == 55) || (old == 55 && key == 54) ? tone67
                        : hotspot ? hotspotCost(Character(UnicodeScalar(key)), Character(UnicodeScalar(old))) : substitution
                    visit(edited, cost - discount(old, key))
                }
                edited[index] = old
            }
            if index + 1 < keys.count && old != keys[index + 1] {
                edited.swapAt(index, index + 1); visit(edited, transposition); edited.swapAt(index, index + 1)
            }
            edited.remove(at: index); visit(edited, deletion); edited.insert(old, at: index)
        }
        for index in 0...keys.count {
            edited.insert(32, at: index)
            for key in inputKeys { edited[index] = key; visit(edited, insertion) }
            edited.remove(at: index)
        }
    }

    /// `discount(typed, intended)`: how much cheaper this user's own slips are (personal hit areas).
    static func alternatives(for keys: String, discount: (UInt8, UInt8) -> Double = { _, _ in 0 },
                             valid: (Int) -> String?) -> [ReadingAlternative] {
        let keys = Array(keys.utf8), exact = code(for: keys)
        var best: [Int: Double] = [:]
        var first: [[UInt8]: Double] = [:]
        func accept(_ edited: [UInt8], _ cost: Double) {
            guard let code = code(for: edited), code != exact, valid(code) != nil else { return }
            best[code] = min(best[code] ?? .infinity, cost)
        }
        edits(keys, discount: discount) { edited, cost in
            first[edited] = min(first[edited] ?? .infinity, cost)
            accept(edited, cost)
        }
        if keys.count >= 4 {
            var cutoff = best.values.sorted().dropFirst(7).first ?? .infinity
            for (edited, cost) in first.sorted(by: { $0.value == $1.value ? $0.key.lexicographicallyPrecedes($1.key) : $0.value < $1.value }) {
                if cost + min(tone34, tone67, substitution, transposition, deletion, insertion) > cutoff { break }
                edits(edited, discount: discount) { twice, extra in
                    if cost + extra <= cutoff { accept(twice, cost + extra) }
                }
                cutoff = best.values.sorted().dropFirst(7).first ?? .infinity
            }
        }
        return best.compactMap { code, cost in valid(code).map { ReadingAlternative(reading: $0, cost: cost) } }.sorted {
            $0.cost == $1.cost ? $0.reading < $1.reading : $0.cost < $1.cost
        }.prefix(8).map { $0 }
    }
}

struct ReadingAlternative {
    var reading: String
    var cost: Double
    /// A fuzzy-sound match: ranked by its cost but not counted as a typing error.
    var fuzzy = false
}
