import AppKit
import ZhuyinCore

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
    /// The document before the insertion point, or nil for an app that does not say.
    var documentText: String?
    func textBeforeInsertion(maxLength: Int) -> String? { documentText.map { String($0.suffix(maxLength)) } }
}

enum ControllerCoreSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(lexiconURL: URL) -> Int32 {
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
            return (ControllerCore(engine: engine, client: client, options: options, isEnglish: english), client)
        }
        func type(_ keys: String, into core: ControllerCore) {
            for key in keys { _ = core.handle(key == " " ? .space : .char(key)) }
        }
        func shift(_ core: ControllerCore, _ key: UInt16 = 56, _ start: TimeInterval = 1) {
            _ = core.flagsChanged(keyCode: key, modifiers: .shift, physicallyDown: true, timestamp: start)
            _ = core.flagsChanged(keyCode: key, modifiers: [], physicallyDown: false, timestamp: start + 0.1)
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
            try expect(!core.keyDown(keyCode: 126, modifiers: .option), "Option-↑ was taken from the app")
            try expect(client.inserts.last == visible, "Option-↑ did not commit before passing: \(client.inserts)")
            // ⇧↑ with no list open opens the corrections list directly.
            type("ji3ap7", into: core)
            try expect(core.keyDown(keyCode: 126, modifiers: .shift) && tolerant == true, "⇧↑ did not open the corrections list")
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
                               layout: .vertical, fontSize: 18, zhuyinHint: hint)
            guard let button = content.subviews.compactMap({ $0 as? NSButton }).first(where: {
                $0.accessibilityLabel()?.hasPrefix("`，") == true
            }) else { throw Failure(description: "missing hint button") }
            button.performClick(nil)
            try expect(core.composer.presentation.text == hint && core.composer.presentation.candidates == nil,
                       "footer click missed highlighted span")
            try expect(client.inserts.isEmpty, "footer click committed")
            panel.onZhuyin = nil
        }

        print("\(cases - failures)/\(cases) ControllerCore self-tests passed")
        return failures == 0 ? 0 : 1
    }
}
