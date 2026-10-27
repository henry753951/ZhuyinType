// User settings, stored in the app's UserDefaults domain.  The settings window binds to these; the input controller
// reads them and listens for `Preferences.didChange`.
import AppKit
import Combine
import Foundation
import ZhuyinCore

enum CandidateLayout: String, CaseIterable, Identifiable {
    case vertical, horizontal
    var id: String { rawValue }
}

enum ShiftToggle: String, CaseIterable, Identifiable {
    case both, left, right, none
    var id: String { rawValue }
}

enum TypoSetting: String, CaseIterable, Identifiable {
    case off, standard, aggressive
    var id: String { rawValue }
    var tolerance: TypoTolerance { TypoTolerance(rawValue: rawValue) ?? .standard }
}

enum AnchorSetting: String, CaseIterable, Identifiable {
    case after, before
    var id: String { rawValue }
}

/// Where the candidate window opens; either side flips to the other when the screen has no room.
enum CandidatePlacement: String, CaseIterable, Identifiable {
    case below, above
    var id: String { rawValue }
}

/// Row height and margins of the candidate window.
enum CandidateDensity: String, CaseIterable, Identifiable {
    case standard, compact
    var id: String { rawValue }
}

/// The floating panels' appearance: the system's, or always light or dark.
enum PanelAppearance: String, CaseIterable, Identifiable {
    case system, light, dark
    var id: String { rawValue }
    var appearance: NSAppearance? {
        switch self {
        case .system: return nil
        case .light: return NSAppearance(named: .aqua)
        case .dark: return NSAppearance(named: .darkAqua)
        }
    }
}

enum SelectionKeySet: String, CaseIterable, Identifiable {
    case digits = "123456789"
    case homeRow = "asdfghjkl"
    var id: String { rawValue }
}

final class Preferences: ObservableObject {
    /// The app's settings.  The UI preview renderer swaps in a throwaway suite so it never touches the user's.
    static var shared = Preferences()
    static let didChange = Notification.Name("ZhuyinTypePreferencesDidChange")

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        defaults.register(defaults: [
            Key.candidateLayout: CandidateLayout.vertical.rawValue,
            Key.candidateFontSize: 18.0,
            Key.selectionKeys: SelectionKeySet.digits.rawValue,
            Key.candidatesPerPage: 9,
            Key.candidatePlacement: CandidatePlacement.below.rawValue,
            Key.candidateDensity: CandidateDensity.standard.rawValue,
            Key.showsZhuyinHint: true,
            Key.showsAnnotations: true,
            Key.showsListSwitches: true,
            Key.panelAppearance: PanelAppearance.system.rawValue,
            Key.panelAnimation: true,
            Key.escapeClearsComposition: true,
            Key.shiftToggle: ShiftToggle.both.rawValue,
            Key.showModeToast: true,
            Key.startInEnglish: false,
            Key.learningEnabled: true,
            Key.popupCompositionApps: [String](),
            Key.typoTolerance: TypoSetting.standard.rawValue,
            Key.candidateAnchor: AnchorSetting.after.rawValue,
            Key.fuzzySounds: 0,
            Key.usesPrecedingText: true,
            Key.characterForms: CharacterForms.common.rawValue,
            Key.punctuationKeys: PunctuationKeys.windows.rawValue,
            Key.recordsCorrections: false,
            Key.recordsCorrectionContext: false,
            Key.disabledKeyFeatures: [String](),
            Key.lastShiftSide: ShiftToggle.both.rawValue,
            Key.recordsUsageStats: true,
        ])
    }

    enum Key {
        static let candidateLayout = "candidateLayout"
        static let candidateFontSize = "candidateFontSize"
        static let selectionKeys = "selectionKeys"
        /// Earlier versions stored Space as a Bool (true: open the candidate window); read while `spaceAction` is unset.
        static let legacySpaceOpensCandidates = "spaceOpensCandidates"
        static let spaceAction = "spaceAction"
        static let candidatesPerPage = "candidatesPerPage"
        static let candidatePlacement = "candidatePlacement"
        static let candidateDensity = "candidateDensity"
        static let showsZhuyinHint = "showsZhuyinHint"
        static let showsAnnotations = "showsAnnotations"
        static let showsListSwitches = "showsListSwitches"
        static let panelAppearance = "panelAppearance"
        static let panelAnimation = "panelAnimation"
        static let escapeClearsComposition = "escapeClearsComposition"
        static let shiftToggle = "shiftToggle"
        static let showModeToast = "showModeToast"
        static let startInEnglish = "startInEnglish"
        static let learningEnabled = "learningEnabled"
        /// Bundle identifiers of apps that get the composition in the candidate window instead of inline.  Empty by
        /// default: every app, Electron ones included, gets inline marked text unless the user adds it here.
        static let popupCompositionApps = "popupCompositionApps"
        static let typoTolerance = "typoTolerance"
        static let candidateAnchor = "candidateAnchor"
        static let fuzzySounds = "fuzzySounds"
        static let usesPrecedingText = "usesPrecedingText"
        /// Before multiple bindings: one shortcut per command.  Read while `keyBindings` is unset.
        static let shortcuts = "shortcuts"
        static let keyBindings = "keyBindings"
        /// 2: `keyBindings` holds only the commands whose keys differ from the default.  Before, every command was
        /// stored once any was changed, so a stored list equal to that version's default means "the default".
        static let keyBindingsVersion = "keyBindingsVersion"
        /// 「到頁底時 ↓ 翻到下一頁」 (`ComposerOptions.pageAtListEdge`).  On by default.
        static let pageAtListEdge = "pageAtListEdge"
        static let characterForms = "characterForms"
        static let punctuationKeys = "punctuationKeys"
        /// 「記錄改字」: compositions with a correction go to corrections.jsonl (CorrectionRecorder).  Off by default.
        static let recordsCorrections = "recordsCorrections"
        /// With it, the text before the insertion point the engine read.  Off by default.
        static let recordsCorrectionContext = "recordsCorrectionContext"
        /// Optional key features turned off on the 快捷鍵 page (`KeyFeature` raw values).  Empty: everything on.
        static let disabledKeyFeatures = "disabledKeyFeatures"
        /// The Shift used for 中/英 before the switch was turned off, to restore when it is turned on again.
        static let lastShiftSide = "lastShiftSide"
        /// 「記錄使用統計」: counts and timings for the 分析 page (UsageStatsStore).  On by default.
        static let recordsUsageStats = "recordsUsageStats"

        static let all = [candidateLayout, candidateFontSize, selectionKeys, legacySpaceOpensCandidates, spaceAction,
                          escapeClearsComposition, shiftToggle, showModeToast, startInEnglish, learningEnabled,
                          popupCompositionApps, typoTolerance, candidateAnchor, fuzzySounds, usesPrecedingText, shortcuts, keyBindings, keyBindingsVersion, pageAtListEdge,
                          characterForms, punctuationKeys, candidatesPerPage, candidatePlacement, candidateDensity,
                          showsZhuyinHint, showsAnnotations, showsListSwitches, panelAppearance, panelAnimation,
                          recordsCorrections, recordsCorrectionContext, disabledKeyFeatures, lastShiftSide,
                          recordsUsageStats]
    }

    func resetToDefaults() {
        Key.all.forEach(defaults.removeObject(forKey:))
        changed()
    }

    private func changed() {
        objectWillChange.send()
        NotificationCenter.default.post(name: Self.didChange, object: self)
    }

    var candidateLayout: CandidateLayout {
        get { CandidateLayout(rawValue: defaults.string(forKey: Key.candidateLayout) ?? "") ?? .vertical }
        set { defaults.set(newValue.rawValue, forKey: Key.candidateLayout); changed() }
    }
    var candidateFontSize: Double {
        get { min(28, max(13, defaults.double(forKey: Key.candidateFontSize))) }
        set { defaults.set(newValue, forKey: Key.candidateFontSize); changed() }
    }
    var selectionKeys: SelectionKeySet {
        get { SelectionKeySet(rawValue: defaults.string(forKey: Key.selectionKeys) ?? "") ?? .digits }
        set { defaults.set(newValue.rawValue, forKey: Key.selectionKeys); changed() }
    }
    /// The choices the app offers for Space: it never confirms a selection, so 「送出文字」 (`.commit`) is not one.
    static let spaceActions: [SpaceAction] = [.commitAndSpace, .insertSpace, .openCandidates]

    /// What Space does with no syllable being typed: send the composition and type a space (the default), or open
    /// the candidate window.  A choice saved by an earlier version is kept, except 「送出文字」, which becomes 「送出並
    /// 輸入空白」; the earlier Bool is read too.  Not registered, so the stored value can be told from the default.
    var spaceAction: SpaceAction {
        get {
            if let action = defaults.string(forKey: Key.spaceAction).flatMap(SpaceAction.init(rawValue:)) {
                return action == .commit ? .commitAndSpace : action
            }
            if let opens = defaults.object(forKey: Key.legacySpaceOpensCandidates) as? Bool { return opens ? .openCandidates : .commitAndSpace }
            return .commitAndSpace
        }
        set {
            defaults.removeObject(forKey: Key.legacySpaceOpensCandidates)
            defaults.set((newValue == .commit ? .commitAndSpace : newValue).rawValue, forKey: Key.spaceAction)
            changed()
        }
    }
    /// Candidates on a page, 3–9: the first that many selection keys pick them.
    var candidatesPerPage: Int {
        get { min(9, max(3, defaults.integer(forKey: Key.candidatesPerPage))) }
        set { defaults.set(min(9, max(3, newValue)), forKey: Key.candidatesPerPage); changed() }
    }
    var candidatePlacement: CandidatePlacement {
        get { CandidatePlacement(rawValue: defaults.string(forKey: Key.candidatePlacement) ?? "") ?? .below }
        set { defaults.set(newValue.rawValue, forKey: Key.candidatePlacement); changed() }
    }
    var candidateDensity: CandidateDensity {
        get { CandidateDensity(rawValue: defaults.string(forKey: Key.candidateDensity) ?? "") ?? .standard }
        set { defaults.set(newValue.rawValue, forKey: Key.candidateDensity); changed() }
    }
    /// The reading of the part being changed, above the candidates.
    var showsZhuyinHint: Bool {
        get { defaults.bool(forKey: Key.showsZhuyinHint) }
        set { defaults.set(newValue, forKey: Key.showsZhuyinHint); changed() }
    }
    /// The corrected reading in grey after a corrected candidate.
    var showsAnnotations: Bool {
        get { defaults.bool(forKey: Key.showsAnnotations) }
        set { defaults.set(newValue, forKey: Key.showsAnnotations); changed() }
    }
    /// The ⇧↓ 單字 and ⇧↑ 修正 switches at the bottom of the window; when off, a switch shows only while it is on.
    var showsListSwitches: Bool {
        get { defaults.bool(forKey: Key.showsListSwitches) }
        set { defaults.set(newValue, forKey: Key.showsListSwitches); changed() }
    }
    var panelAppearance: PanelAppearance {
        get { PanelAppearance(rawValue: defaults.string(forKey: Key.panelAppearance) ?? "") ?? .system }
        set { defaults.set(newValue.rawValue, forKey: Key.panelAppearance); changed() }
    }
    /// The candidate window fades in and the 中/英 badge fades out; with Reduce Motion, nothing slides.
    var panelAnimation: Bool {
        get { defaults.bool(forKey: Key.panelAnimation) }
        set { defaults.set(newValue, forKey: Key.panelAnimation); changed() }
    }

    /// Everything the candidate window's look depends on, for the window and the settings preview alike.
    var candidateStyle: CandidateStyle {
        CandidateStyle(layout: candidateLayout, fontSize: candidateFontSize, density: candidateDensity,
                       showsZhuyinHint: showsZhuyinHint, showsAnnotations: showsAnnotations,
                       showsListSwitches: showsListSwitches, keyLabels: candidateKeyLabels)
    }
    /// The keys that pick candidates on a page, as many as `candidatesPerPage`.
    var pageSelectionKeys: [Character] { Array(Array(selectionKeys.rawValue).prefix(candidatesPerPage)) }
    var escapeClearsComposition: Bool {
        get { defaults.bool(forKey: Key.escapeClearsComposition) }
        set { defaults.set(newValue, forKey: Key.escapeClearsComposition); changed() }
    }
    var shiftToggle: ShiftToggle {
        get { ShiftToggle(rawValue: defaults.string(forKey: Key.shiftToggle) ?? "") ?? .both }
        set { defaults.set(newValue.rawValue, forKey: Key.shiftToggle); changed() }
    }
    var showModeToast: Bool {
        get { defaults.bool(forKey: Key.showModeToast) }
        set { defaults.set(newValue, forKey: Key.showModeToast); changed() }
    }
    var startInEnglish: Bool {
        get { defaults.bool(forKey: Key.startInEnglish) }
        set { defaults.set(newValue, forKey: Key.startInEnglish); changed() }
    }
    var learningEnabled: Bool {
        get { defaults.bool(forKey: Key.learningEnabled) }
        set { defaults.set(newValue, forKey: Key.learningEnabled); changed() }
    }
    var popupCompositionApps: [String] {
        get { defaults.stringArray(forKey: Key.popupCompositionApps) ?? [] }
        set { defaults.set(newValue, forKey: Key.popupCompositionApps); changed() }
    }

    var typoTolerance: TypoSetting {
        get { TypoSetting(rawValue: defaults.string(forKey: Key.typoTolerance) ?? "") ?? .standard }
        set { defaults.set(newValue.rawValue, forKey: Key.typoTolerance); changed() }
    }
    var characterForms: CharacterForms {
        get { CharacterForms(rawValue: defaults.string(forKey: Key.characterForms) ?? "") ?? .common }
        set { defaults.set(newValue.rawValue, forKey: Key.characterForms); changed() }
    }
    var punctuationKeys: PunctuationKeys {
        get { PunctuationKeys(rawValue: defaults.string(forKey: Key.punctuationKeys) ?? "") ?? .windows }
        set { defaults.set(newValue.rawValue, forKey: Key.punctuationKeys); changed() }
    }
    /// 「到頁底時 ↓ 翻到下一頁」: ↓ on a page's last candidate goes to the next page's first, ↑ on the first goes back.
    var pageAtListEdge: Bool {
        get { defaults.object(forKey: Key.pageAtListEdge) as? Bool ?? true }
        set { defaults.set(newValue, forKey: Key.pageAtListEdge); changed() }
    }
    var candidateAnchor: AnchorSetting {
        get { AnchorSetting(rawValue: defaults.string(forKey: Key.candidateAnchor) ?? "") ?? .after }
        set { defaults.set(newValue.rawValue, forKey: Key.candidateAnchor); changed() }
    }

    /// Every rebindable command's bindings, in order (the first is the one shown first): the user's where set,
    /// otherwise the default.  Stored as {command: [shortcut]}; a single shortcut stored by an earlier version
    /// (`shortcuts`, {command: shortcut}) is read as a one-binding list until the bindings are first changed.
    var bindings: [ShortcutCommand: [Shortcut]] {
        get { Self.withDefaults(userBindings) }
        set {
            // Store only the user's own keys.  A command that was on its defaults and still has what they resolved to
            // (all of them, or those left after another action took one) stays on its defaults.
            let before = userBindings
            let resolved = Self.withDefaults(before)
            let own = newValue.filter { command, list in
                before[command] != nil ? list != command.defaultBindings : list != resolved[command] && list != command.defaultBindings
            }
            let stored = Dictionary(uniqueKeysWithValues: own.map { ($0.key.rawValue, $0.value) })
            defaults.set(try? JSONEncoder().encode(stored), forKey: Key.keyBindings)
            defaults.set(2, forKey: Key.keyBindingsVersion)
            defaults.removeObject(forKey: Key.shortcuts)
            changed()
        }
    }

    /// The user set `command`'s keys (they are not its defaults).
    func isCustomized(_ command: ShortcutCommand) -> Bool { userBindings[command] != nil }

    /// The keys the user set, by command; the other commands use their defaults.
    private var userBindings: [ShortcutCommand: [Shortcut]] {
        var user: [ShortcutCommand: [Shortcut]] = [:]
        let current = defaults.integer(forKey: Key.keyBindingsVersion) >= 2
        if let data = defaults.data(forKey: Key.keyBindings),
           let stored = try? JSONDecoder().decode([String: [Shortcut]].self, from: data) {
            for (name, list) in stored {
                guard let command = ShortcutCommand(rawValue: name), current || list != command.legacyDefaultBindings else { continue }
                user[command] = list
            }
        } else if let data = defaults.data(forKey: Key.shortcuts),
                  let stored = try? JSONDecoder().decode([String: Shortcut].self, from: data) {
            for (name, shortcut) in stored {
                guard let command = ShortcutCommand(rawValue: name), [shortcut] != command.legacyDefaultBindings else { continue }
                user[command] = [shortcut]
            }
        }
        return user
    }

    /// The user's keys, and the default keys for every other command, leaving out a default the user gave to another
    /// command (a command keeps all its defaults when every one is taken, and shows the conflict).
    static func withDefaults(_ user: [ShortcutCommand: [Shortcut]]) -> [ShortcutCommand: [Shortcut]] {
        var result = user
        let taken = Set(user.values.joined())
        for command in ShortcutCommand.allCases where result[command] == nil {
            let free = command.defaultBindings.filter { !taken.contains($0) }
            result[command] = free.isEmpty ? command.defaultBindings : free
        }
        return result
    }

    /// Each command's first binding (none: the command has no key).
    var shortcuts: [ShortcutCommand: Shortcut] { bindings.compactMapValues(\.first) }

    /// Sets one binding of `command` (`index` past the end adds one), unless `shortcut` is reserved or bound to
    /// another command; returns why not, naming that command.
    @discardableResult
    func bind(_ command: ShortcutCommand, at index: Int, to shortcut: Shortcut) -> String? {
        var all = bindings
        var list = all[command] ?? []
        if let problem = ShortcutCommand.problem(assigning: shortcut, to: command, at: index, in: all) { return problem }
        if index < list.count { list[index] = shortcut } else { list.append(shortcut) }
        all[command] = list
        bindings = all
        return nil
    }

    /// Removes one binding of `command`; its last binding stays (turn the feature off instead).
    func unbind(_ command: ShortcutCommand, at index: Int) {
        var all = bindings
        guard var list = all[command], list.count > 1, list.indices.contains(index) else { return }
        list.remove(at: index)
        all[command] = list
        bindings = all
    }

    /// Restores `command`'s default bindings, unless another command now uses one of them; returns why not.
    @discardableResult
    func resetBindings(_ command: ShortcutCommand) -> String? {
        var all = bindings
        all[command] = []
        // The defaults another action now uses stay with it; the rest come back.
        let free = command.defaultBindings.filter { ShortcutCommand.problem(assigning: $0, to: command, at: 0, in: all) == nil }
        guard !free.isEmpty else {
            return command.defaultBindings.first.flatMap { ShortcutCommand.problem(assigning: $0, to: command, at: 0, in: all) }
        }
        all[command] = free
        bindings = all
        return nil
    }

    var candidateKeyLabels: CandidateKeyLabels {
        let all = activeShortcuts
        return CandidateKeyLabels(zhuyin: all[.typeZhuyin]?.compact ?? "",
                                  single: all[.singleCharacterList]?.compact ?? "",
                                  corrections: all[.correctionsList]?.compact ?? "",
                                  zhuyinClickable: isEnabled(.zhuyinRowClick))
    }

    func resetShortcuts() {
        defaults.removeObject(forKey: Key.shortcuts)
        defaults.removeObject(forKey: Key.keyBindings)
        changed()
    }

    var usesPrecedingText: Bool {
        get { defaults.bool(forKey: Key.usesPrecedingText) }
        set { defaults.set(newValue, forKey: Key.usesPrecedingText); changed() }
    }

    var recordsUsageStats: Bool {
        get { defaults.bool(forKey: Key.recordsUsageStats) }
        set { defaults.set(newValue, forKey: Key.recordsUsageStats); changed() }
    }

    /// Optional key features that are off.
    var disabledKeyFeatures: Set<KeyFeature> {
        get { Set((defaults.stringArray(forKey: Key.disabledKeyFeatures) ?? []).compactMap(KeyFeature.init(rawValue:))) }
        set { defaults.set(newValue.map(\.rawValue).sorted(), forKey: Key.disabledKeyFeatures); changed() }
    }
    func isEnabled(_ feature: KeyFeature) -> Bool { !disabledKeyFeatures.contains(feature) }
    func setEnabled(_ feature: KeyFeature, _ on: Bool) {
        var disabled = disabledKeyFeatures
        if on { disabled.remove(feature) } else { disabled.insert(feature) }
        disabledKeyFeatures = disabled
    }
    /// Whether `command` works: its feature, if it has one, is on.
    func isEnabled(_ command: ShortcutCommand) -> Bool { command.feature.map(isEnabled) ?? true }
    /// The bindings the controller uses: those of commands whose feature is off are left out, so their keys go to the
    /// app as any unbound key does.
    var activeBindings: [ShortcutCommand: [Shortcut]] { bindings.filter { isEnabled($0.key) } }
    var activeShortcuts: [ShortcutCommand: Shortcut] { activeBindings.compactMapValues(\.first) }

    /// The Shift tap switch on the 快捷鍵 page: on uses the side chosen before.
    var shiftSwitchEnabled: Bool {
        get { shiftToggle != .none }
        set {
            if newValue {
                shiftToggle = ShiftToggle(rawValue: defaults.string(forKey: Key.lastShiftSide) ?? "")
                    .flatMap { $0 == .none ? nil : $0 } ?? .both
            } else {
                if shiftToggle != .none { defaults.set(shiftToggle.rawValue, forKey: Key.lastShiftSide) }
                shiftToggle = .none
            }
        }
    }

    var recordsCorrections: Bool {
        get { defaults.bool(forKey: Key.recordsCorrections) }
        set { defaults.set(newValue, forKey: Key.recordsCorrections); changed() }
    }
    var recordsCorrectionContext: Bool {
        get { defaults.bool(forKey: Key.recordsCorrectionContext) }
        set { defaults.set(newValue, forKey: Key.recordsCorrectionContext); changed() }
    }
    /// The settings recorded with each correction.
    var correctionSettings: CorrectionSettings {
        CorrectionSettings(options: composerOptions, fuzzy: fuzzySounds, precedingText: usesPrecedingText)
    }

    var fuzzySounds: FuzzySounds {
        get { FuzzySounds(rawValue: defaults.integer(forKey: Key.fuzzySounds)) }
        set { defaults.set(newValue.rawValue, forKey: Key.fuzzySounds); changed() }
    }

    var composerOptions: ComposerOptions {
        var o = ComposerOptions()
        o.selectionKeys = pageSelectionKeys
        o.spaceAction = spaceAction
        o.escapeClearsComposition = escapeClearsComposition
        o.learningEnabled = learningEnabled
        o.typoTolerance = typoTolerance.tolerance
        o.candidateAnchor = candidateAnchor == .after ? .afterCursor : .beforeCursor
        o.verticalList = candidateLayout == .vertical
        o.characterForms = characterForms
        o.punctuationKeys = punctuationKeys
        o.pageAtListEdge = pageAtListEdge
        return o
    }
}
