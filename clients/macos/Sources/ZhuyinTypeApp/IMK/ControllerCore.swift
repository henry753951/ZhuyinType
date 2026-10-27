import AppKit
import Carbon.HIToolbox
import ZhuyinCore
import ZhuyinML

protocol TextClient: AnyObject {
    func insertText(_ text: String, replacementRange: NSRange)
    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange)
    var markedRange: NSRange { get }
    var selectedRange: NSRange { get }
    var bundleIdentifier: String { get }
    func caretRect(cursorIndex: Int) -> NSRect
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
        if let characterModel {
            composer.characterWeight = SyllableScorer.appWeight
            composer.characterScorer = { [weak self] syllables, candidates in
                characterModel.scores(context: self?.recentText ?? "", syllables: syllables.map {
                    SyllableScorer.Syllable(base: $0.base, tone: $0.tone, costBucket: $0.costBucket, implicit: $0.implicit)
                }, candidates: candidates)
            }
        }
        if let reranker {
            composer.rerankingCount = reranker.candidateCount
            composer.reranker = { [weak self] in reranker.choose($0, context: self?.recentText ?? "") }
            composer.candidateScorer = { [weak self] before, candidates in
                reranker.candidateScores(context: (self?.recentText ?? "") + before, candidates: candidates)
            }
        }
    }

    private var capsLock = false
    /// Rebindable shortcuts, by key.
    var shortcuts: [Shortcut: ShortcutCommand] = Dictionary(uniqueKeysWithValues: ShortcutCommand.defaults.map { ($1, $0) })

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
        // Caps Lock types English, every key of it, until it is released; what was being composed is kept.
        if modifiers.contains(.capsLock) {
            commit()
            breakContext()
            return false
        }
        if let shortcut = Shortcut(keyCode: keyCode, modifiers: modifiers), let command = shortcuts[shortcut] {
            return handle(command.input, time: time)
        }
        // A command's input comes only through its shortcut; its old default key, now unbound, is the app's.
        guard let key = KeyMapping.input(keyCode: keyCode, modifiers: modifiers),
              !ShortcutCommand.commandInputs.contains(key) else {
            commit()
            breakContext()
            return false
        }
        return handle(key, time: time)
    }

    @discardableResult
    func handle(_ key: KeyInput, time: TimeInterval? = nil) -> Bool {
        guard !rendering else { return false }
        if !composer.isComposing, Self.startsComposition(key) { prepareContext() }
        let visible = composer.presentation.text
        let result = time.map { composer.handle(key, at: $0) } ?? composer.handle(key)
        render(commit: preservingVisibleSuffix(result.commit, visible: visible))
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
        composer.learningSuspended = secureInput()
        if composer.learningSuspended {
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
        let visible = composer.presentation.text
        let result = composer.selectCandidate(at: index)
        render(commit: preservingVisibleSuffix(result.commit, visible: visible))
    }

    /// The candidate window switches between every span from the target and its single character (Shift-↓).
    func toggleSingleCharacter() {
        guard !rendering, composer.presentation.candidates != nil else { return }
        _ = handle(.shiftDown)
    }

    /// The candidate window switches between words for the keys as typed and typing corrections.
    func toggleCorrections() {
        guard !rendering else { return }
        composer.toggleCorrections()
        render()
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
            if on != capsLock {
                capsLock = on
                commit()
                if !isEnglish { modeChanged?(on, client.caretRect(cursorIndex: 0)) }
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
               timestamp >= tap.time, timestamp - tap.time <= 0.35 {
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
    }

    func commit() {
        guard !rendering else { return }
        let visible = composer.presentation.text
        let text = composer.commitAll()
        render(commit: preservingVisibleSuffix(text, visible: visible))
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
