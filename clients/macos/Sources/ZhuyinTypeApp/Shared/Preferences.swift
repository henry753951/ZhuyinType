// User settings, stored in the app's UserDefaults domain.  The settings window binds to these; the input controller
// reads them and listens for `Preferences.didChange`.
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

enum SelectionKeySet: String, CaseIterable, Identifiable {
    case digits = "123456789"
    case homeRow = "asdfghjkl"
    var id: String { rawValue }
}

final class Preferences: ObservableObject {
    static let shared = Preferences()
    static let didChange = Notification.Name("ZhuyinTypePreferencesDidChange")

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        defaults.register(defaults: [
            Key.candidateLayout: CandidateLayout.vertical.rawValue,
            Key.candidateFontSize: 18.0,
            Key.selectionKeys: SelectionKeySet.digits.rawValue,
            Key.spaceOpensCandidates: true,
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
        ])
    }

    enum Key {
        static let candidateLayout = "candidateLayout"
        static let candidateFontSize = "candidateFontSize"
        static let selectionKeys = "selectionKeys"
        static let spaceOpensCandidates = "spaceOpensCandidates"
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

        static let all = [candidateLayout, candidateFontSize, selectionKeys, spaceOpensCandidates, escapeClearsComposition,
                          shiftToggle, showModeToast, startInEnglish, learningEnabled, popupCompositionApps,
                          typoTolerance, candidateAnchor, fuzzySounds, usesPrecedingText, shortcuts, characterForms]
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
    var spaceOpensCandidates: Bool {
        get { defaults.bool(forKey: Key.spaceOpensCandidates) }
        set { defaults.set(newValue, forKey: Key.spaceOpensCandidates); changed() }
    }
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

    var fuzzySounds: FuzzySounds {
        get { FuzzySounds(rawValue: defaults.integer(forKey: Key.fuzzySounds)) }
        set { defaults.set(newValue.rawValue, forKey: Key.fuzzySounds); changed() }
    }

    var composerOptions: ComposerOptions {
        var o = ComposerOptions()
        o.selectionKeys = Array(selectionKeys.rawValue)
        o.spaceOpensCandidates = spaceOpensCandidates
        o.escapeClearsComposition = escapeClearsComposition
        o.learningEnabled = learningEnabled
        o.typoTolerance = typoTolerance.tolerance
        o.candidateAnchor = candidateAnchor == .after ? .afterCursor : .beforeCursor
        o.verticalList = candidateLayout == .vertical
        o.characterForms = characterForms
        return o
    }
}
