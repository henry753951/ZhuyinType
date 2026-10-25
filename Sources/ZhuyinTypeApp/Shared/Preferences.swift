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

        static let all = [candidateLayout, candidateFontSize, selectionKeys, spaceOpensCandidates, escapeClearsComposition,
                          shiftToggle, showModeToast, startInEnglish, learningEnabled, popupCompositionApps]
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

    var composerOptions: ComposerOptions {
        var o = ComposerOptions()
        o.selectionKeys = Array(selectionKeys.rawValue)
        o.spaceOpensCandidates = spaceOpensCandidates
        o.escapeClearsComposition = escapeClearsComposition
        o.learningEnabled = learningEnabled
        return o
    }
}
