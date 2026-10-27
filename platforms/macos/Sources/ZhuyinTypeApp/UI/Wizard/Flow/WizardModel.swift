import SwiftUI

// The setup wizard's state: which steps it goes through, where it is, and the sandbox it changes.  Everything the
// wizard sets goes to `sandbox` (settings in memory); the user's settings change only when 「套用」 applies the list of
// changes.  The steps come from the registry's lessons, so a new lesson appears here without code in the wizard.
// docs/product/SETUP_WIZARD.md describes the flow.

enum WizardStep: Equatable, Identifiable {
    case welcome
    case preset
    case lesson(String)
    case finish
    case keys
    var id: String {
        switch self {
        case .welcome: return "welcome"
        case .preset: return "preset"
        case .lesson(let id): return "lesson-" + id
        case .finish: return "finish"
        case .keys: return "keys"
        }
    }
}

final class WizardModel: ObservableObject {
    enum Mode: Equatable {
        /// The basic lessons.
        case quick
        /// Every lesson.
        case full
        /// Only lessons added since `version` (新功能導覽).
        case newSince(String)
        /// Only the key summary (from 介紹).
        case keysOnly
    }

    /// The user's settings; written only by `apply()`.
    let original: Preferences
    /// A copy in memory the steps change.
    @Published private(set) var sandbox: Preferences
    @Published var mode: Mode
    @Published private(set) var index = 0
    /// 1 going forward, -1 back: which way the step transition moves.
    @Published private(set) var direction = 1
    @Published var preset: String = SettingsPreset.custom.id
    /// Lessons whose practice is done.
    @Published var completed: Set<String> = []
    /// Words added in the 加入自訂詞 practice, to keep only if the user says so at the end.
    @Published var practicePhrases: [(word: String, reading: String)] = []
    @Published var keepPracticePhrases = false
    var onClose: (() -> Void)?

    init(original: Preferences = .shared, mode: Mode = .quick) {
        self.original = original
        sandbox = original.sandbox()
        self.mode = mode
        if mode == .keysOnly { index = 0 }
    }

    var lessons: [Lesson] {
        switch mode {
        case .quick: return Lesson.all.filter { $0.level == .basic }
        case .full: return Lesson.all
        case .newSince(let version): return Lesson.all.filter { $0.since.compare(version, options: .numeric) == .orderedDescending }
        case .keysOnly: return []
        }
    }

    var steps: [WizardStep] {
        switch mode {
        case .keysOnly: return [.keys]
        case .newSince: return lessons.map { .lesson($0.id) } + [.finish, .keys]
        case .quick, .full: return [.welcome, .preset] + lessons.map { .lesson($0.id) } + [.finish, .keys]
        }
    }

    var step: WizardStep { steps[min(index, steps.count - 1)] }
    var lesson: Lesson? {
        if case .lesson(let id) = step { return Lesson.all.first { $0.id == id } }
        return nil
    }
    /// The steps the progress dots count (not the welcome).
    var progress: (index: Int, count: Int) {
        let counted = steps.filter { $0 != .welcome }
        return (counted.firstIndex(of: step).map { $0 + 1 } ?? 0, counted.count)
    }

    // MARK: Moving

    func next() { go(to: index + 1) }
    func back() { go(to: index - 1) }
    /// 全部跳過: change nothing and close.
    func skipAll() { discard() }

    private func go(to target: Int) {
        guard steps.indices.contains(target) else { onClose?(); return }
        direction = target > index ? 1 : -1
        withAnimation(WizardMotion.step) { index = target }
    }

    /// Shows `step` at once (the UI previews).
    func jump(to step: WizardStep) {
        if let target = steps.firstIndex(of: step) { index = target }
    }

    /// Switching between 快速設定 and 完整導覽 on the welcome step keeps the place.
    func choose(_ mode: Mode) {
        guard self.mode != mode else { return }
        self.mode = mode
        index = min(index, steps.count - 1)
    }

    // MARK: Settings

    /// Chooses a preset: the sandbox starts again from the user's settings with the preset's values.
    func choosePreset(_ preset: SettingsPreset) {
        self.preset = preset.id
        let fresh = original.sandbox()
        preset.changes(from: fresh).apply(to: fresh)
        sandbox = fresh
    }

    /// What the wizard would change, 原本 → 改成.
    var changes: [SettingChange] { SettingsSnapshot(original).changes(to: SettingsSnapshot(sandbox)) }

    /// 改回 one change in the sandbox.
    func revert(_ change: SettingChange) {
        objectWillChange.send()
        change.revert(in: sandbox)
    }

    /// 套用: the changes go to the user's settings; practice phrases only if kept.
    func apply(engineAdd: ((String, String) -> Void)? = nil) {
        changes.apply(to: original)
        if keepPracticePhrases { practicePhrases.forEach { engineAdd?($0.word, $0.reading) } }
        WizardProgress.markSeen()
        go(to: steps.firstIndex(of: .keys) ?? steps.count)
    }

    /// 全部捨棄: nothing is written.
    func discard() {
        WizardProgress.markSeen()
        onClose?()
    }
}

/// The wizard's motion: a deliberate push between steps, a spring for success; crossfades under Reduce Motion.
enum WizardMotion {
    static var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    static var step: Animation { reduceMotion ? .easeInOut(duration: 0.2) : .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.34) }
    static var success: Animation { reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.42, dampingFraction: 0.62) }
}

/// Which wizard versions the user has seen: the first install opens it once, an update with new lessons offers
/// 「新功能導覽」 in the input menu and on 介紹.
enum WizardProgress {
    private static let key = "setupWizardSeenVersion"
    static var current: String { Lesson.all.map(\.since).max { $0.compare($1, options: .numeric) == .orderedAscending } ?? "0" }

    static var seenVersion: String? { UserDefaults.standard.string(forKey: key) }
    static var neverSeen: Bool { seenVersion == nil }
    /// New lessons since the version seen, for 「新功能導覽」.
    static var hasNewLessons: Bool {
        guard let seen = seenVersion else { return false }
        return current.compare(seen, options: .numeric) == .orderedDescending
    }
    static func markSeen() {
        guard !SettingsWindowController.isPreview, !Headless.isActive else { return }
        UserDefaults.standard.set(current, forKey: key)
    }
}
