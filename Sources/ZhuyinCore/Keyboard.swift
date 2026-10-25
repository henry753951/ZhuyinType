import Foundation

enum ZhuyinKeyboard {
    static let symbols: [Character: Character] = Dictionary(uniqueKeysWithValues:
        zip(Array("1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-"),
            Array("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ")))
    static let tones: [Character: String] = ["6": "ˊ", "3": "ˇ", "4": "ˋ", "7": "˙"]
    static let punctuation: [Character: String] = ["[": "「", "]": "」", "'": "、", "\\": "＼", "=": "＝", "`": "‵"]
    static let shiftedPunctuation: [Character: String] = [
        ",": "，", ".": "。", "/": "？", ";": "：", "'": "；", "1": "！", "2": "＠",
        "3": "＃", "4": "＄", "5": "％", "6": "…", "7": "＆", "8": "＊", "9": "（",
        "0": "）", "-": "——", "=": "＋", "[": "『", "]": "』", "\\": "｜", "`": "～"
    ]
    static func slot(of symbol: Character) -> Int? {
        if "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ".contains(symbol) { return 0 }
        if "ㄧㄨㄩ".contains(symbol) { return 1 }
        if "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ".contains(symbol) { return 2 }
        return nil
    }
    static func literal(for key: KeyInput) -> String? {
        switch key {
        case .char(let c): return punctuation[c]
        case .shiftedChar(let c):
            if c.isASCII && c.isLetter { return String(c).uppercased() }
            return shiftedPunctuation[c]
        default: return nil
        }
    }
    static func alternatives(for text: String) -> [String] {
        let groups = [
            ["，", ",", "、", "；", "：", "…"], ["。", ".", "．", "…"],
            ["？", "?", "！", "!"], ["「", "『", "（", "〈", "《", "【"],
            ["」", "』", "）", "〉", "》", "】"], ["——", "—", "－", "-"],
            [" ", "　"], ["＼", "\\", "／", "/"], ["＝", "=", "＋", "+"],
            ["‵", "～", "~", "`"], ["｜", "|", "／"]
        ]
        return [text] + (groups.first(where: { $0.contains(text) }) ?? []).filter { $0 != text }
    }
}
