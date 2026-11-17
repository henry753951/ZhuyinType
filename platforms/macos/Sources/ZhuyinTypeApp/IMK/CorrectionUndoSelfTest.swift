import AppKit
import ZhuyinEngine

/// 標示自動修正 and ⌫ 還原自動修正, run by `--self-test` with the ControllerCore ones: the presentation's corrections
/// through the Swift wrapper, the dotted underline in the marked text, the candidate window's 「你打的是」 note, the
/// settings and the usage counter.  A tiny lexicon makes the correction certain: ㄊㄜˇ typed reads 乙 (ㄊㄜˋ)
/// through the keyboard channel's tone correction, since the exact 甲 is far rarer.
enum CorrectionUndoSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    private final class Client: TextClient {
        var bundleIdentifier = "org.zhuyintype.self-test.corrections"
        var markedRange = NSRange(location: NSNotFound, length: 0)
        var selectedRange = NSRange(location: 0, length: 0)
        var marks: [NSAttributedString] = []
        var hasInsertionPoint = true
        func insertText(_ text: String, replacementRange: NSRange) {}
        func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) {
            marks.append(NSAttributedString(attributedString: text))
        }
        func caretRect(cursorIndex: Int) -> NSRect { NSRect(x: 100, y: 100, width: 1, height: 20) }
        func textBeforeInsertion(maxLength: Int) -> String? { nil }
    }

    static func run(test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }
        func session(undo: Bool) throws -> (ControllerCore, Client, UsageStatsStore) {
            let engine = try Engine(lexiconText: "ㄊㄜˇ\t甲\t-9\nㄊㄜˋ\t乙\t0\nㄅㄚ\t八\t-2\n")
            var options = ComposerOptions()
            options.learningEnabled = false
            options.undoCorrectionWithBackspace = undo
            let client = Client()
            let core = ControllerCore(engine: engine, client: client, options: options, isEnglish: false)
            core.secureInput = { false }
            let store = UsageStatsStore(directory: nil)
            core.stats = UsageStatsObserver(store: store)
            return (core, client, store)
        }
        func type(_ keys: String, into core: ControllerCore) {
            for key in keys { _ = core.handle(key == " " ? .space : .char(key)) }
        }
        let dotted = NSUnderlineStyle.single.union(.patternDot).rawValue
        func underline(_ mark: NSAttributedString?, at index: Int) -> Int? {
            mark?.attribute(.underlineStyle, at: index, effectiveRange: nil) as? Int
        }

        test("corrections: the presentation lists them; the marked text underlines them dotted in the accent colour") {
            let (core, client, _) = try session(undo: true)
            type("18 wk3", into: core)
            let p = core.composer.presentation
            try expect(p.text == "八乙" && p.corrections == [1..<2], "text \(p.text), corrections \(p.corrections)")
            try expect(p.backspaceReverts && !p.reverted, "backspaceReverts \(p.backspaceReverts), reverted \(p.reverted)")
            let mark = client.marks.last
            try expect(underline(mark, at: 0) == NSUnderlineStyle.single.rawValue, "the exact 八 lost its plain underline")
            try expect(underline(mark, at: 1) == dotted, "the corrected 乙 is not dotted")
            try expect(mark?.attribute(.underlineColor, at: 1, effectiveRange: nil) as? NSColor == .controlAccentColor,
                       "the dotted underline is not in the accent colour")
            try expect(mark?.attribute(.markedClauseSegment, at: 1, effectiveRange: nil) as? Int != nil, "the clause was lost")
            core.showCorrections = false
            core.render()
            try expect(underline(client.marks.last, at: 1) == NSUnderlineStyle.single.rawValue, "off, the correction is still marked")
        }

        test("corrections: the first ⌫ reverts and is counted, the second deletes") {
            let (core, _, store) = try session(undo: true)
            type("18 wk3", into: core)
            _ = core.handle(.backspace)
            let p = core.composer.presentation
            try expect(p.text == "八甲" && p.corrections.isEmpty && p.reverted, "after ⌫: \(p.text) \(p.corrections) \(p.reverted)")
            try expect(store.data.totals.correctionsReverted == 1, "counted \(store.data.totals.correctionsReverted)")
            try expect(store.data.totals.backspaces == 0, "the reverting ⌫ was also counted as a deletion")
            _ = core.handle(.backspace)
            try expect(core.composer.presentation.text == "八", "the second ⌫ did not delete")
            try expect(store.data.totals.correctionsReverted == 1, "a plain ⌫ was counted as a revert")
            let (off, _, offStore) = try session(undo: false)
            type("18 wk3", into: off)
            _ = off.handle(.backspace)
            try expect(off.composer.presentation.text == "八" && offStore.data.totals.correctionsReverted == 0, "off, ⌫ did not delete")
        }

        test("corrections: revertCorrection converts the correction as typed") {
            let (core, _, _) = try session(undo: false)
            type("18 wk3", into: core)
            try expect(core.composer.revertCorrection(), "nothing reverted")
            try expect(core.composer.presentation.text == "八甲" && core.composer.presentation.reverted, "\(core.composer.presentation)")
            try expect(!core.composer.revertCorrection(), "reverted twice")
        }

        test("corrections: the window open on one shows 你打的是 and ⌫ 還原") {
            let (core, _, _) = try session(undo: true)
            type("wk3", into: core)
            _ = core.handle(.down)
            let reading = core.composer.presentation.typedReading
            try expect(reading == "ㄊㄜˇ", "typed reading \(reading ?? "nil")")
            let preferences = Preferences(defaults: MemorySettingsStorage())
            try expect(CandidatePanelController.correctionNote(reading, preferences) == "你打的是 ㄊㄜˇ · ⌫ 還原", "note with ⌫")
            preferences.undoCorrectionWithBackspace = false
            try expect(CandidatePanelController.correctionNote(reading, preferences) == "你打的是 ㄊㄜˇ", "note without ⌫")
            preferences.showCorrections = false
            try expect(CandidatePanelController.correctionNote(reading, preferences) == nil, "note with 標示自動修正 off")
            let content = CandidateContentView(frame: .zero)
            let list = core.composer.presentation.candidates
            let plain = content.update(list: list, preview: nil, keys: Array("123456789"), style: CandidateStyle())
            content.reset()
            let noted = content.update(list: list, preview: nil, keys: Array("123456789"), style: CandidateStyle(),
                                       correctionNote: "你打的是 ㄊㄜˇ · ⌫ 還原")
            try expect(noted.height > plain.height, "the note took no room: \(plain) \(noted)")
            _ = core.handle(.backspace)
            try expect(core.composer.presentation.candidates == nil && core.composer.presentation.text == "甲", "⌫ did not revert")
        }

        test("corrections: settings default on and reach the composer options") {
            let preferences = Preferences(defaults: MemorySettingsStorage())
            try expect(preferences.showCorrections && preferences.undoCorrectionWithBackspace, "defaults are not on")
            try expect(preferences.composerOptions.undoCorrectionWithBackspace, "the option did not reach the composer")
            preferences.undoCorrectionWithBackspace = false
            try expect(!preferences.composerOptions.undoCorrectionWithBackspace, "turning it off did not reach the composer")
            for id in ["showCorrections", "undoCorrectionWithBackspace"] {
                guard let setting = SettingsRegistry.setting(id) else { throw Failure(description: "\(id) is not registered") }
                try expect(setting.place.pane == .choosing, "\(id) is not on 選字")
            }
        }
    }
}
