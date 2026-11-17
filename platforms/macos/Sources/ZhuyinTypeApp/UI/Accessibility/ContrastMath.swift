import AppKit

/// An sRGB colour, 0…1 per channel, for the WCAG 2 contrast math.
struct RGB: Hashable {
    var red: Double, green: Double, blue: Double

    static let white = RGB(red: 1, green: 1, blue: 1)
    static let black = RGB(red: 0, green: 0, blue: 0)

    /// From `0xRRGGBB`.
    init(hex: UInt32) {
        self.init(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
    }

    init(red: Double, green: Double, blue: Double) {
        self.red = red
        self.green = green
        self.blue = blue
    }

    /// The colour in sRGB, or nil for a pattern or a colour without components.
    init?(_ color: NSColor) {
        guard let srgb = color.usingColorSpace(.sRGB) else { return nil }
        self.init(red: Double(srgb.redComponent), green: Double(srgb.greenComponent), blue: Double(srgb.blueComponent))
    }

    var color: NSColor { NSColor(srgbRed: red, green: green, blue: blue, alpha: 1) }

    /// WCAG 2 relative luminance: each channel linearised from sRGB, weighted 0.2126, 0.7152, 0.0722.
    var luminance: Double {
        func linear(_ channel: Double) -> Double {
            channel <= 0.04045 ? channel / 12.92 : pow((channel + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }

    /// `amount` (0…1) of the way to `other`, per sRGB channel.
    func mixed(with other: RGB, _ amount: Double) -> RGB {
        RGB(red: red + (other.red - red) * amount, green: green + (other.green - green) * amount,
            blue: blue + (other.blue - blue) * amount)
    }
}

enum Contrast {
    /// WCAG 2 contrast ratio, (L1 + 0.05) / (L2 + 0.05) with L1 the lighter: 1 (same colour) to 21 (black on white).
    static func ratio(_ a: RGB, _ b: RGB) -> Double {
        let (la, lb) = (a.luminance, b.luminance)
        return (max(la, lb) + 0.05) / (min(la, lb) + 0.05)
    }

    /// A fill against what is around it (WCAG 1.4.11, non-text contrast).
    static let fill = 3.0
    /// Text against its fill (WCAG 1.4.3, normal text).
    static let text = 4.5

    /// The selection's fill and the text on it.
    struct Selection: Equatable {
        var fill: RGB
        var text: RGB
        /// How many darkening or lightening steps the accent took; 0 when it passed as it is.
        var steps: Int
        /// The accent never passed and the fill is `fallback`.
        var usedFallback: Bool
    }

    /// The fill steps darker (light appearance) or lighter (dark appearance) by this much of the way to black or white.
    static let step = 0.08
    /// At most this many steps (up to 48% of the way), then the fallback.
    static let maxSteps = 6

    /// The selection colours for `accent` on `background`: the accent itself when the fill has 3:1 against the
    /// background and white or black text has 4.5:1 on it; otherwise the accent darker (light appearance) or lighter
    /// (dark appearance) in steps until it does, else `fallback`.  White text is preferred (the platform's look); in
    /// the light appearance the fill only darkens, so white is the text there; in the dark one black is used when
    /// white does not pass.
    static func selection(accent: RGB, background: RGB, dark: Bool, fallback: RGB) -> Selection {
        for steps in 0...maxSteps {
            let fill = accent.mixed(with: dark ? .white : .black, step * Double(steps))
            if let text = text(on: fill, background: background, dark: dark) {
                return Selection(fill: fill, text: text, steps: steps, usedFallback: false)
            }
        }
        let text = ratio(fallback, .white) >= ratio(fallback, .black) ? RGB.white : .black
        return Selection(fill: fallback, text: text, steps: 0, usedFallback: true)
    }

    /// The text colour that passes on `fill`, or nil when the fill does not pass against `background` or no text does.
    private static func text(on fill: RGB, background: RGB, dark: Bool) -> RGB? {
        guard ratio(fill, background) >= Self.fill else { return nil }
        if ratio(fill, .white) >= Self.text { return .white }
        if dark, ratio(fill, .black) >= Self.text { return .black }
        return nil
    }
}
