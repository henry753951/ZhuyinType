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
        test("popup app receives preview without marked text") {
            let (core, client) = session()
            core.popupCompositionApps = [client.bundleIdentifier]
            var preview: String?
            var list: CandidateList?
            var hidden = false
            core.showPanel = { candidates, text, _ in list = candidates; preview = text }
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
            for code: UInt16 in [82, 83, 84, 85, 86, 87, 88, 89, 91, 92] {
                try expect(KeyMapping.input(keyCode: code, modifiers: []) == nil, "keypad \(code)")
            }
        }

        print("\(cases - failures)/\(cases) ControllerCore self-tests passed")
        return failures == 0 ? 0 : 1
    }
}
