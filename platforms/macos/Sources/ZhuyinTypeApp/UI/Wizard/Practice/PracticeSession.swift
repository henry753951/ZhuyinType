import AppKit
import ZhuyinEngine

/// The practice field's text, as `ControllerCore` sees an app's text field: sent text with the composition after it
/// (the insertion point is always at the end), and the same calls the input method makes through InputMethodKit.
final class PracticeTextClient: TextClient {
    /// Never one of the user's apps, so the per-app context and 「在候選字視窗顯示組字的 App」 never apply.
    let bundleIdentifier = "org.zhuyintype.practice"
    private(set) var text = ""
    private(set) var marked = NSAttributedString()
    /// The selection inside the composition, as the core set it.
    private(set) var markedSelection = NSRange(location: 0, length: 0)
    /// Where the composition starts, in the field's view (set by the view).
    var compositionRect: (() -> NSRect)?
    /// After every change of the text or the composition.
    var changed: (() -> Void)?
    /// The longest text kept; the start is dropped beyond it, as a single-line field scrolls.
    static let maxLength = 400

    private var end: Int { text.utf16.count }
    var markedRange: NSRange { marked.length == 0 ? NSRange(location: NSNotFound, length: 0) : NSRange(location: end, length: marked.length) }
    var selectedRange: NSRange {
        marked.length == 0 ? NSRange(location: end, length: 0)
            : NSRange(location: end + markedSelection.location, length: markedSelection.length)
    }
    var hasInsertionPoint: Bool { true }

    func insertText(_ string: String, replacementRange: NSRange) {
        marked = NSAttributedString()
        markedSelection = NSRange(location: 0, length: 0)
        append(string)
    }

    func setMarkedText(_ string: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) {
        marked = NSAttributedString(attributedString: string)
        markedSelection = selectionRange
        changed?()
    }

    func caretRect(cursorIndex: Int) -> NSRect { compositionRect?() ?? .zero }

    func textBeforeInsertion(maxLength: Int) -> String? {
        var result = ""
        var length = 0
        for character in text.reversed() {
            length += character.utf16.count
            guard length <= maxLength else { break }
            result.insert(character, at: result.startIndex)
        }
        return result
    }

    // The field's own editing, for keys the input method gives to the app.

    func append(_ string: String) {
        text += string
        if text.count > Self.maxLength { text = String(text.suffix(Self.maxLength)) }
        changed?()
    }

    func deleteBackward() {
        guard !text.isEmpty else { return }
        text.removeLast()
        changed?()
    }

    /// Starts over with `initial` and no composition.
    func reset(to initial: String) {
        text = initial
        marked = NSAttributedString()
        markedSelection = NSRange(location: 0, length: 0)
        changed?()
    }
}

/// What the field's candidate window shows: the arguments the core passes to `showPanel`.
struct PracticePanel: Equatable {
    var list: CandidateList?
    var preview: String?
    var notice: String?
    var zhuyinHint: String?
    var keys: [Character]
}

/// The practice field without its view: a `ControllerCore` of its own on a `PracticeTextClient`, with the draft
/// settings and the practice data, and the task's goals checked after every key.  Keys come from the field's view
/// (or the self-tests) as the input method receives them; keys the core leaves to the app are typed into the field as
/// a text field would.
final class PracticeSession {
    let client = PracticeTextClient()
    /// The draft settings, in memory only.
    let preferences = Preferences(defaults: MemorySettingsStorage())
    private(set) var settings: SettingsSnapshot
    private(set) var task: PracticeTask
    /// Text already in the field when the task starts (the 前文 lesson).
    private(set) var initialText: String
    /// What the field reports for the task's check.
    private(set) var outcome = PracticeOutcome()
    /// The task's check passed; stays true until the task changes.
    private(set) var isComplete = false
    /// User phrases added while practising, with their readings.
    private(set) var addedPhrases: [(word: String, reading: String)] = []
    private var knownPhrases: Set<String> = []
    private(set) var core: ControllerCore?
    private(set) var data: PracticeUserData?
    /// The candidate window, as the core last asked for it (nil: hidden).
    private(set) var panel: PracticePanel?
    private(set) var isEnglish = false

    // For the view.
    var panelChanged: ((PracticePanel?) -> Void)?
    var textChanged: (() -> Void)?
    var modeChanged: ((Bool) -> Void)?
    var listEdge: ((Int) -> Void)?
    // For the wizard.
    var onProgress: ((PracticeProgress) -> Void)?
    var onComplete: (() -> Void)?

    init(settings: SettingsSnapshot, task: PracticeTask, initialText: String = "") {
        self.settings = settings
        self.task = task
        self.initialText = initialText
        SettingsSnapshot(preferences).changes(to: settings).apply(to: preferences)
        client.reset(to: initialText)
        client.changed = { [weak self] in self?.textChanged?() }
    }

    var isReady: Bool { core != nil }
    var progress: PracticeProgress { PracticeProgress(outcome: outcome, isComplete: isComplete, userPhrases: addedPhrases) }
    var presentation: Presentation { core?.composer.presentation ?? .empty }

    /// Starts with the practice data: builds the core.
    func attach(_ data: PracticeUserData) {
        guard core == nil, let engine = data.engine else { return }
        self.data = data
        knownPhrases = Set(engine.userPhrases().map { "\($0.reading)\t\($0.word)" })
        let core = ControllerCore(engine: engine, client: client, options: preferences.composerOptions,
                                  isEnglish: preferences.startInEnglish, reranker: data.reranker,
                                  characterModel: data.characterModel)
        // The field is not the system's text input: another window's password field must not pause it, and it never
        // reads the system's secure input state (as in the self-tests).
        core.secureInput = { false }
        core.showPanel = { [weak self] list, preview, _, notice, zhuyinHint in
            guard let self else { return }
            self.setPanel(PracticePanel(list: list, preview: preview, notice: notice, zhuyinHint: zhuyinHint,
                                        keys: self.core?.composer.options.selectionKeys ?? []))
        }
        core.hidePanel = { [weak self] in self?.setPanel(nil) }
        core.listEdge = { [weak self] in self?.listEdge?($0) }
        core.modeChanged = { [weak self] english, _ in
            guard let self else { return }
            self.isEnglish = self.core?.isEnglish ?? english
            self.pendingEvents.append(.switchedMode)
            self.modeChanged?(english)
        }
        self.core = core
        isEnglish = core.isEnglish
        refresh()
        check([])
    }

    /// New draft settings, in effect at once, even in the middle of a composition.
    func apply(_ settings: SettingsSnapshot) {
        guard settings != self.settings else { return }
        SettingsSnapshot(preferences).changes(to: settings).apply(to: preferences)
        self.settings = settings
        let open = presentation.candidates
        refresh()
        // New options close the list (as in the input method); in the wizard the point is to compare, so the same
        // kind of list opens again with the new settings, at its first page.  Not a step of the task.
        if let open, let core, presentation.candidates == nil, core.composer.isComposing {
            _ = core.handle(open.singleCharacter ? .shiftDown : open.tolerant == true ? .shiftUp : .down)
        }
    }

    /// A new task: the field starts over with `initialText`.
    func setTask(_ task: PracticeTask, initialText: String = "") {
        core?.endSession()
        self.task = task
        self.initialText = initialText
        outcome = PracticeOutcome()
        isComplete = false
        client.reset(to: initialText)
        core?.breakContext()
        check([])
        onProgress?(progress)
    }

    /// What `ZhuyinInputController.refreshPreferences` does for the input method, from the draft settings, with the
    /// practice data's log and statistics.
    private func refresh() {
        guard let core, let data else { return }
        let p = preferences
        core.usesPrecedingText = p.usesPrecedingText
        core.shortcuts = Dictionary(p.activeBindings.flatMap { command, list in list.map { ($0, command) } },
                                    uniquingKeysWith: { first, _ in first })
        core.disabledFeatures = p.disabledKeyFeatures
        core.stats = p.recordsUsageStats ? core.stats ?? UsageStatsObserver(store: data.stats) : nil
        data.engine?.fuzzySounds = p.fuzzySounds
        core.update(options: p.composerOptions, shiftToggle: p.shiftToggle, popupCompositionApps: p.popupCompositionApps)
        if p.recordsCorrections {
            let recorder = core.recorder ?? CorrectionRecorder { [weak data] in data?.corrections.append($0) }
            recorder.recordsContext = p.recordsCorrectionContext
            recorder.settings = p.correctionSettings
            core.recorder = recorder
        } else {
            core.recorder = nil
        }
        // The window's look may have changed with the list open.
        if let panel { panelChanged?(panel) }
    }

    private func setPanel(_ new: PracticePanel?) {
        panel = new
        panelChanged?(new)
    }

    // MARK: Keys

    /// A key, as the input method receives it; keys the core leaves to the app are typed into the field.  Returns
    /// false for keys neither uses (Tab with nothing composed, ⌘ shortcuts), which go on to the window.
    @discardableResult
    func keyDown(keyCode: UInt16, modifiers: NSEvent.ModifierFlags, characters: String?, time: TimeInterval) -> Bool {
        guard let core else {
            // The engine is still loading: text keys are held back rather than typed as Latin letters.
            return modifiers.isDisjoint(with: [.command]) && Self.printable(characters) != nil
        }
        let input = self.input(keyCode: keyCode, modifiers: modifiers)
        let handled = perform(input) { core.keyDown(keyCode: keyCode, modifiers: modifiers, time: time) }
        if handled { return true }
        return typeIntoField(keyCode: keyCode, modifiers: modifiers, characters: characters)
    }

    /// A modifier key changed (Shift taps switch 中/英, Caps Lock).  `physicallyDown`: the key itself is down.
    func flagsChanged(keyCode: UInt16, modifiers: NSEvent.ModifierFlags, physicallyDown: Bool, time: TimeInterval) {
        guard let core else { return }
        perform(nil) { core.flagsChanged(keyCode: keyCode, modifiers: modifiers, physicallyDown: physicallyDown, timestamp: time) }
    }

    /// A click on a candidate.
    func selectCandidate(at index: Int) {
        guard let core else { return }
        perform(nil, click: index) { core.selectCandidate(at: index); return true }
    }

    /// The ‹ › buttons of the candidate window.
    func page(_ direction: Int) {
        guard let core else { return }
        perform(direction < 0 ? .pageUp : .pageDown) { core.handle(direction < 0 ? .pageUp : .pageDown) }
    }

    func toggleSingleCharacter() { guard let core else { return }; perform(.shiftDown) { core.toggleSingleCharacter(); return true } }
    func toggleCorrections() { guard let core else { return }; perform(.shiftUp) { core.toggleCorrections(); return true } }
    func selectZhuyin() {
        guard let core, preferences.isEnabled(.zhuyinRowClick) else { return }
        perform(.char("`")) { core.selectZhuyin(); return true }
    }

    /// The field lost focus or its window went to the background: the composition is sent, as in an app.
    func endSession() {
        guard let core else { return }
        perform(nil) { core.endSession(); core.breakContext(); return true }
    }

    /// The input a key is for the core, to tell a pick from other keys (the core maps it the same way).
    private func input(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> KeyInput? {
        if let shortcut = Shortcut(keyCode: keyCode, modifiers: modifiers), let command = core?.shortcuts[shortcut] {
            return command.input
        }
        return KeyMapping.input(keyCode: keyCode, modifiers: modifiers)
    }

    /// Keys the input method gives to the app, in a single-line field whose insertion point stays at the end.
    private func typeIntoField(keyCode: UInt16, modifiers: NSEvent.ModifierFlags, characters: String?) -> Bool {
        if modifiers.contains(.command) { return false }
        switch keyCode {
        case 51:
            client.deleteBackward()
        case 48:
            return false // Tab moves to the next control.
        case 36, 76, 53, 115, 116, 119, 121, 123, 124, 125, 126:
            break // Return, Esc and moving keys do nothing in a single line with the insertion point at the end.
        default:
            guard let text = Self.printable(characters) else { return false }
            client.append(text)
        }
        check([])
        return true
    }

    /// The text a key types, without control characters and function keys.
    static func printable(_ characters: String?) -> String? {
        guard let characters, !characters.isEmpty,
              characters.unicodeScalars.allSatisfy({ $0.value >= 0x20 && $0.value != 0x7f && !(0xF700...0xF8FF).contains($0.value) })
        else { return nil }
        return characters
    }

    // MARK: Goals

    private var pendingEvents: [PracticeEvent] = []

    /// Runs `body` (a call into the core) and records what it did: a list opened, a page turned, a candidate picked.
    @discardableResult
    private func perform(_ input: KeyInput?, click: Int? = nil, _ body: () -> Bool) -> Bool {
        let before = presentation
        let handled = body()
        let after = presentation
        var events: [PracticeEvent] = []
        if after.candidates?.tolerant == true, before.candidates?.tolerant != true {
            events.append(.openedCorrections)
        }
        if let old = before.candidates {
            let new = after.candidates
            if let index = pickedIndex(input: input, click: click, list: old),
               new == nil || new?.items != old.items {
                let replaced = before.focusedClause.flatMap { Self.clauseText(before, $0) } ?? ""
                events.append(.picked(text: old.items[index].text, replaced: replaced, corrections: old.tolerant == true))
            } else if let new, new.items == old.items, new.pageSize == old.pageSize, new.page != old.page {
                events.append(.turnedPage)
            }
        } else if after.candidates != nil {
            events.append(.openedList)
        }
        check(events)
        return handled
    }

    private func pickedIndex(input: KeyInput?, click: Int?, list: CandidateList) -> Int? {
        if let click { return list.items.indices.contains(click) ? click : nil }
        switch input {
        case .enter?:
            return list.items.indices.contains(list.selection) ? list.selection : nil
        case .char(let key)?:
            guard let position = (core?.composer.options.selectionKeys ?? []).firstIndex(of: key),
                  position < list.pageRange.count else { return nil }
            return list.pageRange.lowerBound + position
        default:
            return nil
        }
    }

    private static func clauseText(_ p: Presentation, _ clause: Int) -> String? {
        guard p.clauses.indices.contains(clause) else { return nil }
        let utf16 = Array(p.text.utf16)
        let range = p.clauses[clause]
        guard range.lowerBound >= 0, range.upperBound <= utf16.count else { return nil }
        return String(decoding: utf16[range], as: UTF16.self)
    }

    /// Brings the outcome up to date after a change (`events`: what the change did) and checks the task.
    private func check(_ events: [PracticeEvent]) {
        let all = pendingEvents + events
        pendingEvents = []
        var next = outcome
        next.sent = client.text
        next.text = client.text + client.marked.string
        for event in all {
            switch event {
            case .openedList: next.events.append(PracticeEventName.open)
            case .turnedPage: next.events.append(PracticeEventName.page)
            case .openedCorrections: next.events.append(PracticeEventName.corrections)
            case .switchedMode: next.events.append(PracticeEventName.mode)
            case .picked(let text, let replaced, let corrections):
                next.events.append(PracticeEventName.pick)
                next.picks.append(text)
                if text != replaced {
                    next.events.append(PracticeEventName.correct)
                    next.corrected.append(text)
                }
                if corrections { next.correctionPicks += 1 }
            }
        }
        // A user phrase added by this change (⇧← ⇧→ and Return).
        if let engine = data?.engine {
            for phrase in engine.userPhrases() where !knownPhrases.contains("\(phrase.reading)\t\(phrase.word)") {
                knownPhrases.insert("\(phrase.reading)\t\(phrase.word)")
                addedPhrases.append((phrase.word, phrase.reading))
                next.phrases.append(phrase.word)
                next.events.append(PracticeEventName.phrase)
            }
        }
        let changed = next.text != outcome.text || next.sent != outcome.sent || next.events != outcome.events
        outcome = next
        let done = !isComplete && task.isDone(outcome)
        if done { isComplete = true }
        if changed || done { onProgress?(progress) }
        if done { onComplete?() }
    }
}

/// What a change in the field did, for the outcome.
enum PracticeEvent: Equatable {
    case openedList
    case turnedPage
    case openedCorrections
    case picked(text: String, replaced: String, corrections: Bool)
    case switchedMode
}

/// Synthetic key events for the self-tests and the UI previews: made in this process and handed to the field's view,
/// never posted to the system.
enum PracticeKeys {
    /// The key of the US layout that types `character` (KeyMapping's table), and whether Shift is needed.
    static func key(for character: Character) -> (code: UInt16, shift: Bool)? {
        if character == " " { return (49, false) }
        if let code = KeyMapping.printable.first(where: { $0.value == character })?.key { return (code, false) }
        if let lower = KeyMapping.shifted.first(where: { $0.value == character })?.key,
           let code = KeyMapping.printable.first(where: { $0.value == lower })?.key { return (code, true) }
        if character.isUppercase, let code = KeyMapping.printable.first(where: { $0.value == Character(character.lowercased()) })?.key {
            return (code, true)
        }
        return nil
    }

    private static var clock: TimeInterval = 1000

    static func keyDown(_ keyCode: UInt16, _ modifiers: NSEvent.ModifierFlags = [], characters: String = "") -> NSEvent {
        clock += 0.12
        return NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: clock, windowNumber: 0,
                                context: nil, characters: characters, charactersIgnoringModifiers: characters,
                                isARepeat: false, keyCode: keyCode)!
    }

    static func keyDown(_ character: Character, _ extra: NSEvent.ModifierFlags = []) -> NSEvent {
        guard let key = key(for: character) else { preconditionFailure("no key for \(character)") }
        return keyDown(key.code, key.shift ? extra.union(.shift) : extra, characters: String(character))
    }

    /// Named keys: Return, Esc, ⌫, Tab, the arrows and Page Down.
    static func keyDown(named name: String, _ modifiers: NSEvent.ModifierFlags = []) -> NSEvent {
        let codes: [String: UInt16] = ["return": 36, "esc": 53, "delete": 51, "tab": 48, "space": 49, "left": 123,
                                       "right": 124, "down": 125, "up": 126, "pagedown": 121, "pageup": 116]
        guard let code = codes[name] else { preconditionFailure("no key named \(name)") }
        let arrows: NSEvent.ModifierFlags = (115...126).contains(code) ? [.function, .numericPad] : []
        let characters = name == "space" ? " " : name == "return" ? "\r" : ""
        return keyDown(code, modifiers.union(arrows), characters: characters)
    }

    /// A Shift (56 left, 60 right) or Caps Lock (57) change: `down` with the device's own bit for the key, as the
    /// window server sends it.
    static func flags(_ keyCode: UInt16, down: Bool, capsLock: Bool = false) -> NSEvent {
        clock += 0.08
        var raw: UInt64 = 0
        if keyCode == 57 {
            if capsLock { raw |= CGEventFlags.maskAlphaShift.rawValue }
        } else if down {
            raw |= CGEventFlags.maskShift.rawValue | (keyCode == 56 ? 0x02 : 0x04)
        }
        let event = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: down)!
        event.type = .flagsChanged
        event.flags = CGEventFlags(rawValue: raw)
        event.timestamp = UInt64(clock * 1_000_000_000)
        return NSEvent(cgEvent: event)!
    }
}
