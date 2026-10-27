// Shortcuts the user can rebind on the 快捷鍵 page.  Typing itself (Zhuyin and tone keys, Space, Return, ⌫, Esc and
// the plain arrows) stays fixed; the commands here each have one shortcut, stored in Preferences, and the controller
// turns a matching key into the command's own input before the composer sees it.  A command's default key that is no
// longer bound to it goes to the app.
import AppKit
import ZhuyinCore

/// A key with modifiers on the US layout: a printable character, or one of `Shortcut.namedKeys`.
struct Shortcut: Codable, Hashable {
    var key: String
    var control = false
    var option = false
    var shift = false
    var command = false

    static let namedKeys: [UInt16: String] = [
        125: "↓", 126: "↑", 123: "←", 124: "→", 49: "Space", 48: "Tab", 36: "Return", 76: "Return", 53: "Esc",
        51: "⌫", 117: "⌦", 116: "PageUp", 121: "PageDown", 115: "Home", 119: "End",
    ]

    init(_ key: String, control: Bool = false, option: Bool = false, shift: Bool = false, command: Bool = false) {
        self.key = key
        self.control = control
        self.option = option
        self.shift = shift
        self.command = command
    }

    init?(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) {
        guard let key = Self.namedKeys[keyCode] ?? KeyMapping.printable[keyCode].map(String.init) else { return nil }
        self.init(key, control: modifiers.contains(.control), option: modifiers.contains(.option),
                  shift: modifiers.contains(.shift), command: modifiers.contains(.command))
    }

    private var hasModifiers: Bool { control || option || shift || command }

    /// Keycap labels in the macOS order ⌃⌥⇧⌘, then the key.
    var keycaps: [String] {
        var caps: [String] = []
        if control { caps.append("⌃") }
        if option { caps.append("⌥") }
        if shift { caps.append("⇧") }
        if command { caps.append("⌘") }
        let names = ["Space": "空白", "PageUp": "Page Up", "PageDown": "Page Down"]
        caps.append(names[key] ?? (key.count == 1 ? key.uppercased() : key))
        return caps
    }

    /// The compact form for the candidate window's keycaps: "⇧↓", "⌃S".
    var compact: String { keycaps.count == 1 ? keycaps[0] : keycaps.dropLast().joined() + keycaps.last! }

    /// Why this key cannot be a shortcut, or nil when it can.
    var reservedReason: String? {
        if command { return "⌘ 組合鍵留給 App" }
        let printable = key.count == 1 && !["↓", "↑", "←", "→", "⌫", "⌦"].contains(key)
        if !hasModifiers {
            if ["↓", "↑", "←", "→", "Space", "Return", "Esc", "⌫", "⌦", "Home", "End", "PageUp", "PageDown"].contains(key) {
                return "這個鍵用在打字、選字或送出"
            }
            if printable && key != "`" { return "這個鍵用在打注音、聲調或標點" }
        }
        if printable && shift && !control && !option { return "Shift 加這個鍵打中文標點或大寫字母" }
        if printable && option && !control && !key.first!.isLetter { return "Option 加這個鍵打鍵上印的符號" }
        return nil
    }
}

/// The keys the candidate window shows on its keycaps; empty for a command that is turned off.
struct CandidateKeyLabels: Equatable {
    var zhuyin = "`"
    var single = "⇧↓"
    var corrections = "⇧↑"
    /// Clicking the Zhuyin row types the Zhuyin itself.
    var zhuyinClickable = true
}

/// Key features that can be turned off on the 快捷鍵 page.  Each is implemented in the app: the commands' shortcuts
/// are left out (their keys go to the app), or the controller passes the keys on, or a click is ignored.  The engine
/// is unchanged.
enum KeyFeature: String, CaseIterable, Identifiable {
    /// ` (typeZhuyin) and its restore.
    case typeZhuyin
    /// Clicking the Zhuyin row of the candidate window.
    case zhuyinRowClick
    case singleCharacterList
    case correctionsList
    /// ⇧← ⇧→ and Return: add a custom phrase.
    case selectPhrase
    /// Tab and ⇧Tab turn the list's pages.
    case pageKeys
    /// ⌥⌫ deletes a word.
    case deleteWord
    /// ⌃ with a symbol key types Chinese punctuation.
    case controlPunctuation
    /// ⌥ with a digit or symbol key types the symbol printed on it.
    case optionSymbols
    /// Caps Lock types English.
    case capsLockEnglish
    /// Deleting what was just sent and typing again counts as a correction (and keeps the sentence going).
    case deleteAfterCommit
    var id: String { rawValue }
}

/// The commands with a rebindable shortcut, most used first.
enum ShortcutCommand: String, CaseIterable, Codable, Identifiable {
    case singleCharacterList, correctionsList, typeZhuyin, nextPage, previousPage
    case selectLeft, selectRight, deleteWord, commitWithNewline

    var id: String { rawValue }

    var title: String {
        switch self {
        case .singleCharacterList: return "只列一個字，再按一次還原"
        case .correctionsList: return "列出按錯修正的字，再按一次還原"
        case .typeZhuyin: return "打出注音本身"
        case .nextPage: return "下一頁"
        case .previousPage: return "上一頁"
        case .selectLeft: return "往左選取，再按 Return 加入自訂詞"
        case .selectRight: return "往右選取，再按 Return 加入自訂詞"
        case .deleteWord: return "刪掉一個詞"
        case .commitWithNewline: return "送出並換行"
        }
    }

    var defaultShortcut: Shortcut {
        switch self {
        case .singleCharacterList: return Shortcut("↓", shift: true)
        case .correctionsList: return Shortcut("↑", shift: true)
        case .typeZhuyin: return Shortcut("`")
        case .nextPage: return Shortcut("Tab")
        case .previousPage: return Shortcut("Tab", shift: true)
        case .selectLeft: return Shortcut("←", shift: true)
        case .selectRight: return Shortcut("→", shift: true)
        case .deleteWord: return Shortcut("⌫", option: true)
        case .commitWithNewline: return Shortcut("Return", shift: true)
        }
    }

    /// What the composer receives for the command.
    var input: KeyInput {
        switch self {
        case .singleCharacterList: return .shiftDown
        case .correctionsList: return .shiftUp
        case .typeZhuyin: return .char("`")
        case .nextPage: return .tab
        case .previousPage: return .shiftTab
        case .selectLeft: return .shiftLeft
        case .selectRight: return .shiftRight
        case .deleteWord: return .optionBackspace
        case .commitWithNewline: return .shiftEnter
        }
    }

    /// The short name on the 快捷鍵 page and in conflict messages.
    var label: String {
        switch self {
        case .singleCharacterList: return "只列一個字"
        case .correctionsList: return "列出修正的字"
        case .typeZhuyin: return "打出注音本身"
        case .nextPage: return "下一頁"
        case .previousPage: return "上一頁"
        case .selectLeft: return "往左選取"
        case .selectRight: return "往右選取"
        case .deleteWord: return "刪掉一個詞"
        case .commitWithNewline: return "送出並換行"
        }
    }

    /// Why `shortcut` cannot be binding `index` of `command`, naming the action that has it; nil when it can.
    static func problem(assigning shortcut: Shortcut, to command: ShortcutCommand, at index: Int,
                        in bindings: [ShortcutCommand: [Shortcut]]) -> String? {
        if let reason = shortcut.reservedReason { return reason }
        for (other, list) in bindings {
            for (position, bound) in list.enumerated() where bound == shortcut && !(other == command && position == index) {
                return other == command ? "已是這個功能的另一個按鍵" : "已用於「\(other.label)」"
            }
        }
        return nil
    }

    /// The default bindings: one each.
    var defaultBindings: [Shortcut] { [defaultShortcut] }
    static var defaultBindings: [ShortcutCommand: [Shortcut]] {
        Dictionary(uniqueKeysWithValues: allCases.map { ($0, $0.defaultBindings) })
    }

    /// The feature that turns the command off, or nil when it cannot be turned off.
    var feature: KeyFeature? {
        switch self {
        case .singleCharacterList: return .singleCharacterList
        case .correctionsList: return .correctionsList
        case .typeZhuyin: return .typeZhuyin
        case .nextPage, .previousPage: return .pageKeys
        case .selectLeft, .selectRight: return .selectPhrase
        case .deleteWord: return .deleteWord
        case .commitWithNewline: return nil
        }
    }

    /// Inputs that mean a command: produced only through a binding, never straight from a key.
    static let commandInputs = Set(allCases.map(\.input))

    static var defaults: [ShortcutCommand: Shortcut] {
        Dictionary(uniqueKeysWithValues: allCases.map { ($0, $0.defaultShortcut) })
    }
}
