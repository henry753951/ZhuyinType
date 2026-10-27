import AppKit
import SwiftUI
import ZhuyinCore

/// `ZhuyinType --render-ui <dir>`: PNGs of the floating panels, every settings pane, the ⓘ cards and frames of each
/// demo, light and dark, for reviewing the look without typing.  Everything is drawn offscreen into windows the user
/// never sees; it uses a throwaway settings suite and engine with sample words, and never touches personal data,
/// the user's settings or the focused app.
enum UIPreviewRenderer {
    static func render(to directory: URL) {
        precondition(Thread.isMainThread)
        NSApp.setActivationPolicy(.accessory)
        NSApp.finishLaunching()
        // One fixed throwaway suite, emptied before and removed after.
        let suite = "org.zhuyintype.preview"
        guard let defaults = UserDefaults(suiteName: suite) else { return }
        defaults.removePersistentDomain(forName: suite)
        defer {
            defaults.removePersistentDomain(forName: suite)
            CFPreferencesAppSynchronize(suite as CFString)
            let file = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Preferences/\(suite).plist")
            try? FileManager.default.removeItem(at: file)
        }
        Preferences.shared = Preferences(defaults: defaults)
        SettingsWindowController.isPreview = true
        DemoPinning.stage = 12
        DemoPinning.context = 0
        DemoPinning.mini = 99
        do {
            for sub in ["panels", "settings", "popovers", "frames"] {
                try FileManager.default.createDirectory(at: directory.appendingPathComponent(sub), withIntermediateDirectories: true)
            }
            let settings = SettingsWindowController.shared
            settings.engine = try sampleEngine()
            for (name, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
                NSApp.appearance = NSAppearance(named: appearanceName)
                try renderPanels(to: directory.appendingPathComponent("panels"), suffix: name)
                try renderPopovers(to: directory.appendingPathComponent("popovers"), suffix: name)
                try renderFrames(to: directory.appendingPathComponent("frames"), suffix: name)
                try renderSettings(settings, to: directory.appendingPathComponent("settings"), suffix: name)
            }
            print("UI previews: \(directory.path)")
        } catch {
            fputs("UI preview failed: \(error.localizedDescription)\n", stderr)
        }
    }

    // MARK: Panels

    private static let words = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝"]

    private static func renderPanels(to directory: URL, suffix: String) throws {
        let list = CandidateList(items: words.map { Candidate(text: $0) }, selection: 1, pageSize: 9, tolerant: false)
        let corrections = CandidateList(items: [Candidate(text: "校名", annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ")] + list.items,
                                        selection: 0, pageSize: 9, tolerant: true)
        let hint = "ㄒㄧㄠˇ ㄇㄧㄥˊ"
        let keys = Array("123456789")
        for layout in CandidateLayout.allCases {
            for density in CandidateDensity.allCases {
                let style = CandidateStyle(layout: layout, fontSize: 18, density: density)
                try capturePanel(list: list, keys: keys, style: style, zhuyinHint: hint,
                                 to: directory.appendingPathComponent("candidate-\(layout.rawValue)-\(density.rawValue)-\(suffix).png"))
            }
            try capturePanel(list: corrections, keys: keys, style: CandidateStyle(layout: layout), zhuyinHint: hint,
                             to: directory.appendingPathComponent("candidate-\(layout.rawValue)-corrections-\(suffix).png"))
        }
        // Everything optional turned off, five to a page, large type.
        let minimal = CandidateStyle(layout: .vertical, fontSize: 24, density: .compact, showsZhuyinHint: false,
                                     showsAnnotations: false, showsListSwitches: false)
        var fivePerPage = corrections
        fivePerPage.pageSize = 5
        try capturePanel(list: fivePerPage, keys: Array("asdfg"), style: minimal, zhuyinHint: hint,
                         to: directory.appendingPathComponent("candidate-minimal-\(suffix).png"))
        for (file, notice) in [("select", "Enter 加入自訂詞：王小明"), ("saved", "已加入自訂詞：王小明"), ("invalid", "自訂詞需為 1 到 6 個字")] {
            try capturePanel(list: nil, keys: [], style: CandidateStyle(layout: .horizontal), notice: notice,
                             to: directory.appendingPathComponent("notice-\(file)-\(suffix).png"))
        }
        let badge = InputSurface(content: ModeBadgeView(frame: .zero))
        try capture(view: Backdrop(content: badge), to: directory.appendingPathComponent("mode-toast-\(suffix).png"))
    }

    private static func capturePanel(list: CandidateList?, keys: [Character], style: CandidateStyle, zhuyinHint: String? = nil,
                                     notice: String? = nil, to url: URL) throws {
        let content = CandidateContentView(frame: .zero)
        content.update(list: list, preview: nil, keys: keys, style: style, notice: notice, zhuyinHint: zhuyinHint)
        try capture(view: Backdrop(content: InputSurface(content: content)), to: url)
    }

    /// A document-coloured margin around a panel, so its material and contrast read as they do over text.
    private final class Backdrop: NSView {
        init(content: NSView) {
            super.init(frame: NSRect(x: 0, y: 0, width: content.frame.width + 40, height: content.frame.height + 40))
            content.setFrameOrigin(NSPoint(x: 20, y: 20))
            addSubview(content)
        }
        required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
        override func draw(_ dirtyRect: NSRect) {
            NSColor.textBackgroundColor.setFill()
            bounds.fill()
            NSColor.tertiaryLabelColor.withAlphaComponent(0.25).setFill()
            for (index, y) in stride(from: 10.0, to: bounds.height, by: 18).enumerated() {
                NSBezierPath(roundedRect: NSRect(x: 12, y: y, width: (bounds.width - 24) * (index % 2 == 0 ? 0.9 : 0.65),
                                                 height: 7), xRadius: 3.5, yRadius: 3.5).fill()
            }
        }
    }

    // MARK: Popovers and demos

    private static func renderPopovers(to directory: URL, suffix: String) throws {
        let cards: [(String, AnyView)] = [
            ("space", AnyView(InfoCard(title: "空白鍵") { SpaceInfo(current: .commit) })),
            ("escape", AnyView(InfoCard(title: "Esc") {
                ChoiceDemo(title: "清除整段組字", selected: true, frames: OptionDemos.escape(clearsComposition: true), pinned: 99)
            })),
            ("punctuation", AnyView(InfoCard(title: "標點按法") { PunctuationTable(current: .windows) })),
            ("anchor", AnyView(InfoCard(title: "游標在句中時改") {
                VStack(alignment: .leading, spacing: 14) {
                    ChoiceDemo(title: "游標右邊的字", selected: true, frames: OptionDemos.anchor(.after), pinned: 99)
                    ChoiceDemo(title: "游標左邊的字", selected: false, frames: OptionDemos.anchor(.before), pinned: 99)
                }
            })),
            ("typo", AnyView(InfoCard(title: "按錯鍵修正") { TypoInfo(current: .standard, correctionsKey: "⇧↑") })),
            ("fuzzy", AnyView(InfoCard(title: "模糊音") { FuzzyInfo() })),
            ("learned", AnyView(InfoCard(title: "記住的字詞") { LearnedInfo() })),
        ]
        for (name, card) in cards {
            try capture(swiftUI: card.background(Color(nsColor: .windowBackgroundColor)), width: 340,
                        to: directory.appendingPathComponent("info-\(name)-\(suffix).png"))
        }
    }

    private static func renderFrames(to directory: URL, suffix: String) throws {
        let preferences = Preferences.shared
        let stageSteps = [0, 6, 11, 12, 13, 14, 16, 19, 20, 22]
        let variants: [(String, () -> Void)] = [
            ("vertical", {}),
            ("horizontal", { preferences.candidateLayout = .horizontal }),
            ("above-compact", { preferences.candidatePlacement = .above; preferences.candidateDensity = .compact
                preferences.candidatesPerPage = 5 }),
        ]
        for (name, apply) in variants {
            preferences.resetToDefaults()
            apply()
            for step in stageSteps {
                let view = CandidateStage(preferences: preferences, pinned: step)
                    .padding(16)
                    .background(Color(nsColor: .windowBackgroundColor))
                try capture(swiftUI: view, width: 552, to: directory.appendingPathComponent(
                    "stage-\(name)-step\(String(format: "%02d", step))-\(suffix).png"))
            }
        }
        preferences.resetToDefaults()
        for scene in 0..<4 {
            let view = ContextIllustration(pinned: scene).padding(16).background(Color(nsColor: .windowBackgroundColor))
            try capture(swiftUI: view, width: 552, to: directory.appendingPathComponent("context-scene\(scene)-\(suffix).png"))
        }
        let demos: [(String, [DemoFrame])] = [
            ("space-open", OptionDemos.space(.openCandidates)), ("space-commit", OptionDemos.space(.commit)),
            ("space-commit-space", OptionDemos.space(.commitAndSpace)), ("anchor-after", OptionDemos.anchor(.after)),
            ("typo", OptionDemos.typo(correctionsKey: "⇧↑")),
        ]
        for (name, frames) in demos {
            for index in frames.indices {
                let view = MiniDemo(frames: frames, pinned: index).padding(12).background(Color(nsColor: .windowBackgroundColor))
                try capture(swiftUI: view, width: 340, to: directory.appendingPathComponent("demo-\(name)-frame\(index)-\(suffix).png"))
            }
        }
    }

    // MARK: Settings window

    private static func renderSettings(_ settings: SettingsWindowController, to directory: URL, suffix: String) throws {
        Preferences.shared.resetToDefaults()
        for pane in SettingsPane.allCases {
            let window = settings.makeWindow(pane: pane)
            window.setFrame(NSRect(x: -30000, y: -30000, width: 760, height: 600), display: false)
            window.alphaValue = 0
            window.orderFrontRegardless()
            settings.model.reloadLists()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.45))
            try capture(window: window, to: directory.appendingPathComponent("settings-\(pane.rawValue)-\(suffix).png"))
            window.orderOut(nil)
        }
        // Whole pages, taller than a screen: the pane alone in a borderless window, which is not fitted to the screen.
        let tall: [SettingsPane: CGFloat] = [.general: 1060, .keys: 1230, .candidates: 1370, .context: 740, .about: 620]
        for (pane, height) in tall.sorted(by: { $0.key.rawValue < $1.key.rawValue }) {
            let hosting = NSHostingView(rootView: SettingsPaneView(pane: pane, model: settings.model))
            hosting.frame = NSRect(x: 0, y: 0, width: 570, height: height)
            let window = NSWindow(contentRect: NSRect(x: -30000, y: -30000, width: 570, height: height),
                                  styleMask: [.borderless], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.backgroundColor = .windowBackgroundColor
            window.contentView = hosting
            window.alphaValue = 0
            window.orderFrontRegardless()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.45))
            try write(view: hosting, rect: hosting.bounds, to: directory.appendingPathComponent("settings-\(pane.rawValue)-full-\(suffix).png"))
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }
    }

    static func sampleEngine() throws -> Engine {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypePreview-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let now = Int(Date().timeIntervalSince1970)
        let learned = [("ㄐㄧㄣ ㄊㄧㄢ", "今天", 12), ("ㄊㄧㄢ ㄑㄧˋ", "天氣", 7), ("ㄒㄧㄝˋ ㄒㄧㄝ˙", "謝謝", 5),
                       ("ㄆㄥˊ ㄧㄡˇ", "朋友", 3), ("ㄍㄨㄥ ㄩㄢˊ", "公園", 2)]
        try learned.map { "\($0.0)\t\($0.1)\t\($0.2)\t\(now)" }.joined(separator: "\n")
            .write(to: directory.appendingPathComponent("learned.tsv"), atomically: true, encoding: .utf8)
        let phrases = [("ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ", "王小明"), ("ㄓㄨˋ ㄧㄣ", "注音"), ("ㄊㄞˊ ㄋㄢˊ", "台南")]
        try phrases.map { "\($0.0)\t\($0.1)" }.joined(separator: "\n")
            .write(to: directory.appendingPathComponent("user_phrases.tsv"), atomically: true, encoding: .utf8)
        let lexicon = (learned.map { "\($0.0)\t\($0.1)\t-8" } + phrases.map { "\($0.0)\t\($0.1)\t-9" }).joined(separator: "\n")
        return try Engine(lexiconText: lexicon, userDataDirectory: directory)
    }

    // MARK: Capture

    private static func capture(window: NSWindow, to url: URL) throws {
        guard let frameView = window.contentView?.superview else { return }
        frameView.layoutSubtreeIfNeeded()
        frameView.display()
        try write(view: frameView, rect: frameView.bounds, to: url)
    }

    private static func capture<V: View>(swiftUI view: V, width: CGFloat, to url: URL) throws {
        let hosting = NSHostingView(rootView: view.frame(width: width).fixedSize(horizontal: false, vertical: true))
        hosting.frame.size = NSSize(width: width, height: max(1, hosting.fittingSize.height))
        try capture(view: hosting, to: url, settle: 0.15)
    }

    private static func capture(view: NSView, to url: URL, settle: TimeInterval = 0) throws {
        let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: -30000, y: -30000), size: view.frame.size),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.backgroundColor = .clear
        window.isOpaque = false
        window.alphaValue = 0
        window.contentView = view
        window.orderFrontRegardless()
        view.layoutSubtreeIfNeeded()
        if settle > 0 { RunLoop.main.run(until: Date(timeIntervalSinceNow: settle)) }
        window.displayIfNeeded()
        try write(view: view, rect: view.bounds, to: url)
        window.orderOut(nil)
        window.contentView = nil
        window.close()
    }

    private static func write(view: NSView, rect: NSRect, to url: URL) throws {
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: rect) else { throw CocoaError(.fileWriteUnknown) }
        view.cacheDisplay(in: rect, to: bitmap)
        guard let data = bitmap.representation(using: .png, properties: [:]) else { throw CocoaError(.fileWriteUnknown) }
        try data.write(to: url, options: .atomic)
    }
}
