import AppKit
import ZhuyinEngine

private final class ShortcutTestClient: TextClient {
    var bundleIdentifier = "org.zhuyintype.self-test.shortcuts"
    var markedRange = NSRange(location: NSNotFound, length: 0)
    var selectedRange = NSRange(location: 0, length: 0)
    var inserts: [String] = []
    func insertText(_ text: String, replacementRange: NSRange) { inserts.append(text) }
    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) {}
    func caretRect(cursorIndex: Int) -> NSRect { NSRect(x: 100, y: 100, width: 0, height: 20) }
    var hasInsertionPoint = true
    func textBeforeInsertion(maxLength: Int) -> String? { "" }
}

/// The default keys against what they do and what the app says about them, run by `ControllerCoreSelfTest`: every
/// default is a key the keyboard can press and reaches its command, the texts that name a key name the bound one, and
/// 加入自訂詞 acts on the highlighted candidate only (DESIGN.md §2.1「按鍵的原則」).
enum ShortcutSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(engine: Engine, test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
            if !condition() { throw Failure(description: message) }
        }
        /// The hardware key that types `shortcut`, with its modifiers (nil: no key does).
        func event(for shortcut: Shortcut) -> (keyCode: UInt16, modifiers: NSEvent.ModifierFlags)? {
            let code = Shortcut.namedKeys.first { $0.value == shortcut.key }?.key
                ?? KeyMapping.printable.first { String($0.value) == shortcut.key }?.key
            guard let code else { return nil }
            var modifiers: NSEvent.ModifierFlags = []
            if shortcut.control { modifiers.insert(.control) }
            if shortcut.option { modifiers.insert(.option) }
            if shortcut.shift { modifiers.insert(.shift) }
            if shortcut.command { modifiers.insert(.command) }
            return (code, modifiers)
        }
        func session(_ preferences: Preferences) -> (ControllerCore, ShortcutTestClient) {
            let client = ShortcutTestClient()
            var options = ComposerOptions()
            options.learningEnabled = false
            let core = ControllerCore(engine: engine, client: client, options: options, isEnglish: false)
            core.secureInput = { false }
            core.shortcuts = Dictionary(preferences.activeBindings.flatMap { command, list in list.map { ($0, command) } },
                                        uniquingKeysWith: { first, _ in first })
            return (core, client)
        }

        test("keys: every default is a key the keyboard presses, free of the reserved keys, and reaches its command") {
            let preferences = Preferences(defaults: MemorySettingsStorage())
            let (core, _) = session(preferences)
            for command in ShortcutCommand.allCases {
                for shortcut in command.defaultBindings {
                    guard let (code, modifiers) = event(for: shortcut) else {
                        return try expect(false, "\(command): no key types \(shortcut.compact)")
                    }
                    try expect(Shortcut(keyCode: code, modifiers: modifiers) == shortcut, "\(command): \(shortcut.compact) reads back differently")
                    try expect(core.shortcuts[shortcut] == command, "\(command): \(shortcut.compact) runs \(String(describing: core.shortcuts[shortcut]))")
                    try expect(shortcut.reservedReason(for: command) == nil, "\(command): \(shortcut.compact) is reserved")
                }
            }
        }

        test("keys: the candidate window's switches and the 排列 text name the keys bound now") {
            let preferences = Preferences(defaults: MemorySettingsStorage())
            let labels = preferences.candidateKeyLabels
            try expect(labels.single == "⇧↓" && labels.corrections == "⇧↑" && labels.zhuyin == "`", "default chips \(labels)")
            let layout = SettingsRegistry.setting("candidateLayout")
            try expect(layout?.detail?(preferences) == "↑ ↓ 移動反白，← → 或 ⌥← ⌥→ 改前一個或後一個字",
                       "vertical: \(layout?.detail?(preferences) ?? "nil")")
            _ = preferences.bind(.singleCharacterList, at: 0, to: Shortcut("s", control: true))
            _ = preferences.bind(.previousCharacter, at: 0, to: Shortcut("j", control: true))
            preferences.candidateLayout = .horizontal
            try expect(preferences.candidateKeyLabels.single == "⌃S", "rebound chip \(preferences.candidateKeyLabels.single)")
            try expect(layout?.detail?(preferences) == "← → 移動反白，↑ ↓ 翻頁，⌃J ⌥→ 改前一個或後一個字",
                       "horizontal: \(layout?.detail?(preferences) ?? "nil")")
            preferences.setEnabled(.singleCharacterList, false)
            try expect(preferences.candidateKeyLabels.single.isEmpty, "a turned-off switch still shows its key")
        }

        test("keys: ⌃Return adds the highlighted candidate; with no list it adds nothing and keeps the composition") {
            let (core, client) = session(Preferences(defaults: MemorySettingsStorage()))
            let before = Set(engine.userPhrases().map(\.word))
            for key in "gj zj6" { _ = core.handle(key == " " ? .space : .char(key)) }
            let composition = core.composer.presentation.text
            try expect(core.keyDown(keyCode: 36, modifiers: .control), "⌃Return went to the app while composing")
            try expect(Set(engine.userPhrases().map(\.word)) == before, "⌃Return with no list added a custom word")
            try expect(core.composer.presentation.text == composition && client.inserts.isEmpty, "⌃Return changed the composition")
            _ = core.handle(.down)
            guard let list = core.composer.presentation.candidates else { return try expect(false, "↓ opened no list") }
            let highlighted = list.items[list.selection].text
            try expect(core.keyDown(keyCode: 36, modifiers: .control), "⌃Return in the list not handled")
            try expect(engine.userPhrases().contains { $0.word == highlighted }, "the highlighted \(highlighted) was not added")
            try expect(core.keyDown(keyCode: 6, modifiers: .control), "⌃Z did not undo")
            try expect(Set(engine.userPhrases().map(\.word)) == before, "⌃Z left the custom word")
        }
    }
}
