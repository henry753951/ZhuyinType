// Word actions from the candidate window (不要再推薦, 忘記這個選字, 加入自訂詞): their shortcuts, the right-click menu
// on a candidate, the notice with 復原, and the hint shown the first few times the list opens.  The engine does the
// work and keeps one level of undo; this part decides when keys are claimed.
import AppKit
import ZhuyinEngine

extension ControllerCore {
    /// How long a word action can be undone with ⌃Z or a click on the notice.
    static let wordUndoSeconds: TimeInterval = 5
    /// The list openings that show the hint.
    static let wordActionHintLimit = 5
    static let wordActionHintKey = "wordActionHintCount"

    /// Does `action` on candidate `index` of the open list (nil: the highlighted one; nothing with no list).  Claims
    /// the key only while composing; with nothing to compose it goes to the app.
    @discardableResult
    func wordAction(_ action: CandidateAction, at index: Int? = nil) -> Bool {
        guard composer.isComposing else { return false }
        if composer.performCandidateAction(action, at: index) != nil { wordActionTime = Date() }
        render()
        return true
    }

    /// The actions that apply to candidate `index`, for the right-click menu.
    func availableWordActions(at index: Int) -> [CandidateAction] {
        CandidateAction.allCases.filter { composer.candidateActionAvailable($0, at: index) }
    }

    /// ⌃Z undoes only while the composition is still there, so the app's own undo is never taken.
    var wordUndoOffered: Bool {
        guard let time = wordActionTime, Date().timeIntervalSince(time) <= Self.wordUndoSeconds else { return false }
        return composer.isComposing && composer.canUndoCandidateAction
    }

    @discardableResult
    func undoWordAction() -> Bool {
        guard wordUndoOffered else { return false }
        wordActionTime = nil
        composer.undoCandidateAction()
        render()
        return true
    }

    func isUndoKey(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool {
        keyCode == 6 && modifiers.intersection([.control, .option, .shift, .command]) == .control
    }

    /// The notice the window shows: the composer's, with 「⌃Z 復原」 while a word action can be undone; else, the
    /// first few times a list opens, how to reach the word actions.
    func wordActionNotice(_ notice: String?, listOpen: Bool) -> String? {
        defer { listWasOpen = listOpen }
        if let notice {
            return wordUndoOffered && notice != "已復原" ? notice + "　⌃Z 復原" : notice
        }
        guard listOpen else { return nil }
        let defaults = UserDefaults.standard
        let count = defaults.integer(forKey: Self.wordActionHintKey)
        guard count < Self.wordActionHintLimit else { return nil }
        if !listWasOpen { defaults.set(count + 1, forKey: Self.wordActionHintKey) }
        let key = shortcuts.first { $0.value == .blockCandidate }?.key.compact
        return (key.map { "\($0) 不要再推薦・" } ?? "") + "右鍵看更多"
    }
}

extension ControllerCore {
    /// The longest selection read for 「選取的字加入自訂詞」, in UTF-16 units (a custom word has at most 6 characters;
    /// more is read so the form can say why it is too long).
    static let selectionLimit = 32

    /// 「選取的字加入自訂詞」: with nothing being composed, the app's selected text goes to the 加入自訂詞 form.  While
    /// composing the key does nothing; in a secure field nothing is read.
    @discardableResult
    func addSelectedText() -> Bool {
        guard !composer.isComposing else { return true }
        let text = secureInput() ? nil : client.selectedText(maxLength: Self.selectionLimit)
        addPhraseForm?(text?.trimmingCharacters(in: .whitespacesAndNewlines))
        return true
    }
}
