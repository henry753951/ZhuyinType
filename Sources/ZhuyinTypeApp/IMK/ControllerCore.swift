import AppKit
import ZhuyinCore

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
    var showPanel: ((CandidateList?, String?, NSRect) -> Void)?
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

    init(engine: Engine, client: TextClient, options: ComposerOptions, isEnglish: Bool) {
        self.composer = Composer(engine: engine, options: options)
        self.client = client
        self.isEnglish = isEnglish
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
        // Caps Lock types English, every key of it, until it is released.
        guard !rendering, !isEnglish, !modifiers.contains(.capsLock) else { return false }
        guard let key = KeyMapping.input(keyCode: keyCode, modifiers: modifiers) else {
            commit()
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
        return result.handled
    }

    func selectCandidate(at index: Int) {
        guard !rendering, let list = composer.presentation.candidates,
              list.items.indices.contains(index) else { return }
        let visible = composer.presentation.text
        let result = composer.selectCandidate(at: index)
        render(commit: preservingVisibleSuffix(result.commit, visible: visible))
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
                    text.addAttributes([
                        .underlineStyle: (p.focusedClause == index ? NSUnderlineStyle.thick : .single).rawValue,
                        .markedClauseSegment: index,
                    ], range: NSRange(location: range.lowerBound, length: range.count))
                }
                client.setMarkedText(text, selectionRange: NSRange(location: p.cursor, length: 0),
                                     replacementRange: insertionRange)
                lastMark = mark
            }
        }
        if !p.text.isEmpty, p.candidates != nil || popup {
            showPanel?(p.candidates, popup ? p.text : nil, client.caretRect(cursorIndex: p.cursor))
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
