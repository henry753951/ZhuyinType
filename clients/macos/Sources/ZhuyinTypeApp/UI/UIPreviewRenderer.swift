import AppKit
import ZhuyinCore

/// `ZhuyinType --render-ui-preview <dir>`: PNGs of the candidate window and every settings pane, light and dark, for
/// reviewing the look without typing.  Uses a throwaway engine with sample words; never touches personal data.
enum UIPreviewRenderer {
    static func render(to directory: URL) {
        precondition(Thread.isMainThread)
        NSApp.setActivationPolicy(.regular)
        NSApp.finishLaunching()
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let words = ["今天", "金天", "今", "斤", "巾", "筋", "津", "襟", "矜"]
            let candidates = CandidateList(items: (0..<3).flatMap { _ in words.map { Candidate(text: $0) } }, tolerant: false)
            let corrections = CandidateList(items: [Candidate(text: "今天"), Candidate(text: "經天", annotation: "ㄐㄧㄥ ㄊㄧㄢ"),
                                                    Candidate(text: "親天", annotation: "ㄑㄧㄣ ㄊㄧㄢ")] + candidates.items,
                                            tolerant: true)
            let settings = SettingsWindowController.shared
            settings.engine = try sampleEngine()
            for (name, appearanceName) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
                let appearance = NSAppearance(named: appearanceName)!
                NSApp.appearance = appearance
                for layout in CandidateLayout.allCases {
                    let content = CandidateContentView(frame: .zero)
                    content.update(list: candidates, preview: nil, keys: Array("123456789"), layout: layout, fontSize: 18,
                                   zhuyinHint: "ㄐㄧㄣ ㄊㄧㄢ")
                    try capture(view: InputSurface(content: content), to: directory.appendingPathComponent("candidate-\(layout.rawValue)-\(name).png"))
                    let tolerant = CandidateContentView(frame: .zero)
                    tolerant.update(list: corrections, preview: nil, keys: Array("123456789"), layout: layout, fontSize: 18,
                                    zhuyinHint: "ㄐㄧㄣ ㄊㄧㄢ")
                    try capture(view: InputSurface(content: tolerant),
                                to: directory.appendingPathComponent("candidate-\(layout.rawValue)-corrections-\(name).png"))
                }
                for (file, notice) in [("select", "Enter 加入自訂詞：你好"), ("saved", "已加入自訂詞：你好")] {
                    let content = CandidateContentView(frame: .zero)
                    content.update(list: nil, preview: nil, keys: [], layout: .horizontal, fontSize: 18, notice: notice)
                    try capture(view: InputSurface(content: content), to: directory.appendingPathComponent("notice-\(file)-\(name).png"))
                }
                let toast = ModeBadgeView(frame: .zero)
                try capture(view: InputSurface(content: toast), to: directory.appendingPathComponent("mode-toast-\(name).png"))
                for pane in SettingsPane.allCases {
                    let window = settings.makeWindow(pane: pane)
                    window.setFrameOrigin(NSPoint(x: 40, y: 40))
                    NSApp.activate(ignoringOtherApps: true)
                    window.makeKeyAndOrderFront(nil)
                    settings.model.reloadLists()
                    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.35))
                    try capture(window: window, to: directory.appendingPathComponent("settings-\(pane.rawValue)-\(name).png"))
                    window.orderOut(nil)
                }
            }
            print("UI previews: \(directory.path)")
        } catch {
            fputs("UI preview failed: \(error.localizedDescription)\n", stderr)
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

    private static func capture(window: NSWindow, to url: URL) throws {
        guard let frameView = window.contentView?.superview else { return }
        frameView.layoutSubtreeIfNeeded()
        frameView.display()
        try write(view: frameView, rect: frameView.bounds, to: url)
    }

    private static func capture(view: NSView, to url: URL) throws {
        let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: -10000, y: -10000), size: view.frame.size),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.backgroundColor = .clear
        window.isOpaque = false
        window.contentView = view
        view.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        try write(view: view, rect: view.bounds, to: url)
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
