import AppKit
import ZhuyinCore
import ZhuyinML

protocol TextClient: AnyObject {
    func insertText(_ text: String, replacementRange: NSRange)
    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange)
    var markedRange: NSRange { get }
    var selectedRange: NSRange { get }
    var bundleIdentifier: String { get }
    func caretRect(cursorIndex: Int) -> NSRect
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
    }
    private var lastMark: Mark?
    private var heldShifts: Set<UInt16> = []
    private var shiftTap: (key: UInt16, time: TimeInterval)?
    private var rendering = false
    private let insertionRange = NSRange(location: NSNotFound, length: 0)

    /// The last characters committed into this client: the context the reranker reads.
    private(set) var recentText = ""

    init(engine: Engine, client: TextClient, options: ComposerOptions, isEnglish: Bool,
         reranker: NeuralReranker? = nil) {
        self.composer = Composer(engine: engine, options: options)
        self.client = client
        self.isEnglish = isEnglish
        if let reranker {
            composer.rerankingCount = reranker.candidateCount
            composer.reranker = { [weak self] in reranker.choose($0, context: self?.recentText ?? "") }
            composer.candidateScorer = { [weak self] before, candidates in
                reranker.candidateScores(context: (self?.recentText ?? "") + before, candidates: candidates)
            }
        }
    }

    private var capsLock = false

    func update(options: ComposerOptions, shiftToggle: ShiftToggle, popupCompositionApps: [String]) {
        composer.options = options
        self.shiftToggle = shiftToggle
        self.popupCompositionApps = popupCompositionApps
        shiftTap = nil
        render()
    }

    @discardableResult
    func keyDown(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool {
        shiftTap = nil
        guard !rendering, !isEnglish else { return false }
        // Caps Lock types English, every key of it, until it is released; what was being composed is kept.
        if modifiers.contains(.capsLock) {
            commit()
            breakContext()
            return false
        }
        guard let key = KeyMapping.input(keyCode: keyCode, modifiers: modifiers) else {
            commit()
            breakContext()
            return false
        }
        return handle(key)
    }

    @discardableResult
    func handle(_ key: KeyInput) -> Bool {
        guard !rendering else { return false }
        let visible = composer.presentation.text
        let result = composer.handle(key)
        render(commit: preservingVisibleSuffix(result.commit, visible: visible))
        // Backspace passed to the app deletes the last committed character; other keys may move away from it.
        if !result.handled { recentText = key == .backspace ? String(recentText.dropLast()) : "" }
        return result.handled
    }

    func selectCandidate(at index: Int) {
        guard !rendering, let list = composer.presentation.candidates,
              list.items.indices.contains(index) else { return }
        let visible = composer.presentation.text
        let result = composer.selectCandidate(at: index)
        render(commit: preservingVisibleSuffix(result.commit, visible: visible))
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

    /// The text before the insertion point is no longer what this controller committed (a click, a shortcut, a key
    /// passed to the app): the reranker and the next composition stop reading it.
    func breakContext() {
        recentText = ""
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
            recentText = String((recentText + commit).suffix(32))
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
            let mark = Mark(text: p.text, cursor: p.cursor, clauses: p.clauses, focus: p.focusedClause)
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
