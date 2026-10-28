import AppKit
import SwiftUI
import ZhuyinEngine

private final class FakeTextClient: TextClient {
    var bundleIdentifier = "org.zhuyintype.self-test"
    var markedRange = NSRange(location: NSNotFound, length: 0)
    var selectedRange = NSRange(location: 0, length: 0)
    var inserts: [String] = []
    var marks: [NSAttributedString] = []
    var selections: [NSRange] = []
    var operations: [String] = []

    func insertText(_ text: String, replacementRange: NSRange) {
        inserts.append(text)
        operations.append("insert")
        markedRange = NSRange(location: NSNotFound, length: 0)
    }

    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) {
        marks.append(NSAttributedString(attributedString: text))
        selections.append(selectionRange)
        operations.append("mark")
        markedRange = text.length == 0 ? NSRange(location: NSNotFound, length: 0)
            : NSRange(location: 0, length: text.length)
    }

    func caretRect(cursorIndex: Int) -> NSRect { NSRect(x: 100, y: 100, width: 1, height: 20) }
    var hasInsertionPoint = true
    /// The document before the insertion point, or nil for an app that does not say.
    var documentText: String?
    func textBeforeInsertion(maxLength: Int) -> String? { documentText.map { String($0.suffix(maxLength)) } }
}

enum ControllerCoreSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(lexiconURL: URL) -> Int32 {
        let focus = Headless.FocusGuard()
        let engine: Engine
        do {
            engine = try Engine(lexiconURL: lexiconURL, userDataDirectory: nil)
        } catch {
            print("FAIL load lexicon: \(error)")
            return 1
        }
        var failures = 0
        var cases = 0
        func test(_ name: String, _ check: () throws -> Void) {
            cases += 1
            do { try check(); print("PASS \(name)") }
            catch { failures += 1; print("FAIL \(name): \(error)") }
        }
        func expect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
            if !condition() { throw Failure(description: message) }
        }
        func session(english: Bool = false) -> (ControllerCore, FakeTextClient) {
            let client = FakeTextClient()
            var options = ComposerOptions()
            options.learningEnabled = false
            let core = ControllerCore(engine: engine, client: client, options: options, isEnglish: english)
            // The system's secure input (a password field or prompt anywhere, e.g. SecurityAgent) would pause reading,
            // learning and recording in every test; the secure-input test turns it on itself.
            core.secureInput = { false }
            return (core, client)
        }
        func type(_ keys: String, into core: ControllerCore) {
            for key in keys { _ = core.handle(key == " " ? .space : .char(key)) }
        }
        func shift(_ core: ControllerCore, _ key: UInt16 = 56, _ start: TimeInterval = 1) {
            _ = core.flagsChanged(keyCode: key, modifiers: .shift, physicallyDown: true, timestamp: start)
            _ = core.flagsChanged(keyCode: key, modifiers: [], physicallyDown: false, timestamp: start + 0.1)
        }

        test("secure input: nothing is read, remembered or learned") {
            let (core, _) = session()
            core.secureInput = { true }
            type("su3cl3", into: core)
            try expect(core.composer.learningSuspended, "learning was not paused under secure input")
            _ = core.keyDown(keyCode: 36, modifiers: [])
            try expect(core.recentText.isEmpty, "secure input left text as context")
            core.secureInput = { false }
            type("su3", into: core)
            try expect(!core.composer.learningSuspended, "learning stayed paused after secure input ended")
        }

        test("normal typing and Enter commit") {
            let (core, client) = session()
            type("su3cl3", into: core)
            let visible = core.composer.presentation.text
            try expect(!visible.isEmpty && !client.marks.isEmpty, "typing did not produce inline text")
            try expect(core.keyDown(keyCode: 36, modifiers: []), "Enter was not handled")
            try expect(client.inserts == [visible], "Enter did not preserve the visible text")
            try expect(!core.composer.isComposing, "Enter left a composition")
            try expect(client.operations.last == "insert", "unnecessary empty mark after commit")
        }
        test("identical marked text is suppressed") {
            let (core, client) = session()
            type("su", into: core)
            let count = client.marks.count
            core.render()
            core.render()
            try expect(count > 0 && client.marks.count == count, "identical text was resent")
            _ = core.handle(.left)
            try expect(client.selections.last?.location == core.composer.presentation.cursor, "cursor was not rendered")
        }
        test("empty composition clears marked text exactly once") {
            let (core, client) = session()
            type("su", into: core)
            _ = core.handle(.escape)
            try expect(client.marks.last?.string == "", "Escape did not clear the client's marked text")
            let count = client.marks.count
            core.render()
            try expect(client.marks.count == count && client.inserts.isEmpty, "clear repeated or committed text")
        }
        test("Shift tap commits composition before switching mode") {
            let (core, client) = session()
            type("su3", into: core)
            let visible = core.composer.presentation.text
            var callbackSawCommit = false
            core.modeChanged = { english, _ in callbackSawCommit = english && client.inserts == [visible] }
            shift(core)
            try expect(core.isEnglish && callbackSawCommit, "mode changed without preserving typed text")
            try expect(!core.keyDown(keyCode: 0, modifiers: []) && client.inserts == [visible], "English key consumed")
        }
        test("switching to another input source forgets English; coming back starts in Chinese") {
            let saved = EngineHost.lastEnglishMode
            defer { EngineHost.lastEnglishMode = saved }
            EngineHost.lastEnglishMode = true
            EngineHost.inputSourceChanged(toOurs: true)
            try expect(EngineHost.lastEnglishMode == true, "moving between fields lost the mode")
            EngineHost.inputSourceChanged(toOurs: false)
            try expect(EngineHost.lastEnglishMode == nil, "another input source kept English")
        }
        test("Shift outside a text field neither switches the mode nor shows the badge") {
            let (core, client) = session()
            client.hasInsertionPoint = false
            var toggles = 0
            core.modeChanged = { _, _ in toggles += 1 }
            shift(core)
            _ = core.flagsChanged(keyCode: 57, modifiers: .capsLock, physicallyDown: true, timestamp: 2)
            try expect(!core.isEnglish && toggles == 0, "mode switched or badge shown without a text field")
            client.hasInsertionPoint = true
            shift(core)
            try expect(core.isEnglish && toggles == 1, "Shift no longer switches in a text field")
        }
        test("Chromium duplicate and phantom Shift flags") {
            let (core, _) = session()
            var toggles = 0
            core.modeChanged = { _, _ in toggles += 1 }
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: false, timestamp: 0.5)
            _ = core.flagsChanged(keyCode: 56, modifiers: [], physicallyDown: false, timestamp: 0.6)
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: true, timestamp: 1)
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: true, timestamp: 1.05)
            _ = core.flagsChanged(keyCode: 56, modifiers: [], physicallyDown: false, timestamp: 1.1)
            _ = core.flagsChanged(keyCode: 56, modifiers: [], physicallyDown: false, timestamp: 1.15)
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: false, timestamp: 1.2)
            _ = core.flagsChanged(keyCode: 56, modifiers: [], physicallyDown: false, timestamp: 1.25)
            try expect(core.isEnglish && toggles == 1, "duplicate flags caused additional mode switches")
        }
        test("Shift tap side, timing, and intervening key") {
            let (core, _) = session(english: true)
            core.shiftToggle = .right
            shift(core, 56)
            try expect(core.isEnglish, "left Shift toggled in right-only mode")
            shift(core, 60, 2)
            try expect(!core.isEnglish, "right Shift did not toggle")
            _ = core.flagsChanged(keyCode: 60, modifiers: .shift, physicallyDown: true, timestamp: 3)
            _ = core.flagsChanged(keyCode: 60, modifiers: [], physicallyDown: false, timestamp: 3.5)
            try expect(!core.isEnglish, "long Shift hold toggled")
            _ = core.flagsChanged(keyCode: 60, modifiers: .shift, physicallyDown: true, timestamp: 4)
            _ = core.keyDown(keyCode: 0, modifiers: .shift)
            _ = core.flagsChanged(keyCode: 60, modifiers: [], physicallyDown: false, timestamp: 4.1)
            try expect(!core.isEnglish, "Shift chord toggled")
            core.shiftToggle = .none
            shift(core, 60, 5)
            try expect(!core.isEnglish, "disabled Shift toggled")
        }
        test("overlapping Shift presses do not poison the next tap") {
            let (core, _) = session()
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: true, timestamp: 1)
            _ = core.flagsChanged(keyCode: 60, modifiers: .shift, physicallyDown: true, timestamp: 1.02)
            _ = core.flagsChanged(keyCode: 56, modifiers: .shift, physicallyDown: false, timestamp: 1.05)
            _ = core.flagsChanged(keyCode: 60, modifiers: [], physicallyDown: false, timestamp: 1.1)
            try expect(!core.isEnglish, "overlapping Shift keys toggled")
            shift(core, 56, 2)
            try expect(core.isEnglish, "overlapping Shift keys left stale held state")
        }
        test("Command shortcut commits then passes through") {
            let (core, client) = session()
            type("su3", into: core)
            let visible = core.composer.presentation.text
            try expect(!core.keyDown(keyCode: 8, modifiers: .command), "Command+C was consumed")
            try expect(client.inserts == [visible] && !core.composer.isComposing, "shortcut lost composition")
        }
        test("Control, Option, Caps Lock letters, and keypad pass through after commit") {
            for (key, modifiers): (UInt16, NSEvent.ModifierFlags) in [
                (0, .control), (0, .option), (0, .capsLock), (83, []),
            ] {
                let (core, client) = session()
                type("su3", into: core)
                let visible = core.composer.presentation.text
                try expect(!core.keyDown(keyCode: key, modifiers: modifiers), "key \(key) was consumed")
                try expect(client.inserts == [visible], "pass-through key \(key): visible=\(visible), inserted=\(client.inserts)")
            }
        }
        test("a composition continues the app's text before the insertion point") {
            let (fresh, _) = session()
            type("g4", into: fresh)
            let alone = fresh.composer.presentation.text
            let (core, client) = session()
            client.documentText = "我們"
            type("g4", into: core)
            try expect(core.composer.presentation.text == "是" && alone != "是",
                       "after 我們: \(core.composer.presentation.text), alone: \(alone)")
            try expect(core.recentText == "我們", "reranker context \(core.recentText)")
        }
        test("Shift-Up switches the corrections list; Option combinations stay the app's") {
            let (core, client) = session()
            type("ji3ap7", into: core)
            try expect(core.keyDown(keyCode: 125, modifiers: []), "↓ was not handled")
            var tolerant: Bool? { core.composer.presentation.candidates?.tolerant }
            try expect(tolerant == false, "list does not start exact")
            try expect(core.keyDown(keyCode: 126, modifiers: .shift), "⇧↑ was not handled")
            try expect(tolerant == true && core.composer.presentation.candidates?.items.dropFirst().first?.annotation != nil,
                       "⇧↑ did not list corrections first")
            try expect(core.keyDown(keyCode: 125, modifiers: []) && core.composer.presentation.candidates?.selection == 1,
                       "plain ↓ did not move in the corrections list")
            try expect(core.keyDown(keyCode: 126, modifiers: .shift) && tolerant == false, "⇧↑ again did not restore the list")
            _ = core.flagsChanged(keyCode: 58, modifiers: .option, physicallyDown: true, timestamp: 1)
            _ = core.flagsChanged(keyCode: 58, modifiers: [], physicallyDown: false, timestamp: 1.1)
            try expect(tolerant == false, "Option alone changed the list")
            let visible = core.composer.presentation.text
            try expect(!core.keyDown(keyCode: 126, modifiers: [.option, .control]), "⌃⌥↑ was taken from the app")
            try expect(client.inserts.last == visible, "⌃⌥↑ did not commit before passing: \(client.inserts)")
            // ⇧↑ with no list open opens the corrections list directly.
            type("ji3ap7", into: core)
            try expect(core.keyDown(keyCode: 126, modifiers: .shift) && tolerant == true, "⇧↑ did not open the corrections list")
        }
        test("a rebound shortcut runs its command; the old default key goes to the app") {
            let (core, client) = session()
            core.shortcuts = [Shortcut("s", control: true): .singleCharacterList]
            type("ji3ap7", into: core)
            _ = core.handle(.home)
            try expect(core.keyDown(keyCode: 1, modifiers: .control), "⌃S was not handled")
            try expect(core.composer.presentation.candidates?.singleCharacter == true, "⌃S did not open the single list")
            let visible = core.composer.presentation.text
            try expect(!core.keyDown(keyCode: 125, modifiers: .shift), "unbound ⇧↓ was taken from the app")
            try expect(client.inserts.last == visible, "unbound ⇧↓ did not commit first")
            try expect(Shortcut("↓").reservedReason != nil && Shortcut("a").reservedReason != nil
                       && Shortcut("1", shift: true).reservedReason != nil && Shortcut("s", command: true).reservedReason != nil
                       && Shortcut("s", control: true).reservedReason == nil && Shortcut("`").reservedReason == nil,
                       "reserved keys")
        }
        test("the character ↓ would change is underlined once the cursor moves into the composition") {
            let (core, client) = session()
            type("ji3ap7g4", into: core)
            _ = core.handle(.left)
            guard let mark = client.marks.last, let target = core.composer.presentation.targetClause else {
                throw Failure(description: "no target after ←")
            }
            let range = core.composer.presentation.clauses[target]
            let attributes = mark.attributes(at: range.lowerBound, effectiveRange: nil)
            try expect(mark.attributedSubstring(from: NSRange(location: range.lowerBound, length: range.count)).string == "是",
                       "target is not the character right of the cursor")
            try expect(attributes[.underlineStyle] as? Int == NSUnderlineStyle.thick.rawValue
                       && attributes[.underlineColor] != nil, "target is not marked with the accent underline")
            _ = core.handle(.end)
            try expect(core.composer.presentation.targetClause == nil, "target still marked at the end")
        }
        test("Shift-Down opens the single-character list") {
            try expect(KeyMapping.input(keyCode: 125, modifiers: .shift) == .shiftDown, "Shift-↓ mapping")
            try expect(KeyMapping.input(keyCode: 125, modifiers: []) == .down, "↓ mapping")
        }
        test("Option types the symbol printed on a key into the composition") {
            let (core, client) = session()
            type("su3", into: core)
            try expect(core.keyDown(keyCode: 47, modifiers: .option), "Option-. was not consumed")
            try expect(core.keyDown(keyCode: 47, modifiers: [.option, .shift]), "Option-Shift-. was not consumed")
            core.commit()
            try expect(client.inserts == ["你.>"], "inserted \(client.inserts)")
        }
        test("the 中/英 badge keeps its size on the input surface") {
            let badge = ModeBadgeView(frame: .zero)
            let surface = InputSurface(content: badge)
            badge.setFrameSize(ModeBadgeView.size)
            surface.fitContent()
            try expect(surface.frame.size == ModeBadgeView.size && badge.frame.size == ModeBadgeView.size,
                       "badge \(badge.frame.size), surface \(surface.frame.size)")
        }
        test("Windows keys: Control types Chinese punctuation, Shift the symbol on the key") {
            let (core, client) = session()
            type("su3", into: core)
            try expect(core.keyDown(keyCode: 43, modifiers: .control), "Control-, was not consumed")
            try expect(core.keyDown(keyCode: 44, modifiers: [.control, .shift]), "Control-Shift-/ was not consumed")
            try expect(core.keyDown(keyCode: 27, modifiers: .shift), "Shift-- was not consumed")
            try expect(!core.keyDown(keyCode: 33, modifiers: .control), "Control-[ was taken from the app")
            try expect(client.inserts == ["你，？_"], "inserted \(client.inserts)")
            try expect(KeyMapping.input(keyCode: 18, modifiers: .control) == nil, "Control-1 is the app's")
        }
        test("popup app receives preview without marked text") {
            let (core, client) = session()
            core.popupCompositionApps = [client.bundleIdentifier]
            var preview: String?
            var list: CandidateList?
            var hidden = false
            core.showPanel = { candidates, text, _, _, _ in list = candidates; preview = text }
            core.hidePanel = { hidden = true }
            type("su", into: core)
            try expect(client.marks.isEmpty && preview == core.composer.presentation.text && list == nil,
                       "popup composed inline or did not send a preview without candidates")
            type("3", into: core)
            let visible = core.composer.presentation.text
            _ = core.handle(.enter)
            try expect(client.inserts == [visible] && hidden && client.marks.isEmpty,
                       "popup commit: visible=\(visible), inserted=\(client.inserts), hidden=\(hidden)")
        }
        test("Backspace removes one unfinished symbol at a time") {
            let (core, client) = session()
            type("su", into: core)
            let before = core.composer.presentation.text.count
            try expect(before == 2, "expected two unfinished Zhuyin symbols")
            _ = core.keyDown(keyCode: 51, modifiers: [])
            try expect(core.composer.presentation.text.count == before - 1, "Backspace erased multiple symbols")
            _ = core.keyDown(keyCode: 51, modifiers: [])
            try expect(core.composer.presentation.text.isEmpty && client.marks.last?.string == "", "last symbol not cleared")
        }
        test("Backspace removes one converted character at a time") {
            let (core, _) = session()
            type("su3cl3", into: core)
            let before = core.composer.presentation.text.count
            try expect(before >= 2, "expected a converted phrase")
            _ = core.handle(.backspace)
            try expect(core.composer.presentation.text.count == before - 1, "Backspace erased multiple characters")
        }
        test("clauses and focused candidate underline") {
            let (core, client) = session()
            type("su3", into: core)
            _ = core.handle(.down)
            let p = core.composer.presentation
            try expect(p.candidates != nil && p.focusedClause != nil, "candidate window did not open")
            guard let mark = client.marks.last else { throw Failure(description: "missing marked text") }
            for (index, range) in p.clauses.enumerated() where !range.isEmpty {
                let attributes = mark.attributes(at: range.lowerBound, effectiveRange: nil)
                try expect(attributes[.markedClauseSegment] as? Int == index, "clause index missing")
                let style = (p.focusedClause == index ? NSUnderlineStyle.thick : .single).rawValue
                try expect(attributes[.underlineStyle] as? Int == style, "incorrect focused underline")
            }
        }
        test("candidate click validates index and renders") {
            let (core, client) = session()
            type("su3", into: core)
            _ = core.handle(.down)
            let before = core.composer.presentation
            core.selectCandidate(at: -1)
            core.selectCandidate(at: Int.max)
            try expect(core.composer.presentation == before, "invalid candidate changed composition")
            guard let first = before.candidates?.items.first else { throw Failure(description: "no candidates") }
            core.selectCandidate(at: 0)
            try expect(core.composer.presentation.text.contains(first.text) || client.inserts.contains(first.text),
                       "candidate selection did not reach client")
        }
        test("focus loss commits exactly once") {
            let (core, client) = session()
            type("su3", into: core)
            let visible = core.composer.presentation.text
            core.endSession()
            core.endSession()
            try expect(client.inserts == [visible], "deactivation: visible=\(visible), inserted=\(client.inserts)")
        }
        test("unfinished suffix survives Enter, shortcuts, and focus loss") {
            for action in 0..<3 {
                let (core, client) = session()
                type("su3s", into: core)
                let visible = core.composer.presentation.text
                try expect(visible.hasSuffix("ㄋ"), "missing unfinished suffix")
                switch action {
                case 0: _ = core.handle(.enter)
                case 1: _ = core.keyDown(keyCode: 8, modifiers: .command)
                default: core.endSession()
                }
                try expect(client.inserts == [visible], "unfinished input was discarded: \(client.inserts)")
                try expect(!core.composer.isComposing, "unfinished suffix left a stale composition")
            }
        }
        test("hardware key mapping") {
            for (code, char) in KeyMapping.printable {
                try expect(KeyMapping.input(keyCode: code, modifiers: []) == .char(char), "unshifted \(code)")
                try expect(KeyMapping.input(keyCode: code, modifiers: .shift) == .shiftedChar(char), "shifted \(code)")
            }
            let special: [(UInt16, KeyInput)] = [
                (36, .enter), (76, .enter), (49, .space), (51, .backspace), (117, .forwardDelete),
                (53, .escape), (48, .tab), (123, .left), (124, .right), (125, .down), (126, .up),
                (115, .home), (119, .end), (116, .pageUp), (121, .pageDown),
            ]
            for (code, key) in special {
                try expect(KeyMapping.input(keyCode: code, modifiers: []) == key, "special key \(code)")
            }
            try expect(KeyMapping.input(keyCode: 51, modifiers: .option) == .optionBackspace, "Option+Delete")
            try expect(KeyMapping.input(keyCode: 49, modifiers: .shift) == .shiftSpace, "Shift+Space")
            try expect(KeyMapping.input(keyCode: 36, modifiers: .shift) == .shiftEnter, "Shift+Return")
            try expect(KeyMapping.input(keyCode: 48, modifiers: .shift) == .shiftTab, "Shift+Tab")
            try expect(KeyMapping.input(keyCode: 123, modifiers: .shift) == .shiftLeft, "Shift+Left")
            try expect(KeyMapping.input(keyCode: 124, modifiers: .shift) == .shiftRight, "Shift+Right")
            for code: UInt16 in [82, 83, 84, 85, 86, 87, 88, 89, 91, 92] {
                try expect(KeyMapping.input(keyCode: code, modifiers: []) == nil, "keypad \(code)")
            }
        }

        test("Shift selection shows notice-only panel and never commits") {
            let (core, client) = session()
            var notice: String?
            var candidates: CandidateList?
            core.showPanel = { list, _, _, message, _ in candidates = list; notice = message }
            type("su3cl3", into: core)
            _ = core.keyDown(keyCode: 123, modifiers: .shift)
            try expect(candidates == nil && notice == "Enter 加入自訂詞：好", "missing selection notice")
            try expect(client.selections.last == NSRange(location: 1, length: 1), "client selection missed focus")
            _ = core.handle(.enter)
            try expect(client.inserts.isEmpty && core.composer.isComposing && notice == "已加入自訂詞：好", "phrase save committed")
            let panel = CandidatePanelController.shared
            panel.show(list: nil, preview: nil, caret: client.caretRect(cursorIndex: 1), keys: [],
                       windowLevel: .popUpMenu, notice: notice)
            try expect(panel.isVisible, "notice-only panel hidden")
            panel.hide()
        }
        test("Zhuyin footer click reaches composer") {
            let (core, client) = session()
            type("su3cl3", into: core)
            _ = core.handle(.down)
            let hint = core.composer.presentation.zhuyinHint
            let content = CandidateContentView(frame: .zero)
            let panel = CandidatePanelController.shared
            panel.onZhuyin = { core.selectZhuyin() }
            content.onZhuyin = { panel.onZhuyin?() }
            _ = content.update(list: core.composer.presentation.candidates, preview: nil, keys: [],
                               style: CandidateStyle(), zhuyinHint: hint)
            guard let button = content.subviews.compactMap({ $0 as? NSButton }).first(where: {
                $0.accessibilityLabel()?.hasPrefix("`，") == true
            }) else { throw Failure(description: "missing hint button") }
            button.performClick(nil)
            try expect(core.composer.presentation.text == hint && core.composer.presentation.candidates == nil,
                       "footer click missed highlighted span")
            try expect(client.inserts.isEmpty, "footer click committed")
            panel.onZhuyin = nil
        }

        // MARK: Correction log

        final class Records { var all: [[String: Any]] = [] }
        func recording(_ core: ControllerCore, context: Bool = false) -> Records {
            let records = Records()
            let recorder = CorrectionRecorder { record in
                guard let data = CorrectionLog.encode(record),
                      let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
                records.all.append(object)
            }
            recorder.recordsContext = context
            core.recorder = recorder
            return records
        }
        var clock = 100.0
        /// Keys in zhuyin-cli syntax (characters, <bs>, <esc>, <down>, <enter>), 0.15 s apart.
        func press(_ keys: String, into core: ControllerCore) {
            var index = keys.startIndex
            while index < keys.endIndex {
                let key: KeyInput
                if keys[index] == "<", let end = keys[index...].firstIndex(of: ">") {
                    let name = keys[keys.index(after: index)..<end]
                    key = ["bs": .backspace, "esc": .escape, "down": .down, "enter": .enter, "space": .space][String(name)]!
                    index = keys.index(after: end)
                } else {
                    key = .char(keys[index])
                    index = keys.index(after: index)
                }
                clock += 0.15
                _ = core.handle(key, time: clock)
            }
        }
        func plain(_ value: Any?) -> String {
            (value as? String ?? "").replacingOccurrences(of: "<dt:[0-9]+>", with: "", options: .regularExpression)
        }
        func events(_ record: [String: Any], _ type: String) -> [[String: Any]] {
            (record["events"] as? [[String: Any]] ?? []).filter { $0["type"] as? String == type }
        }

        test("correction log: nothing without a correction, nothing once turned off") {
            let (core, client) = session()
            let records = recording(core)
            press("su3cl3<enter>", into: core)
            try expect(records.all.isEmpty && client.inserts.count == 1, "a composition without corrections was recorded")
            core.recorder = nil
            press("su3cl3<down>", into: core)
            core.selectCandidate(at: (core.composer.presentation.candidates?.items.count ?? 1) - 1)
            press("<enter>", into: core)
            try expect(records.all.isEmpty, "recorded with the recorder removed")
        }
        test("correction log: a pick gives one record with before, after and keys") {
            let (core, client) = session()
            let records = recording(core)
            press("su3cl3<down>", into: core)
            guard let list = core.composer.presentation.candidates,
                  let index = list.items.indices.first(where: { !"你好".hasSuffix(list.items[$0].text) }),
                  index < 9 else { throw Failure(description: "no other candidate for 你好") }
            press(String(index + 1), into: core)
            let picked = core.composer.presentation.text
            press("<enter>", into: core)
            guard records.all.count == 1, let record = records.all.first else {
                throw Failure(description: "\(records.all.count) records")
            }
            let pick = events(record, "pick").first ?? [:]
            try expect(record["corrections"] as? [String] == ["pick"], "corrections \(record["corrections"] ?? "")")
            try expect(pick["before"] as? String == "你好" && pick["after"] as? String == picked
                       && pick["rank"] as? Int == index + 1 && pick["list"] as? String == "normal"
                       && pick["via"] as? String == "key", "pick \(pick)")
            try expect(record["committed"] as? String == picked && client.inserts == [picked], "committed")
            try expect(plain(record["typing"]) == "su3cl3" && record["replayable"] as? Bool == true, "typing \(record)")
            try expect(plain(record["keys"]) == "su3cl3<down>\(index + 1)<enter>"
                       && (record["keys"] as? String ?? "").contains("<dt:150>"), "keys \(record["keys"] ?? "")")
            try expect(record["shown"] as? String == "你好" && record["schema"] as? Int == 1
                       && record["context"] == nil && record["original_typing"] != nil, "record \(record)")
            try expect(events(record, "open_list").first?["list"] as? String == "normal", "open_list")
        }
        test("correction log: Backspace and retyping keep the deleted and retyped keys") {
            let (core, _) = session()
            let records = recording(core)
            press("su31j6<bs>1j4<enter>", into: core)
            guard let record = records.all.first, let deletion = events(record, "backspace").first else {
                throw Failure(description: "no record: \(records.all)")
            }
            try expect(record["corrections"] as? [String] == ["backspace"], "corrections \(record["corrections"] ?? "")")
            try expect(deletion["deleted_keys"] as? String == "1j6" && deletion["retyped_keys"] as? String == "1j4",
                       "deletion \(deletion)")
            try expect(plain(record["typing"]) == "su31j4" && plain(record["original_typing"]) == "su31j6"
                       && record["original_replayable"] as? Bool == true, "typing \(record)")
        }
        test("correction log: Esc that clears the composition, then typing again") {
            let (core, _) = session()
            let records = recording(core)
            press("su3<esc>su4<enter>", into: core)
            guard let record = records.all.first, let escape = events(record, "escape").first else {
                throw Failure(description: "no record: \(records.all)")
            }
            try expect(escape["cleared"] as? String == "composition" && escape["deleted_keys"] as? String == "su3"
                       && escape["retyped_keys"] as? String == "su4", "escape \(escape)")
            try expect(plain(record["typing"]) == "su4" && plain(record["original_typing"]) == "su3", "typing \(record)")
        }
        test("correction log: deleting the commit within 30 seconds and typing again") {
            let (core, _) = session()
            let records = recording(core)
            press("su3<enter>", into: core)
            clock += 1
            _ = core.handle(.backspace, time: clock)
            press("su4<enter>", into: core)
            guard let record = records.all.first, let deletion = events(record, "delete_after_commit").first else {
                throw Failure(description: "no record: \(records.all)")
            }
            try expect(deletion["deleted"] as? String == "你" && plain(deletion["previous_typing"]) == "su3"
                       && record["corrections"] as? [String] == ["delete_after_commit"], "record \(record)")
        }
        test("correction log: nothing under secure input") {
            let (core, _) = session()
            let records = recording(core)
            core.secureInput = { true }
            press("su31j6<bs>1j4<enter>", into: core)
            press("su3cl3<down>", into: core)
            core.selectCandidate(at: (core.composer.presentation.candidates?.items.count ?? 1) - 1)
            press("<enter>", into: core)
            try expect(records.all.isEmpty, "recorded under secure input")
        }
        test("correction log: the text before the cursor only with its option") {
            for option in [false, true] {
                let (core, client) = session()
                client.documentText = "我們"
                let records = recording(core, context: option)
                press("su31j6<bs>1j4<enter>", into: core)
                let context = records.all.first?["context"] as? String
                try expect(records.all.count == 1 && context == (option ? "我們" : nil),
                           "option \(option): context \(context ?? "nil")")
            }
        }
        test("correction log: literal Zhuyin, selections and custom phrases are corrections") {
            let (core, _) = session()
            let records = recording(core)
            press("su3cl3`<enter>", into: core)
            press("su3cl3", into: core)
            _ = core.handle(.shiftLeft, time: clock + 0.1)
            _ = core.handle(.enter, time: clock + 0.2)
            clock += 0.2
            press("<enter>", into: core)
            try expect(records.all.map { $0["corrections"] as? [String] ?? [] } == [["literal_zhuyin"], ["select", "add_phrase"]],
                       "corrections \(records.all.map { $0["corrections"] ?? "" })")
            try expect(events(records.all[1], "add_phrase").first?["phrase"] as? String == "好", "phrase")
        }
        test("correction log: ` and its undo (` again or Esc)") {
            for undo in ["`", "<esc>"] {
                let (core, client) = session()
                let records = recording(core)
                press("su3cl3`" + undo + "<enter>", into: core)
                let undone = events(records.all.first ?? [:], "undo_literal").first
                try expect(records.all.first?["corrections"] as? [String] == ["literal_zhuyin", "undo_literal"]
                           && undone?["shown"] as? String == "你好" && client.inserts == ["你好"],
                           "undo \(undo): \(records.all) inserted \(client.inserts)")
                try expect(plain(records.all.first?["typing"]) == "su3cl3`" + undo, "typing \(undo)")
            }
        }

        // MARK: Long compositions

        func han(_ text: String) -> Int {
            text.unicodeScalars.filter { (0x4E00...0x9FFF).contains($0.value) }.count
        }
        test("a full composition sends its head and keeps every key, the list open or not") {
            let (core, client) = session()
            var syllables = 0
            var heads = 0
            for round in 0..<60 {
                // From the 20th round on, every fifth has the list open when its first key comes.
                if round >= 20, round % 5 == 0 {
                    _ = core.handle(.down)
                    try expect(core.composer.presentation.candidates != nil, "round \(round): the list did not open")
                }
                let visible = core.composer.presentation.text
                let inserted = client.inserts.count
                let operations = client.operations.count
                type("su3cl3", into: core)
                syllables += 2
                if client.inserts.count > inserted {
                    heads += 1
                    let head = client.inserts[inserted]
                    try expect(client.inserts.count == inserted + 1, "round \(round): more than one commit")
                    try expect(visible.hasPrefix(head) && han(head) >= 18 && han(head) <= 20,
                               "round \(round): the head \(head) is not about a third of what was shown: \(visible)")
                    try expect(core.composer.isComposing && client.operations[operations...].first == "insert"
                               && client.operations.last == "mark", "round \(round): the rest is not marked after the head")
                    try expect(core.recentText.hasSuffix(head), "round \(round): the head is not the text before the rest")
                }
                try expect(han(core.composer.presentation.text) <= 60, "round \(round): past the limit")
            }
            _ = core.keyDown(keyCode: 36, modifiers: [])
            let text = client.inserts.joined()
            try expect(han(text) == syllables && text.count == syllables,
                       "\(han(text)) of \(syllables) syllables arrived: \(text)")
            try expect(heads == 3, "\(heads) heads for \(syllables) syllables (at 61, 81 and 101)")
        }
        test("correction log: a pick, then a full composition sends its head: one record of all of it") {
            let (core, client) = session()
            let records = recording(core)
            press("su3cl3<down>", into: core)
            guard let list = core.composer.presentation.candidates,
                  let index = list.items.indices.first(where: { !"你好".hasSuffix(list.items[$0].text) }),
                  index < 9 else { throw Failure(description: "no other candidate for 你好") }
            press(String(index + 1), into: core)
            let picked = core.composer.presentation.text
            press(String(repeating: "su3cl3", count: 35), into: core)
            press("<enter>", into: core)
            guard records.all.count == 1, let record = records.all.first else {
                throw Failure(description: "\(records.all.count) records")
            }
            let all = client.inserts.joined()
            try expect(client.inserts.count >= 2 && client.inserts[0].hasPrefix(picked),
                       "the head did not keep the pick: \(client.inserts)")
            try expect(record["committed"] as? String == all && han(all) == 72, "committed \(record["committed"] ?? "")")
            try expect(events(record, "commit").map { $0["text"] as? String ?? "" } == client.inserts,
                       "commit events \(events(record, "commit"))")
            try expect(plain(record["typing"]) == String(repeating: "su3cl3", count: 36)
                       && record["replayable"] as? Bool == true, "typing \(record["typing"] ?? "")")
            try expect((record["shown"] as? String)?.hasPrefix("你好") == true, "shown \(record["shown"] ?? "")")
        }
        test("secure input: a full composition still sends every key, and nothing is kept or recorded") {
            let (core, client) = session()
            let records = recording(core)
            core.secureInput = { true }
            press(String(repeating: "su3cl3", count: 40) + "<enter>", into: core)
            let text = client.inserts.joined()
            try expect(client.inserts.count == 2 && han(text) == 80, "\(han(text)) of 80 syllables arrived")
            try expect(core.recentText.isEmpty && core.composer.learningSuspended && records.all.isEmpty,
                       "secure input kept context, learned or recorded")
        }
        test("correction log file: appended, exported and cleared") {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypeTest-\(UUID().uuidString)")
            defer { try? FileManager.default.removeItem(at: directory) }
            let log = CorrectionLog(directory: directory)
            let (core, _) = session()
            let recorder = CorrectionRecorder { log.append($0) }
            core.recorder = recorder
            press("su31j6<bs>1j4<enter>su3<esc>su4<enter>", into: core)
            log.flush()
            let lines = (try? String(contentsOf: log.url, encoding: .utf8))?.split(separator: "\n") ?? []
            let attributes = try? FileManager.default.attributesOfItem(atPath: log.url.path)
            try expect(lines.count == 2 && lines.allSatisfy { $0.hasPrefix("{") }, "\(lines.count) lines")
            try expect((attributes?[.posixPermissions] as? Int) == 0o600, "file is readable by others")
            let copy = directory.appendingPathComponent("copy.jsonl")
            try log.export(to: copy)
            try expect((try? Data(contentsOf: copy)) == (try? Data(contentsOf: log.url)), "export differs")
            log.clear {}
            log.flush()
            try expect(!FileManager.default.fileExists(atPath: log.url.path), "clear left the file")
        }

        // MARK: Usage statistics

        func statsSession() -> (ControllerCore, FakeTextClient, UsageStatsStore) {
            let (core, client) = session()
            let store = UsageStatsStore(directory: nil)
            core.stats = UsageStatsObserver(store: store)
            return (core, client, store)
        }
        test("usage stats: keys, characters, compositions, picks and corrections") {
            let (core, client, store) = statsSession()
            press("su3cl3<enter>", into: core)
            press("su31j6<bs>1j4<enter>", into: core)
            press("su3cl3<down>", into: core)
            core.selectCandidate(at: (core.composer.presentation.candidates?.items.count ?? 1) - 1)
            press("<enter>", into: core)
            let totals = store.data.totals
            let sent = client.inserts.reduce(0) { $0 + $1.count }
            try expect(totals.compositions == 3 && totals.characters == sent, "compositions \(totals.compositions), characters \(totals.characters) of \(sent)")
            try expect(totals.keys >= 18 && totals.backspaces == 1 && totals.picksNormal == 1, "counters \(totals)")
            try expect(store.data.latency.total == totals.keys && store.data.latency.maximum > 0, "latency \(store.data.latency.total)")
            try expect(store.data.days.count == 1, "days \(store.data.days.keys)")
        }
        test("usage stats: the file holds numbers only, never text, readings or apps") {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypeStats-\(UUID().uuidString)")
            defer { try? FileManager.default.removeItem(at: directory) }
            let (core, client) = session()
            let store = UsageStatsStore(directory: directory)
            core.stats = UsageStatsObserver(store: store)
            press("su3cl3<enter>su3`<esc><enter>", into: core)
            store.flush()
            let file = (try? String(contentsOf: directory.appendingPathComponent(UsageStatsStore.fileName), encoding: .utf8)) ?? ""
            let attributes = try? FileManager.default.attributesOfItem(atPath: directory.appendingPathComponent(UsageStatsStore.fileName).path)
            try expect(!file.isEmpty && (attributes?[.posixPermissions] as? Int) == 0o600, "file missing or readable by others")
            let forbidden = client.inserts + ["ㄋ", "ㄧ", "ㄏ", client.bundleIdentifier, "su3"]
            try expect(!forbidden.contains { !$0.isEmpty && file.contains($0) }, "text in stats.json: \(file)")
            try expect(!file.unicodeScalars.contains { $0.value > 0x2E80 }, "CJK text in stats.json")
            try expect(store.data.totals.literalZhuyin == 1 && store.data.totals.undoLiteral == 1, "` and its undo \(store.data.totals)")
        }
        test("usage stats: nothing under secure input") {
            let (core, _, store) = statsSession()
            core.secureInput = { true }
            press("su31j6<bs>1j4<enter>", into: core)
            try expect(store.data.totals.isZero && store.data.latency.total == 0, "counted under secure input: \(store.data.totals)")
        }
        test("usage stats: clear starts over; off counts nothing") {
            let (core, _, store) = statsSession()
            press("su3<enter>", into: core)
            store.clear()
            try expect(store.data.totals.isZero && store.data.days.isEmpty && store.data.latency.total == 0, "clear left data")
            core.stats = nil
            press("su3<enter>", into: core)
            try expect(store.data.totals.isZero, "counted with statistics off")
        }

        /// A settings suite for one test: always the same name, emptied before and removed after.
        func scratchDefaults() -> UserDefaults {
            let suite = "org.zhuyintype.selftest"
            let defaults = UserDefaults(suiteName: suite)!
            defaults.removePersistentDomain(forName: suite)
            return defaults
        }
        defer {
            UserDefaults(suiteName: "org.zhuyintype.selftest")?.removePersistentDomain(forName: "org.zhuyintype.selftest")
            CFPreferencesAppSynchronize("org.zhuyintype.selftest" as CFString)
            try? FileManager.default.removeItem(at: FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Preferences/org.zhuyintype.selftest.plist"))
        }

        test("usage stats: typing time, thinking pauses, runs and first-time-right compositions") {
            let (core, _, store) = statsSession()
            let observer = core.stats!
            let threshold = observer.pauseThreshold
            var t = 1000.0
            func key(_ k: KeyInput, after gap: Double) { t += gap; _ = core.handle(k, time: t) }
            for k in "su3cl3" { key(.char(k), after: 0.2) }
            key(.enter, after: 0.2)
            key(.char("s"), after: threshold + 3)         // thinking
            for k in "u3" { key(.char(k), after: 0.2) }
            key(.backspace, after: 0.2)                    // a correction
            for k in "su3" { key(.char(k), after: 0.2) }
            key(.enter, after: 0.2)
            key(.char("s"), after: 120)                    // away: neither
            for k in "u3" { key(.char(k), after: 0.2) }
            key(.enter, after: 0.2)
            let c = store.data.totals
            try expect(c.compositions == 3 && c.cleanCompositions == 2, "first time right \(c.cleanCompositions)/\(c.compositions)")
            let expectedTyping = 0.2 * Double(6 + 7 + 3)
            try expect(abs(Double(c.typingMilliseconds) / 1000 - expectedTyping) < 0.05, "typing \(c.typingMilliseconds) ms")
            try expect(abs(Double(c.thinkingMilliseconds) / 1000 - (threshold + 3)) < 0.05, "thinking \(c.thinkingMilliseconds) ms")
            try expect(store.rhythm.longestRun >= 2 && store.rhythm.longestCleanRun >= 2, "runs \(store.rhythm.longestRun)")
            try expect(store.rhythm.hours.reduce(0, +) == c.characters, "hours")
        }
        test("usage stats: per-key cost stays under 2 µs, with no file write while typing") {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypeStats-\(UUID().uuidString)")
            defer { try? FileManager.default.removeItem(at: directory) }
            let store = UsageStatsStore(directory: directory)
            var uptime = 0.0
            store.uptime = { uptime }
            let observer = UsageStatsObserver(store: store)
            let before = Presentation(text: "你好", cursor: 2, clauses: [0..<2])
            let after = Presentation(text: "你好ㄇ", cursor: 3, clauses: [0..<2, 2..<3])
            let options = ComposerOptions()
            let count = 200_000
            var time = 0.0
            let start = DispatchTime.now().uptimeNanoseconds
            for _ in 0..<count {
                time += 0.15
                observer.observe(.key(.char("a"), time: time), before: before, after: after, commit: nil, handled: true,
                                 options: options, secure: false, corrected: false, latency: 1_500_000)
            }
            let perKey = Double(DispatchTime.now().uptimeNanoseconds - start) / Double(count)
            print("TIMING usage stats observer: \(String(format: "%.0f", perKey)) ns per key")
            try expect(perKey < 2000 || ProcessInfo.processInfo.environment["ZHUYIN_CHECKS_TIMING"] == "warn",
                       "\(perKey) ns per key")
            try expect(store.writes == 0, "wrote \(store.writes) times while typing")
            uptime += UsageStatsStore.saveInterval + 1
            observer.observe(.key(.char("a"), time: time + 0.1), before: before, after: after, commit: nil, handled: true,
                             options: options, secure: false, corrected: false, latency: nil)
            try expect(store.writes == 1, "no write after 10 minutes: \(store.writes)")
            store.saveIfNeeded()
            try expect(store.writes == 1, "wrote without a change")
            store.flush()
            try expect(FileManager.default.fileExists(atPath: directory.appendingPathComponent(UsageStatsStore.fileName).path),
                       "no file")
        }

        // MARK: Optional key features

        test("a feature turned off: its key goes to the app; on again restores it") {
            let (core, client) = session()
            let all = core.shortcuts
            type("su3", into: core)
            try expect(core.keyDown(keyCode: 50, modifiers: []), "` with typeZhuyin on")
            press("<enter>", into: core)
            core.shortcuts = all.filter { $0.value != .typeZhuyin }
            type("su3", into: core)
            client.inserts.removeAll()
            try expect(!core.keyDown(keyCode: 50, modifiers: []) && client.inserts.count == 1 && !core.composer.isComposing,
                       "` with typeZhuyin off: \(client.inserts)")
            core.shortcuts = all
            type("su3", into: core)
            try expect(core.keyDown(keyCode: 50, modifiers: []), "` after turning typeZhuyin on again")
            press("<enter>", into: core)
            for (feature, keyCode, modifiers) in [(KeyFeature.controlPunctuation, UInt16(43), NSEvent.ModifierFlags.control),
                                                  (.optionSymbols, 47, .option)] {
                core.disabledFeatures = []
                try expect(core.keyDown(keyCode: keyCode, modifiers: modifiers), "\(feature) on")
                press("<enter>", into: core)
                core.disabledFeatures = [feature]
                try expect(!core.keyDown(keyCode: keyCode, modifiers: modifiers), "\(feature) off still handled")
            }
            core.disabledFeatures = []
            try expect(!core.keyDown(keyCode: 1, modifiers: .capsLock), "Caps Lock types English")
            core.disabledFeatures = [.capsLockEnglish]
            try expect(core.keyDown(keyCode: 1, modifiers: .capsLock) && core.composer.isComposing,
                       "with Caps Lock English off, keys still type Zhuyin")
            press("<esc><esc>", into: core)
        }
        test("bindings: two both work, one removed, reset, migration and conflicts") {
            let defaults = scratchDefaults()
            // An earlier version's single shortcut is read as one binding.
            defaults.set(try? JSONEncoder().encode(["correctionsList": Shortcut("u", control: true)]), forKey: "shortcuts")
            let preferences = Preferences(defaults: defaults)
            try expect(preferences.bindings[.correctionsList] == [Shortcut("u", control: true)], "migration \(preferences.bindings[.correctionsList] ?? [])")
            try expect(preferences.bind(.correctionsList, at: 1, to: Shortcut("↑", shift: true)) == nil, "second binding refused")
            try expect(preferences.bind(.typeZhuyin, at: 1, to: Shortcut("u", control: true)) == "已用於「列出修正的字」",
                       "conflict not refused")
            try expect(preferences.bind(.typeZhuyin, at: 0, to: Shortcut("a")) != nil, "a typing key was accepted")
            let (core, _) = session()
            core.shortcuts = Dictionary(preferences.activeBindings.flatMap { command, list in list.map { ($0, command) } },
                                        uniquingKeysWith: { first, _ in first })
            for (keyCode, modifiers) in [(UInt16(32), NSEvent.ModifierFlags.control), (126, .shift)] {
                type("su3cl3", into: core)
                _ = core.keyDown(keyCode: keyCode, modifiers: modifiers)
                try expect(core.composer.presentation.candidates?.tolerant == true, "binding \(keyCode) did not open corrections")
                press("<esc><esc><esc>", into: core)
            }
            preferences.unbind(.correctionsList, at: 0)
            try expect(preferences.bindings[.correctionsList] == [Shortcut("↑", shift: true)], "remove")
            preferences.unbind(.correctionsList, at: 0)
            try expect(preferences.bindings[.correctionsList] == [], "an optional command could not lose its last key")
            try expect(preferences.resetBindings(.correctionsList) == nil
                       && preferences.bindings[.correctionsList] == ShortcutCommand.correctionsList.defaultBindings, "reset")
            preferences.setEnabled(.correctionsList, false)
            try expect(preferences.activeBindings[.correctionsList] == nil, "turned off but still active")
            preferences.setEnabled(.correctionsList, true)
            try expect(preferences.activeBindings[.correctionsList] != nil, "turned on again but not active")
        }

        // MARK: Space

        test("Space: commits the composition and the app types the space; stored choices migrate") {
            let (core, client) = session()
            var options = core.composer.options
            options.spaceAction = .commitAndSpace
            core.update(options: options, shiftToggle: .both, popupCompositionApps: [])
            type("su3", into: core)
            try expect(!core.keyDown(keyCode: 49, modifiers: []) && client.inserts.last == "你" && !core.composer.isComposing,
                       "space did not commit and pass: \(client.inserts)")
            type("su", into: core)
            try expect(core.keyDown(keyCode: 49, modifiers: []) && core.composer.isComposing, "space did not finish the first tone")
            press("<down>", into: core)
            try expect(core.keyDown(keyCode: 49, modifiers: []) && core.composer.presentation.candidates != nil, "space picked from the list")
            press("<esc><esc><esc>", into: core)
            for (stored, expected) in [("commit", SpaceAction.commitAndSpace), ("commitAndSpace", .commitAndSpace),
                                       ("openCandidates", .openCandidates)] {
                let defaults = scratchDefaults()
                defaults.set(stored, forKey: "spaceAction")
                try expect(Preferences(defaults: defaults).spaceAction == expected, "stored \(stored)")
            }
            for (legacy, expected) in [(false, SpaceAction.commitAndSpace), (true, .openCandidates)] {
                let defaults = scratchDefaults()
                defaults.set(legacy, forKey: "spaceOpensCandidates")
                try expect(Preferences(defaults: defaults).spaceAction == expected, "legacy \(legacy)")
            }
            let defaults = scratchDefaults()
            try expect(Preferences(defaults: defaults).spaceAction == .commitAndSpace, "default")
        }

        // MARK: Settings

        test("key field: hover and recording never change its height; a conflict adds its line") {
            _ = NSApplication.shared
            let preferences = Preferences(defaults: scratchDefaults())
            func height(_ command: ShortcutCommand, _ state: ShortcutFieldPreview?, width: CGFloat = 300) -> CGFloat {
                let view = NSHostingView(rootView: ShortcutField(preferences: preferences, command: command, preview: state)
                    .frame(width: width))
                view.layoutSubtreeIfNeeded()
                return view.fittingSize.height
            }
            for command in [ShortcutCommand.deleteWord, .correctionsList, .singleCharacterList] {
                let count = preferences.bindings[command]?.count ?? 0
                let rest = height(command, nil)
                try expect(rest >= Keycap.height(12), "\(command) measured \(rest) pt")
                for state: ShortcutFieldPreview in [.hover, .recording(index: 0, held: []), .recording(index: 0, held: ["⌃", "⇧"]),
                                                     .recording(index: count, held: []), .accepted(index: 0)] {
                    let other = height(command, state)
                    try expect(abs(other - rest) < 0.5, "\(command) \(state): \(other) pt, at rest \(rest) pt")
                }
            }
            var bindings = preferences.bindings
            bindings[.nextPage] = [Shortcut("Tab"), Shortcut("←", control: true, shift: true)]
            bindings[.selectLeft] = [Shortcut("←", shift: true), Shortcut("←", control: true, shift: true)]
            preferences.bindings = bindings
            let conflicted = height(.nextPage, nil)
            try expect(abs(height(.nextPage, .hover) - conflicted) < 0.5, "hover changed a conflicted field")
            bindings[.selectLeft] = [Shortcut("←", shift: true)]
            preferences.bindings = bindings
            try expect(height(.nextPage, nil) < conflicted, "the conflict line was not added")
        }

        test("key recorder: fn with an arrow or ⌫ is picked up and reads as on a Mac keyboard") {
            let preferences = Preferences(defaults: scratchDefaults())
            func key(_ code: UInt16, _ flags: NSEvent.ModifierFlags = []) -> NSEvent {
                NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags.union([.function, .numericPad]),
                                 timestamp: 0, windowNumber: 0, context: nil, characters: "", charactersIgnoringModifiers: "",
                                 isARepeat: false, keyCode: code)!
            }
            func record(_ event: NSEvent, _ command: ShortcutCommand, at index: Int = 1) -> ShortcutField.Recorded {
                ShortcutField.record(event, command: command, at: index, in: preferences)
            }
            // fn ↓ / fn ↑ (Page Down / Page Up) become keys of 下一頁 / 上一頁.
            var only = preferences.bindings
            only[.nextPage] = [Shortcut("Tab")]
            only[.previousPage] = [Shortcut("Tab", shift: true)]
            preferences.bindings = only
            try expect(record(key(121), .nextPage) == .accept, "fn ↓ for 下一頁: \(record(key(121), .nextPage))")
            try expect(record(key(116), .previousPage) == .accept, "fn ↑ for 上一頁")
            try expect(preferences.bindings[.nextPage]?.last == Shortcut("PageDown"), "fn ↓ not stored")
            try expect(Shortcut("PageDown").keycaps == ["fn", "↓"] && Shortcut("PageDown").spoken == "fn ↓（Page Down）",
                       "fn ↓ reads \(Shortcut("PageDown").keycaps)")
            try expect(Shortcut("PageUp", option: true).keycaps == ["⌥", "fn", "↑"], "⌥ fn ↑")
            // With a modifier any of them can be a key; alone, fn ← → ⌫ and the other page key say what they already do.
            try expect(record(key(115, .option), .selectLeft) == .accept, "⌥ fn ← refused")
            for (code, command, needle) in [(UInt16(119), ShortcutCommand.deleteWord, "fn →（End）"), (115, .deleteWord, "fn ←（Home）"),
                                            (117, .deleteWord, "fn ⌫（向後刪除）"), (121, .previousPage, "fn ↓（Page Down）")] {
                guard case .refuse(let message) = record(key(code), command) else {
                    throw Failure(description: "key \(code) for \(command) was not refused")
                }
                try expect(message.contains(needle), "key \(code): \(message)")
            }
            // fn shows while held.
            let fn = NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: [.function, .shift], timestamp: 0,
                                      windowNumber: 0, context: nil, characters: "", charactersIgnoringModifiers: "",
                                      isARepeat: false, keyCode: 63)!
            try expect(record(fn, .nextPage) == .held(["⇧", "fn"]), "held \(record(fn, .nextPage))")
            // The controller turns the key into the command.
            try expect(Shortcut(keyCode: 121, modifiers: [.function, .numericPad]) == Shortcut("PageDown"), "fn ↓ event")
        }

        test("paging: Tab, fn ↓ and ⌥↓ by default; ⌥↑ ⌥↓ go to the app with nothing composed") {
            try expect(ShortcutCommand.nextPage.defaultBindings == [Shortcut("Tab"), Shortcut("PageDown"), Shortcut("↓", option: true)],
                       "下一頁 defaults")
            try expect(ShortcutCommand.previousPage.defaultBindings == [Shortcut("Tab", shift: true), Shortcut("PageUp"),
                                                                       Shortcut("↑", option: true)], "上一頁 defaults")
            let (core, client) = session()
            try expect(!core.keyDown(keyCode: 125, modifiers: .option) && !core.keyDown(keyCode: 126, modifiers: .option)
                       && !core.keyDown(keyCode: 121, modifiers: .function) && client.operations.isEmpty, "idle ⌥↓ ⌥↑ fn ↓ were taken")
            type("u4", into: core)
            press("<down>", into: core)
            let first = core.composer.presentation.candidates
            try expect((first?.items.count ?? 0) > (first?.pageSize ?? 99), "the sample list has one page")
            for (code, flags) in [(UInt16(125), NSEvent.ModifierFlags.option), (121, .function), (48, [])] {
                press("<esc><down>", into: core)
                try expect(core.keyDown(keyCode: code, modifiers: flags), "key \(code) not handled")
                let list = core.composer.presentation.candidates
                try expect(list.map { $0.selection == $0.pageSize } == true, "key \(code) did not turn the page: \(String(describing: list?.selection))")
                try expect(core.keyDown(keyCode: code == 48 ? 48 : code == 125 ? 126 : 116, modifiers: code == 48 ? .shift : flags),
                           "back key for \(code) not handled")
                try expect(core.composer.presentation.candidates?.selection == 0, "key \(code): back did not return")
            }
            press("<esc><esc><esc>", into: core)
        }

        test("key defaults: stored keys of earlier versions read as the new defaults; a taken default stays with its user") {
            let defaults = scratchDefaults()
            var old = Dictionary(uniqueKeysWithValues: ShortcutCommand.allCases.map { ($0.rawValue, $0.legacyDefaultBindings) })
            old[ShortcutCommand.deleteWord.rawValue] = [Shortcut("d", control: true)]
            defaults.set(try JSONEncoder().encode(old), forKey: "keyBindings")
            let preferences = Preferences(defaults: defaults)
            try expect(preferences.bindings[.nextPage] == ShortcutCommand.nextPage.defaultBindings, "old [Tab] not upgraded")
            try expect(preferences.bindings[.deleteWord] == [Shortcut("d", control: true)], "the user's key was lost")
            var all = preferences.bindings
            all[.singleCharacterList] = [Shortcut("↓", option: true)]
            preferences.bindings = all
            let stored = try JSONDecoder().decode([String: [Shortcut]].self, from: defaults.data(forKey: "keyBindings") ?? Data())
            try expect(Set(stored.keys) == ["deleteWord", "singleCharacterList"], "stored more than the changes: \(stored.keys)")
            try expect(preferences.bindings[.nextPage] == [Shortcut("Tab"), Shortcut("PageDown")], "⌥↓ stayed on 下一頁")
            try expect(preferences.conflicts(of: .nextPage).isEmpty, "a default the user took shows as a conflict")
            try expect(preferences.resetBindings(.singleCharacterList) == nil
                       && preferences.bindings[.nextPage] == ShortcutCommand.nextPage.defaultBindings, "⌥↓ did not come back")
        }

        test("horizontal list: ⌥← ⌥→ list the neighbouring character; with no list they are ← →") {
            let (core, _) = session()
            var options = core.composer.options
            options.verticalList = false
            core.update(options: options, shiftToggle: .both, popupCompositionApps: [])
            type("su3cl3", into: core)
            press("<down>", into: core)
            let target = core.composer.presentation.candidates?.items.first?.text
            try expect(core.keyDown(keyCode: 123, modifiers: .option), "⌥← not handled")
            let moved = core.composer.presentation.candidates?.items.first?.text
            try expect(moved != nil && moved != target, "⌥← kept the list on \(String(describing: target))")
            try expect(core.keyDown(keyCode: 124, modifiers: .option)
                       && core.composer.presentation.candidates?.items.first?.text == target, "⌥→ did not come back")
            press("<esc>", into: core)
            let cursor = core.composer.presentation.cursor
            try expect(core.keyDown(keyCode: 123, modifiers: .option) && core.composer.presentation.cursor == cursor - 1,
                       "⌥← with no list did not move the cursor")
            press("<esc><esc><esc>", into: core)
            try expect(!core.keyDown(keyCode: 123, modifiers: .option), "idle ⌥← was taken")
        }

        test("candidate list ends: paging and moving past the first or last page report the end, nowhere else") {
            let (core, _) = session()
            var edges: [Int] = []
            core.listEdge = { edges.append($0) }
            type("u4", into: core)
            press("<down>", into: core)
            guard let first = core.composer.presentation.candidates, first.pageCount > 1 else {
                throw Failure(description: "the sample list has one page")
            }
            _ = core.handle(.shiftTab)
            _ = core.handle(.up)
            try expect(edges == [-1, -1], "before the first page: \(edges)")
            edges = []
            for _ in 1..<first.pageCount { _ = core.handle(.tab) }
            try expect(edges.isEmpty, "turning pages reported an end: \(edges)")
            _ = core.handle(.tab)
            _ = core.handle(.pageDown)
            try expect(edges == [1, 1], "past the last page: \(edges)")
            edges = []
            let last = core.composer.presentation.candidates!
            for _ in 0..<(last.items.count - 1 - last.selection) { _ = core.handle(.down) }
            try expect(edges.isEmpty, "moving down the last page reported an end")
            _ = core.handle(.down)
            try expect(edges == [1], "↓ on the last candidate: \(edges)")
            _ = core.handle(.space)
            try expect(edges == [1], "Space went round and reported an end")
            press("<esc><esc><esc>", into: core)
        }

        test("candidate window: pages keep the window's size; animated paging costs no more than plain paging") {
            _ = NSApplication.shared
            let saved = Preferences.shared
            defer { Preferences.shared = saved }
            let preferences = Preferences(defaults: scratchDefaults())
            Preferences.shared = preferences
            let items = (0..<14).map { Candidate(text: $0 % 9 == 0 ? "很長很長的候選字\($0)" : "字\($0)") }
            let content = CandidateContentView(frame: .zero)
            let window = HeadlessWindow(size: NSSize(width: 400, height: 600))
            window.isReleasedWhenClosed = false
            window.alphaValue = 0
            window.contentView = content
            window.orderFrontRegardless()
            defer { window.orderOut(nil); window.contentView = nil; window.close() }
            for layout in CandidateLayout.allCases {
                preferences.candidateLayout = layout
                let style = preferences.candidateStyle
                content.reset()
                var sizes = Set<String>()
                for page in [0, 1, 0, 1] {
                    let list = CandidateList(items: items, selection: page * 9, pageSize: 9, tolerant: false)
                    let size = content.update(list: list, preview: nil, keys: Array("123456789"), style: style)
                    sizes.insert("\(size.width)x\(size.height)")
                }
                try expect(sizes.count == 1, "\(layout): the window changed size between pages: \(sizes)")
            }
            func flips(animated: Bool) -> Double {
                preferences.panelAnimation = animated
                preferences.candidateLayout = .vertical
                let style = preferences.candidateStyle
                content.reset()
                let start = DispatchTime.now().uptimeNanoseconds
                for index in 0..<200 {
                    let list = CandidateList(items: items, selection: (index % 2) * 9, pageSize: 9, tolerant: false)
                    content.update(list: list, preview: nil, keys: Array("123456789"), style: style)
                }
                return Double(DispatchTime.now().uptimeNanoseconds - start) / 200 / 1000
            }
            _ = flips(animated: false)
            let plain = flips(animated: false), animated = flips(animated: true)
            print("     page change: \(String(format: "%.0f", plain)) µs plain, \(String(format: "%.0f", animated)) µs animated")
            try expect(animated < plain * 1.5 + 150, "animated paging costs \(animated) µs, plain \(plain) µs")
        }

        test("hover: one row at most; scrolling, paging and leaving clear it") {
            _ = NSApplication.shared
            let state = HoverState()
            state.enter("a")
            state.enter("b")
            state.leave("a")
            try expect(state.hovered == AnyHashable("b"), "two hovered or the wrong one: \(String(describing: state.hovered))")
            state.scrolled()
            try expect(state.hovered == nil, "scrolling left a row hovered")
            let content = CandidateContentView(frame: .zero)
            let window = HeadlessWindow(size: NSSize(width: 300, height: 600))
            window.isReleasedWhenClosed = false
            window.contentView = content
            defer { window.contentView = nil; window.close() }
            let items = (0..<14).map { Candidate(text: "字\($0)") }
            let style = CandidateStyle(layout: .vertical)
            content.update(list: CandidateList(items: items, selection: 0, pageSize: 9, tolerant: false), preview: nil,
                           keys: Array("123456789"), style: style)
            var seen = 0
            for y in stride(from: 20.0, to: content.frame.height - 20, by: 15) {
                content.hover(at: NSPoint(x: 40, y: y))
                try expect(content.hoveredRowCount <= 1, "\(content.hoveredRowCount) rows hovered at y \(y)")
                seen += content.hoveredRowCount
            }
            try expect(seen > 5, "the pointer over the rows hovered none")
            content.update(list: CandidateList(items: items, selection: 9, pageSize: 9, tolerant: false), preview: nil,
                           keys: Array("123456789"), style: style)
            try expect(content.hoveredRowCount == 0, "a row stayed hovered after the page changed (the window is not shown)")
            content.hover(at: NSPoint(x: 40, y: 60))
            content.hover(at: nil)
            try expect(content.hoveredRowCount == 0, "a row stayed hovered after the pointer left")
            content.hoverEnabled = false
            content.hover(at: NSPoint(x: 40, y: 60))
            try expect(content.hoveredRowCount == 0, "an illustration showed hover")
        }

        test("keys: any key can be removed or added, defaults too; a required command keeps one; only changes are stored") {
            let defaults = scratchDefaults()
            let preferences = Preferences(defaults: defaults)
            func stored() -> [String: [Shortcut]] {
                (try? JSONDecoder().decode([String: [Shortcut]].self, from: defaults.data(forKey: "keyBindings") ?? Data())) ?? [:]
            }
            // Remove fn ↓ from 下一頁: Tab and ⌥↓ stay.
            try expect(preferences.unbind(.nextPage, at: 1) == nil, "fn ↓ could not be removed")
            try expect(preferences.bindings[.nextPage] == [Shortcut("Tab"), Shortcut("↓", option: true)], "下一頁 after removing fn ↓")
            try expect(Set(stored().keys) == ["nextPage"], "stored more than the change: \(stored().keys)")
            // A fourth and a fifth key.
            for (index, key) in [(2, Shortcut("n", control: true)), (3, Shortcut("PageDown"))] {
                try expect(preferences.bind(.nextPage, at: index, to: key) == nil, "could not add \(key.spoken)")
            }
            try expect(preferences.bindings[.nextPage]?.count == 4, "four keys")
            try expect(preferences.bind(.nextPage, at: 4, to: Shortcut("m", control: true)) == nil
                       && preferences.bindings[.nextPage]?.count == 5, "the fifth key")
            // An optional command may be left with no key, and stays so.
            for _ in 0..<(preferences.bindings[.singleCharacterList]?.count ?? 0) {
                try expect(preferences.unbind(.singleCharacterList, at: 0) == nil, "只列一個字 refused to lose its key")
            }
            try expect(preferences.bindings[.singleCharacterList] == [], "只列一個字 has keys left")
            try expect(Preferences(defaults: defaults).bindings[.singleCharacterList] == [], "no key did not persist")
            try expect(Preferences(defaults: defaults).activeBindings[.singleCharacterList]?.isEmpty ?? true, "an empty command is active")
            // A required command keeps its last key.
            try expect(preferences.unbind(.commitWithNewline, at: 0) != nil && preferences.bindings[.commitWithNewline]?.count == 1,
                       "送出並換行 lost its only key")
            // 還原預設 brings the defaults back and stores nothing for the command.
            _ = preferences.resetBindings(.nextPage)
            _ = preferences.resetBindings(.singleCharacterList)
            try expect(preferences.bindings[.nextPage] == ShortcutCommand.nextPage.defaultBindings, "下一頁 not restored")
            try expect(stored().isEmpty, "restored commands are still stored: \(stored().keys)")
        }

        test("settings registry: every setting reads and writes each of its values in a sandbox") {
            let original = Preferences(defaults: scratchDefaults())
            let sandbox = original.sandbox()
            for setting in SettingsRegistry.all {
                var values: [SettingValue] = []
                switch setting.kind {
                case .toggle: values = [.bool(true), .bool(false)]
                case .choice(let options): values = options.map(\.value)
                case .number(let range, _, _): values = [.number(range.lowerBound), .number(range.upperBound)]
                case .flags(let flags): values = [.number(Double(flags.reduce(0) { $0 | $1.bit })), .number(0)]
                case .keys(let command): values = [.keys([Shortcut("F", control: true, option: true)]), .keys(command.defaultBindings)]
                }
                for value in values {
                    setting.write(sandbox, value)
                    try expect(setting.read(sandbox) == value, "\(setting.id): wrote \(value), read \(setting.read(sandbox))")
                    try expect(!setting.label(value).isEmpty && setting.label(value) != "—", "\(setting.id): no label for \(value)")
                }
            }
            try expect(SettingsSnapshot(original) == SettingsRegistry.defaults, "the sandbox wrote through to the original")
        }

        test("settings registry: defaults, and 原本 → 改成 changes apply and revert") {
            let defaults = SettingsRegistry.defaults
            let fresh = Preferences(defaults: MemorySettingsStorage())
            let options = fresh.composerOptions
            try expect(options.spaceAction == .commitAndSpace && options.punctuationKeys == .windows && options.typoTolerance == .standard
                       && options.verticalList && options.pageAtListEdge && options.escapeClearsComposition && options.learningEnabled,
                       "app defaults changed: \(options)")
            try expect(defaults.values["spaceAction"] == .text(SpaceAction.commitAndSpace.rawValue), "registry default of 空白鍵")
            let base = Preferences(defaults: MemorySettingsStorage())
            let edited = base.sandbox()
            edited.spaceAction = .openCandidates
            edited.typoTolerance = .aggressive
            edited.candidatesPerPage = 6
            edited.setEnabled(.deleteWord, false)
            _ = edited.unbind(.nextPage, at: 1)
            let changes = SettingsSnapshot(base).changes(to: SettingsSnapshot(edited))
            try expect(Set(changes.map(\.id)) == ["spaceAction", "typoTolerance", "candidatesPerPage", "feature.deleteWord", "keys.nextPage"],
                       "changes \(changes.map(\.id))")
            try expect(changes.first { $0.id == "typoTolerance" }.map { $0.oldLabel == "標準" && $0.newLabel == "積極" } == true, "labels")
            let target = base.sandbox()
            changes.apply(to: target)
            try expect(SettingsSnapshot(target) == SettingsSnapshot(edited), "apply did not reach the edited settings")
            changes.revert(in: target)
            try expect(SettingsSnapshot(target) == SettingsSnapshot(base), "revert did not return to the original")
        }

        test("presets and lessons: every preset applies as changes; every style turns itself on") {
            for preset in SettingsPreset.all {
                let sandbox = Preferences(defaults: MemorySettingsStorage())
                let changes = preset.changes(from: sandbox)
                changes.apply(to: sandbox)
                for (id, value) in preset.values {
                    try expect(SettingsRegistry.setting(id)?.read(sandbox) == value, "\(preset.id): \(id) not set")
                }
            }
            try expect(SettingsPreset.custom.changes(from: Preferences(defaults: MemorySettingsStorage())).isEmpty, "自訂 changed settings")
            try expect(Lesson.all.filter { $0.level == .basic }.map(\.id) == ["typing", "punctuation", "modeSwitch", "paging", "editing"],
                       "basic lessons")
            for group in Lesson.all.flatMap(\.groups) {
                for style in group.styles {
                    let sandbox = Preferences(defaults: MemorySettingsStorage())
                    if style.isActive(sandbox), !group.exclusive { style.set(sandbox, false) }
                    style.set(sandbox, true)
                    try expect(style.isActive(sandbox), "\(group.id): \(style.id) did not turn on")
                    if !group.exclusive {
                        style.set(sandbox, false)
                        try expect(!style.isActive(sandbox), "\(group.id): \(style.id) did not turn off")
                    }
                }
            }
        }

        test("setup wizard: steps from the lessons; nothing is written until 套用; 全部捨棄 writes nothing") {
            let original = Preferences(defaults: MemorySettingsStorage())
            let quick = WizardModel(original: original, mode: .quick)
            try expect(quick.steps.count == 2 + 5 + 2, "快速設定 has \(quick.steps.count) steps")
            try expect(WizardModel(original: original, mode: .full).steps.count == 2 + Lesson.all.count + 2, "完整導覽 steps")
            try expect(WizardModel(original: original, mode: .newSince("0.4.2")).lessons.isEmpty, "新功能導覽 listed old lessons")
            quick.choosePreset(.mac)
            quick.sandbox.typoTolerance = .aggressive
            try expect(original.spaceAction == .commitAndSpace && original.typoTolerance == .standard, "the sandbox wrote through")
            // Applied, not merely listed: turning single Shift off also clears which Shift switches.
            let applied = Preferences(defaults: MemorySettingsStorage())
            SettingsPreset.mac.changes(from: applied).apply(to: applied)
            let presetIDs = Set(SettingsRegistry.defaults.changes(to: SettingsSnapshot(applied)).map(\.id))
            try expect(Set(quick.changes.map(\.id)) == presetIDs.union(["typoTolerance"]), "changes \(quick.changes.map(\.id))")
            if let typo = quick.changes.first(where: { $0.id == "typoTolerance" }) { quick.revert(typo) }
            try expect(Set(quick.changes.map(\.id)) == presetIDs, "改回 did not revert")
            let discarded = WizardModel(original: original, mode: .quick)
            discarded.choosePreset(.mac)
            discarded.discard()
            try expect(original.spaceAction == .commitAndSpace, "全部捨棄 wrote")
            quick.apply()
            try expect(original.spaceAction == .openCandidates && original.typoTolerance == .standard, "套用 did not write the changes")
        }

        // The setup wizard's practice field (UI/Wizard/Practice).
        PracticeSelfTest.run(lexiconURL: lexiconURL, test: test)

        test("live: the 視窗與提示 preview plays — moments and states change over time, and the 中英 badge appears") {
            _ = NSApplication.shared
            let saved = (Preferences.shared, PreviewDirector.step)
            defer { Preferences.shared = saved.0; PreviewDirector.step = saved.1 }
            Preferences.shared = Preferences(defaults: MemorySettingsStorage())
            PreviewDirector.step = 0.12
            let director = PreviewDirector()
            let hosting = NSHostingView(rootView: UnifiedPreview(preferences: Preferences.shared, director: director).frame(width: 560))
            let window = HeadlessWindow(size: NSSize(width: 600, height: 420))
            window.contentView = hosting
            window.orderFrontRegardless()
            defer { window.orderOut(nil); window.contentView = nil; window.close() }
            var states: Set<PreviewState> = [director.state]
            let until = Date().addingTimeInterval(2.5)
            while Date() < until {
                RunLoop.main.run(until: Date().addingTimeInterval(0.05))
                states.insert(director.state)
            }
            try expect(director.ticks >= 6, "the preview did not play: \(director.ticks) moments")
            try expect(states.count >= 3, "the preview stayed on \(states)")
            director.show(.mode, last: false)
            var badge = false
            let modeUntil = Date().addingTimeInterval(1.5)
            while Date() < modeUntil, !badge {
                RunLoop.main.run(until: Date().addingTimeInterval(0.05))
                func find(_ view: NSView) -> Bool { view is ModeBadgeView || view.subviews.contains(where: find) }
                badge = find(hosting)
            }
            try expect(badge, "the 中英 badge never appeared in the preview")
        }

        test("live: the wizard's parts arrive, and closing it leaves the settings window open") {
            _ = NSApplication.shared
            let saved = Preferences.shared
            defer { Preferences.shared = saved }
            Preferences.shared = Preferences(defaults: MemorySettingsStorage())
            let before = WizardEntrance.played
            let model = WizardModel(original: Preferences.shared, mode: .quick)
            let hosting = NSHostingView(rootView: WizardView(model: model).frame(width: 760, height: 600))
            let window = HeadlessWindow(size: NSSize(width: 760, height: 600))
            window.contentView = hosting
            window.orderFrontRegardless()
            RunLoop.main.run(until: Date().addingTimeInterval(0.4))
            model.next()
            RunLoop.main.run(until: Date().addingTimeInterval(0.6))
            window.orderOut(nil); window.contentView = nil; window.close()
            try expect(WizardEntrance.played >= before + 2, "the steps' entrances did not play: \(WizardEntrance.played - before)")

            let settings = SettingsWindowController.shared.makeWindow(pane: .intro)
            settings.setFrameOrigin(NSPoint(x: -30000, y: -30000))
            settings.alphaValue = 0
            settings.orderFrontRegardless()
            WizardWindowController.shared.show(.quick)
            RunLoop.main.run(until: Date().addingTimeInterval(0.3))
            try expect(WizardWindowController.shared.isOpen, "the wizard did not open")
            WizardWindowController.shared.model?.skipAll()
            RunLoop.main.run(until: Date().addingTimeInterval(0.3))
            try expect(!WizardWindowController.shared.isOpen, "全部跳過 did not close the wizard")
            try expect(settings.isVisible, "closing the wizard closed the settings window")
            settings.orderOut(nil)
        }

        test("headless: the self-tests never made ZhuyinType active or one of its windows key") {
            try expect(NSRunningApplication.current.activationPolicy == .prohibited, "the self-test can become active")
            let problems = focus.check()
            try expect(problems.isEmpty, problems.joined(separator: "; "))
        }

        print("\(cases - failures)/\(cases) ControllerCore self-tests passed")
        return failures == 0 ? 0 : 1
    }
}
