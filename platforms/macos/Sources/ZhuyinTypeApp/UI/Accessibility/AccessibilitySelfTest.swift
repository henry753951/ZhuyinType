import AppKit
import ZhuyinEngine

/// The self-tests of the contrast math, the accent colours, the VoiceOver announcements and the solid panel, run by
/// `--self-test` with the ControllerCore ones.  Nothing here shows a window or posts to VoiceOver.
enum AccessibilitySelfTest {
    private struct Failure: Error, CustomStringConvertible { var description: String }

    static func run(test: (String, () throws -> Void) -> Void) {
        func expect(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String) throws {
            if !condition() { throw Failure(description: message()) }
        }
        func near(_ a: Double, _ b: Double, _ tolerance: Double = 0.01) -> Bool { abs(a - b) <= tolerance }

        test("contrast: WCAG luminance and ratios") {
            try expect(near(RGB.white.luminance, 1) && near(RGB.black.luminance, 0), "white or black luminance")
            try expect(near(Contrast.ratio(.black, .white), 21), "black on white is \(Contrast.ratio(.black, .white))")
            try expect(near(Contrast.ratio(.white, .black), 21), "the ratio depends on the order")
            try expect(near(Contrast.ratio(RGB(hex: 0x4257D8), RGB(hex: 0x4257D8)), 1), "a colour on itself is not 1")
            let grey = Contrast.ratio(RGB(hex: 0x777777), .white)
            try expect(near(grey, 4.48), "#777 on white is \(grey), not 4.48")
            // The sRGB curve's linear part.
            try expect(near(RGB(hex: 0x0A0A0A).luminance, (10.0 / 255) / 12.92, 1e-6), "the linear part of sRGB")
        }

        test("contrast: white text at 4.5:1 on the selection, or the indigo") {
            func passes(_ s: Contrast.Selection) -> Bool { s.text == .white && Contrast.ratio(s.fill, .white) >= Contrast.text }
            // The light indigo passes as it is; the dark one is darkened a little (white on #7A8CF0 is 3.1:1).
            let indigoLight = Contrast.selection(accent: CandidateColors.indigoLight, fallback: CandidateColors.indigoLight)
            try expect(indigoLight.steps == 0 && !indigoLight.usedFallback && passes(indigoLight), "indigo (light): \(indigoLight)")
            let indigoDark = Contrast.selection(accent: CandidateColors.indigoDark, fallback: CandidateColors.indigoLight)
            try expect(indigoDark.steps > 0 && !indigoDark.usedFallback && passes(indigoDark), "indigo (dark): \(indigoDark)")
            // macOS's accents in both appearances: darkened until white text passes, never black text.
            for (name, hex) in [("blue", UInt32(0x007AFF)), ("dark blue", 0x0A84FF), ("yellow", 0xFFC600), ("graphite", 0x8C8C8C),
                                ("pink", 0xF74F9E), ("purple", 0x6E3BD9)] {
                let s = Contrast.selection(accent: RGB(hex: hex), fallback: CandidateColors.indigoLight)
                try expect(!s.usedFallback && passes(s), "\(name): \(s)")
                try expect(s.steps == 0 || s.fill.luminance < RGB(hex: hex).luminance, "\(name) was not darkened")
            }
            // An accent that cannot pass even at its darkest usable shade: the indigo with white.
            let white = Contrast.selection(accent: .white, fallback: CandidateColors.indigoLight)
            try expect(white.usedFallback && white.fill == CandidateColors.indigoLight && white.text == .white,
                       "white did not fall back: \(white)")
        }

        test("accent: both settings pass in light and dark, and the setting is stored as the other platforms do") {
            for name in [NSAppearance.Name.aqua, .darkAqua] {
                guard let appearance = NSAppearance(named: name) else { continue }
                for setting in AccentColorSetting.allCases {
                    let s = CandidateColors.selection(setting, in: appearance)
                    try expect(Contrast.ratio(s.fill, s.text) >= Contrast.text, "\(setting) in \(name.rawValue): \(s)")
                }
                let ours = CandidateColors.selection(.zhuyintype, in: appearance)
                try expect(ours.text == .white && (name == .darkAqua || ours.fill == CandidateColors.indigoLight),
                           "ZhuyinType 藍 in \(name.rawValue) is \(ours)")
            }
            let p = Preferences(defaults: MemorySettingsStorage())
            try expect(p.accentColor == .system && p.candidateStyle.accentColor == .system, "the default is not 跟隨系統")
            p.accentColor = .zhuyintype
            try expect(p.candidateStyle.accentColor == .zhuyintype, "the window's style does not follow 強調色")
            try expect(AccentColorSetting.allCases.map(\.rawValue) == ["system", "zhuyintype"], "values differ from Linux/Windows")
            try expect(Preferences.Key.all.contains(Preferences.Key.accentColor), "還原預設 leaves 強調色")
            p.candidateFontSize = 40
            try expect(p.candidateFontSize == 36, "字級 is not clamped to 36: \(p.candidateFontSize)")
            p.candidateFontSize = 36
            try expect(p.candidateFontSize == 36, "字級 36 is not kept")
            p.candidateFontSize = 9
            try expect(p.candidateFontSize == 13, "字級 is not clamped to 13")
        }

        test("VoiceOver: what a new list, a page turn and a moved highlight say") {
            let items = ["在", "再", "載", "仔", "宰", "崽", "縡", "酨", "睵", "賳", "儎"].map { Candidate(text: $0) }
            let keys = Array("123456789")
            var list = CandidateList(items: items, selection: 0, pageSize: 9)
            let opened = CandidateAnnouncer.announcement(for: list, keys: keys, reading: "ㄗㄞˋ", previous: nil)
            try expect(opened == "ㄗㄞˋ，候選字清單，共 11 個，2 頁，候選字 1，在", "opened: \(opened ?? "nil")")
            let shown = CandidateAnnouncer.Shown(list)
            try expect(CandidateAnnouncer.announcement(for: list, keys: keys, reading: "ㄗㄞˋ", previous: shown) == nil,
                       "the same list was announced again")
            list.selection = 2
            let moved = CandidateAnnouncer.announcement(for: list, keys: keys, reading: "ㄗㄞˋ", previous: shown)
            try expect(moved == "候選字 3，載", "moved: \(moved ?? "nil")")
            let before = CandidateAnnouncer.Shown(list)
            list.selection = 9
            let paged = CandidateAnnouncer.announcement(for: list, keys: keys, reading: nil, previous: before)
            try expect(paged == "第 2 頁，共 2 頁，候選字 1，賳", "paged: \(paged ?? "nil")")
            // Fewer keys than rows: the row's position.
            let unkeyed = CandidateAnnouncer.announcement(for: CandidateList(items: items, selection: 4, pageSize: 9), keys: Array("12"),
                                                          reading: nil, previous: shown)
            try expect(unkeyed == "候選字 5，宰", "unkeyed: \(unkeyed ?? "nil")")
            // Another list: its own name; a corrected reading is read after the text.
            let corrections = CandidateList(items: [Candidate(text: "再", annotation: "ㄗㄞˋ")], pageSize: 9, tolerant: true)
            let switched = CandidateAnnouncer.announcement(for: corrections, keys: keys, reading: nil, previous: before)
            try expect(switched == "修正的字，共 1 個，候選字 1，再，ㄗㄞˋ", "corrections: \(switched ?? "nil")")
            try expect(CandidateAnnouncer.mode(english: true) == "英文" && CandidateAnnouncer.mode(english: false) == "中文", "mode")
            try expect(CandidateAnnouncer.announcement(for: CandidateList(items: []), keys: keys, reading: nil, previous: nil) == nil,
                       "an empty list was announced")
        }

        test("VoiceOver: the candidate window is a list of its rows, the selected one marked") {
            _ = NSApplication.shared
            let content = CandidateContentView(frame: .zero)
            let list = CandidateList(items: ["在", "再", "載"].map { Candidate(text: $0) }, selection: 1, pageSize: 9)
            content.update(list: list, preview: nil, keys: Array("123"), style: CandidateStyle())
            func find(_ view: NSView) -> NSView? {
                if view.accessibilityRole() == .list { return view }
                return view.subviews.lazy.compactMap(find).first
            }
            guard let element = find(content) else { throw Failure(description: "no list element") }
            let rows = (element.accessibilityChildren() ?? []).compactMap { $0 as? NSView }
            try expect(rows.map { $0.accessibilityLabel() ?? "" } == ["1，在", "2，再", "3，載"],
                       "rows: \(rows.map { $0.accessibilityLabel() ?? "" })")
            let selected = (element.accessibilitySelectedChildren() ?? []).compactMap { ($0 as? NSView)?.accessibilityLabel() }
            try expect(selected == ["2，再"], "selected: \(selected)")
        }

        test("Increase Contrast and Reduce Transparency: a solid panel, the border only with Increase Contrast; 36 pt fits") {
            _ = NSApplication.shared
            let saved = DisplayOptions.override
            defer { DisplayOptions.override = saved }
            for (contrast, transparency) in [(true, false), (false, true), (true, true), (false, false)] {
                DisplayOptions.override = (contrast, transparency)
                let content = CandidateContentView(frame: .zero)
                content.update(list: CandidateList(items: [Candidate(text: "在")]), preview: nil, keys: [], style: CandidateStyle())
                let surface = InputSurface(content: content)
                let layer = content.layer
                try expect(surface.isSolid == (contrast || transparency), "solid \(surface.isSolid) for \(contrast), \(transparency)")
                try expect((layer?.backgroundColor?.alpha ?? 0) == (contrast || transparency ? 1 : 0),
                           "background alpha \(layer?.backgroundColor?.alpha ?? 0) for \(contrast), \(transparency)")
                try expect((layer?.borderWidth ?? 0) == (contrast ? 2 : 0), "border \(layer?.borderWidth ?? 0) for \(contrast)")
            }
            // Live: switching back off clears the background and the border.
            DisplayOptions.override = (true, false)
            let content = CandidateContentView(frame: .zero)
            let surface = InputSurface(content: content)
            DisplayOptions.override = (false, false)
            surface.applyDisplayOptions()
            try expect(!surface.isSolid && content.layer?.backgroundColor == nil && content.layer?.borderWidth == 0, "not cleared")
            // The largest 字級: rows as tall as the text, the window within the screen's width.
            let big = CandidateContentView(frame: .zero)
            var style = CandidateStyle()
            style.fontSize = 36
            let items = (0..<9).map { Candidate(text: String(repeating: "注音", count: $0 % 3 + 1)) }
            let size = big.update(list: CandidateList(items: items + items), preview: "注音輸入", keys: Array("123456789"),
                                  style: style, maxWidth: 800, zhuyinHint: "ㄓㄨˋ ㄧㄣ")
            try expect(size.width <= 800 && size.height >= 9 * ceil(36 * 1.55), "36 pt: \(size)")
            style.layout = .horizontal
            let wide = big.update(list: CandidateList(items: items + items), preview: nil, keys: Array("123456789"),
                                  style: style, maxWidth: 800)
            try expect(wide.width <= 800 && wide.height >= ceil(36 * 1.55), "36 pt horizontal: \(wide)")
        }
    }
}
