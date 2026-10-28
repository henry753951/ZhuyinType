import AppKit
import SwiftUI
import ZhuyinEngine

/// `ZhuyinType --render-ui <dir>`: PNGs of the floating panels, every settings pane, the ⓘ cards and frames of each
/// demo, light and dark, for reviewing the look without typing.  Everything is drawn offscreen into windows the user
/// never sees; it uses a throwaway settings suite and engine with sample words, and never touches personal data,
/// the user's settings or the focused app.
enum UIPreviewRenderer {
    /// Rows whose trailing block was not centred on the leading block, by page; the render fails when any.
    private static var misaligned: [String] = []
    /// Wizard pages whose content is taller than the window gives it (nothing scrolls, so it would be cut off).
    private static var wizardOverflow: [String] = []

    /// Returns false when a page has a misaligned row.
    @discardableResult
    /// `pages`: only the settings window's page with this raw value (and the panels), light only, for a short check.
    static func render(to directory: URL, pages only: String? = nil) -> Bool {
        precondition(Thread.isMainThread)
        // Never active, never key: the user keeps typing in their own app while this runs (Headless).
        NSApp.setActivationPolicy(.prohibited)
        NSApp.finishLaunching()
        let focus = Headless.FocusGuard()
        // One fixed throwaway suite, emptied before and removed after.
        let suite = "org.zhuyintype.preview"
        guard let defaults = UserDefaults(suiteName: suite) else { return false }
        defaults.removePersistentDomain(forName: suite)
        defer {
            defaults.removePersistentDomain(forName: suite)
            CFPreferencesAppSynchronize(suite as CFString)
            let file = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Preferences/\(suite).plist")
            try? FileManager.default.removeItem(at: file)
        }
        Preferences.shared = Preferences(defaults: defaults)
        UsageStatsStore.shared = sampleStats()
        SettingsWindowController.isPreview = true
        DemoPinning.still = true
        DemoPinning.context = 0
        DemoPinning.intro = 12
        DemoPinning.mini = 99
        do {
            for sub in ["panels", "settings", "popovers", "frames"] {
                try FileManager.default.createDirectory(at: directory.appendingPathComponent(sub), withIntermediateDirectories: true)
            }
            let settings = SettingsWindowController.shared
            CorrectionLog.shared = try sampleCorrectionLog()
            settings.engine = try sampleEngine()
            if only == "live" || only == nil {
                try renderLive(to: directory.appendingPathComponent("live"))
            }
            if only == "storyboard" || only == "wizard" || only == nil {
                try renderWizard(to: directory.appendingPathComponent("wizard"))
            }
            if only == "storyboard" {
                // The planned 設定嚮導, frame by frame, for docs/product/SETUP_WIZARD.md.
                try FileManager.default.createDirectory(at: directory.appendingPathComponent("storyboard"), withIntermediateDirectories: true)
                for (name, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
                    NSApp.appearance = NSAppearance(named: appearanceName)
                    Preferences.shared.resetToDefaults()
                    Preferences.shared.punctuationKeys = .windows
                    for frame in StoryboardFrame.allCases {
                        // The practice field's window, small enough to sit in the wizard.
                        Preferences.shared.candidateFontSize = [.paging, .success].contains(frame) ? 14 : 18
                        Preferences.shared.candidateDensity = [.paging, .success].contains(frame) ? .compact : .standard
                        try capture(swiftUI: WizardStoryboard(frame: frame, preferences: Preferences.shared), width: 720,
                                    to: directory.appendingPathComponent("storyboard/\(frame.rawValue)-\(name).png"), settle: 0.3)
                    }
                }
            }
            if only == "practice" {
                // The setup wizard's practice field, driven by synthetic keys (UI/Wizard/Practice).
                try PracticeRender.render(to: directory.appendingPathComponent("practice"))
            }
            if let only, let pane = SettingsPane(rawValue: only) {
                NSApp.appearance = NSAppearance(named: .aqua)
                try renderPanels(to: directory.appendingPathComponent("panels"), suffix: "light")
                Preferences.shared.resetToDefaults()
                RowGeometry.recording = true
                RowGeometry.reset()
                let window = settings.makeWindow(pane: pane)
                window.setContentSize(NSSize(width: 760, height: 600))
                window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
                window.alphaValue = 0
                window.orderFrontRegardless()
                settings.model.reloadLists()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
                try capture(window: window, to: directory.appendingPathComponent("settings/grid-\(pane.rawValue)-light.png"), grid: true)
                RowGeometry.recording = false
                retire(window)
            }
            for (name, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] where only == nil {
                NSApp.appearance = NSAppearance(named: appearanceName)
                try renderPanels(to: directory.appendingPathComponent("panels"), suffix: name)
                try renderPopovers(to: directory.appendingPathComponent("popovers"), suffix: name)
                try renderFrames(to: directory.appendingPathComponent("frames"), suffix: name)
                try renderSettings(settings, to: directory.appendingPathComponent("settings"), suffix: name)
            }
            print("UI previews: \(directory.path)")
        } catch {
            fputs("UI preview failed: \(error.localizedDescription)\n", stderr)
            return false
        }
        let focusProblems = focus.check()
        if !focusProblems.isEmpty {
            print("FAIL focus: the render took focus from the user:")
            focusProblems.forEach { print("  \($0)") }
            return false
        }
        print("Focus: ZhuyinType never became active and no window became key")
        if only == nil || only == "wizard" {
            if wizardOverflow.isEmpty {
                print("Wizard fit: every page fits the window at \(Int(WizardLayout.minSize.width))×\(Int(WizardLayout.minSize.height)), "
                      + "\(Int(WizardLayout.defaultSize.width))×\(Int(WizardLayout.defaultSize.height)) and "
                      + "\(Int(WizardLayout.maxSize.width))×\(Int(WizardLayout.maxSize.height)) without scrolling")
            } else {
                print("FAIL wizard fit: \(wizardOverflow.count) pages taller than the window:")
                wizardOverflow.forEach { print("  \($0)") }
                return false
            }
        }
        if misaligned.isEmpty {
            print("Row geometry: every trailing block centred on its leading block; every card balanced")
            return true
        }
        print("FAIL row geometry: \(misaligned.count) problems (trailing block not centred, or a card unbalanced):")
        misaligned.forEach { print("  \($0)") }
        return false
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
        let fivePerPage = CandidateList(items: corrections.items, selection: corrections.selection, pageSize: 5,
                                        tolerant: corrections.tolerant, singleCharacter: corrections.singleCharacter)
        try capturePanel(list: fivePerPage, keys: Array("asdfg"), style: minimal, zhuyinHint: hint,
                         to: directory.appendingPathComponent("candidate-minimal-\(suffix).png"))
        for (file, notice) in [("select", "Enter 加入自訂詞：王小明"), ("saved", "已加入自訂詞：王小明"), ("invalid", "自訂詞需為 1 到 6 個字")] {
            try capturePanel(list: nil, keys: [], style: CandidateStyle(layout: .horizontal), notice: notice,
                             to: directory.appendingPathComponent("notice-\(file)-\(suffix).png"))
        }
        let badge = InputSurface.modeBadge()
        try capture(view: Backdrop(content: badge), to: directory.appendingPathComponent("mode-toast-\(suffix).png"))
        // Turning pages and the end of the list, frame by frame (the animation's own easing, drawn still).
        let many = CandidateList(items: (words + ["瞑", "洺", "螟蛉", "明明", "名命"]).map { Candidate(text: $0) }, selection: 0,
                                 pageSize: 9, tolerant: false)
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("paging"), withIntermediateDirectories: true)
        for layout in CandidateLayout.allCases {
            let style = CandidateStyle(layout: layout, fontSize: 18)
            for (name, from, to, kind, direction) in [("next", 0, 9, CandidateContentView.PinnedMotion.page, 1),
                                                      ("previous", 9, 0, .page, -1), ("end", 9, 9, .bounce, 1)] {
                for progress in [0.0, 0.2, 0.35, 0.5, 1.0] {
                    let content = CandidateContentView(frame: .zero)
                    var list = many
                    list.selection = from
                    content.update(list: list, preview: nil, keys: keys, style: style, zhuyinHint: hint)
                    content.pinnedMotion = (kind, direction, progress)
                    list.selection = to
                    content.update(list: list, preview: nil, keys: keys, style: style, zhuyinHint: hint)
                    try capture(view: Backdrop(content: InputSurface(content: content)), to: directory.appendingPathComponent(
                        "paging/\(layout.rawValue)-\(name)-\(Int(progress * 100))-\(suffix).png"))
                }
            }
        }
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
            ("space", AnyView(InfoCard(title: "空白鍵") { SpaceInfo(current: .insertSpace) })),
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
            ("corrections", AnyView(InfoCard(title: "記錄改字") { CorrectionLogInfo(keys: CurrentKeys(Preferences.shared)) })),
        ]
        for (name, card) in cards {
            try capture(swiftUI: card.background(Color(nsColor: .windowBackgroundColor)), width: 340,
                        to: directory.appendingPathComponent("info-\(name)-\(suffix).png"))
        }
        // The correction log's keys follow the shortcuts.
        customKeys(Preferences.shared)
        let custom = InfoCard(title: "記錄改字") { CorrectionLogInfo(keys: CurrentKeys(Preferences.shared)) }
        try capture(swiftUI: custom.background(Color(nsColor: .windowBackgroundColor)), width: 340,
                    to: directory.appendingPathComponent("info-corrections-custom-keys-\(suffix).png"))
        Preferences.shared.resetToDefaults()
    }

    private static func renderFrames(to directory: URL, suffix: String) throws {
        let preferences = Preferences.shared
        let variants: [(String, () -> Void)] = [
            ("default", {}),
            ("custom-keys", { customKeys(preferences) }),
            ("features-off", { featuresOff(preferences) }),
            ("horizontal", { preferences.candidateLayout = .horizontal }),
            ("above-compact", { preferences.candidatePlacement = .above; preferences.candidateDensity = .compact
                preferences.candidatesPerPage = 5; preferences.candidateFontSize = 22 }),
        ]
        for (name, apply) in variants {
            preferences.resetToDefaults()
            apply()
            try capturePreview(preferences, to: directory.appendingPathComponent("preview-\(name)-\(suffix).png"))
        }
        preferences.resetToDefaults()
        // The intro story, frame by frame (keycap at rest).
        let introSteps = IntroScript(preferences: preferences).steps.count
        for step in [0, 6, 11, 12, 14, 16, 19, 20, 21, introSteps - 2] {
            let view = IntroStage(preferences: preferences, pinned: step).padding(16).background(Color(nsColor: .windowBackgroundColor))
            try capture(swiftUI: view, width: 552, to: directory.appendingPathComponent(
                "intro-step\(String(format: "%02d", step))-\(suffix).png"), settle: 0.3)
        }
        // The keycap's press, frame by frame: appearing, pressed, at rest.
        for (name, frame) in [("1-appearing", PressedKeycap.PressFrame.appearing), ("2-down", .down), ("3-rest", .rest)] {
            let view = HStack(spacing: 7) {
                DemoLine(segments: [DemoSegment("約了", .document), DemoSegment("王"), DemoSegment("曉明", .focus)],
                         caretBlinks: false)
                PressedKeycap(keys: CurrentKeys(preferences).assigned(.correctionsList), animated: false, frame: frame)
            }
            .padding(14)
            .background(Color(nsColor: .textBackgroundColor))
            try capture(swiftUI: view.scaleEffect(2, anchor: .topLeading).frame(width: 380, height: 110, alignment: .topLeading),
                        width: 380, to: directory.appendingPathComponent("keycap-press-\(name)-\(suffix).png"))
        }
        preferences.resetToDefaults()
        for scene in 0..<4 {
            let view = ContextIllustration(pinned: scene).padding(16).background(Color(nsColor: .windowBackgroundColor))
            try capture(swiftUI: view, width: 552, to: directory.appendingPathComponent("context-scene\(scene)-\(suffix).png"))
        }
        // The 視窗與提示 preview: every state at every moment of its flow (the carousel), and a switch after a change.
        let director = PreviewDirector()
        for state in PreviewState.allCases {
            for frame in 0..<PreviewFlows(preferences: preferences).frames(state).count {
                DemoPinning.previewState = (state, frame)
                try capture(swiftUI: UnifiedPreview(preferences: preferences, director: director).padding(16)
                                .background(Color(nsColor: .textBackgroundColor)), width: 560,
                            to: directory.appendingPathComponent("preview-\(state.rawValue)-\(frame)-\(suffix).png"), settle: 0.2)
            }
        }
        // A change of 標出修正的讀音 while the list plays: before, and after the switch to the corrections list.
        let before = PreviewSettings(preferences)
        DemoPinning.previewState = (.list, 1)
        try capture(swiftUI: UnifiedPreview(preferences: preferences, director: director).padding(16)
                        .background(Color(nsColor: .textBackgroundColor)), width: 560,
                    to: directory.appendingPathComponent("preview-switch-0-before-\(suffix).png"), settle: 0.2)
        preferences.showsAnnotations = false
        let switched = PreviewFlows.state(showing: before, PreviewSettings(preferences)) ?? .list
        DemoPinning.previewState = (switched, 1)
        try capture(swiftUI: UnifiedPreview(preferences: preferences, director: director).padding(16)
                        .background(Color(nsColor: .textBackgroundColor)), width: 560,
                    to: directory.appendingPathComponent("preview-switch-1-\(switched.rawValue)-\(suffix).png"), settle: 0.2)
        DemoPinning.previewState = nil
        preferences.resetToDefaults()
        let demos: [(String, [DemoFrame])] = [
            ("space-open", OptionDemos.space(.openCandidates)),
            ("space-commit-space", OptionDemos.space(.commitAndSpace)), ("space-insert", OptionDemos.space(.insertSpace)),
            ("anchor-after", OptionDemos.anchor(.after)),
            ("typo", OptionDemos.typo(correctionsKey: "⇧↑")),
        ]
        for (name, frames) in demos {
            for index in frames.indices {
                let view = MiniDemo(frames: frames, pinned: index).padding(12).background(Color(nsColor: .windowBackgroundColor))
                try capture(swiftUI: view, width: 340, to: directory.appendingPathComponent("demo-\(name)-frame\(index)-\(suffix).png"))
            }
        }
    }

    private static func capturePreview(_ preferences: Preferences, to url: URL) throws {
        let view = CandidatePreview(preferences: preferences, animated: false)
            .padding(16)
            .background(Color(nsColor: .windowBackgroundColor))
        try capture(swiftUI: view, width: 552, to: url, settle: 0.3)
    }

    /// Several optional features off: the corrections list, phrase selection, ⌃ punctuation, Caps Lock, Shift.
    static func featuresOff(_ preferences: Preferences) {
        preferences.disabledKeyFeatures = [.correctionsList, .selectPhrase, .controlPunctuation, .typeZhuyin]
        preferences.shiftSwitchEnabled = false
    }

    /// Sample statistics for the 分析 page: 30 days of typing with a few days off.
    static func sampleStats() -> UsageStatsStore {
        let store = UsageStatsStore(directory: nil)
        let calendar = Calendar.current
        var data = UsageData(since: calendar.date(byAdding: .day, value: -45, to: Date())!)
        var generator = SystemRandomNumberGenerator()
        for back in 0..<45 where ![3, 4, 11, 18, 25].contains(back % 30) {
            let date = calendar.date(byAdding: .day, value: -back, to: Date())!
            var day = UsageCounters()
            day.characters = Int.random(in: 300...2400, using: &generator)
            day.compositions = day.characters / 9
            day.keys = day.characters * 3
            day.picksNormal = day.characters / 60
            day.picksCorrections = day.characters / 400
            day.picksSingle = day.characters / 500
            day.picksPunctuation = day.characters / 700
            day.backspaces = day.characters / 45
            day.escapes = day.characters / 300
            day.deleteAfterCommit = day.characters / 600
            day.literalZhuyin = day.characters / 1500
            day.undoLiteral = day.characters / 4000
            day.correctedShown = day.compositions / 25
            day.correctedCommitted = day.compositions / 30
            day.phrasesAdded = back % 9 == 0 ? 1 : 0
            day.cleanCompositions = day.compositions * 82 / 100
            day.typingMilliseconds = day.characters * 60_000 / Int.random(in: 38...52, using: &generator)
            day.thinkingMilliseconds = day.typingMilliseconds * Int.random(in: 35...70, using: &generator) / 100
            data.days[store.dayKey(date)] = day
            data.totals = data.totals + day
        }
        for _ in 0..<8000 {
            let ms = max(0.4, (-log(Double.random(in: 0.0001...1, using: &generator))) * 3.2)
            data.latency.add(milliseconds: ms)
        }
        data.latency.add(milliseconds: 87)
        for _ in 0..<6000 { data.rhythm.intervals.add(0.08 + Double.random(in: 0...0.35, using: &generator)) }
        for _ in 0..<900 { data.rhythm.pauses.add(2 + (-log(Double.random(in: 0.001...1, using: &generator))) * 5) }
        data.rhythm.longestRun = 186
        data.rhythm.longestCleanRun = 74
        for day in 0..<7 {
            for hour in 0..<24 {
                let work = (9...12).contains(hour) || (14...18).contains(hour)
                let evening = (20...23).contains(hour)
                let weekend = day == 0 || day == 6
                let base = work ? (weekend ? 40 : 420) : evening ? 260 : hour < 7 ? 0 : 60
                data.rhythm.hours[day * 24 + hour] = base == 0 ? 0 : base + Int.random(in: 0...base / 2, using: &generator)
            }
        }
        return UsageStatsStore(directory: nil, data: data)
    }

    /// Non-default keys everywhere, to show the pages follow the settings: rebound shortcuts, home-row selection keys,
    /// six to a page, right Shift, Space opening the list.
    static func customKeys(_ preferences: Preferences) {
        var bindings = preferences.bindings
        bindings[.correctionsList] = [Shortcut("u", control: true), Shortcut("↑", shift: true)]
        bindings[.singleCharacterList] = [Shortcut("↓", option: true)]
        bindings[.typeZhuyin] = [Shortcut("z", control: true)]
        bindings[.selectLeft] = [Shortcut("←", control: true, shift: true)]
        bindings[.nextPage] = [Shortcut("]", control: true), Shortcut("Tab"), Shortcut("PageDown")]
        bindings[.previousPage] = [Shortcut("Tab", shift: true), Shortcut("PageUp", option: true)]
        preferences.bindings = bindings
        preferences.selectionKeys = .homeRow
        preferences.candidatesPerPage = 6
        preferences.shiftToggle = .right
        preferences.spaceAction = .openCandidates
    }

    // MARK: Settings window

    /// Every page and sub-page, and how tall it is drawn (wide); narrow pages get more room for wrapping.
    private static let pages: [(SettingsPane, SettingsSubpage?, CGFloat)] = [
        (.intro, nil, 1320), (.typing, nil, 1260), (.choosing, nil, 1180), (.candidates, nil, 1720), (.words, nil, 440),
        (.analytics, nil, 2500), (.keys, nil, 2500), (.advanced, nil, 660), (.about, nil, 640),
        (.typing, .punctuation, 820), (.typing, .typingKeys, 760), (.choosing, .fuzzy, 680), (.choosing, .context, 780),
        (.words, .learnedWords, 620), (.words, .phrases, 620),
        (.analytics, .performance, 760), (.advanced, .apps, 460),
    ]

    private static func renderSettings(_ settings: SettingsWindowController, to directory: URL, suffix: String) throws {
        // Every row reports its blocks while the settings render, for the geometry check and the grid.
        RowGeometry.recording = true
        defer { RowGeometry.recording = false }
        Preferences.shared.resetToDefaults()
        settings.model.subpage = nil
        // The window, with its sidebar, at its usual size, at the minimum size and large.
        for (width, height, label) in [(CGFloat(760), CGFloat(600), ""), (640, 440, "-min"), (1280, 900, "-large")] {
            for pane in SettingsPane.allCases {
                RowGeometry.reset()
                let window = settings.makeWindow(pane: pane)
                window.setContentSize(NSSize(width: width, height: height))
                window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
                window.alphaValue = 0
                window.orderFrontRegardless()
                settings.model.reloadLists()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
                try capture(window: window, to: directory.appendingPathComponent("settings-\(pane.rawValue)\(label)-\(suffix).png"))
                try capture(window: window, to: directory.appendingPathComponent("grid-\(pane.rawValue)\(label)-\(suffix).png"), grid: true)
                retire(window)
            }
        }
        // Every sub-page in the window, at its top and scrolled (content under the pinned bar), at each size.
        let subpages = SettingsSubpage.allCases
        for (width, height, label) in [(CGFloat(760), CGFloat(600), ""), (640, 440, "-min"), (1280, 900, "-large")] {
            for subpage in subpages {
                RowGeometry.reset()
                settings.model.subpage = nil
                let window = settings.makeWindow(pane: subpage.pane)
                window.setContentSize(NSSize(width: width, height: height))
                window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
                window.alphaValue = 0
                window.orderFrontRegardless()
                settings.model.subpage = subpage
                settings.model.reloadLists()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.6))
                let name = "sub-\(subpage.rawValue)\(label)"
                remeasure(window)
                try capture(window: window, to: directory.appendingPathComponent("\(name)-top-\(suffix).png"), grid: true)
                if let scroll = firstScrollView(in: window.contentView) {
                    scroll.contentView.scroll(to: NSPoint(x: 0, y: scroll.contentView.bounds.minY + 140))
                    scroll.reflectScrolledClipView(scroll.contentView)
                    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
                    try capture(window: window, to: directory.appendingPathComponent("\(name)-scrolled-\(suffix).png"))
                }
                retire(window)
            }
        }
        // 視窗與提示 scrolled down, with the floating preview a change shows while the top preview is out of view.
        for (width, height, label) in [(CGFloat(760), CGFloat(600), ""), (640, 440, "-min"), (1280, 900, "-large")] {
            RowGeometry.reset()
            let window = settings.makeWindow(pane: .candidates)
            window.setContentSize(NSSize(width: width, height: height))
            window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
            window.alphaValue = 0
            window.orderFrontRegardless()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
            if let scroll = firstScrollView(in: window.contentView) {
                scroll.contentView.scroll(to: NSPoint(x: 0, y: 900))
                scroll.reflectScrolledClipView(scroll.contentView)
            }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
            try capture(window: window, to: directory.appendingPathComponent("candidates-scrolled\(label)-\(suffix).png"))
            DemoPinning.flashAt = CGPoint(x: width * 0.72, y: height * 0.62)
            settings.model.objectWillChange.send()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.4))
            try capture(window: window, to: directory.appendingPathComponent("candidates-flash\(label)-\(suffix).png"))
            DemoPinning.flashAt = nil
            retire(window)
        }
        // A push into 記住的字詞 and the pop back, frame by frame; a row of it being removed.
        for (push, kind) in [(true, "push"), (false, "pop")] {
            for progress in [0.0, 0.25, 0.5, 1.0] {
                settings.model.subpage = nil
                let window = settings.makeWindow(pane: .words)
                window.setContentSize(NSSize(width: 760, height: 600))
                window.setFrameOrigin(NSPoint(x: -30000, y: -30000))
                window.alphaValue = 0
                window.orderFrontRegardless()
                DemoPinning.navigation = (push, progress)
                settings.model.subpage = .learnedWords
                settings.model.reloadLists()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
                try capture(window: window, to: directory.appendingPathComponent("nav-\(kind)-\(Int(progress * 100))-\(suffix).png"))
                DemoPinning.navigation = nil
                retire(window)
            }
        }
        settings.model.reloadLists()
        if let first = settings.model.learned.list.visible.dropFirst().first {
            for progress in [0.0, 0.3, 0.6, 1.0] {
                DemoPinning.removal = (first.id, progress)
                try renderPages(settings, [(.words, .learnedWords, 560)], to: directory, label: "-remove-\(Int(progress * 100))",
                                suffix: suffix)
            }
            DemoPinning.removal = nil
        }
        settings.model.learned.list.query = "天"
        try renderPages(settings, [(.words, .learnedWords, 560)], to: directory, label: "-search", suffix: suffix)
        settings.model.learned.list.query = "沒有這個"
        try renderPages(settings, [(.words, .learnedWords, 560)], to: directory, label: "-no-results", suffix: suffix)
        settings.model.learned.list.query = ""
        settings.model.subpage = nil
        // Every page and sub-page whole, at the narrowest window (the page 440 pt wide) and a wide one (860 pt).
        try renderPages(settings, pages, to: directory, label: "", suffix: suffix)
        // 快捷鍵 with the fixed keys unfolded.
        DemoPinning.showsFixedKeys = true
        try renderPages(settings, [(.keys, nil, 2500)], to: directory, label: "-fixed-open", suffix: suffix)
        DemoPinning.showsFixedKeys = false
        // 進階 with the correction log on.
        Preferences.shared.recordsCorrections = true
        try renderPages(settings, [(.advanced, nil, 660)], to: directory, label: "-recording", suffix: suffix)
        Preferences.shared.resetToDefaults()
        // Non-default keys (two and three keys for some actions): every key shown must follow.
        customKeys(Preferences.shared)
        try renderPages(settings, pages.filter { [.typing, .choosing, .candidates, .keys, .intro].contains($0.0) && $0.1 == nil },
                        to: directory, label: "-custom-keys", suffix: suffix)
        Preferences.shared.resetToDefaults()
        // Features turned off, and a conflict between two actions.
        featuresOff(Preferences.shared)
        var bindings = Preferences.shared.bindings
        bindings[.nextPage] = [Shortcut("Tab"), Shortcut("←", control: true, shift: true)]
        bindings[.selectLeft] = [Shortcut("←", shift: true), Shortcut("←", control: true, shift: true)]
        Preferences.shared.bindings = bindings
        Preferences.shared.setEnabled(.selectPhrase, true)
        try renderPages(settings, pages.filter { [.typing, .choosing, .candidates, .keys, .intro].contains($0.0) && $0.1 == nil }
                        + [(.typing, .punctuation, 820)], to: directory, label: "-features-off", suffix: suffix)
        // A key field while recording, accepted and refused.
        Preferences.shared.resetToDefaults()
        DemoPinning.fieldStates = [.singleCharacterList: .recording(index: 0, held: ["⌃", "⇧"]),
                                   .correctionsList: .refused("已用於「打出注音本身」"), .typeZhuyin: .accepted(index: 0),
                                   .deleteWord: .hover]
        try renderPages(settings, [(.choosing, nil, 1180), (.candidates, nil, 1720), (.typing, nil, 1260)], to: directory,
                        label: "-field-states", suffix: suffix)
        DemoPinning.fieldStates = [:]
        // Key chips in every state, on the overview and on the feature pages: at rest (with +N), the row hovered
        // (ghost ＋, 還原預設), a chip hovered (its ✕), +N opened, no key (— and, hovered, ＋ 新增按鍵), recording,
        // accepted and refused.
        Preferences.shared.resetToDefaults()
        customKeys(Preferences.shared)
        var edited = Preferences.shared.bindings
        edited[.nextPage] = [Shortcut("Tab"), Shortcut("PageDown"), Shortcut("↓", option: true), Shortcut("]", control: true)]
        edited[.previousPage] = [Shortcut("Tab", shift: true), Shortcut("PageUp"), Shortcut("↑", option: true)]
        edited[.singleCharacterList] = []
        edited[.typeZhuyin] = []
        Preferences.shared.bindings = edited
        DemoPinning.fieldStates = [.previousPage: .expanded, .selectLeft: .chipHover(0), .selectRight: .hover,
                                   .typeZhuyin: .hover, .correctionsList: .recording(index: 1, held: ["⌥"]),
                                   .deleteWord: .accepted(index: 0), .commitWithNewline: .refused("已用於「下一頁」")]
        try renderPages(settings, [(.keys, nil, 2600), (.choosing, nil, 1300), (.typing, nil, 1300)], to: directory,
                        label: "-chips", suffix: suffix)
        DemoPinning.fieldStates = [:]
        Preferences.shared.resetToDefaults()
        // The arrival cue, frame by frame: on a row (its section's header in the accent colour), then on a section.
        for (target, row, section) in [("row", Optional(ShortcutCommand.correctionsList.keysRow), "按錯鍵"),
                                       ("section", nil, "前文")] {
            for (frame, opacity) in [("before", 0.0), ("rising", 0.09), ("peak", ArrivalHighlight.peakOpacity), ("fading", 0.05)] {
                settings.model.arrivedRow = frame == "before" ? nil : row
                settings.model.arrivedSection = frame == "before" ? nil : section
                DemoPinning.arrivalOpacity = opacity
                try renderPages(settings, [(.choosing, nil, 1180)], to: directory, label: "-arrival-\(target)-\(frame)", suffix: suffix)
            }
        }
        settings.model.arrivedRow = nil
        settings.model.arrivedSection = nil
        DemoPinning.arrivalOpacity = nil
        settings.model.subpage = nil
    }

    private static func renderPages(_ settings: SettingsWindowController, _ pages: [(SettingsPane, SettingsSubpage?, CGFloat)],
                                    to directory: URL, label: String, suffix: String) throws {
        for (pane, subpage, height) in pages {
            settings.model.subpage = subpage
            for (width, name) in [(CGFloat(440), "narrow"), (860, "wide")] {
                let tall = width < 500 ? height * 1.3 : height
                RowGeometry.reset()
                let hosting = NSHostingView(rootView: SettingsPaneView(pane: pane, model: settings.model))
                hosting.frame = NSRect(x: 0, y: 0, width: width, height: tall)
                let window = HeadlessWindow(size: NSSize(width: width, height: tall))
                window.backgroundColor = .windowBackgroundColor
                window.contentView = hosting
                window.alphaValue = 0
                window.orderFrontRegardless()
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.45))
                let file = "page-\(pane.rawValue)" + (subpage.map { "-" + $0.rawValue } ?? "") + "\(label)-\(name)-\(suffix).png"
                hosting.layoutSubtreeIfNeeded()
                hosting.display()
                try write(view: hosting, rect: hosting.bounds, to: directory.appendingPathComponent(file), grid: label.isEmpty ? .zero : nil,
                          page: file, checks: true)
                window.orderOut(nil)
                window.contentView = nil
                window.close()
            }
        }
        settings.model.subpage = nil
    }

    /// A correction log with three sample lines in a temporary folder, so the 進階 page never reads the user's.
    static func sampleCorrectionLog() throws -> CorrectionLog {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypePreview-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try String(repeating: "{\"schema\":1}\n", count: 3)
            .write(to: directory.appendingPathComponent(CorrectionLog.fileName), atomically: true, encoding: .utf8)
        return CorrectionLog(directory: directory)
    }

    static func sampleEngine() throws -> Engine {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinTypePreview-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let now = Int(Date().timeIntervalSince1970)
        let learned = [("ㄐㄧㄣ ㄊㄧㄢ", "今天", 12), ("ㄊㄧㄢ ㄑㄧˋ", "天氣", 7), ("ㄒㄧㄝˋ ㄒㄧㄝ˙", "謝謝", 5),
                       ("ㄆㄥˊ ㄧㄡˇ", "朋友", 3), ("ㄍㄨㄥ ㄩㄢˊ", "公園", 2)]
            + [("ㄨㄛˇ ㄇㄣ˙", "我們"), ("ㄕˊ ㄐㄧㄢ", "時間"), ("ㄍㄨㄥ ㄗㄨㄛˋ", "工作"), ("ㄒㄩㄝˊ ㄒㄧˊ", "學習"),
               ("ㄉㄧㄢˋ ㄋㄠˇ", "電腦"), ("ㄕㄡˇ ㄐㄧ", "手機"), ("ㄎㄞ ㄒㄧㄣ", "開心"), ("ㄇㄧㄥˊ ㄊㄧㄢ", "明天"),
               ("ㄗㄨㄛˊ ㄊㄧㄢ", "昨天"), ("ㄏㄨㄟˋ ㄧˋ", "會議"), ("ㄉㄧㄢˋ ㄧㄥˇ", "電影"), ("ㄔ ㄈㄢˋ", "吃飯"),
               ("ㄒㄧㄤˇ ㄈㄚˇ", "想法"), ("ㄨㄣˋ ㄊㄧˊ", "問題"), ("ㄐㄧㄚ ㄧㄡˊ", "加油"), ("ㄊㄞˊ ㄅㄟˇ", "台北")]
                .map { ($0.0, $0.1, 1) }
        try learned.map { "\($0.0)\t\($0.1)\t\($0.2)\t\(now)" }.joined(separator: "\n")
            .write(to: directory.appendingPathComponent("learned.tsv"), atomically: true, encoding: .utf8)
        let phrases = [("ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ", "王小明"), ("ㄓㄨˋ ㄧㄣ", "注音"), ("ㄊㄞˊ ㄋㄢˊ", "台南")]
        try phrases.map { "\($0.0)\t\($0.1)" }.joined(separator: "\n")
            .write(to: directory.appendingPathComponent("user_phrases.tsv"), atomically: true, encoding: .utf8)
        let lexicon = (learned.map { "\($0.0)\t\($0.1)\t-8" } + phrases.map { "\($0.0)\t\($0.1)\t-9" }).joined(separator: "\n")
        return try Engine(lexiconText: lexicon, userDataDirectory: directory)
    }

    // MARK: Capture

    private static func capture(window: NSWindow, to url: URL, grid: Bool = false) throws {
        guard let frameView = window.contentView?.superview else { return }
        frameView.layoutSubtreeIfNeeded()
        frameView.display()
        try write(view: frameView, rect: frameView.bounds, to: url, grid: grid ? .zero : nil, page: url.lastPathComponent,
                  checks: grid)
    }

    /// The live views over real time, sampled (0, 100, 200, 400 ms…): the preview playing, the 中英切換 state picked,
    /// the wizard's parts arriving and a step moving in.  Drawn from the running views, not from still frames.
    private static func renderLive(to directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let saved = (DemoPinning.still, DemoPinning.live, PreviewDirector.step)
        defer { DemoPinning.still = saved.0; DemoPinning.live = saved.1; PreviewDirector.step = saved.2 }
        DemoPinning.still = false
        DemoPinning.live = true
        PreviewDirector.step = 0.6
        func sample<V: View>(_ view: V, size: NSSize, name: String, at times: [Int], before: (() -> Void)? = nil) throws {
            let hosting = NSHostingView(rootView: view.frame(width: size.width, height: size.height))
            hosting.frame = NSRect(origin: .zero, size: size)
            let window = HeadlessWindow(size: size)
            window.backgroundColor = .windowBackgroundColor
            window.contentView = hosting
            window.orderFrontRegardless()
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            before?()
            let start = Date()
            for time in times {
                RunLoop.main.run(until: start.addingTimeInterval(Double(time) / 1000))
                hosting.layoutSubtreeIfNeeded()
                hosting.display()
                try write(view: hosting, rect: hosting.bounds, to: directory.appendingPathComponent("\(name)-\(time)ms.png"))
            }
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }
        for (suffix, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
            NSApp.appearance = NSAppearance(named: appearanceName)
            Preferences.shared.resetToDefaults()
            let director = PreviewDirector()
            try sample(UnifiedPreview(preferences: Preferences.shared, director: director).padding(16), size: NSSize(width: 600, height: 380),
                       name: "preview-playing-\(suffix)", at: [0, 100, 200, 400, 800, 1600, 2400, 3200])
            let modeDirector = PreviewDirector()
            try sample(UnifiedPreview(preferences: Preferences.shared, director: modeDirector).padding(16), size: NSSize(width: 600, height: 380),
                       name: "preview-mode-\(suffix)", at: [0, 100, 200, 400, 800, 1200, 1600],
                       before: { modeDirector.show(.mode, last: false) })
            let model = WizardModel(original: Preferences.shared, mode: .quick)
            try sample(WizardView(model: model), size: WizardLayout.defaultSize, name: "wizard-welcome-\(suffix)",
                       at: [0, 100, 200, 300, 450, 600, 800, 1100, 1600, 3000, 4500])
            let next = WizardModel(original: Preferences.shared, mode: .quick)
            try sample(WizardView(model: next), size: WizardLayout.defaultSize, name: "wizard-next-\(suffix)",
                       at: [0, 50, 100, 200, 300, 400, 600], before: {
                RunLoop.main.run(until: Date().addingTimeInterval(0.8))
                next.next()
            })
        }
        Preferences.shared.resetToDefaults()
    }

    /// The setup wizard: every kind of step at the three window sizes, light and dark; the transition between two
    /// steps frame by frame; the finish page with changes.
    private static func renderWizard(to directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for (name, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
            NSApp.appearance = NSAppearance(named: appearanceName)
            Preferences.shared.resetToDefaults()
            func model() -> WizardModel {
                let model = WizardModel(original: Preferences.shared, mode: .quick)
                model.choosePreset(.windows)
                model.sandbox.typoTolerance = .aggressive
                _ = model.sandbox.unbind(.nextPage, at: 2)
                model.completed = ["paging"]
                return model
            }
            let steps: [(String, WizardStep, ((WizardModel) -> Void)?)] = [
                ("welcome", .welcome, nil), ("preset", .preset, nil), ("typing", .lesson("typing"), nil),
                ("punctuation", .lesson("punctuation"), nil), ("selecting", .lesson("selecting"), nil),
                ("selecting-horizontal", .lesson("selecting"), { $0.sandbox.candidateLayout = .horizontal }),
                ("paging", .lesson("paging"), nil), ("look", .lesson("look"), nil), ("finish", .finish, nil), ("keys", .keys, nil),
            ]
            let sizes = [(WizardLayout.defaultSize, ""), (WizardLayout.minSize, "-min"), (WizardLayout.maxSize, "-large")]
            for (size, label) in sizes {
                for (stepName, step, adjust) in steps {
                    let wizard = model()
                    wizard.choose(.full)
                    wizard.jump(to: step)
                    adjust?(wizard)
                    WizardFit.reset()
                    try capture(swiftUI: WizardView(model: wizard).frame(width: size.width, height: size.height), width: size.width,
                                to: directory.appendingPathComponent("\(stepName)\(label)-\(name).png"), settle: 0.3)
                    wizardOverflow += WizardFit.overflows.map { "\(stepName)\(label)-\(name) \($0)" }
                }
            }
            // preset → punctuation, frame by frame.
            for progress in [0.0, 0.3, 0.6, 1.0] {
                let wizard = model()
                wizard.jump(to: .lesson("punctuation"))
                DemoPinning.wizardTransition = (.preset, progress)
                try capture(swiftUI: WizardView(model: wizard).frame(width: WizardLayout.defaultSize.width, height: WizardLayout.defaultSize.height),
                            width: WizardLayout.defaultSize.width,
                            to: directory.appendingPathComponent("transition-\(Int(progress * 100))-\(name).png"), settle: 0.2)
            }
            DemoPinning.wizardTransition = nil
        }
        Preferences.shared.resetToDefaults()
    }

    /// Measures the rows again from scratch: views that left the window (the page a sub-page replaced) report nothing,
    /// the ones on screen report again after a 1 pt resize.
    private static func remeasure(_ window: NSWindow) {
        RowGeometry.reset()
        let size = window.contentView?.frame.size ?? .zero
        window.setContentSize(NSSize(width: size.width + 1, height: size.height))
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
        window.setContentSize(size)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
    }

    /// Takes a settings window apart after its capture, so its views stop laying out (and reporting geometry) while
    /// later pages are drawn.
    private static func retire(_ window: NSWindow) {
        SettingsWindowController.shared.discardWindow()
    }

    private static func firstScrollView(in view: NSView?) -> NSScrollView? {
        guard let view else { return nil }
        if let scroll = view as? NSScrollView, scroll.documentView != nil,
           (scroll.documentView?.frame.height ?? 0) > scroll.contentView.bounds.height + 50 { return scroll }
        for sub in view.subviews { if let found = firstScrollView(in: sub) { return found } }
        return nil
    }

    static func capture<V: View>(swiftUI view: V, width: CGFloat, to url: URL, settle: TimeInterval = 0.15) throws {
        let hosting = NSHostingView(rootView: view.frame(width: width).fixedSize(horizontal: false, vertical: true))
        hosting.frame.size = NSSize(width: width, height: max(1, hosting.fittingSize.height))
        try capture(view: hosting, to: url, settle: settle)
    }

    private static func capture(view: NSView, to url: URL, settle: TimeInterval = 0) throws {
        let window = HeadlessWindow(size: view.frame.size)
        window.backgroundColor = .clear
        window.isOpaque = false
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

    /// Writes `view` as a PNG; with `grid` (an offset; rows report window coordinates, so zero), draws the rows' geometry over it: title
    /// left edges in red, trailing right edges in blue, row centre lines in green, misaligned rows in magenta.
    private static func write(view: NSView, rect: NSRect, to url: URL, grid: CGPoint? = nil, page: String = "",
                              checks: Bool = false) throws {
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: rect) else { throw CocoaError(.fileWriteUnknown) }
        view.cacheDisplay(in: rect, to: bitmap)
        let rows = RowGeometry.all
        let cards = RowGeometry.cards
        let bad = RowGeometry.misaligned(rows)
        if checks {
            misaligned += bad.map { "\(page): 「\($0.id)」 leading midY \($0.leading.midY), trailing midY \($0.trailing.midY)" }
            misaligned += RowGeometry.imbalances(cards: cards, rows: rows).map { "\(page): \($0)" }
            misaligned += RowGeometry.underBar(rows).map { "\(page): \($0)" }
        }
        if let offset = grid, let context = NSGraphicsContext(bitmapImageRep: bitmap) {
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = context
            let height = bitmap.size.height
            func flip(_ y: CGFloat) -> CGFloat { height - (offset.y + y) }
            var lefts = Set<Int>(), rights = Set<Int>()
            for row in rows {
                let isBad = bad.contains { $0.leading == row.leading }
                lefts.insert(Int((offset.x + row.leading.minX).rounded()))
                rights.insert(Int((offset.x + row.trailing.maxX).rounded()))
                let centre = NSBezierPath()
                centre.move(to: NSPoint(x: offset.x + row.leading.minX, y: flip(row.leading.midY)))
                centre.line(to: NSPoint(x: offset.x + row.trailing.maxX, y: flip(row.trailing.midY)))
                centre.lineWidth = isBad ? 2 : 0.5
                (isBad ? NSColor.magenta : NSColor.systemGreen.withAlphaComponent(0.8)).setStroke()
                centre.stroke()
            }
            // Cards: their outline in orange, and their content edges (the card inset) dashed.
            for card in cards {
                let frame = NSRect(x: offset.x + card.frame.minX, y: flip(card.frame.maxY), width: card.frame.width, height: card.frame.height)
                NSColor.systemOrange.withAlphaComponent(0.7).setStroke()
                let outline = NSBezierPath(rect: frame.insetBy(dx: 0.25, dy: 0.25))
                outline.lineWidth = 0.5
                outline.stroke()
                let content = NSBezierPath(rect: frame.insetBy(dx: RowMetrics.cardInset, dy: 0))
                content.setLineDash([3, 3], count: 2, phase: 0)
                content.lineWidth = 0.5
                content.stroke()
                NSColor.systemPurple.withAlphaComponent(0.7).setStroke()
                for child in card.children {
                    let box = NSBezierPath(rect: NSRect(x: offset.x + child.minX, y: flip(child.maxY), width: child.width, height: child.height))
                    box.lineWidth = 0.5
                    box.stroke()
                }
            }
            for (xs, color) in [(lefts, NSColor.systemRed), (rights, NSColor.systemBlue)] {
                color.withAlphaComponent(0.6).setStroke()
                for x in xs {
                    let line = NSBezierPath()
                    line.move(to: NSPoint(x: CGFloat(x) + 0.5, y: 0))
                    line.line(to: NSPoint(x: CGFloat(x) + 0.5, y: height))
                    line.lineWidth = 0.5
                    line.stroke()
                }
            }
            NSGraphicsContext.restoreGraphicsState()
        }
        guard let data = bitmap.representation(using: .png, properties: [:]) else { throw CocoaError(.fileWriteUnknown) }
        try data.write(to: url, options: .atomic)
    }
}
