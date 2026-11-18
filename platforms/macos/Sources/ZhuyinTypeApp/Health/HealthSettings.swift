import Foundation
import ZhuyinEngine

/// 回報問題's 改過的設定: the stored settings that differ from their defaults, by key.  A value is written only when it
/// comes from a closed set (on/off, a choice, a number); anything else (lists such as 使用候選字視窗組字的 App, key
/// bindings, text, keys this version does not know) is the key alone, so no app list or word ever reaches the report.
enum HealthSettings {
    struct Change: Equatable {
        var key: String
        /// 「開啟」, 「hsu」, 「18」; nil: written 「（已設定）」.
        var value: String?

        var detail: HealthDetail { HealthDetail(key, value ?? "（已設定）") }
    }

    /// `stored`: the app's own UserDefaults domain, as persisted (not the registered defaults).
    static func changes(_ stored: [String: Any]) -> [Change] {
        stored.keys.sorted().compactMap { key -> Change? in
            guard !ignored(key), let value = stored[key] else { return nil }
            let fallback = defaults[key]
            if let fallback, (value as AnyObject).isEqual(fallback) { return nil }
            guard Preferences.Key.all.contains(key) else { return Change(key: key, value: nil) }
            return Change(key: key, value: closedValue(value, key: key, default: fallback))
        }
    }

    /// The value as the report writes it, when it is one of a closed set.
    private static func closedValue(_ value: Any, key: String, default fallback: Any?) -> String? {
        if isBoolean(value) { return fallback.map(isBoolean) == false ? nil : ((value as? Bool) == true ? "開啟" : "關閉") }
        if let number = value as? NSNumber {
            guard let fallback = fallback as? NSNumber, !isBoolean(fallback) else { return nil }
            let double = number.doubleValue
            guard double.isFinite else { return nil }
            return double == double.rounded() && abs(double) < 1e15 ? String(Int64(double)) : String(double)
        }
        if let text = value as? String, choices[key]?.contains(text) == true { return text }
        return nil
    }

    private static func isBoolean(_ value: Any) -> Bool { CFGetTypeID(value as CFTypeRef) == CFBooleanGetTypeID() }

    /// Every default `Preferences` registers, and those it applies in code without registering.
    private static let defaults: [String: Any] = {
        let storage = MemorySettingsStorage()
        _ = Preferences(defaults: storage)
        var values: [String: Any] = [
            Preferences.Key.spaceAction: SpaceAction.commitAndSpace.rawValue,
            Preferences.Key.pageAtListEdge: true,
            Preferences.Key.legacySpaceOpensCandidates: false,
        ]
        for key in Preferences.Key.all { if let value = storage.object(forKey: key) { values[key] = value } }
        return values
    }()

    /// The settings whose values are one of a fixed list.
    private static let choices: [String: Set<String>] = {
        typealias Key = Preferences.Key
        func raw<T: CaseIterable & RawRepresentable>(_: T.Type) -> Set<String> where T.RawValue == String {
            Set(T.allCases.map(\.rawValue))
        }
        return [
            Key.candidateLayout: raw(CandidateLayout.self), Key.selectionKeys: raw(SelectionKeySet.self),
            Key.spaceAction: raw(SpaceAction.self), Key.candidatePlacement: raw(CandidatePlacement.self),
            Key.candidateDensity: raw(CandidateDensity.self), Key.panelAppearance: raw(PanelAppearance.self),
            Key.accentColor: raw(AccentColorSetting.self), Key.shiftToggle: raw(ShiftToggle.self),
            Key.typoTolerance: raw(TypoSetting.self), Key.candidateAnchor: raw(AnchorSetting.self),
            Key.characterForms: raw(CharacterForms.self), Key.outputScript: raw(OutputScript.self),
            Key.punctuationKeys: raw(PunctuationKeys.self),
        ]
    }()

    /// Not settings: the system's and Sparkle's own keys, and what the app remembers for itself (the last settings
    /// page, the setup wizard's version, 中/英 across a restart, a hint's count, bookkeeping beside a setting).
    private static func ignored(_ key: String) -> Bool {
        if ["NS", "SU", "Apple", "com.apple."].contains(where: key.hasPrefix) { return true }
        return ["settingsLastPane", "setupWizardSeenVersion", "ZhuyinTypeModeAcrossRestart", "wordActionHintCount",
                Preferences.Key.keyBindingsVersion, Preferences.Key.lastShiftSide].contains(key)
    }
}
