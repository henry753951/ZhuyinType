import AppKit
import Carbon.HIToolbox
import ZhuyinEngine

protocol TextClient: AnyObject {
    func insertText(_ text: String, replacementRange: NSRange)
    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange)
    var markedRange: NSRange { get }
    var selectedRange: NSRange { get }
    var bundleIdentifier: String { get }
    func caretRect(cursorIndex: Int) -> NSRect
    /// Whether the focus is a place to type: the app reports a selection, a composition or the caret's line.  False in
    /// games and other windows without a text field, where Shift is not a mode switch.
    var hasInsertionPoint: Bool { get }
    /// Up to `maxLength` UTF-16 units of the document before the insertion point; "" at its start, nil when the app
    /// does not say.
    func textBeforeInsertion(maxLength: Int) -> String?
}

final class ControllerCore {
    let composer: Composer
    let client: TextClient
    private(set) var isEnglish: Bool
    var shiftToggle: ShiftToggle = .both
    var popupCompositionApps: [String] = []
    var showPanel: ((CandidateList?, String?, NSRect, String?, String?) -> Void)?
    var hidePanel: (() -> Void)?
    var modeChanged: ((Bool, NSRect) -> Void)?
    /// 「記錄改字」 when it is on: observes keys, clicks and commits, never changes what happens.
    var recorder: CorrectionRecorder?
    /// 「記錄使用統計」 when it is on: counts keys, picks and corrections, and times each key.  Never changes anything.
    var stats: UsageStatsObserver?
    /// Optional key features turned off on the 快捷鍵 page; their keys go to the app.
    var disabledFeatures: Set<KeyFeature> = []
    /// The sentence shown has a typing correction (as last chosen by the reranker).
    private(set) var shownCorrected = false

    private struct Mark: Equatable {
        var text: String
        var cursor: Int
        var clauses: [Range<Int>]
        var focus: Int?
        var target: Int?
    }
    private var lastMark: Mark?
    private var heldShifts: Set<UInt16> = []
    private var shiftTap: (key: UInt16, time: TimeInterval)?
    private var rendering = false
    private let insertionRange = NSRange(location: NSNotFound, length: 0)

    /// The text before the insertion point as far as the input method knows it, which the reranker reads: the app's own
    /// text when a composition starts, otherwise what was committed in this app within `contextWindow`.
    private(set) var recentText = ""
    private var recentTime = Date.distantPast
    /// What a full composition committed by itself during the key being handled, not yet inserted: the models read it
    /// after `recentText` while the rest is converted again.  Empty under secure input.
    private var sentDuringKey = ""
    /// The composition started under secure input (the composer's `learningSuspended`, kept here so the commit
    /// listener need not ask the composer while it is busy).
    private var secureComposition = false
    /// The text before the composition, as the models read it.
    private var modelContext: String { recentText + sentDuringKey }
    /// Read the app's text before the insertion point and remember commits per app (a setting).
    var usesPrecedingText = true
    /// How long what was committed in an app stays context once typing there stops.
    static let contextWindow: TimeInterval = 600
    /// Characters read from the app before the insertion point, and kept as context: the character model reads up to
    /// 255, the reranker its last 24.
    static let contextRead = 128
    static let contextKept = 255
    /// Per app, what was last committed there: context across fields and focus changes, for `contextWindow`.
    private static var appContext: [String: (text: String, time: Date)] = [:]
    var now: () -> Date = Date.init

    init(engine: Engine, client: TextClient, options: ComposerOptions, isEnglish: Bool,
         reranker: NeuralReranker? = nil, characterModel: SyllableScorer? = nil) {
        self.composer = Composer(engine: engine, options: options)
        self.client = client
        self.isEnglish = isEnglish
        // A full composition commits its head before converting the rest: the head is text before the rest.
        composer.commitListener = { [weak self] text in
            guard let self, !self.secureComposition else { return }
            self.sentDuringKey += text
        }
        if let characterModel {
            composer.characterWeight = SyllableScorer.appWeight
            composer.characterScorer = { [weak self] syllables, candidates in
                characterModel.scores(context: self?.modelContext ?? "", syllables: syllables.map {
                    SyllableScorer.Syllable(base: $0.base, tone: $0.tone, costBucket: $0.costBucket, implicit: $0.implicit)
                }, candidates: candidates)
            }
        }
        if let reranker {
            composer.rerankingCount = reranker.candidateCount
            composer.reranker = { [weak self] sentences in
                let index = reranker.choose(sentences, context: self?.modelContext ?? "")
                let shown = sentences.indices.contains(index) ? sentences[index] : sentences.first
                self?.shownCorrected = shown?.corrected ?? false
                return index
            }
            composer.candidateScorer = { [weak self] before, candidates in
                reranker.candidateScores(context: (self?.modelContext ?? "") + before, candidates: candidates)
            }
        }
    }

    private var capsLock = false
    /// Rebindable shortcuts, by key.
    /// A key that pages or moves through the open list stopped at its end (1: past the last, -1: before the first).
    var listEdge: ((Int) -> Void)?
    var shortcuts: [Shortcut: ShortcutCommand] = Dictionary(
        ShortcutCommand.defaultBindings.flatMap { command, list in list.map { ($0, command) } }, uniquingKeysWith: { first, _ in first })

    func update(options: ComposerOptions, shiftToggle: ShiftToggle, popupCompositionApps: [String]) {
        composer.options = options
        self.shiftToggle = shiftToggle
        self.popupCompositionApps = popupCompositionApps
        shiftTap = nil
        render()
    }

    @discardableResult
    func keyDown(keyCode: UInt16, modifiers: NSEvent.ModifierFlags, time: TimeInterval? = nil) -> Bool {
        shiftTap = nil
        guard !rendering, !isEnglish else { return false }
        // With 「Caps Lock 打英文」 off, Caps Lock does not change what the keys type.
        let modifiers = disabledFeatures.contains(.capsLockEnglish) ? modifiers.subtracting(.capsLock) : modifiers
        // Caps Lock types English, every key of it, until it is released; what was being composed is kept.
        if modifiers.contains(.capsLock) {
            commit()
            breakContext()
            return false
        }
        if let shortcut = Shortcut(keyCode: keyCode, modifiers: modifiers), let command = shortcuts[shortcut] {
            if let input = command.input { return handle(input, time: time) }
            return neighbour(command == .nextCharacter ? 1 : -1, time: time)
        }
        // A command's input comes only through its shortcut; its old default key, now unbound, is the app's.
        guard let key = KeyMapping.input(keyCode: keyCode, modifiers: modifiers),
              !ShortcutCommand.commandInputs.contains(key), !turnedOff(key) else {
            commit()
            breakContext()
            return false
        }
        return handle(key, time: time)
    }

    /// ⌃ and ⌥ symbol keys whose feature is off go to the app.
    private func turnedOff(_ key: KeyInput) -> Bool {
        switch key {
        case .controlChar: return disabledFeatures.contains(.controlPunctuation)
        case .printed: return disabledFeatures.contains(.optionSymbols)
        default: return false
        }
    }

    @discardableResult
    func handle(_ key: KeyInput, time: TimeInterval? = nil) -> Bool {
        guard !rendering else { return false }
        if !composer.isComposing, Self.startsComposition(key) { prepareContext() }
        // With 「送出後刪掉重打」 off, ⌫ on sent text is an ordinary key for the app: the next composition starts a
        // sentence and nothing is learned from retyping.
        if key == .backspace, !composer.isComposing, disabledFeatures.contains(.deleteAfterCommit) { composer.breakContext() }
        let started = stats == nil ? 0 : DispatchTime.now().uptimeNanoseconds
        let before = composer.presentation
        let result = time.map { composer.handle(key, at: $0) } ?? composer.handle(key)
        if let move = Self.listMove(key, vertical: composer.options.verticalList), let old = before.candidates,
           let new = composer.presentation.candidates, old.items.count == new.items.count, old.pageSize == new.pageSize,
           move.page ? old.page == new.page : old.selection == new.selection {
            listEdge?(move.direction)
        }
        let commit = preservingVisibleSuffix(result.commit, visible: before.text)
        render(commit: commit)
        if !composer.learningSuspended, (before.candidates != nil && composer.presentation.candidates == nil ||
            before.notice != composer.presentation.notice && composer.presentation.notice != nil || commit != nil) {
            NotificationCenter.default.post(name: .personalDataMayHaveChanged, object: nil)
        }
        let latency = stats == nil ? nil : DispatchTime.now().uptimeNanoseconds - started
        observe(.key(key, time: time), before: before, commit: commit, handled: result.handled, latency: latency)
        if !result.handled {
            // Keys the app receives extend or trim the text before the insertion point.  Others (arrows, shortcuts)
            // may move it; the next composition reads the app again, or the remembered text ages out.
            switch key {
            case .backspace: recentText = String(recentText.dropLast())
            case .space: recentText += " "
            case .enter, .shiftEnter: recentText += "\n"
            default: break
            }
            remember()
        }
        return result.handled
    }

    /// 改前一個字／改後一個字: with the list open, the list moves to the neighbouring character in the same mode.  A
    /// vertical list does it with ← →; a horizontal one uses ← → for the highlight, so the list is closed, the cursor
    /// moved and the list opened again.  With no list the key is ← or →: it moves the cursor, or goes to the app.
    private func neighbour(_ delta: Int, time: TimeInterval?) -> Bool {
        let arrow: KeyInput = delta < 0 ? .left : .right
        guard let list = composer.presentation.candidates, !composer.options.verticalList else { return handle(arrow, time: time) }
        let reopen: KeyInput = list.singleCharacter ? .shiftDown : list.tolerant == true ? .shiftUp : .down
        _ = composer.handle(.escape)
        _ = composer.handle(arrow)
        _ = composer.handle(reopen)
        render()
        return true
    }

    /// How `key` moves through an open list, for keys that stop at its ends: the direction (1 on, -1 back) and whether
    /// it turns pages (or moves the highlight); nil for other keys.  Space goes round to the first page, so it never stops.
    static func listMove(_ key: KeyInput, vertical: Bool) -> (direction: Int, page: Bool)? {
        switch key {
        case .tab, .pageDown: return (1, true)
        case .shiftTab, .pageUp: return (-1, true)
        case .down: return (1, !vertical)
        case .up: return (-1, !vertical)
        case .right where !vertical: return (1, false)
        case .left where !vertical: return (-1, false)
        default: return nil
        }
    }

    private static func startsComposition(_ key: KeyInput) -> Bool {
        switch key {
        case .char, .shiftedChar, .printed, .controlChar: return true
        default: return false
        }
    }

    /// Before a composition starts: the app's text before the insertion point, or what was committed in this app
    /// recently.  The composer continues its last words; the reranker reads all of it.
    private func prepareContext() {
        // Secure input (a password field, or an app protecting what is typed): no text is read or remembered, and
        // nothing is learned from this composition.
        secureComposition = secureInput()
        composer.learningSuspended = secureComposition
        if secureComposition {
            recentText = ""
            return
        }
        let time = now()
        if time.timeIntervalSince(recentTime) > Self.contextWindow { recentText = "" }
        guard usesPrecedingText else { return }
        if let text = client.textBeforeInsertion(maxLength: Self.contextRead) {
            recentText = text
            composer.setPrecedingText(text)
        } else if recentText.isEmpty, let saved = Self.appContext[client.bundleIdentifier],
                  time.timeIntervalSince(saved.time) <= Self.contextWindow {
            recentText = saved.text
        }
        recentTime = time
    }

    private func observe(_ input: CorrectionRecorder.Input, before: Presentation, commit: String?, handled: Bool,
                         latency: UInt64? = nil) {
        guard recorder != nil || stats != nil else { return }
        let secure = composer.learningSuspended || secureInput()
        recorder?.observe(input, before: before, after: composer.presentation, commit: commit, handled: handled,
                          options: composer.options, secure: secure, context: recentText)
        stats?.observe(input, before: before, after: composer.presentation, commit: commit, handled: handled,
                       options: composer.options, secure: secure, corrected: shownCorrected, latency: latency)
        if composer.presentation.isEmpty { shownCorrected = false }
    }

    /// Whether the system's secure event input is on; a closure so the self-test can switch it.
    var secureInput: () -> Bool = { IsSecureEventInputEnabled() }

    private func remember() {
        guard !secureInput() else { recentText = ""; return }
        recentText = String(recentText.suffix(Self.contextKept))
        recentTime = now()
        if usesPrecedingText { Self.appContext[client.bundleIdentifier] = (recentText, recentTime) }
    }

    func selectCandidate(at index: Int) {
        guard !rendering, let list = composer.presentation.candidates,
              list.items.indices.contains(index) else { return }
        let before = composer.presentation
        let result = composer.selectCandidate(at: index)
        let commit = preservingVisibleSuffix(result.commit, visible: before.text)
        render(commit: commit)
        if !composer.learningSuspended { NotificationCenter.default.post(name: .personalDataMayHaveChanged, object: nil) }
        observe(.click(index: index), before: before, commit: commit, handled: true)
    }

    /// The candidate window switches between every span from the target and its single character (Shift-↓).
    func toggleSingleCharacter() {
        guard !rendering, composer.presentation.candidates != nil else { return }
        _ = handle(.shiftDown)
    }

    /// The candidate window switches between words for the keys as typed and typing corrections.
    func toggleCorrections() {
        guard !rendering else { return }
        let before = composer.presentation
        composer.toggleCorrections()
        render()
        observe(.toggleCorrections, before: before, commit: nil, handled: true)
    }

    func selectZhuyin() {
        guard composer.presentation.zhuyinHint != nil else { return }
        _ = handle(.char("`"))
    }

    // Physical state rejects Chromium's synthetic Shift-down events; remembered state rejects duplicates.
    @discardableResult
    func flagsChanged(keyCode: UInt16, modifiers: NSEvent.ModifierFlags,
                      physicallyDown: Bool, timestamp: TimeInterval) -> Bool {
        guard !rendering else { return false }
        if keyCode == 57 {
            let on = modifiers.contains(.capsLock)
            // With 「Caps Lock 打英文」 off, Caps Lock neither sends the composition nor shows the badge.
            if disabledFeatures.contains(.capsLockEnglish) { capsLock = on; return false }
            if on != capsLock {
                capsLock = on
                commit()
                if !isEnglish, client.hasInsertionPoint { modeChanged?(on, client.caretRect(cursorIndex: 0)) }
            }
            return false
        }
        guard keyCode == 56 || keyCode == 60 else {
            shiftTap = nil
            return false
        }
        let claimsDown = modifiers.contains(.shift)
        // Releasing one Shift while the other remains down keeps the aggregate Shift flag set.
        if claimsDown && !physicallyDown && heldShifts.count > 1 {
            heldShifts.remove(keyCode)
            shiftTap = nil
            return false
        }
        guard claimsDown == physicallyDown else { return false }
        if physicallyDown {
            guard heldShifts.insert(keyCode).inserted else { return false }
            let allowed = shiftToggle == .both || (shiftToggle == .left && keyCode == 56)
                || (shiftToggle == .right && keyCode == 60)
            let otherModifiers = modifiers.intersection([.command, .control, .option, .capsLock, .function])
            shiftTap = allowed && heldShifts.count == 1 && otherModifiers.isEmpty ? (keyCode, timestamp) : nil
        } else {
            guard heldShifts.remove(keyCode) != nil else { return false }
            let tap = shiftTap
            shiftTap = nil
            if let tap, tap.key == keyCode, heldShifts.isEmpty,
               modifiers.intersection([.command, .control, .option, .capsLock, .function]).isEmpty,
               timestamp >= tap.time, timestamp - tap.time <= 0.35, client.hasInsertionPoint {
                commit()
                isEnglish.toggle()
                breakContext()
                modeChanged?(isEnglish, client.caretRect(cursorIndex: 0))
            }
        }
        return false
    }

    func endSession() {
        commit()
        heldShifts.removeAll()
        shiftTap = nil
        hidePanel?()
    }

    /// The insertion point may have moved (a click, switching to English): the next composition starts a sentence.
    /// The reranker keeps the recent text as loose context until the app is read again or it ages out.
    func breakContext() {
        composer.breakContext()
        recorder?.breakContext()
    }

    func commit() {
        guard !rendering else { return }
        let before = composer.presentation
        let text = composer.commitAll()
        let commit = preservingVisibleSuffix(text, visible: before.text)
        render(commit: commit)
        if commit != nil, !composer.learningSuspended { NotificationCenter.default.post(name: .personalDataMayHaveChanged, object: nil) }
        observe(.commit, before: before, commit: commit, handled: true)
    }

    private func preservingVisibleSuffix(_ commit: String?, visible: String) -> String? {
        guard let commit else { return nil }
        // A finalized syllable may change spelling, but a shortened prefix must not discard unfinished input.
        return composer.presentation.isEmpty && visible.hasPrefix(commit) ? visible : commit
    }

    func render(commit: String? = nil) {
        guard !rendering else { return }
        rendering = true
        defer { rendering = false }
        // A head the composer committed by itself comes back as (the start of) `commit`: from here it is recentText.
        sentDuringKey = ""
        if let commit, !commit.isEmpty {
            client.insertText(commit, replacementRange: insertionRange)
            recentText += commit
            remember()
            lastMark = nil // insertText replaces the old marked range, even if the next string is identical.
        }

        let p = composer.presentation
        let length = p.text.utf16.count
        guard (0...length).contains(p.cursor),
              p.clauses.allSatisfy({ $0.lowerBound >= 0 && $0.upperBound <= length }),
              p.focusedClause.map({ p.clauses.indices.contains($0) }) ?? true else {
            let recovered = composer.commitAll()
            composer.reset()
            let text = recovered.isEmpty ? p.text : recovered
            if !text.isEmpty { client.insertText(text, replacementRange: insertionRange) }
            else { clearMark() }
            lastMark = nil
            hidePanel?()
            return
        }

        let popup = popupCompositionApps.contains(client.bundleIdentifier)
        if popup || p.text.isEmpty {
            clearMark()
        } else {
            let mark = Mark(text: p.text, cursor: p.cursor, clauses: p.clauses, focus: p.focusedClause,
                            target: p.targetClause)
            if mark != lastMark {
                let text = NSMutableAttributedString(string: p.text)
                for (index, range) in p.clauses.enumerated() {
                    var attributes: [NSAttributedString.Key: Any] = [
                        .underlineStyle: NSUnderlineStyle.single.rawValue,
                        .markedClauseSegment: index,
                    ]
                    if p.focusedClause == index {
                        // The part the candidate window replaces: a thick accent underline on a light accent wash,
                        // so it is clear which side of the caret is being changed.
                        attributes[.underlineStyle] = NSUnderlineStyle.thick.rawValue
                        attributes[.underlineColor] = NSColor.controlAccentColor
                        attributes[.backgroundColor] = NSColor.controlAccentColor.withAlphaComponent(0.18)
                    } else if p.targetClause == index {
                        // The character ↓ would change, with the cursor inside the composition: the same accent
                        // underline without the wash, so the side of the cursor is visible before the list opens.
                        attributes[.underlineStyle] = NSUnderlineStyle.thick.rawValue
                        attributes[.underlineColor] = NSColor.controlAccentColor
                    }
                    text.addAttributes(attributes, range: NSRange(location: range.lowerBound, length: range.count))
                }
                // While choosing, the replaced part is the marked text's selection; clients that draw their own
                // composition (web engines) show the selected clause with a thick underline.
                let selection = p.focusedClause.flatMap { p.clauses.indices.contains($0) ? p.clauses[$0] : nil }
                    .map { NSRange(location: $0.lowerBound, length: $0.count) }
                    ?? NSRange(location: p.cursor, length: 0)
                client.setMarkedText(text, selectionRange: selection, replacementRange: insertionRange)
                lastMark = mark
            }
        }
        if !p.text.isEmpty, p.candidates != nil || popup || p.notice != nil {
            showPanel?(p.candidates, popup ? p.text : nil, client.caretRect(cursorIndex: p.cursor), p.notice, p.zhuyinHint)
        } else {
            hidePanel?()
        }
    }

    private func clearMark() {
        guard lastMark != nil else { return }
        client.setMarkedText(NSAttributedString(string: ""), selectionRange: NSRange(location: 0, length: 0),
                             replacementRange: insertionRange)
        lastMark = nil
    }
}

extension Notification.Name {
    static let personalDataMayHaveChanged = Notification.Name("ZhuyinTypePersonalDataMayHaveChanged")
}
