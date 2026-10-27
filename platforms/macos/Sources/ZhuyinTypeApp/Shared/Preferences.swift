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
        static let shortcuts = "shortcuts"
        static let characterForms = "characterForms"
        static let punctuationKeys = "punctuationKeys"
        /// 「記錄改字」: compositions with a correction go to corrections.jsonl (CorrectionRecorder).  Off by default.
        static let recordsCorrections = "recordsCorrections"
        /// With it, the text before the insertion point the engine read.  Off by default.
        static let recordsCorrectionContext = "recordsCorrectionContext"

        static let all = [candidateLayout, candidateFontSize, selectionKeys, legacySpaceOpensCandidates, spaceAction,
                          escapeClearsComposition, shiftToggle, showModeToast, startInEnglish, learningEnabled,
                          popupCompositionApps, typoTolerance, candidateAnchor, fuzzySounds, usesPrecedingText, shortcuts,
                          characterForms, punctuationKeys, candidatesPerPage, candidatePlacement, candidateDensity,
                          showsZhuyinHint, showsAnnotations, showsListSwitches, panelAppearance, panelAnimation,
                          recordsCorrections, recordsCorrectionContext]
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
    /// What Space does with no syllable being typed; by default it commits.  Not registered, so that a choice saved
    /// by an earlier version (the Bool) is kept.
    var spaceAction: SpaceAction {
        get {
            if let action = defaults.string(forKey: Key.spaceAction).flatMap(SpaceAction.init(rawValue:)) { return action }
            if let opens = defaults.object(forKey: Key.legacySpaceOpensCandidates) as? Bool { return opens ? .openCandidates : .commit }
            return .commit
        }
        set {
            defaults.removeObject(forKey: Key.legacySpaceOpensCandidates)
            defaults.set(newValue.rawValue, forKey: Key.spaceAction)
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
    var candidateAnchor: AnchorSetting {
        get { AnchorSetting(rawValue: defaults.string(forKey: Key.candidateAnchor) ?? "") ?? .after }
        set { defaults.set(newValue.rawValue, forKey: Key.candidateAnchor); changed() }
    }

    /// Every rebindable command's shortcut: the user's where set, otherwise the default.
    var shortcuts: [ShortcutCommand: Shortcut] {
        get {
            var result = ShortcutCommand.defaults
            if let data = defaults.data(forKey: Key.shortcuts),
               let stored = try? JSONDecoder().decode([String: Shortcut].self, from: data) {
                for (name, shortcut) in stored {
                    if let command = ShortcutCommand(rawValue: name) { result[command] = shortcut }
                }
            }
            return result
        }
        set {
            let stored = Dictionary(uniqueKeysWithValues: newValue.map { ($0.key.rawValue, $0.value) })
            defaults.set(try? JSONEncoder().encode(stored), forKey: Key.shortcuts)
            changed()
        }
    }

    var candidateKeyLabels: CandidateKeyLabels {
        let all = shortcuts
        return CandidateKeyLabels(zhuyin: all[.typeZhuyin]?.compact ?? "`",
                                  single: all[.singleCharacterList]?.compact ?? "⇧↓",
                                  corrections: all[.correctionsList]?.compact ?? "⇧↑")
    }

    func resetShortcuts() {
        defaults.removeObject(forKey: Key.shortcuts)
        changed()
    }

    var usesPrecedingText: Bool {
        get { defaults.bool(forKey: Key.usesPrecedingText) }
        set { defaults.set(newValue, forKey: Key.usesPrecedingText); changed() }
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
        return o
    }
}
