import AppKit
import ZhuyinEngine

/// The practice field's self-tests, run by `--self-test` with the ControllerCore ones.  Keys are synthetic events
/// handed to the field's view inside this process (`PracticeKeys`); nothing is posted to the system, and the field's
/// windows are headless (never key, never active).
enum PracticeSelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(lexiconURL: URL, test: (String, () throws -> Void) -> Void) {
        let made = PracticeUserData.madeDirectories.count
        // The fields and their data are released when the pool drains (the self-test runs outside the run loop).
        autoreleasepool { runFields(lexiconURL: lexiconURL, test: test) }
        test("practice: every practice folder is gone once its fields are") {
            let left = PracticeUserData.madeDirectories.dropFirst(made).filter { FileManager.default.fileExists(atPath: $0.path) }
            if !left.isEmpty { throw Failure(description: "left behind: \(left.map(\.lastPathComponent))") }
        }
    }

    private static func runFields(lexiconURL: URL, test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }
        let shared: PracticeUserData
        do {
            shared = try PracticeUserData.make(lexiconURL: lexiconURL, reranker: nil, characterModel: nil)
        } catch {
            test("practice: load the practice engine") { throw Failure(description: "\(error)") }
            return
        }
        // Learning off for the shared data, so one test's picks never change the next one's conversions.
        let quiet = SettingsRegistry.defaults.with { $0.learningEnabled = false }
        var windows: [HeadlessWindow] = []
        defer { windows.forEach { $0.contentView = nil; $0.close() } }

        /// A field in a headless window, 520 pt wide, laid out.
        func field(_ task: PracticeTask = .free, settings: SettingsSnapshot = quiet,
                   data: PracticeUserData? = nil) -> PracticeFieldModel {
            let model = PracticeFieldModel(settings: settings, task: task, data: data ?? shared)
            let window = HeadlessWindow(size: NSSize(width: 520, height: model.view.intrinsicContentSize.height))
            window.contentView = model.view
            model.view.frame = NSRect(x: 0, y: 0, width: 520, height: model.view.intrinsicContentSize.height)
            model.view.layoutSubtreeIfNeeded()
            windows.append(window)
            return model
        }
        func type(_ keys: String, _ model: PracticeFieldModel) {
            for key in keys { model.view.textView.keyDown(with: PracticeKeys.keyDown(key)) }
        }
        func press(_ name: String, _ model: PracticeFieldModel, _ modifiers: NSEvent.ModifierFlags = []) {
            model.view.textView.keyDown(with: PracticeKeys.keyDown(named: name, modifiers))
        }
        func shiftTap(_ model: PracticeFieldModel, _ key: UInt16 = 56) {
            model.view.textView.flagsChanged(with: PracticeKeys.flags(key, down: true))
            model.view.textView.flagsChanged(with: PracticeKeys.flags(key, down: false))
        }
        func clause(_ p: Presentation) -> String {
            guard let index = p.focusedClause, p.clauses.indices.contains(index) else { return "" }
            return String(decoding: Array(p.text.utf16)[p.clauses[index]], as: UTF16.self)
        }

        test("practice: distinct tasks with the same prompt reset the field") {
            let first = PracticeTask.action("練習", .contains("甲"))
            let second = PracticeTask.action("練習", .contains("乙"))
            let model = field(first)
            type("su3", model)
            model.update(settings: quiet, task: first, initialText: "")
            try expect(!model.session.presentation.isEmpty, "same task cleared the composition")
            model.update(settings: quiet, task: second, initialText: "")
            try expect(model.session.presentation.isEmpty && model.session.task.id == second.id, "new task was not installed")
        }

        test("practice settings: the field's settings are a copy in memory that follows each new snapshot") {
            let before = realSettings()
            let snapshot = SettingsRegistry.defaults.with {
                $0.candidateLayout = .horizontal
                $0.candidateFontSize = 22
                $0.candidatesPerPage = 6
                $0.shiftToggle = .left
                $0.showModeToast = false
                $0.pageAtListEdge = false
                $0.fuzzySounds = [.zZh, .enEng]
                _ = $0.bind(.nextPage, at: 3, to: Shortcut("n", control: true))
            }
            let session = PracticeSession(settings: snapshot, task: .free)
            let p = session.preferences
            try expect(p.candidateLayout == .horizontal && p.candidateFontSize == 22 && p.candidatesPerPage == 6
                       && p.shiftToggle == .left && !p.showModeToast && !p.pageAtListEdge
                       && p.fuzzySounds == [.zZh, .enEng] && p.bindings[.nextPage]?.count == 4,
                       "values did not reach the field: \(SettingsRegistry.defaults.changes(to: SettingsSnapshot(p)).map(\.id))")
            try expect(p.spaceAction == .commitAndSpace && p.learningEnabled && p.typoTolerance == .standard, "defaults changed")
            try expect(SettingsSnapshot(p) == snapshot, "the field's copy differs from the snapshot")
            session.apply(SettingsRegistry.defaults)
            try expect(SettingsSnapshot(p) == SettingsRegistry.defaults,
                       "back to the defaults: \(SettingsRegistry.defaults.changes(to: SettingsSnapshot(p)).map(\.id))")
            try expect(realSettings() == before, "the user's settings changed")
        }

        test("practice: typing a sentence and pressing Return completes the task, once") {
            let model = field(.sentence("你好"))
            var completions = 0
            model.session.onComplete = { completions += 1 }
            type("su3cl3", model)
            try expect(model.session.client.marked.string == "你好" && model.session.client.text.isEmpty,
                       "composition \(model.session.client.marked.string) text \(model.session.client.text)")
            try expect(!model.progress.isComplete, "done before Return")
            try expect(model.view.textView.accessibilityValue() as? String == "你好", "VoiceOver value while composing")
            press("return", model)
            try expect(model.session.client.text == "你好" && model.progress.isComplete && completions == 1,
                       "text \(model.session.client.text), complete \(model.progress.isComplete), \(completions) completions")
            try expect(model.view.textView.isDone, "the outline did not turn green")
            type("su3", model)
            press("return", model)
            try expect(completions == 1 && model.progress.isComplete, "completed again or undone")
            let sent = model.session.client.text
            press("delete", model)
            try expect(model.session.client.text == String(sent.dropLast()), "⌫ with nothing composed: \(sent) → \(model.session.client.text)")
        }

        test("practice: open the list, turn a page and pick, by keys and by a click") {
            let model = field(.action("翻頁並選字", .openCandidates, .turnPage, .pick(nil)))
            type("su3", model)
            press("down", model)
            guard let list = model.session.presentation.candidates else { throw Failure(description: "↓ opened no list") }
            try expect(model.progress.outcome.events == ["open"] && !model.progress.isComplete, "opening: \(model.progress.outcome.events)")
            try expect(list.pageCount > 1, "only \(list.items.count) candidates for ㄋㄧˇ")
            press("tab", model)
            let page = model.session.presentation.candidates
            try expect(page?.page == 1 && model.progress.outcome.events == ["open", "page"], "Tab: page \(page?.page ?? -1), \(model.progress.outcome.events)")
            let chosen = page!.items[page!.pageRange.lowerBound + 1].text
            type("2", model)
            try expect(model.progress.isComplete && model.session.presentation.candidates == nil, "2 did not pick")
            press("return", model)
            try expect(model.session.client.text == chosen, "picked \(chosen), sent \(model.session.client.text)")

            let clicked = field(.action("點選候選字", .pick(nil)))
            type("su3", clicked)
            press("down", clicked)
            let items = clicked.session.presentation.candidates?.items ?? []
            try expect(items.count > 2, "no list to click")
            clicked.view.candidates.onSelect?(2)
            try expect(clicked.progress.isComplete && clicked.session.client.marked.string == items[2].text,
                       "a click picked \(clicked.session.client.marked.string), not \(items[2].text)")
        }

        test("practice: correcting a character through the list") {
            let model = field()
            type("su3cl3", model)
            press("down", model)
            let p = model.session.presentation
            guard let list = p.candidates else { throw Failure(description: "no list") }
            let replaced = clause(p)
            guard let offset = list.items[list.pageRange].firstIndex(where: { $0.text != replaced }) else {
                throw Failure(description: "every candidate is \(replaced)")
            }
            let target = list.items[offset].text
            model.session.setTask(.action("改字", .correct(target)))
            // setTask starts over: type again.
            type("su3cl3", model)
            press("down", model)
            type(String(model.session.preferences.pageSelectionKeys[offset - list.pageRange.lowerBound]), model)
            try expect(model.progress.isComplete, "picking \(target) for \(replaced) did not count as a correction")
            let same = field(.action("改字", .correct(replaced)))
            type("su3cl3", same)
            press("down", same)
            press("return", same)
            try expect(!same.progress.isComplete, "picking the text already shown counted as a correction")
        }

        test("practice: Shift switches to English, English keys type into the field, the badge shows; Caps Lock types capitals") {
            let model = field(.goals("切換到英文打 OK，再切回中文", hint: nil, [.contains("OK"), .switchMode(times: 2)]))
            shiftTap(model)
            try expect(model.session.isEnglish && model.view.badgeText == "英", "Shift tap: english \(model.session.isEnglish), badge \(model.view.badgeText ?? "none")")
            type("OK", model)
            try expect(model.session.client.text == "OK" && model.session.client.marked.length == 0, "English typed \(model.session.client.text)")
            try expect(model.progress.outcome.events == ["mode"] && model.progress.outcome.text == "OK" && !model.progress.isComplete,
                       "progress \(model.progress.outcome.events)")
            shiftTap(model, 60)
            try expect(!model.session.isEnglish && model.view.badgeText == "中" && model.progress.isComplete, "right Shift back")
            type("cl3", model)
            press("return", model)
            try expect(model.session.client.text.hasPrefix("OK") && model.session.client.text.count == 3, "after: \(model.session.client.text)")
            model.view.textView.flagsChanged(with: PracticeKeys.flags(57, down: true, capsLock: true))
            model.view.textView.keyDown(with: PracticeKeys.keyDown(0, .capsLock, characters: "A"))
            model.view.textView.flagsChanged(with: PracticeKeys.flags(57, down: false, capsLock: false))
            try expect(model.session.client.text.hasSuffix("A"), "Caps Lock: \(model.session.client.text)")
            let quietBadge = field(settings: quiet.with { $0.showModeToast = false })
            shiftTap(quietBadge)
            try expect(quietBadge.session.isEnglish && quietBadge.view.badgeText == nil, "the badge showed with 中英提示 off")
        }

        test("practice: punctuation with ⌃ (Windows) and, after a live change, with ⇧ (macOS)") {
            let model = field(.action("打出「，」與「。」", .contains("，"), .contains("。")))
            model.view.textView.keyDown(with: PracticeKeys.keyDown(43, .control, characters: ","))
            model.view.textView.keyDown(with: PracticeKeys.keyDown(47, .control, characters: "."))
            press("return", model)
            try expect(model.session.client.text == "，。" && model.progress.isComplete, "⌃ punctuation: \(model.session.client.text)")
            model.update(settings: quiet.with { $0.punctuationKeys = .mac }, task: model.session.task, initialText: "")
            model.view.textView.keyDown(with: PracticeKeys.keyDown(43, .shift, characters: "<"))
            press("return", model)
            try expect(model.session.client.text == "，。，", "⇧, with macOS punctuation: \(model.session.client.text)")
        }

        test("practice: adding a user phrase goes to the practice data only") {
            let data = try PracticeUserData.make(lexiconURL: lexiconURL, reranker: nil, characterModel: nil)
            let model = field(.userPhrase("你好"), data: data)
            type("su3cl3", model)
            press("left", model, .shift)
            press("left", model, .shift)
            try expect(model.session.panel?.notice?.contains("你好") == true, "selection notice: \(model.session.panel?.notice ?? "none")")
            try expect(PracticeTextView.announcement(for: model.session.panel) == model.session.panel?.notice, "VoiceOver notice")
            press("return", model)
            try expect(data.userPhrases.contains("你好") && model.progress.isComplete, "not added: \(data.userPhrases)")
            try expect(!shared.userPhrases.contains("你好"), "added to other practice data")
            data.flush()
            let file = (try? String(contentsOf: data.directory.appendingPathComponent("user_phrases.tsv"), encoding: .utf8)) ?? ""
            try expect(file.contains("你好"), "user_phrases.tsv in the practice folder: \(file)")
        }

        test("practice: nothing is written to the user's settings or data (paths and modification times)") {
            let before = realFiles(), settings = realSettings()
            var loaded: PracticeUserData?
            var answered = false
            PracticeUserData.shared(lexiconURL: lexiconURL) { loaded = $0; answered = true }
            let deadline = Date(timeIntervalSinceNow: 60)
            while !answered, Date() < deadline { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05)) }
            guard let directory = loaded?.directory else { throw Failure(description: "the practice data did not load") }
            try expect(!directory.path.hasPrefix(EngineHost.userDirectory.path), "practice data inside the user's folder")
            try autoreleasepool {
                let data = loaded!
                loaded = nil
                let model = field(.free, settings: SettingsRegistry.defaults.with {
                    $0.learningEnabled = true
                    $0.recordsCorrections = true
                    $0.recordsCorrectionContext = true
                    $0.recordsUsageStats = true
                }, data: data)
                type("su3cl3", model)
                press("down", model)
                type("3", model)
                press("return", model)
                type("su3cl3", model)
                press("left", model, .shift)
                press("left", model, .shift)
                press("return", model)
                press("return", model)
                try expect(model.session.core?.recorder != nil && model.session.core?.stats != nil, "recording was not on")
                try expect(model.session.core?.secureInput() == false && model.session.core?.composer.learningSuspended == false,
                           "the system's secure input reached the field")
                data.flush()
                let written = (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
                try expect(written.contains(CorrectionLog.fileName) && written.contains(UsageStatsStore.fileName)
                           && written.contains("user_phrases.tsv"), "practice folder: \(written)")
                // The last field lets go: the folder goes with it.
                for window in windows where window.contentView === model.view { window.contentView = nil; window.close() }
                windows.removeAll { $0.contentView == nil }
            }
            try expect(!FileManager.default.fileExists(atPath: directory.path), "the practice folder was left behind")
            try expect(realFiles() == before, "the user's data changed: \(difference(before, realFiles()))")
            try expect(realSettings() == settings, "the user's settings changed")
        }

        test("practice: draft settings apply while practising") {
            let model = field(.action("選字", .pick(nil)))
            type("su3", model)
            press("down", model)
            try expect(model.session.panel?.keys == Array("123456789"), "keys \(String(model.session.panel?.keys ?? []))")
            let homeRow = quiet.with { $0.selectionKeys = .homeRow; $0.candidatesPerPage = 5 }
            model.update(settings: homeRow, task: model.session.task, initialText: "")
            let list = model.session.presentation.candidates
            try expect(model.session.panel?.keys == Array("asdfg") && list?.pageSize == 5, "after the change: keys \(String(model.session.panel?.keys ?? [])), page \(list?.pageSize ?? 0)")
            let second = list!.items[list!.pageRange.lowerBound + 1].text
            type("s", model)
            try expect(model.progress.isComplete && model.session.client.marked.string == second, "s picked \(model.session.client.marked.string), not \(second)")
            press("return", model)

            let horizontal = homeRow.with { $0.candidateLayout = .horizontal; $0.panelAnimation = false }
            model.update(settings: horizontal, task: model.session.task, initialText: "")
            type("su3", model)
            press("down", model)
            try expect(model.session.core?.composer.options.verticalList == false && model.view.candidates.animatesOverride == false,
                       "layout or animation did not follow")
            try expect(model.view.panelFrame.width > model.view.panelFrame.height * 2, "the window is not horizontal: \(model.view.panelFrame)")
            try expect(model.view.panelScale == 1 && model.view.panelFrame.size == model.view.surface.frame.size,
                       "the window is not drawn at 100 %: \(model.view.panelFrame), scale \(model.view.panelScale)")
            press("esc", model)
            press("esc", model)

            model.update(settings: horizontal.with { $0.shiftToggle = .none }, task: model.session.task, initialText: "")
            shiftTap(model)
            try expect(!model.session.isEnglish, "Shift switched with 單按 Shift off")

            model.update(settings: horizontal.with { $0.spaceAction = .openCandidates }, task: model.session.task, initialText: "")
            type("su3", model)
            press("space", model)
            try expect(model.session.presentation.candidates != nil, "Space did not open the list")
            press("esc", model)
            press("esc", model)

            model.update(settings: quiet.with { $0.candidatePlacement = .above }, task: model.session.task, initialText: "")
            // What the wizard's layout does with the new height (no display pass runs in the self-test).
            model.view.setFrameSize(NSSize(width: 520, height: model.height))
            model.view.layoutSubtreeIfNeeded()
            type("su3", model)
            press("down", model)
            let caret = model.view.compositionRect()
            try expect(model.view.panelShown && model.view.panelFrame.minY >= caret.maxY, "not above: \(model.view.panelFrame) caret \(caret)")
            try expect(model.view.bounds.contains(model.view.panelFrame), "outside the field: \(model.view.panelFrame) in \(model.view.bounds)")
        }

        test("practice: the real candidate window sits under the composition; VoiceOver reads the text and the candidates") {
            let model = field()
            type("su3cl3", model)
            press("down", model)
            let view = model.view
            guard let list = model.session.presentation.candidates else { throw Failure(description: "no list") }
            let caret = view.compositionRect()
            try expect(view.panelShown && view.surface.superview === view.panelHost && view.panelHost.superview === view && view.surface.content === view.candidates, "not shown in the field")
            try expect(view.bounds.contains(view.panelFrame), "outside the field: \(view.panelFrame) in \(view.bounds)")
            try expect(view.panelFrame.maxY <= caret.minY - InputPanelPlacement.gap + 0.5, "not under the line: \(view.panelFrame) caret \(caret)")
            try expect(abs(view.panelFrame.minX - max(0, caret.minX - view.candidates.candidateTextInset)) < 1, "not lined up with the composition")
            try expect(view.textView.accessibilityRole() == .textField
                       && view.textView.accessibilityValue() as? String == model.session.presentation.text, "text field value")
            let selected = list.items[list.selection].text
            let labels = accessibleLabels(view.candidates)
            try expect(labels.contains { $0.contains(selected) }, "no candidate VoiceOver can read: \(labels)")
            try expect(PracticeTextView.announcement(for: model.session.panel)?.hasPrefix("\(selected)，第 1 個") == true,
                       "announcement \(PracticeTextView.announcement(for: model.session.panel) ?? "none")")
            press("esc", model)
            try expect(!view.panelShown, "Esc left the window")
            let frame = NSRect(x: 10, y: 10, width: 100, height: 80), line = NSRect(x: 20, y: 100, width: 0, height: 20)
            try expect(PracticeFieldView.entranceFrame(frame, caret: line, reduceMotion: true) == frame
                       && PracticeFieldView.entranceFrame(frame, caret: line, reduceMotion: false).minY == frame.minY - PanelMotion.slide,
                       "Reduce Motion: the window still slides")
            try expect(!(view.window?.isKeyWindow ?? true) && !NSApp.isActive, "the field took the keyboard")
        }

        test("practice: the registry's basic lessons can be done in the field") {
            /// Turns pages (at least once) to `word` in the open list and picks it with its selection key.
            func pick(_ word: String, _ model: PracticeFieldModel) throws {
                guard let list = model.session.presentation.candidates,
                      let index = list.items.firstIndex(where: { $0.text == word }) else {
                    throw Failure(description: "「\(word)」 is not in the list")
                }
                if index / list.pageSize == 0 { press("tab", model); press("tab", model, .shift) }
                while let now = model.session.presentation.candidates, now.page < index / list.pageSize { press("tab", model) }
                let keys = model.session.preferences.pageSelectionKeys
                type(String(keys[index % list.pageSize]), model)
            }
            let lessons = Dictionary(uniqueKeysWithValues: Lesson.all.map { ($0.id, $0) })
            func lesson(_ id: String) throws -> PracticeTask {
                guard let task = lessons[id]?.tasks.first else { throw Failure(description: "no lesson \(id)") }
                return task
            }
            let typing = field(try lesson("typing"))
            type("rup wu0 wu0 fu4cp3cl3", typing)
            press("return", typing)
            try expect(typing.progress.isComplete, "打字: \(typing.progress.outcome.text)")

            let punctuation = field(try lesson("punctuation"))
            type("cl3", punctuation)
            punctuation.view.textView.keyDown(with: PracticeKeys.keyDown(43, .control, characters: ","))
            punctuation.view.textView.keyDown(with: PracticeKeys.keyDown(47, .control, characters: "."))
            press("return", punctuation)
            try expect(punctuation.progress.isComplete, "標點: \(punctuation.progress.outcome.text)")

            let mode = field(try lesson("modeSwitch"))
            shiftTap(mode)
            type("OK", mode)
            shiftTap(mode)
            type("cl3", mode)
            press("return", mode)
            try expect(mode.progress.isComplete, "中英切換: \(mode.progress.outcome.text) \(mode.progress.outcome.events)")

            let selecting = field(try lesson("selecting"))
            type("au/6", selecting)
            press("down", selecting)
            try pick("名", selecting)
            try expect(selecting.progress.isComplete, "選字: picks \(selecting.progress.outcome.picks)")

            let paging = field(try lesson("paging"))
            type("au/6", paging)
            press("down", paging)
            try pick("暝", paging)
            try expect(paging.progress.isComplete, "翻頁: picks \(paging.progress.outcome.picks) \(paging.progress.outcome.events)")

            let editing = field(try lesson("editing"))
            // 「你」 is the last character, so changing it to 「妳」 leaves 「謝謝」 as it is.
            type("vu,4vu,7su3", editing)
            press("down", editing)
            try pick("妳", editing)
            press("return", editing)
            try expect(editing.progress.isComplete && editing.progress.outcome.corrected.contains("妳"),
                       "改字: \(editing.progress.outcome.text), picks \(editing.progress.outcome.picks)")
        }

        test("practice: Return and Esc stay in the field; Tab with nothing composed goes to the window") {
            let model = field()
            let session = model.session
            try expect(session.keyDown(keyCode: 36, modifiers: [], characters: "\r", time: 1), "Return left the field")
            try expect(session.keyDown(keyCode: 53, modifiers: [], characters: "\u{1b}", time: 2), "Esc left the field")
            try expect(!session.keyDown(keyCode: 48, modifiers: [], characters: "\t", time: 3), "Tab did not go to the window")
            try expect(!session.keyDown(keyCode: 8, modifiers: .command, characters: "c", time: 4), "⌘C stayed in the field")
            try expect(session.client.text.isEmpty, "typed \(session.client.text)")
        }

        test("practice: ← → move in the composition, and through the sent text with nothing composed; a click moves the insertion point") {
            let model = field()
            let client = model.session.client, textView = model.view.textView
            func caret() -> Int { textView.accessibilitySelectedTextRange().location }
            type("su3cl3", model)
            press("left", model)
            try expect(model.session.presentation.cursor == 1 && caret() == 1,
                       "← in the composition: cursor \(model.session.presentation.cursor), caret \(caret())")
            press("right", model)
            press("return", model)
            let sent = client.text
            try expect(sent.count == 2 && client.insertion == sent.utf16.count, "sent \(sent), insertion \(client.insertion)")
            press("left", model)
            try expect(client.insertion == 1 && caret() == 1, "← with nothing composed: insertion \(client.insertion), caret \(caret())")
            press("up", model)
            press("left", model)
            try expect(client.insertion == 0, "↑ then ← at the start: \(client.insertion)")
            press("down", model)
            try expect(client.insertion == sent.utf16.count, "↓: \(client.insertion)")
            press("left", model, .command)
            try expect(client.insertion == 0, "⌘←: \(client.insertion)")
            press("right", model, .command)
            try expect(client.insertion == sent.utf16.count, "⌘→: \(client.insertion)")

            // Typing in the middle: the composition and the sent text go to the insertion point, and the text before
            // it is what the core reads.
            press("left", model)
            let head = String(sent.prefix(1)), tail = String(sent.suffix(1))
            try expect(client.textBeforeInsertion(maxLength: 10) == head, "text before the insertion point: \(client.textBeforeInsertion(maxLength: 10) ?? "nil")")
            type("cl3", model)
            let composed = client.marked.string
            try expect(!composed.isEmpty && client.markedRange.location == head.utf16.count && client.shown == head + composed + tail
                       && textView.accessibilityValue() as? String == client.shown && model.progress.outcome.text == client.shown,
                       "composing in the middle: \(client.shown), marked at \(client.markedRange.location)")
            try expect(model.view.compositionRect().minX > textView.frame.minX + 14, "the window is not at the composition: \(model.view.compositionRect())")
            press("left", model)
            press("right", model)
            try expect(client.markedRange.location == head.utf16.count, "← → while composing moved the insertion point")
            press("return", model)
            try expect(client.text == head + composed + tail && client.insertion == (head + composed).utf16.count,
                       "sent in the middle: \(client.text), insertion \(client.insertion)")
            press("delete", model)
            try expect(client.text == sent && client.insertion == head.utf16.count, "⌫ in the middle: \(client.text), insertion \(client.insertion)")

            // A click sends the composition, as InputMethodKit does, then puts the insertion point where it was.
            func click(_ x: CGFloat) {
                let point = textView.convert(NSPoint(x: x, y: textView.bounds.midY), to: nil)
                let event = NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: 0,
                                               windowNumber: textView.window?.windowNumber ?? 0, context: nil, eventNumber: 0,
                                               clickCount: 1, pressure: 1)!
                textView.mouseDown(with: event)
            }
            press("up", model)
            type("su3", model)
            let start = client.marked.string
            click(textView.bounds.maxX - 4)
            try expect(client.marked.length == 0 && client.text == start + sent && client.insertion == client.text.utf16.count
                       && !model.view.panelShown, "a click while composing: \(client.text), insertion \(client.insertion)")
            guard let x = stride(from: CGFloat(0), to: textView.bounds.width, by: 1).first(where: { textView.offset(atX: $0) == 1 }) else {
                throw Failure(description: "no place in the field for offset 1")
            }
            click(x)
            try expect(client.insertion == 1 && caret() == 1, "a click after the first character: \(client.insertion)")
            click(1)
            try expect(client.insertion == 0, "a click at the start: \(client.insertion)")
            try expect(!(model.view.window?.isKeyWindow ?? true) && !NSApp.isActive, "the click took the keyboard")
        }
    }

    // MARK: The user's files and settings

    private struct FileState: Equatable { var size: Int; var modified: Date }

    /// Every file in the user's ZhuyinType folder (not the models) and the app's settings file, with size and time.
    private static func realFiles() -> [String: FileState] {
        var files: [String: FileState] = [:]
        let manager = FileManager.default
        var paths = ((try? manager.contentsOfDirectory(atPath: EngineHost.userDirectory.path)) ?? [])
            .map { EngineHost.userDirectory.appendingPathComponent($0) }
        if let bundle = Bundle.main.bundleIdentifier {
            paths.append(manager.homeDirectoryForCurrentUser.appendingPathComponent("Library/Preferences/\(bundle).plist"))
        }
        paths.append(CorrectionLog.shared.url)
        if let stats = UsageStatsStore.shared.url { paths.append(stats) }
        for url in paths {
            guard let attributes = try? manager.attributesOfItem(atPath: url.path),
                  attributes[.type] as? FileAttributeType == .typeRegular else { continue }
            files[url.path] = FileState(size: (attributes[.size] as? Int) ?? -1,
                                        modified: (attributes[.modificationDate] as? Date) ?? .distantPast)
        }
        return files
    }

    private static func difference(_ a: [String: FileState], _ b: [String: FileState]) -> String {
        Set(a.keys).union(b.keys).filter { a[$0] != b[$0] }.sorted().joined(separator: ", ")
    }

    /// The app's own settings as stored now.
    private static func realSettings() -> NSDictionary {
        NSDictionary(dictionary: Dictionary(uniqueKeysWithValues: Preferences.Key.all.compactMap { key in
            UserDefaults.standard.object(forKey: key).map { (key, $0) }
        }))
    }

    private static func accessibleLabels(_ view: NSView) -> [String] {
        var labels: [String] = []
        if view.isAccessibilityElement(), let label = view.accessibilityLabel(), !view.isHidden { labels.append(label) }
        for sub in view.subviews where !sub.isHidden { labels += accessibleLabels(sub) }
        return labels
    }
}
