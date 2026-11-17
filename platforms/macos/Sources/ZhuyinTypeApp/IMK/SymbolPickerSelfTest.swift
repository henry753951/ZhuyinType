import AppKit
import ZhuyinEngine

private final class SymbolTestClient: TextClient {
    var bundleIdentifier = "org.zhuyintype.self-test.symbols"
    var markedRange = NSRange(location: NSNotFound, length: 0)
    var selectedRange = NSRange(location: 0, length: 0)
    var inserts: [String] = []
    var marks: [String] = []
    func insertText(_ text: String, replacementRange: NSRange) { inserts.append(text) }
    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) { marks.append(text.string) }
    func caretRect(cursorIndex: Int) -> NSRect { NSRect(x: 100, y: 100, width: 0, height: 20) }
    var hasInsertionPoint = true
    func textBeforeInsertion(maxLength: Int) -> String? { "" }
}

/// The 符號與 Emoji picker's checks, run by `ControllerCoreSelfTest`: the shortcut, the keys while the picker is open,
/// the prefilled search, inserting and 最近使用.
enum SymbolPickerSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(engine: Engine, test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
            if !condition() { throw Failure(description: message) }
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("zt-symbols-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = SampleSymbolSource()
        let comma: UInt16 = 43
        let shortcut: NSEvent.ModifierFlags = [.control, .option]

        func session(english: Bool = false) -> (ControllerCore, SymbolTestClient, RecentSymbols) {
            let client = SymbolTestClient()
            var options = ComposerOptions()
            options.learningEnabled = false
            let core = ControllerCore(engine: engine, client: client, options: options, isEnglish: english)
            core.secureInput = { false }
            core.symbolSource = source
            let recent = RecentSymbols(url: directory.appendingPathComponent(UUID().uuidString).appendingPathComponent(RecentSymbols.fileName))
            core.recentSymbols = recent
            return (core, client, recent)
        }
        func press(_ core: ControllerCore, _ keyCode: UInt16, _ modifiers: NSEvent.ModifierFlags = []) -> Bool {
            core.keyDown(keyCode: keyCode, modifiers: modifiers)
        }

        test("symbols: ⌃⌥, is a free key for the picker, with no conflict among the defaults") {
            let key = ShortcutCommand.symbolPicker.defaultShortcut
            try expect(key == Shortcut(",", control: true, option: true), "the default is not ⌃⌥,")
            let saved = Shortcut.controlPunctuation
            defer { Shortcut.controlPunctuation = saved }
            Shortcut.controlPunctuation = { true }
            try expect(key.reservedReason == nil, "⌃⌥, is reserved: \(key.reservedReason ?? "")")
            try expect(ShortcutCommand.problem(assigning: key, to: .symbolPicker, at: 0, in: ShortcutCommand.defaultBindings) == nil,
                       "⌃⌥, conflicts with another default")
            try expect(ShortcutCommand.symbolPicker.feature == .symbolPicker, "the picker's key cannot be turned off")
            try expect(Preferences(defaults: MemorySettingsStorage()).isEnabled(KeyFeature.symbolPicker), "the picker is off by default")
        }

        test("symbols: the shortcut opens and closes the picker; Esc closes; nothing is typed; Shift does not switch") {
            let (core, client, _) = session()
            var shown = 0
            var hidden = 0
            core.showSymbolPicker = { _, _ in shown += 1 }
            core.hideSymbolPicker = { hidden += 1 }
            try expect(press(core, comma, shortcut) && core.isSymbolPickerOpen && shown == 1, "⌃⌥, did not open the picker")
            try expect(press(core, comma, shortcut) && !core.isSymbolPickerOpen && hidden == 1, "⌃⌥, again did not close it")
            _ = press(core, comma, shortcut)
            try expect(press(core, 53) && !core.isSymbolPickerOpen, "Esc did not close the picker")
            try expect(client.inserts.isEmpty, "opening and closing typed something: \(client.inserts)")
            _ = press(core, comma, shortcut)
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: true, timestamp: 1)
            _ = core.flagsChanged(keyCode: 56, modifiers: [], physicallyDown: false, timestamp: 1.1)
            try expect(!core.isEnglish && core.isSymbolPickerOpen, "a Shift tap with the picker open switched to English")
        }

        test("symbols: turned off, the shortcut goes to the app; in English mode it opens the picker too") {
            let (core, _, _) = session()
            core.shortcuts = [:]
            try expect(!press(core, comma, shortcut) && !core.isSymbolPickerOpen, "with the key unbound the picker opened")
            let (english, _, _) = session(english: true)
            try expect(press(english, comma, shortcut) && english.isSymbolPickerOpen, "English mode: the picker did not open")
        }

        test("symbols: arrows move round the rows, Tab changes the category, digits pick from the highlighted row") {
            let (core, client, recent) = session()
            _ = press(core, comma, shortcut)
            guard let picker = core.symbolPicker else { throw Failure(description: "not open") }
            try expect(picker.tabs.first?.id == "punctuation", "with no recent symbols the first tab is \(picker.tabs.first?.id ?? "")")
            _ = press(core, 124)
            try expect(picker.highlight == 1, "→ did not move the highlight")
            _ = press(core, 123); _ = press(core, 123)
            try expect(picker.highlight == picker.items.count - 1, "← at the first item did not go round to the last")
            _ = press(core, 125)
            try expect(picker.highlight == picker.items.count - 1, "↓ on the last row moved off the items")
            _ = press(core, 126)
            try expect(picker.highlight == picker.items.count - 1 - SymbolPicker.columns, "↑ did not move a row up")
            _ = press(core, 48)
            try expect(picker.tab == 1 && picker.highlight == 0, "Tab did not go to the next category")
            _ = press(core, 48, .shift); _ = press(core, 48, .shift)
            try expect(picker.tab == picker.tabs.count - 1, "⇧Tab did not go round to the last category")
            _ = press(core, 48)
            _ = press(core, 125)
            let expected = picker.items[SymbolPicker.columns + 2].text
            try expect(press(core, 20) && client.inserts == [expected], "3 did not insert the third item of the row: \(client.inserts)")
            try expect(!core.isSymbolPickerOpen, "inserting did not close the picker")
            try expect(recent.items == [expected], "the symbol was not kept as recent: \(recent.items)")
            _ = press(core, comma, shortcut)
            try expect(core.symbolPicker?.tabs.first?.id == "recent" && core.symbolPicker?.items.first?.text == expected,
                       "最近使用 is not the first tab with the symbol")
            try expect(core.symbolPicker?.items.first?.name.isEmpty == false, "a recent symbol has no name")
        }

        test("symbols: typed keys search; Space and Return insert; Esc clears the search first; ⌘ keys go to the app") {
            let (core, client, _) = session()
            _ = press(core, comma, shortcut)
            guard let picker = core.symbolPicker else { throw Failure(description: "not open") }
            for key: UInt16 in [9, 32, 37] { _ = press(core, key) } // v u l: ㄒㄧㄠ
            try expect(picker.query == "vul" && picker.shownQuery == "ㄒㄧㄠ", "the query is \(picker.query) shown as \(picker.shownQuery)")
            try expect(!picker.items.isEmpty && picker.items.allSatisfy { $0.name.contains("笑") }, "ㄒㄧㄠ found \(picker.items.map(\.text))")
            _ = press(core, 51)
            try expect(picker.query == "vu", "⌫ did not delete the last key")
            _ = press(core, 53)
            try expect(core.isSymbolPickerOpen && picker.query.isEmpty, "Esc did not clear the search first")
            try expect(press(core, 49) && client.inserts == [picker.tabs[0].symbols[0].text], "Space did not insert the highlighted item")
            _ = press(core, comma, shortcut)
            _ = press(core, 48)
            _ = press(core, 124)
            let second = core.symbolPicker?.highlighted?.text
            try expect(second == source.categories[0].symbols[1].text, "→ after Tab did not highlight the second item")
            try expect(press(core, 36) && client.inserts.last == second, "Return did not insert the highlighted item")
            _ = press(core, comma, shortcut)
            try expect(!press(core, 9, .command) && !core.isSymbolPickerOpen, "⌘V did not close the picker and go to the app")
        }

        test("symbols: the composition becomes the search, is cancelled, and the first key replaces it") {
            let (core, client, _) = session()
            for key in "su3" { _ = core.handle(.char(key)) }
            let composed = core.composer.presentation.text
            try expect(!composed.isEmpty, "nothing composed")
            _ = press(core, comma, shortcut)
            guard let picker = core.symbolPicker else { throw Failure(description: "not open") }
            try expect(picker.query == composed && picker.prefilled && picker.shownQuery == composed, "the search is not the composition")
            try expect(!core.composer.isComposing && client.inserts.isEmpty && client.marks.last == "", "the composition was not cancelled")
            _ = press(core, 9)
            try expect(picker.query == "v" && !picker.prefilled, "the first key did not replace the prefilled search")
            _ = press(core, 53); _ = press(core, 53)
            for key in "su3" { _ = core.handle(.char(key)) }
            _ = press(core, comma, shortcut)
            _ = press(core, 51)
            try expect(core.symbolPicker?.query.isEmpty == true, "⌫ did not clear the prefilled search")
            _ = press(core, 53)
            try expect(!core.isSymbolPickerOpen && client.inserts.isEmpty && !core.composer.isComposing, "Esc restored or sent the composition")
        }

        test("symbols: recent-symbols.txt keeps the newest first, once each, and is removed by clear") {
            let recent = RecentSymbols(url: directory.appendingPathComponent("recent").appendingPathComponent(RecentSymbols.fileName))
            for symbol in ["😀", "，", "😀"] { recent.push(symbol, source: source) }
            try expect(recent.items == ["😀", "，"], "the list is \(recent.items)")
            let text = (try? String(contentsOf: recent.url, encoding: .utf8)) ?? ""
            try expect(text == "😀\n，\n", "the file is \(text.debugDescription)")
            for index in 0..<40 { recent.push("\(index)", source: source) }
            try expect(recent.items.count == 32 && recent.items.first == "39", "the list is not capped at 32: \(recent.items.count)")
            recent.clear()
            try expect(!FileManager.default.fileExists(atPath: recent.url.path) && recent.items.isEmpty, "clear left the file")
        }

        #if ZT_SYMBOLS
        test("symbols: the engine's categories, Zhuyin search, display and recent list") {
            let engineSource = EngineSymbolSource()
            let ids = engineSource.categories.map(\.id)
            try expect(ids.first == "punctuation" && ids.contains("smileys") && ids.contains("flags") && !ids.contains("recent"),
                       "the categories are \(ids)")
            try expect(engineSource.displayQuery("vul") == "ㄒㄧㄠ" && engineSource.displayQuery("heart") == "heart",
                       "vul is shown as \(engineSource.displayQuery("vul"))")
            try expect(!engineSource.search("vul", limit: 20).isEmpty && !engineSource.search("heart", limit: 20).isEmpty,
                       "ㄒㄧㄠ or heart found nothing")
            try expect(engineSource.recentPush(stored: "", symbol: "😂") == "😂\n" || engineSource.recentPush(stored: "", symbol: "😂") == "😂",
                       "recent push: \(engineSource.recentPush(stored: "", symbol: "😂").debugDescription)")
            try expect(engineSource.name(of: "😂")?.isEmpty == false, "😂 has no name")
        }
        #endif

        test("symbols: the picker's window never becomes key and lays out a numbered row") {
            let picker = SymbolPicker(source: source, recent: [])
            let view = SymbolPickerView(frame: .zero)
            let size = view.update(picker)
            try expect(size.width > 300 && size.height > 250, "the picker is \(size)")
            let surface = InputSurface(content: view)
            let panel = InputPanel(content: surface)
            try expect(!panel.canBecomeKey && !panel.canBecomeMain && panel.styleMask.contains(.nonactivatingPanel),
                       "the picker's window can take the keyboard")
            try expect(view.accessibilityChildren()?.count ?? 0 > picker.tabs.count, "the items have no accessibility elements")
        }
    }
}
