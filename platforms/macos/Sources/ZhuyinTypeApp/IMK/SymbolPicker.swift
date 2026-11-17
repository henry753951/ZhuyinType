// 符號與 Emoji: the picker the symbol shortcut opens while typing.  The client app keeps the keyboard focus; the
// controller routes keys here while the picker is open (docs/product/DESIGN.md).  This file is the picker's state and
// key handling, with no UI: `SymbolPickerPanel` draws it.  The symbols, their names and the search come from the
// engine's zhuyin-symbols crate (`SymbolSource`); 最近使用 is a file of this Mac (`RecentSymbols`).
import AppKit
import ZhuyinEngine

struct SymbolItem: Equatable {
    let text: String
    /// The CLDR name (zh_Hant), or a short Chinese name: tooltips, VoiceOver and the picker's footer.
    let name: String
}

struct SymbolCategory: Equatable {
    let id: String
    let title: String
    let symbols: [SymbolItem]
}

/// Where the picker's symbols come from: the engine (`EngineSymbolSource`), or a fixed set for previews and tests.
protocol SymbolSource: AnyObject {
    /// Every category except 最近使用, in the picker's order.
    var categories: [SymbolCategory] { get }
    /// Ranked matches for the raw keys typed (read as Zhuyin on the standard layout and as English); empty for "".
    func search(_ query: String, limit: Int) -> [SymbolItem]
    /// The query as shown: Zhuyin when the keys decode as Zhuyin, otherwise as typed.
    func displayQuery(_ query: String) -> String
    /// The recent list's stored text after `symbol` was inserted (newest first, at most the capacity).
    func recentPush(stored: String, symbol: String) -> String
    /// The name of a symbol in any category, for 最近使用.
    func name(of text: String) -> String?
}

extension SymbolSource {
    func name(of text: String) -> String? {
        for category in categories { if let item = category.symbols.first(where: { $0.text == text }) { return item.name } }
        return nil
    }
}

/// 最近使用: `recent-symbols.txt` in the app's data directory, one symbol per line, newest first.  Removed by
/// 回復預設設定 and by 記住的字詞 › 全部清除.  What is inserted is never logged.
final class RecentSymbols {
    static let fileName = "recent-symbols.txt"
    static let shared = RecentSymbols(url: EngineHost.userDirectory.appendingPathComponent(fileName))
    let url: URL

    init(url: URL) { self.url = url }

    var items: [String] {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        return text.split(whereSeparator: \.isNewline).map(String.init).filter { !$0.isEmpty }
    }

    func push(_ symbol: String, source: SymbolSource) {
        let stored = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        let text = source.recentPush(stored: stored, symbol: symbol)
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? text.write(to: url, atomically: true, encoding: .utf8)
    }

    func clear() { try? FileManager.default.removeItem(at: url) }
}

/// The open picker: its tabs, the search query, the highlight and the rows in view.
final class SymbolPicker {
    /// Items per row; digits 1–9 pick from the highlighted row.
    static let columns = 9
    /// Rows in view; Page Up and Page Down move this many.
    static let visibleRows = 6
    static let searchLimit = 180

    enum Outcome: Equatable {
        /// The picker changed (or the key did nothing): draw it again.
        case handled
        /// Insert this symbol and close.
        case insert(String)
        case close
        /// Not a picker key (⌘ and other modifier combinations): close, and the app gets the key.
        case passThrough
    }

    let source: SymbolSource
    private(set) var tabs: [SymbolCategory]
    private(set) var tab = 0
    /// The raw keys typed (or the prefilled composition).
    private(set) var query = ""
    /// The query is the composition the picker was opened from: shown as is, replaced by the first key.
    private(set) var prefilled = false
    private(set) var results: [SymbolItem] = []
    /// Index into `items`.
    private(set) var highlight = 0
    /// The first row in view.
    private(set) var topRow = 0

    init(source: SymbolSource, recent: [String], prefill: String = "") {
        self.source = source
        let recentItems = recent.map { SymbolItem(text: $0, name: source.name(of: $0) ?? "") }
        tabs = (recentItems.isEmpty ? [] : [SymbolCategory(id: "recent", title: "最近使用", symbols: recentItems)])
            + source.categories.filter { !$0.symbols.isEmpty }
        if !prefill.isEmpty {
            query = prefill
            prefilled = true
            search()
        }
    }

    var isSearching: Bool { !query.isEmpty }
    var items: [SymbolItem] { isSearching ? results : tabs.indices.contains(tab) ? tabs[tab].symbols : [] }
    var shownQuery: String { prefilled ? query : source.displayQuery(query) }
    var rowCount: Int { (items.count + Self.columns - 1) / Self.columns }
    var highlightedRow: Int { highlight / Self.columns }
    var highlighted: SymbolItem? { items.indices.contains(highlight) ? items[highlight] : nil }

    func handle(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Outcome {
        let modifiers = modifiers.intersection([.command, .control, .option, .shift])
        if !modifiers.isDisjoint(with: [.command, .control, .option]) { return .passThrough }
        let shift = modifiers.contains(.shift)
        switch keyCode {
        case 123: move(by: -1)
        case 124: move(by: 1)
        case 126: move(by: -Self.columns)
        case 125: move(by: Self.columns)
        case 116: move(by: -Self.columns * Self.visibleRows)
        case 121: move(by: Self.columns * Self.visibleRows)
        case 48:
            if isSearching { clearQuery() } else { selectTab(tab + (shift ? -1 : 1)) }
        case 36, 76:
            return highlighted.map { .insert($0.text) } ?? .handled
        case 53:
            if isSearching { clearQuery(); return .handled }
            return .close
        case 51:
            if prefilled { clearQuery() } else if isSearching { query.removeLast(); search() }
        case 49:
            if !isSearching || prefilled { return highlighted.map { .insert($0.text) } ?? .handled }
            append(" ")
        default:
            guard let character = KeyMapping.printable[keyCode] else { return .handled }
            if let digit = character.wholeNumberValue {
                // Digits always pick, from the highlighted row; 0 does nothing.
                guard digit > 0 else { return .handled }
                let index = highlightedRow * Self.columns + digit - 1
                return items.indices.contains(index) ? .insert(items[index].text) : .handled
            }
            append(shift ? String(KeyMapping.shifted[character] ?? Character(character.uppercased())) : String(character))
        }
        return .handled
    }

    /// A click on an item in view.
    func item(at index: Int) -> String? { items.indices.contains(index) ? items[index].text : nil }

    func selectTab(_ index: Int) {
        guard !tabs.isEmpty else { return }
        tab = (index % tabs.count + tabs.count) % tabs.count
        if isSearching { query = ""; prefilled = false; results = [] }
        highlight = 0
        topRow = 0
    }

    /// Scrolls the rows in view (the mouse wheel), keeping the highlight where it is when it stays in view.
    func scroll(rows: Int) {
        let maxTop = max(0, rowCount - Self.visibleRows)
        topRow = min(max(0, topRow + rows), maxTop)
    }

    private func move(by delta: Int) {
        let count = items.count
        guard count > 0 else { return }
        if abs(delta) == 1 {
            // ← → go round the rows: past the end of a row to the next one, past the last item to the first.
            highlight = (highlight + delta + count) % count
        } else {
            highlight = min(max(0, highlight + delta), count - 1)
        }
        keepInView()
    }

    private func keepInView() {
        let row = highlightedRow
        if row < topRow { topRow = row }
        if row >= topRow + Self.visibleRows { topRow = row - Self.visibleRows + 1 }
    }

    private func append(_ key: String) {
        if prefilled { query = ""; prefilled = false }
        query += key
        search()
    }

    private func clearQuery() {
        query = ""
        prefilled = false
        results = []
        highlight = 0
        topRow = 0
    }

    private func search() {
        results = query.isEmpty ? [] : source.search(query, limit: Self.searchLimit)
        highlight = 0
        topRow = 0
    }
}

enum SymbolSources {
    /// The input method's symbols; nil when this build has none, which leaves the feature out (its key goes to the
    /// app and the settings do not show it).
    #if ZT_SYMBOLS
    static let app: SymbolSource? = EngineSymbolSource()
    #else
    static let app: SymbolSource? = SampleSymbolSource()
    #endif
    static var isAvailable: Bool { app != nil }
}

/// A small fixed set of symbols, for the UI previews and the self-tests (and until the engine provides them).
final class SampleSymbolSource: SymbolSource {
    let categories: [SymbolCategory] = [
        SymbolCategory(id: "punctuation", title: "常用標點", symbols: [
            ("，", "逗號"), ("。", "句號"), ("、", "頓號"), ("；", "分號"), ("：", "冒號"), ("？", "問號"), ("！", "驚嘆號"),
            ("「", "上引號"), ("」", "下引號"), ("『", "上雙引號"), ("』", "下雙引號"), ("（", "左括號"), ("）", "右括號"),
            ("…", "刪節號"), ("—", "破折號"), ("·", "間隔號"),
        ].map { SymbolItem(text: $0.0, name: $0.1) }),
        SymbolCategory(id: "math", title: "數學", symbols: [
            ("＋", "加號"), ("－", "減號"), ("×", "乘號"), ("÷", "除號"), ("＝", "等號"), ("≠", "不等於"), ("±", "正負號"),
            ("≤", "小於或等於"), ("≥", "大於或等於"), ("∞", "無限大"), ("√", "根號"), ("π", "圓周率"),
        ].map { SymbolItem(text: $0.0, name: $0.1) }),
        SymbolCategory(id: "arrows", title: "箭頭", symbols: [
            ("←", "左箭頭"), ("→", "右箭頭"), ("↑", "上箭頭"), ("↓", "下箭頭"), ("↔", "左右箭頭"), ("⇒", "雙線右箭頭"),
        ].map { SymbolItem(text: $0.0, name: $0.1) }),
        SymbolCategory(id: "smileys", title: "笑臉與情緒", symbols: [
            ("😀", "笑臉"), ("😃", "張嘴笑臉"), ("😄", "眯眼張嘴笑臉"), ("😁", "露齒笑"), ("😆", "瞇眼笑"), ("😅", "冒汗笑臉"),
            ("🤣", "笑到在地上打滾"), ("😂", "笑哭"), ("🙂", "微笑"), ("😉", "眨眼"), ("😊", "害羞微笑"), ("😍", "愛心眼"),
            ("🥰", "帶愛心的笑臉"), ("😘", "飛吻"), ("😭", "大哭"), ("😡", "生氣"), ("🤔", "思考"), ("😴", "睡著"),
            ("❤️", "紅心"),
        ].map { SymbolItem(text: $0.0, name: $0.1) }),
        SymbolCategory(id: "animals", title: "動物與自然", symbols: [
            ("🐶", "狗臉"), ("🐱", "貓臉"), ("🐭", "老鼠臉"), ("🐰", "兔子臉"), ("🐻", "熊臉"), ("🐼", "熊貓臉"),
        ].map { SymbolItem(text: $0.0, name: $0.1) }),
    ]

    func search(_ query: String, limit: Int) -> [SymbolItem] {
        let words: [String: String] = ["vul": "笑", "smile": "笑", "heart": "心", "ru": "哭", "cry": "哭"]
        let needle = words[query.lowercased()] ?? query
        guard !needle.isEmpty else { return [] }
        return Array(categories.flatMap(\.symbols).filter { $0.name.contains(needle) || $0.text == needle }.prefix(limit))
    }

    func displayQuery(_ query: String) -> String { query == "vul" ? "ㄒㄧㄠ" : query == "ru" ? "ㄐㄧ" : query }

    func recentPush(stored: String, symbol: String) -> String {
        let old = stored.split(whereSeparator: \.isNewline).map(String.init).filter { !$0.isEmpty && $0 != symbol }
        return ([symbol] + old).prefix(32).map { $0 + "\n" }.joined()
    }
}
