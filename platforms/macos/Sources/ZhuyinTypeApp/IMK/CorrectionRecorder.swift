// The correction log (「記錄改字」, docs/product/DESIGN.md §2.7): when the user turns it on, every composition in which
// they corrected the engine becomes one JSON line of ~/Library/Application Support/ZhuyinType/corrections.jsonl, so
// the engine can be improved from real typing (ml: `zt eval corrections`, `zt data corrections`).
//
// `CorrectionRecorder` only observes a `ControllerCore`: the core tells it each key (with its key-down time), click
// and commit together with the presentation before and after, and it never changes what the core does.
// `CorrectionLog` appends the records off the main thread, as the engine writes personal data.
import AppKit
import ZhuyinEngine

/// Engine settings that change what the keys produce, recorded with each composition.
struct CorrectionSettings: Encodable, Equatable {
    var typoTolerance = "standard"
    /// zh, ch, sh, en, an: the fuzzy pairs that are on (zhuyin-cli --fuzzy names).
    var fuzzySounds: [String] = []
    var punctuationKeys = "windows"
    var spaceAction = "commit"
    var characterForms = "common"
    var candidateAnchor = "after"
    var selectionKeys = "123456789"
    var verticalList = true
    var learning = true
    var precedingText = true

    init() {}

    init(options: ComposerOptions, fuzzy: FuzzySounds, precedingText: Bool) {
        typoTolerance = options.typoTolerance.rawValue
        fuzzySounds = [(FuzzySounds.zZh, "zh"), (.cCh, "ch"), (.sSh, "sh"), (.enEng, "en"), (.anAng, "an")]
            .filter { fuzzy.contains($0.0) }.map(\.1)
        punctuationKeys = options.punctuationKeys.rawValue
        spaceAction = options.spaceAction.rawValue
        characterForms = options.characterForms.rawValue
        candidateAnchor = options.candidateAnchor == .afterCursor ? "after" : "before"
        selectionKeys = String(options.selectionKeys)
        verticalList = options.verticalList
        learning = options.learningEnabled
        self.precedingText = precedingText
    }
}

/// One step of a composition.  `shown` is the text on screen after it; the other fields depend on `type`.
struct CorrectionEvent: Encodable {
    /// type, open_list, switch_list, close_list, pick, backspace, escape, literal_zhuyin, undo_literal, cursor,
    /// select, cancel_select, add_phrase, delete_after_commit, commit.
    var type: String
    /// Milliseconds since the first key of the record.
    var t: Int
    var shown: String
    var shownBefore: String?
    /// type: the keys (zhuyin-cli syntax, without delays).
    var keys: String?
    /// backspace, escape: what disappeared from the screen (converted text or Zhuyin symbols).
    var removed: String?
    /// backspace, escape: the typing keys behind what was removed, and the keys typed in their place; nil when they
    /// cannot be told (an edit inside the composition, or nothing typed after).
    var deletedKeys: String?
    var retypedKeys: String?
    /// backspace, escape: the text that took the place of what was removed, once retyped.
    var replacedBy: String?
    /// escape: "symbols" (the unfinished syllable) or "composition".
    var cleared: String?
    /// The edit happened with the cursor inside the composition, not at its end.
    var mid: Bool?
    /// open_list, switch_list, pick: normal, corrections, single or single_corrections.
    var list: String?
    /// open_list: the part the list changes.
    var target: String?
    /// pick: the text before and after, what was replaced and by what (at `start`, in Unicode scalars of `before`).
    var before: String?
    var after: String?
    var inserted: String?
    var start: Int?
    /// pick: 1 for the first item of the list.
    var rank: Int?
    /// pick: "key" or "click".
    var via: String?
    /// cursor: the cursor position after the moves, in Unicode scalars.
    var position: Int?
    /// select: the selected text; add_phrase: the phrase and whether it was added.
    var selected: String?
    var phrase: String?
    var added: Bool?
    /// commit: the committed text.
    var text: String?
    /// delete_after_commit: the committed characters deleted with Backspace within 30 seconds, and the composition
    /// they came from.
    var deleted: String?
    var previousCommitted: String?
    var previousTyping: String?
    var previousReplayable: Bool?
    var previousContext: String?

    init(type: String, t: Int, shown: String) {
        self.type = type
        self.t = t
        self.shown = shown
    }
}

/// One line of corrections.jsonl.
struct CorrectionRecord: Encodable {
    static let schemaVersion = 1
    var schema = CorrectionRecord.schemaVersion
    var id = UUID().uuidString
    /// The day only (yyyy-MM-dd, local time).
    var date: String
    var appVersion: String
    var settings: CorrectionSettings
    /// The text before the insertion point that the engine read, only with 「一併記錄游標前的文字」.
    var context: String?
    /// Every key and click, in zhuyin-cli batch syntax with `<dt:MS>` before each key after the first.
    var keys: String
    /// The keys that type the final composition, without list keys, picks and the corrected keys; append `<enter>`
    /// to replay it.
    var typing: String
    /// The typing replays the final composition: every edit happened at its end.
    var replayable: Bool
    /// The keys as first typed: `typing` with each Backspace or Esc correction undone (the deleted keys instead of
    /// the retyped ones).
    var originalTyping: String
    var originalReplayable: Bool
    /// The text on screen just before the first correction.
    var shown: String
    var committed: String
    /// The kinds of correction in this composition, in the order they first happened: pick, backspace, escape,
    /// literal_zhuyin, undo_literal, select, add_phrase, cursor_edit, delete_after_commit.
    var corrections: [String]
    var events: [CorrectionEvent]
}

final class CorrectionRecorder {
    /// What `ControllerCore` reports.
    enum Input {
        case key(KeyInput, time: TimeInterval?)
        case click(index: Int)
        /// The 修正 switch of the candidate window, as ⇧↑.
        case toggleCorrections
        /// A commit the core made itself (focus loss, a shortcut for the app, Caps Lock, Shift).
        case commit
    }

    /// Seconds after a commit in which deleting it with Backspace and typing again is a correction (as the engine's
    /// own rule), and in which typing after Esc cleared the composition continues the same record.
    static let window: TimeInterval = 30

    var recordsContext = false
    var settings = CorrectionSettings()
    var appVersion = CorrectionRecorder.bundleVersion
    /// Key-down times are seconds since the system started (`NSEvent.timestamp`); clicks and commits use the same clock.
    var clock: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    var today: () -> Date = Date.init
    private let sink: (CorrectionRecord) -> Void

    init(sink: @escaping (CorrectionRecord) -> Void) {
        self.sink = sink
    }

    static let bundleVersion: String = {
        let info = Bundle.main.infoDictionary
        guard let short = info?["CFBundleShortVersionString"] as? String else { return "dev" }
        return (info?["CFBundleVersion"] as? String).map { "\(short) (\($0))" } ?? short
    }()

    // MARK: State

    private struct Shape: Equatable {
        var converted: Int
        var pending: String
        init(_ text: String) {
            let scalars = Array(text.unicodeScalars)
            let count = scalars.reversed().prefix { CorrectionRecorder.isZhuyin($0) }.count
            converted = scalars.count - count
            pending = String(String.UnicodeScalarView(scalars.suffix(count)))
        }
    }

    private struct Key {
        var label: String
        var time: TimeInterval
    }

    /// Keys retyped after a Backspace or Esc correction, until the composition is as long as before it.
    private struct Retyping {
        var event: Int
        var target: Int
        var position: Int
        var keys: [Key] = []
    }

    private final class Session {
        let start: TimeInterval
        var context: String?
        var keys = ""
        var lastKey: TimeInterval?
        var typing: [Key] = []
        var original: [Key] = []
        /// Typing count and screen shape after each typing key: a deletion goes back to the latest same shape.
        var history: [(typed: Int, shape: Shape)] = [(0, Shape(""))]
        var tracking = true
        var replayable = true
        var originalReplayable = true
        var events: [CorrectionEvent] = []
        var corrections: [String] = []
        var shown: String?
        var retyping: Retyping?
        var selecting = false
        var literal: Int?
        var afterCommit: (deleted: String, previous: String)?
        /// What the composition already committed by itself when it grew past its limit; the record's `committed`
        /// and `shown` start with it, so `typing` still replays the whole record.
        var sent = ""

        init(start: TimeInterval) { self.start = start }
    }

    private struct LastCommit {
        var time: TimeInterval
        var text: String
        var committed: String
        var passed: Int
        var deleted = 0
        var typing: String
        var replayable: Bool
        var context: String?
    }

    private var session: Session?
    /// A composition Esc or Backspace emptied, kept for `window` in case the user types it again.
    private var suspended: (session: Session, time: TimeInterval)?
    private var lastCommit: LastCommit?

    // MARK: Observing

    /// The core lost track of the text before the insertion point (a click, a mode switch): no retype follows.
    func breakContext() {
        lastCommit = nil
        suspended = nil
    }

    func observe(_ input: Input, before: Presentation, after: Presentation, commit: String?, handled: Bool,
                 options: ComposerOptions, secure: Bool, context: @autoclosure () -> String) {
        let time: TimeInterval
        if case .key(_, let keyTime?) = input { time = keyTime } else { time = clock() }
        let committed = commit.flatMap { $0.isEmpty ? nil : $0 }
        if secure {
            session = nil
            suspended = nil
            lastCommit = nil
            return
        }
        if before.isEmpty && after.isEmpty && committed == nil {
            // A key for the app with nothing composed.
            guard case .key(let key, _) = input, !handled else { return }
            if key == .backspace { followDeletion(at: time) } else { lastCommit = nil }
            suspended = nil
            return
        }
        if session == nil {
            // A composition that started before recording was turned on is not recorded.
            guard before.isEmpty else { return }
            if let kept = suspended, time - kept.time < Self.window {
                session = kept.session
            } else {
                let fresh = Session(start: time)
                if recordsContext { fresh.context = context() }
                if let last = lastCommit, last.deleted > last.passed {
                    let deleted = String(last.committed.suffix(last.deleted - last.passed))
                    var event = CorrectionEvent(type: "delete_after_commit", t: 0, shown: before.text)
                    event.deleted = deleted
                    event.previousCommitted = last.committed
                    event.previousTyping = last.typing
                    event.previousReplayable = last.replayable
                    event.previousContext = last.context
                    fresh.events.append(event)
                    fresh.afterCommit = (deleted, last.committed)
                }
                session = fresh
            }
            suspended = nil
            lastCommit = nil
        }
        guard let s = session else { return }
        let label = keyLabel(input, list: before.candidates, options: options)
        if let last = s.lastKey { s.keys += "<dt:\(milliseconds(time - last))>" }
        s.keys += label ?? "<?>"
        s.lastKey = time
        let t = milliseconds(time - s.start)
        // A typing key that made the composition send its head first: the key is typing, and the record goes on.
        let carried = committed != nil && !after.isEmpty ? committed : nil
        if let carried {
            var whole = after
            whole.text = carried + after.text
            endRetyping(s, after: whole)
        }

        switch input {
        case .commit:
            break
        case .click(let index):
            pick(s, index: index, via: "click", before: before, after: after, t: t)
        case .toggleCorrections:
            if let list = after.candidates, list != before.candidates {
                var event = CorrectionEvent(type: "switch_list", t: t, shown: after.text)
                event.list = Self.kind(list)
                s.events.append(event)
            }
        case .key(let key, _):
            if let list = before.candidates, Self.isListKey(key, options: options) {
                listKey(s, key: key, list: list, before: before, after: after, options: options, t: t)
            } else {
                if before.candidates != nil {
                    s.events.append(CorrectionEvent(type: "close_list", t: t, shown: after.text))
                }
                composingKey(s, key: key, label: label, before: before, after: after,
                             committed: carried == nil ? committed : nil, handled: handled, t: t)
            }
        }
        if let carried {
            carry(s, committed: carried, after: after, t: t)
        } else if let committed {
            finish(s, committed: committed, input: input, handled: handled, time: time, t: t)
        } else if after.isEmpty, session === s, !s.sent.isEmpty {
            // Part of it was sent already: the record ends here, with what was sent.
            finish(s, committed: "", input: input, handled: handled, time: time, t: t)
        } else if after.isEmpty, session === s {
            // Emptied without a commit (Escape or Backspace): typing it again continues this record.
            s.literal = nil
            s.selecting = false
            session = nil
            suspended = (s, time)
        }
    }

    // MARK: Keys with the list open

    private func listKey(_ s: Session, key: KeyInput, list: CandidateList, before: Presentation, after: Presentation,
                         options: ComposerOptions, t: Int) {
        endRetyping(s, after: after)
        s.literal = nil
        if key == .char("`") {
            // The list's span as literal Zhuyin: depends on the list, so the typing no longer replays.
            if before != after {
                var event = CorrectionEvent(type: "literal_zhuyin", t: t, shown: after.text)
                event.shownBefore = before.text
                s.events.append(event)
                correction(s, "literal_zhuyin", shown: before.text)
                s.literal = s.events.count - 1
                s.replayable = false
                s.originalReplayable = false
                s.tracking = false
            }
            return
        }
        if after.candidates == nil {
            if case .char(let character) = key, let number = options.selectionKeys.firstIndex(of: character) {
                pick(s, index: list.page * list.pageSize + number, via: "key", before: before, after: after, t: t)
            } else if key == .enter {
                pick(s, index: list.selection, via: "key", before: before, after: after, t: t)
            } else {
                s.events.append(CorrectionEvent(type: "close_list", t: t, shown: after.text))
            }
        } else if let now = after.candidates, Self.kind(now) != Self.kind(list) {
            var event = CorrectionEvent(type: "switch_list", t: t, shown: after.text)
            event.list = Self.kind(now)
            s.events.append(event)
        }
    }

    private func pick(_ s: Session, index: Int, via: String, before: Presentation, after: Presentation, t: Int) {
        guard let list = before.candidates, after.candidates == nil else { return }
        endRetyping(s, after: after)
        s.literal = nil
        var event = CorrectionEvent(type: "pick", t: t, shown: after.text)
        let (start, removed, inserted) = Self.difference(before.text, after.text)
        event.before = before.text
        event.after = after.text
        event.start = start
        event.removed = removed
        event.inserted = inserted
        event.rank = index + 1
        event.list = Self.kind(list)
        event.via = via
        s.events.append(event)
        if before.text != after.text { correction(s, "pick", shown: before.text) }
    }

    // MARK: Keys with the list closed

    private func composingKey(_ s: Session, key: KeyInput, label: String?, before: Presentation, after: Presentation,
                              committed: String?, handled: Bool, t: Int) {
        let literal = s.literal
        s.literal = nil
        if s.selecting {
            switch key {
            case .shiftLeft where s.events.last?.type == "select", .shiftRight where s.events.last?.type == "select":
                s.events[s.events.count - 1].selected = Self.focused(after)
                s.events[s.events.count - 1].shown = after.text
                return
            case .escape:
                s.selecting = false
                s.events.append(CorrectionEvent(type: "cancel_select", t: t, shown: after.text))
                return
            case .enter where committed == nil:
                s.selecting = false
                var event = CorrectionEvent(type: "add_phrase", t: t, shown: after.text)
                event.phrase = s.events.last?.selected
                event.added = after.notice?.hasPrefix("已加入") == true
                s.events.append(event)
                if event.added == true { correction(s, "add_phrase", shown: before.text) }
                return
            default:
                s.selecting = false
            }
        }
        if committed != nil {
            // The key that sent the composition is not typing; a replay ends with <enter>.
            endRetyping(s, after: after)
            return
        }
        switch key {
        case .shiftLeft, .shiftRight:
            endRetyping(s, after: after)
            s.selecting = true
            var event = CorrectionEvent(type: "select", t: t, shown: after.text)
            event.shownBefore = before.text
            event.selected = Self.focused(after)
            s.events.append(event)
            correction(s, "select", shown: before.text)
            return
        case .left, .right, .home, .end:
            guard before.cursor != after.cursor else { break }
            endRetyping(s, after: after)
            let position = Self.scalarOffset(after.text, utf16: after.cursor)
            if s.events.last?.type == "cursor" {
                s.events[s.events.count - 1].position = position
                s.events[s.events.count - 1].t = t
            } else {
                var event = CorrectionEvent(type: "cursor", t: t, shown: after.text)
                event.position = position
                s.events.append(event)
            }
            return
        default:
            break
        }
        if before.candidates == nil, let list = after.candidates {
            // Opening the list may first finish the unfinished syllable: keep the key, closed again, as typing.
            endRetyping(s, after: after)
            if before.text != after.text, let label {
                appendTyping(s, Key(label: label + "<esc>", time: s.lastKey ?? 0), after: after)
            }
            var event = CorrectionEvent(type: "open_list", t: t, shown: after.text)
            event.list = Self.kind(list)
            event.target = Self.focused(after)
            s.events.append(event)
            return
        }
        let mid = !Self.atEnd(before) || !Self.atEnd(after)
        let content = Self.isContent(key)
        guard content || before.text != after.text else { return }
        if mid {
            if s.tracking { s.tracking = false }
            s.replayable = false
            s.originalReplayable = false
            if content { correction(s, "cursor_edit", shown: before.text) }
        }
        // Esc or ` right after ` restores the Zhuyin as it was.
        if let literal, key == .char("`") || key == .escape, before != after, !after.isEmpty,
           after.text == s.events[literal].shownBefore {
            var event = CorrectionEvent(type: "undo_literal", t: t, shown: after.text)
            event.shownBefore = before.text
            s.events.append(event)
            correction(s, "undo_literal", shown: before.text)
            appendTyping(s, Key(label: label ?? "<?>", time: s.lastKey ?? 0), after: after)
            return
        }
        switch key {
        case .char("`"):
            guard before != after else { return }
            endRetyping(s, after: after)
            var event = CorrectionEvent(type: "literal_zhuyin", t: t, shown: after.text)
            event.shownBefore = before.text
            event.mid = mid ? true : nil
            s.events.append(event)
            correction(s, "literal_zhuyin", shown: before.text)
            appendTyping(s, Key(label: "`", time: s.lastKey ?? 0), after: after)
            s.literal = s.events.count - 1
        case .backspace, .optionBackspace, .forwardDelete, .escape:
            deletion(s, key: key, label: label, before: before, after: after, mid: mid, t: t)
        default:
            typed(s, Key(label: label ?? "<?>", time: s.lastKey ?? 0), after: after, mid: mid, t: t)
            if label == nil {
                s.replayable = false
                s.originalReplayable = false
            }
        }
    }

    private func typed(_ s: Session, _ key: Key, after: Presentation, mid: Bool, t: Int) {
        if let last = s.events.last, last.type == "type", (last.mid ?? false) == mid {
            s.events[s.events.count - 1].keys = (last.keys ?? "") + key.label
            s.events[s.events.count - 1].shown = after.text
            s.events[s.events.count - 1].t = t
        } else {
            var event = CorrectionEvent(type: "type", t: t, shown: after.text)
            event.keys = key.label
            event.mid = mid ? true : nil
            s.events.append(event)
        }
        appendTyping(s, key, after: after)
        if let retyping = s.retyping, Shape(after.text).converted >= retyping.target { endRetyping(s, after: after) }
    }

    private func appendTyping(_ s: Session, _ key: Key, after: Presentation) {
        s.typing.append(key)
        if s.retyping != nil { s.retyping?.keys.append(key) } else { s.original.append(key) }
        if s.tracking { s.history.append((s.typing.count, Shape(after.text))) }
    }

    /// Backspace, ⌥⌫, ⌦ or Esc removed text or Zhuyin symbols.
    private func deletion(_ s: Session, key: KeyInput, label: String?, before: Presentation, after: Presentation,
                          mid: Bool, t: Int) {
        guard before != after else { return }
        let type = key == .escape ? "escape" : "backspace"
        let cleared = key == .escape ? (after.isEmpty ? "composition" : "symbols") : nil
        let removed = Self.difference(before.text, after.text).1
        let shape = Shape(after.text)
        // The typing keys behind what was removed: back to the last time the screen had the shape it has now.
        guard s.tracking, !mid, let index = s.history.lastIndex(where: { $0.shape == shape }),
              s.history[index].typed < s.typing.count else {
            endRetyping(s, after: after)
            let typed = Key(label: label ?? "<?>", time: s.lastKey ?? 0)
            s.typing.append(typed)
            s.original.append(typed)
            if s.tracking { s.history.append((s.typing.count, shape)) }
            var event = CorrectionEvent(type: type, t: t, shown: after.text)
            event.shownBefore = before.text
            event.removed = removed
            event.cleared = cleared
            event.mid = mid ? true : nil
            s.events.append(event)
            if mid { correction(s, "cursor_edit", shown: before.text) }
            return
        }
        let mark = s.history[index]
        let deleted = Array(s.typing[mark.typed...])
        s.typing.removeLast(s.typing.count - mark.typed)
        s.history.removeLast(s.history.count - index - 1)
        let deletedKeys = deleted.map(\.label).joined()
        if var retyping = s.retyping {
            let last = s.events.count - 1
            if retyping.keys.isEmpty, retyping.event == last, s.events[last].type == type {
                // Backspace held or pressed again: one deletion.
                s.events[last].deletedKeys = deletedKeys + (s.events[last].deletedKeys ?? "")
                s.events[last].removed = Self.difference(s.events[last].shownBefore ?? "", after.text).1
                s.events[last].shown = after.text
                s.events[last].t = t
                s.events[last].cleared = cleared
                retyping.position = shape.converted
                s.retyping = retyping
                return
            }
            if deleted.count <= retyping.keys.count {
                // A slip while retyping: the retyped keys lose it and the correction goes on.
                retyping.keys.removeLast(deleted.count)
                s.retyping = retyping
                var event = CorrectionEvent(type: type, t: t, shown: after.text)
                event.shownBefore = before.text
                event.removed = removed
                event.deletedKeys = deletedKeys
                event.cleared = cleared
                s.events.append(event)
                return
            }
            // Deleting past the retyped keys: the keys as first typed can no longer be told apart.
            s.originalReplayable = false
        }
        endRetyping(s, after: after)
        var event = CorrectionEvent(type: type, t: t, shown: after.text)
        event.shownBefore = before.text
        event.removed = removed
        event.deletedKeys = deletedKeys
        event.cleared = cleared
        s.events.append(event)
        let was = Shape(before.text)
        s.retyping = Retyping(event: s.events.count - 1, target: was.converted + (was.pending.isEmpty ? 0 : 1),
                              position: shape.converted)
    }

    /// Ends the keys typed after a deletion: with keys, the deletion was a correction; without, the deletion is
    /// part of the keys as first typed too.
    private func endRetyping(_ s: Session, after: Presentation) {
        guard let retyping = s.retyping else { return }
        s.retyping = nil
        let event = s.events[retyping.event]
        if retyping.keys.isEmpty {
            let deleted = event.deletedKeys ?? ""
            let labels = s.original.map(\.label).joined()
            if labels.hasSuffix(deleted) {
                var remaining = deleted.count
                while remaining > 0, let last = s.original.popLast() { remaining -= last.label.count }
            } else {
                s.originalReplayable = false
            }
            return
        }
        s.events[retyping.event].retypedKeys = retyping.keys.map(\.label).joined()
        let scalars = Array(after.text.unicodeScalars)
        let end = scalars.count - Shape(after.text).pending.unicodeScalars.count
        if retyping.position <= end {
            s.events[retyping.event].replacedBy = String(String.UnicodeScalarView(scalars[retyping.position..<end]))
        }
        correction(s, event.type, shown: event.shownBefore ?? event.shown)
    }

    private func correction(_ s: Session, _ kind: String, shown: String) {
        if s.shown == nil { s.shown = s.sent + shown }
        if !s.corrections.contains(kind) { s.corrections.append(kind) }
    }

    // MARK: Commit

    /// The composition grew past its limit and sent its head (`committed`) by itself; `after` is the rest.  The record
    /// goes on: its typing replays the head and the rest together (zhuyin-cli prints every commit of a line).
    private func carry(_ s: Session, committed: String, after: Presentation, t: Int) {
        var event = CorrectionEvent(type: "commit", t: t, shown: after.text)
        event.text = committed
        s.events.append(event)
        s.sent += committed
        // A deletion can no longer go back past the head: from here, deleting keys are recorded as typed.
        if s.tracking { s.history = [(s.typing.count, Shape(after.text))] }
    }

    /// The record ends with `committed` (after what the composition sent by itself, if anything).
    private func finish(_ s: Session, committed final: String, input: Input, handled: Bool, time: TimeInterval,
                        t: Int) {
        endRetyping(s, after: .empty)
        if !final.isEmpty || s.sent.isEmpty {
            var event = CorrectionEvent(type: "commit", t: t, shown: "")
            event.text = final
            s.events.append(event)
        }
        let committed = s.sent + final
        if let afterCommit = s.afterCommit, String(committed.prefix(afterCommit.deleted.count)) != afterCommit.deleted {
            correction(s, "delete_after_commit", shown: afterCommit.previous)
        }
        let typing = Self.serialize(s.typing)
        if !s.corrections.isEmpty {
            let formatter = DateFormatter()
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = "yyyy-MM-dd"
            sink(CorrectionRecord(date: formatter.string(from: today()), appVersion: appVersion, settings: settings,
                                  context: s.context, keys: s.keys, typing: typing, replayable: s.replayable,
                                  originalTyping: Self.serialize(s.original),
                                  originalReplayable: s.replayable && s.originalReplayable,
                                  shown: s.shown ?? "", committed: committed, corrections: s.corrections,
                                  events: s.events))
        }
        session = nil
        // What the app received after the commit: Backspace deletes it first.
        var passed = ""
        if !handled, case .key(let key, _) = input {
            switch key {
            case .space: passed = " "
            case .enter, .shiftEnter: passed = "\n"
            default:
                lastCommit = nil
                return
            }
        }
        lastCommit = LastCommit(time: time, text: committed + passed, committed: committed, passed: passed.count,
                                typing: typing, replayable: s.replayable, context: s.context)
    }

    private func followDeletion(at time: TimeInterval) {
        guard var last = lastCommit, time - last.time < Self.window, last.deleted < last.text.count else {
            lastCommit = nil
            return
        }
        last.deleted += 1
        lastCommit = last
    }

    // MARK: Helpers

    private func milliseconds(_ seconds: TimeInterval) -> Int { max(0, Int((seconds * 1000).rounded())) }

    private static func serialize(_ keys: [Key]) -> String {
        var result = ""
        var previous: TimeInterval?
        for key in keys {
            if let previous { result += "<dt:\(max(0, Int(((key.time - previous) * 1000).rounded())))>" }
            result += key.label
            previous = key.time
        }
        return result
    }

    private func keyLabel(_ input: Input, list: CandidateList?, options: ComposerOptions) -> String? {
        switch input {
        case .key(let key, _): return Self.label(key)
        case .toggleCorrections: return "<sup>"
        case .commit: return "<enter>"
        case .click(let index):
            // A click is written as the selection key at that place on the page, which picks the same item.
            guard let list else { return nil }
            let number = index - list.page * list.pageSize
            return options.selectionKeys.indices.contains(number) ? String(options.selectionKeys[number]) : nil
        }
    }

    /// The key in zhuyin-cli batch syntax; nil for the few keys it cannot write (> with Option or Control).
    static func label(_ key: KeyInput) -> String? {
        switch key {
        case .char(let c): return c == " " ? "<space>" : c == "<" || c == ">" ? nil : String(c)
        case .shiftedChar(let c): return c == ">" ? nil : "<S-\(c)>"
        case .printed(let c): return c == ">" ? nil : "<O-\(c)>"
        case .controlChar(let c): return c == ">" ? nil : "<C-\(c)>"
        case .space: return "<space>"
        case .shiftSpace: return "<sspace>"
        case .enter: return "<enter>"
        case .shiftEnter: return "<senter>"
        case .backspace: return "<bs>"
        case .optionBackspace: return "<obs>"
        case .forwardDelete: return "<del>"
        case .escape: return "<esc>"
        case .tab: return "<tab>"
        case .shiftTab: return "<stab>"
        case .left: return "<left>"
        case .right: return "<right>"
        case .up: return "<up>"
        case .down: return "<down>"
        case .home: return "<home>"
        case .end: return "<end>"
        case .pageUp: return "<pgup>"
        case .pageDown: return "<pgdown>"
        case .shiftLeft: return "<sleft>"
        case .shiftRight: return "<sright>"
        case .shiftDown: return "<sdown>"
        case .shiftUp: return "<sup>"
        }
    }

    /// Keys the open list handles itself; any other key closes it and is typed (docs/product/DESIGN.md §2.1 改字).
    private static func isListKey(_ key: KeyInput, options: ComposerOptions) -> Bool {
        switch key {
        case .char(let c): return c == "`" || options.selectionKeys.contains(c)
        case .enter, .shiftDown, .shiftUp, .escape, .backspace, .left, .right, .up, .down, .pageUp, .pageDown, .tab,
             .shiftTab, .space:
            return true
        default: return false
        }
    }

    /// Keys that change what is composed: they are typing when the list is closed.
    private static func isContent(_ key: KeyInput) -> Bool {
        switch key {
        case .char, .shiftedChar, .printed, .controlChar, .space, .shiftSpace, .backspace, .optionBackspace,
             .forwardDelete, .escape, .enter, .shiftEnter:
            return true
        default: return false
        }
    }

    private static func kind(_ list: CandidateList) -> String {
        switch (list.singleCharacter, list.tolerant == true) {
        case (true, true): return "single_corrections"
        case (true, false): return "single"
        case (false, true): return "corrections"
        case (false, false): return "normal"
        }
    }

    private static func atEnd(_ p: Presentation) -> Bool { p.isEmpty || p.cursor == p.text.utf16.count }

    /// The focused clause's text: what the list changes, or what is selected.
    private static func focused(_ p: Presentation) -> String? {
        guard let index = p.focusedClause, p.clauses.indices.contains(index) else { return nil }
        let range = p.clauses[index]
        let units = Array(p.text.utf16)
        guard range.lowerBound >= 0, range.upperBound <= units.count else { return nil }
        return String(decoding: units[range], as: UTF16.self)
    }

    private static func scalarOffset(_ text: String, utf16 offset: Int) -> Int {
        let index = String.Index(utf16Offset: min(max(0, offset), text.utf16.count), in: text)
        return text.unicodeScalars[..<index].count
    }

    /// Where two texts differ: the start (in Unicode scalars), the part of `a` replaced and the part of `b` in its place.
    static func difference(_ a: String, _ b: String) -> (Int, String, String) {
        let x = Array(a.unicodeScalars), y = Array(b.unicodeScalars)
        var prefix = 0
        while prefix < min(x.count, y.count), x[prefix] == y[prefix] { prefix += 1 }
        var suffix = 0
        while suffix < min(x.count, y.count) - prefix, x[x.count - 1 - suffix] == y[y.count - 1 - suffix] { suffix += 1 }
        let removed = String(String.UnicodeScalarView(x[prefix..<(x.count - suffix)]))
        let inserted = String(String.UnicodeScalarView(y[prefix..<(y.count - suffix)]))
        return (prefix, removed, inserted)
    }

    /// Zhuyin symbols and tone marks, as an unfinished syllable shows them.
    fileprivate static func isZhuyin(_ scalar: Unicode.Scalar) -> Bool {
        (0x3105...0x3129).contains(scalar.value) || [0x02CA, 0x02C7, 0x02CB, 0x02D9].contains(scalar.value)
    }
}

/// corrections.jsonl: appended on a background queue, counted, exported and cleared for the settings window.
final class CorrectionLog {
    static var shared = CorrectionLog(directory: EngineHost.userDirectory)
    /// Posted on the main thread after the file changed.
    static let didChange = Notification.Name("ZhuyinTypeCorrectionLogDidChange")
    static let fileName = "corrections.jsonl"

    let url: URL
    private let queue = DispatchQueue(label: "ZhuyinType.corrections", qos: .utility)

    init(directory: URL) {
        url = directory.appendingPathComponent(Self.fileName)
    }

    static func encode(_ record: CorrectionRecord) -> Data? {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try? encoder.encode(record)
    }

    func append(_ record: CorrectionRecord) {
        queue.async { [url] in
            guard var line = Self.encode(record) else { return }
            line.append(0x0A)
            let manager = FileManager.default
            do {
                if !manager.fileExists(atPath: url.path) {
                    try manager.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                    guard manager.createFile(atPath: url.path, contents: nil, attributes: [.posixPermissions: 0o600]) else {
                        return
                    }
                }
                let handle = try FileHandle(forWritingTo: url)
                defer { try? handle.close() }
                try handle.seekToEnd()
                try handle.write(contentsOf: line)
            } catch {
                EngineHost.logger.error("Correction log write failed: \(error.localizedDescription, privacy: .public)")
                return
            }
            DispatchQueue.main.async { NotificationCenter.default.post(name: Self.didChange, object: nil) }
        }
    }

    /// Waits for pending writes: before the process exits, and before the file is read.
    func flush() { queue.sync {} }

    /// The number of records, on the main thread.
    func count(_ completion: @escaping (Int) -> Void) {
        queue.async { [url] in
            let data = (try? Data(contentsOf: url)) ?? Data()
            let lines = data.reduce(0) { $1 == 0x0A ? $0 + 1 : $0 }
            DispatchQueue.main.async { completion(lines) }
        }
    }

    func clear(_ completion: @escaping () -> Void) {
        queue.async { [url] in
            try? FileManager.default.removeItem(at: url)
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: Self.didChange, object: nil)
                completion()
            }
        }
    }

    /// A copy of the file at `destination`, replacing what is there.
    func export(to destination: URL) throws {
        try queue.sync {
            let manager = FileManager.default
            if manager.fileExists(atPath: destination.path) { try manager.removeItem(at: destination) }
            if manager.fileExists(atPath: url.path) {
                try manager.copyItem(at: url, to: destination)
            } else {
                try Data().write(to: destination)
            }
        }
    }
}
