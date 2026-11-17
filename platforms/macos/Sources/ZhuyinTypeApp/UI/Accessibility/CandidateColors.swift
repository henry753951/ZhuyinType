import AppKit

/// The 強調色 setting (`accentColor`): the system's accent, or ZhuyinType's indigo.  The same key and values on every
/// platform.
enum AccentColorSetting: String, CaseIterable, Identifiable {
    case system, zhuyintype
    var id: String { rawValue }
}

/// The candidate window's accent colours: the selection's fill, the text on it, and the footer switches while on.
/// Every accent in the window comes from here, checked for contrast (`Contrast.selection`) in the appearance it is
/// drawn in.
enum CandidateColors {
    /// ZhuyinType's indigo, and the fallback for an accent that cannot be made to pass.
    static let indigoLight = RGB(hex: 0x4257D8)
    static let indigoDark = RGB(hex: 0x7A8CF0)

    /// The selection's fill for `setting`, resolved in the appearance it is drawn in.
    static func accent(_ setting: AccentColorSetting) -> NSColor {
        NSColor(name: nil) { selection(setting, in: $0).fill.color }
    }

    /// The text (and key label) on the selection's fill.
    static func onAccent(_ setting: AccentColorSetting) -> NSColor {
        NSColor(name: nil) { selection(setting, in: $0).text.color }
    }

    private struct Key: Hashable {
        var setting: AccentColorSetting, accent: RGB, background: RGB, dark: Bool
    }
    private static var cache: [Key: Contrast.Selection] = [:]

    /// The selection colours in `appearance`: the accent against the window's background colour there.
    static func selection(_ setting: AccentColorSetting, in appearance: NSAppearance) -> Contrast.Selection {
        let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        var accent = dark ? indigoDark : indigoLight
        var background = dark ? RGB(hex: 0x323232) : RGB(hex: 0xECECEC)
        appearance.performAsCurrentDrawingAppearance {
            if setting == .system, let system = RGB(NSColor.controlAccentColor) { accent = system }
            if let window = RGB(NSColor.windowBackgroundColor) { background = window }
        }
        let key = Key(setting: setting, accent: accent, background: background, dark: dark)
        if let known = cache[key] { return known }
        let result = Contrast.selection(accent: accent, background: background, dark: dark,
                                        fallback: dark ? indigoDark : indigoLight)
        cache[key] = result
        return result
    }
}

/// Increase Contrast and Reduce Transparency, for the floating panels: either one gives them a solid background;
/// Increase Contrast also a 2 pt border and an outlined selection.
enum DisplayOptions {
    /// Set by the UI previews and self-tests instead of the system's settings.
    static var override: (increaseContrast: Bool, reduceTransparency: Bool)?

    static var increaseContrast: Bool {
        override?.increaseContrast ?? NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
    }
    static var reduceTransparency: Bool {
        override?.reduceTransparency ?? NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
    }
    /// No glass or material: an opaque background.
    static var solidBackground: Bool { increaseContrast || reduceTransparency }

    /// The panel's border under Increase Contrast.
    static let borderWidth: CGFloat = 2
    static var borderColor: NSColor { NSColor.labelColor.withAlphaComponent(0.6) }
    /// The outline drawn around the selection under Increase Contrast.
    static let selectionOutline: CGFloat = 2
}
