import Foundation

enum ZhuyinKeyboard {
    static let symbols: [Character: Character] = Dictionary(uniqueKeysWithValues:
        zip(Array("1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-"),
            Array("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ")))
    static let tones: [Character: String] = ["6": "ˊ", "3": "ˇ", "4": "ˋ", "7": "˙"]
    static let punctuation: [Character: String] = ["[": "「", "]": "」", "'": "、", "\\": "＼", "=": "＝"]
    static let shiftedPunctuation: [Character: String] = [
        ",": "，", ".": "。", "/": "？", ";": "：", "'": "；", "1": "！", "2": "＠",
        "3": "＃", "4": "＄", "5": "％", "6": "…", "7": "＆", "8": "＊", "9": "（",
        "0": "）", "-": "——", "=": "＋", "[": "『", "]": "』", "\\": "｜", "`": "～"
    ]
    /// The upper character printed on each symbol key of the US layout.
    static let shiftedSymbols: [Character: Character] = Dictionary(uniqueKeysWithValues:
        zip(Array("1234567890-=[]\\;',./`"), Array("!@#$%^&*()_+{}|:\"<>?~")))
    /// Control with a symbol key: Chinese punctuation for the marks Chinese text uses, otherwise the full-width form of
    /// the printed symbol.  Control+[ and Control+] stay the app's (Escape in terminals); [ and ] type 「」.
    static let controlPunctuation: [Character: String] = {
        var table: [Character: String] = [:]
        for scalar in UnicodeScalar(0x21)!.value...UnicodeScalar(0x7e)!.value {
            let character = Character(UnicodeScalar(scalar)!)
            guard !character.isLetter, !character.isNumber, character != "[", character != "]", character != "/" else { continue }
            table[character] = String(UnicodeScalar(scalar - 0x21 + 0xff01)!)
        }
        let chinese: [Character: String] = [",": "，", ".": "。", "'": "、", "?": "？", "!": "！", ":": "：", ";": "；",
                                            "{": "『", "}": "』", "<": "《", ">": "》", "_": "——", "^": "…"]
        return table.merging(chinese) { $1 }
    }()
    static func slot(of symbol: Character) -> Int? {
        if "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ".contains(symbol) { return 0 }
        if "ㄧㄨㄩ".contains(symbol) { return 1 }
        if "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ".contains(symbol) { return 2 }
        return nil
    }
    static func plain(_ reading: String) -> String {
        reading.split(separator: " ").map { part in
            part.last == "˙" ? "˙" + part.dropLast() : String(part)
        }.joined(separator: " ")
    }
    static func literal(for key: KeyInput, keys: PunctuationKeys = .windows) -> String? {
        switch key {
        case .char(let c): return punctuation[c]
        case .shiftedChar(let c):
            if c.isASCII && c.isLetter { return String(c).uppercased() }
            return keys == .windows ? shiftedSymbols[c].map(String.init) : shiftedPunctuation[c]
        case .printed(let c): return String(c)
        case .controlChar(let c): return controlPunctuation[c]
        default: return nil
        }
    }
    static func alternatives(for text: String) -> [String] {
        let groups = [
            ["，", ",", "、", "；", "：", "…"], ["。", ".", "．", "…"],
            ["？", "?", "！", "!"], ["「", "『", "（", "〈", "《", "【"],
            ["」", "』", "）", "〉", "》", "】"], ["——", "—", "－", "-", "_", "＿"],
            ["《", "〈", "＜", "<"], ["》", "〉", "＞", ">"],
            [" ", "　"], ["＼", "\\", "／", "/"], ["＝", "=", "＋", "+"],
            ["‵", "～", "~", "`"], ["｜", "|", "／"]
        ]
        return [text] + (groups.first(where: { $0.contains(text) }) ?? []).filter { $0 != text }
    }
}
