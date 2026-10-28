import Foundation

// The practice field's side of a lesson's task.  `PracticeTask` and `PracticeOutcome` are the registry's
// (UI/Settings/Registry/Habits.swift): a prompt and a check of what the field reports.  The field keeps the outcome
// up to date after every key; here are the outcome's events, ready-made tasks of the three kinds, and the progress the
// field reports.

/// The events the field puts in `PracticeOutcome.events`.
enum PracticeEventName {
    /// A candidate list was opened.
    static let open = "open"
    /// The candidate list turned to another page.
    static let page = "page"
    /// A candidate was picked from a list (its text goes to `picks`).
    static let pick = "pick"
    /// A candidate was picked in place of different text: a character or word changed through the list.
    static let correct = "correct"
    /// The corrections list (⇧↑) was opened.
    static let corrections = "corrections"
    /// 中/英 switched (Shift or Caps Lock).
    static let mode = "mode"
    /// A user phrase was added (to the practice field's own data).
    static let phrase = "phrase"
}

/// One thing a task asks for, for tasks made here.  Goals about what happened stay met; goals about the text are
/// checked against the field as it is.
enum PracticeGoal: Hashable {
    /// The field's sent text is exactly this (spaces and line breaks at either end aside).
    case text(String)
    /// The field's text (sent and composed) contains this, for example a punctuation mark.
    case contains(String)
    case openCandidates
    case turnPage
    /// A candidate was picked: this one, or any when nil.
    case pick(String?)
    /// This candidate was picked in place of different text.
    case correct(String)
    /// The corrections list was opened and a candidate picked from it.
    case pickCorrection
    /// 中/英 switched at least this many times.
    case switchMode(times: Int)
    /// This word was added as a user phrase.
    case addUserPhrase(String)

    func isMet(_ outcome: PracticeOutcome) -> Bool {
        switch self {
        case .text(let target): return outcome.sent.trimmingCharacters(in: .whitespacesAndNewlines) == target
        case .contains(let part): return outcome.text.contains(part)
        case .openCandidates: return outcome.events.contains(PracticeEventName.open)
        case .turnPage: return outcome.events.contains(PracticeEventName.page)
        case .pick(let word): return word.map(outcome.picks.contains) ?? !outcome.picks.isEmpty
        case .correct(let word): return outcome.corrected.contains(word)
        case .pickCorrection: return outcome.correctionPicks > 0
        case .switchMode(let times): return outcome.events.filter { $0 == PracticeEventName.mode }.count >= times
        case .addUserPhrase(let word): return outcome.phrases.contains(word)
        }
    }
}

extension PracticeTask {
    /// Type a sentence: done when the field's sent text is `text`.
    static func sentence(_ text: String, prompt: String? = nil, hint: String? = nil) -> PracticeTask {
        goals(prompt ?? "打出「\(text)」", hint: hint, [.text(text)])
    }

    /// Use a key or an action: done when every goal is met, for example `.action("翻到下一頁", .turnPage)`.
    static func action(_ prompt: String, hint: String? = nil, _ goals: PracticeGoal...) -> PracticeTask {
        Self.goals(prompt, hint: hint, goals)
    }

    /// Add a user phrase: done when `word` is one of the practice data's user phrases.
    static func userPhrase(_ word: String, prompt: String? = nil) -> PracticeTask {
        goals(prompt ?? "選取「\(word)」，按 Return 加入自訂詞", hint: nil, [.addUserPhrase(word)])
    }

    /// A task from goals, all of which must be met.
    static func goals(_ prompt: String, hint: String?, _ goals: [PracticeGoal]) -> PracticeTask {
        PracticeTask(prompt: prompt, hint: hint) { outcome in
            !goals.isEmpty && goals.allSatisfy { $0.isMet(outcome) }
        }
    }

    /// No task: practise freely; never done.
    static let free = PracticeTask(prompt: "", hint: nil) { _ in false }
}

extension SettingsSnapshot {
    /// These settings with `change` made through the setters of `Preferences`, on a copy in memory, for example
    /// `SettingsRegistry.defaults.with { $0.candidateLayout = .horizontal }`.
    func with(_ change: (Preferences) -> Void) -> SettingsSnapshot {
        let preferences = Preferences(defaults: MemorySettingsStorage())
        SettingsSnapshot(preferences).changes(to: self).apply(to: preferences)
        change(preferences)
        return SettingsSnapshot(preferences)
    }
}

/// How far a task has got, as the field reports it after every change.
struct PracticeProgress {
    var outcome: PracticeOutcome
    /// The task's check passed at some point; stays true.
    var isComplete: Bool
    /// User phrases added while practising, with their readings, for the wizard to offer to keep.
    var userPhrases: [(word: String, reading: String)]
}
