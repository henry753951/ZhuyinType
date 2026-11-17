// 快速設定: the switches people change while typing — 中／英, 全形／半形 標點, 繁／簡 and 暫停學習 — in the input menu and
// in a small panel by the caret (the 開啟快速設定 shortcut).  The panel never takes focus from the app being typed in:
// it is a non-activating panel that cannot become key, its rows take the first click, and while it is open the input
// controller hands it the keys 1–4 and Esc (`QuickSettingsPanel.handle`).
import AppKit
import ZhuyinEngine

/// The state 快速設定 shows.  暫停學習 lives only in memory: every launch of the input method starts unpaused.
enum QuickSettings {
    static let didChange = Notification.Name("ZhuyinType.QuickSettings.didChange")

    /// 暫停學習: nothing is learned, no correction is recorded and no text before the caret is read or kept, until it
    /// is turned off or the input method quits.
    static var learningPaused = false {
        didSet { if learningPaused != oldValue { NotificationCenter.default.post(name: didChange, object: nil) } }
    }

    /// The 中／英 badge: 「英」, or 「中」 (「简」 with 簡體 output).
    static func modeLabel(english: Bool, script: OutputScript) -> String {
        english ? "英" : script == .simplified ? "简" : "中"
    }

    /// One switch, in the order of the panel and the menus.
    enum Item: Int, CaseIterable {
        case english, halfWidth, simplified, pauseLearning

        var title: String {
            switch self {
            case .english: return "英文"
            case .halfWidth: return "半形標點"
            case .simplified: return "簡體"
            case .pauseLearning: return "暫停學習"
            }
        }
    }

    /// What the switches read now.
    struct State: Equatable {
        var english: Bool
        var halfWidth: Bool
        var simplified: Bool
        var learningPaused: Bool

        func isOn(_ item: Item) -> Bool {
            switch item {
            case .english: return english
            case .halfWidth: return halfWidth
            case .simplified: return simplified
            case .pauseLearning: return learningPaused
            }
        }
    }

    /// The links under the switches.  `symbols` is the hook for a symbol picker: shown only once one registers
    /// (`QuickSettings.openSymbols`).
    enum Link: CaseIterable {
        case settings, words, symbols

        var title: String {
            switch self {
            case .settings: return "設定…"
            case .words: return "自訂詞與學到的詞…"
            case .symbols: return "符號…"
            }
        }
    }

    /// Set by a symbol picker to appear as 「符號…」 in 快速設定 and the input menu; nil hides the item.
    static var openSymbols: (() -> Void)?

    static var links: [Link] { Link.allCases.filter { $0 != .symbols || openSymbols != nil } }

    static func open(_ link: Link) {
        switch link {
        case .settings: SettingsWindowController.shared.show()
        case .words: SettingsWindowController.shared.show(SettingsPlace(.words))
        case .symbols: openSymbols?()
        }
    }
}

/// One clickable row of the panel: a switch with its check mark and key, or a link.
private final class QuickRow: NSView {
    override var isFlipped: Bool { true }
    static let height: CGFloat = 26
    private let title = NSTextField(labelWithString: "")
    private let key = NSTextField(labelWithString: "")
    private let check = NSImageView()
    private let highlight = NSView()
    var action: (() -> Void)?

    init(title text: String, key keyText: String?, checked: Bool?) {
        super.init(frame: .zero)
        highlight.wantsLayer = true
        highlight.layer?.cornerRadius = 6
        highlight.isHidden = true
        addSubview(highlight)
        check.image = NSImage(systemSymbolName: "checkmark", accessibilityDescription: nil)
        check.contentTintColor = .controlAccentColor
        check.isHidden = checked != true
        addSubview(check)
        title.stringValue = text
        title.font = .systemFont(ofSize: 13)
        title.textColor = .labelColor
        addSubview(title)
        key.stringValue = keyText ?? ""
        key.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        key.textColor = .tertiaryLabelColor
        addSubview(key)
        setAccessibilityElement(true)
        setAccessibilityRole(checked == nil ? .button : .checkBox)
        setAccessibilityLabel(text)
        if let checked { setAccessibilityValue(checked ? 1 : 0) }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layout() {
        super.layout()
        highlight.frame = bounds
        check.frame = NSRect(x: 8, y: (bounds.height - 14) / 2, width: 14, height: 14)
        title.sizeToFit()
        title.frame.origin = NSPoint(x: 28, y: (bounds.height - title.frame.height) / 2)
        key.sizeToFit()
        key.frame.origin = NSPoint(x: bounds.width - key.frame.width - 10, y: (bounds.height - key.frame.height) / 2)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { action?() }
    override func updateTrackingAreas() {
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                       owner: self))
    }
    override func mouseEntered(with event: NSEvent) {
        highlight.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.08).cgColor
        highlight.isHidden = false
    }
    override func mouseExited(with event: NSEvent) { highlight.isHidden = true }
}

/// The panel's content: a title, the four switches (keys 1–4), a divider and the links.
final class QuickSettingsView: InputMaterialView {
    static let width: CGFloat = 220
    private static let inset: CGFloat = 6

    var onToggle: ((QuickSettings.Item) -> Void)?
    var onLink: ((QuickSettings.Link) -> Void)?

    /// Lays out the rows for `state`; returns the size.
    @discardableResult
    func update(_ state: QuickSettings.State) -> NSSize {
        subviews.forEach { $0.removeFromSuperview() }
        var y = Self.inset
        let header = NSTextField(labelWithString: "快速設定")
        header.font = .systemFont(ofSize: 11, weight: .semibold)
        header.textColor = .secondaryLabelColor
        header.sizeToFit()
        header.frame.origin = NSPoint(x: Self.inset + 8, y: y + 2)
        addSubview(header)
        y += header.frame.height + 6
        for item in QuickSettings.Item.allCases {
            let row = QuickRow(title: item.title, key: "\(item.rawValue + 1)", checked: state.isOn(item))
            row.action = { [weak self] in self?.onToggle?(item) }
            row.frame = NSRect(x: Self.inset, y: y, width: Self.width - 2 * Self.inset, height: QuickRow.height)
            addSubview(row)
            y += QuickRow.height
        }
        let divider = NSBox()
        divider.boxType = .separator
        divider.frame = NSRect(x: Self.inset + 8, y: y + 4, width: Self.width - 2 * Self.inset - 16, height: 1)
        addSubview(divider)
        y += 9
        for link in QuickSettings.links {
            let row = QuickRow(title: link.title, key: nil, checked: nil)
            row.action = { [weak self] in self?.onLink?(link) }
            row.frame = NSRect(x: Self.inset, y: y, width: Self.width - 2 * Self.inset, height: QuickRow.height)
            addSubview(row)
            y += QuickRow.height
        }
        let size = NSSize(width: Self.width, height: y + Self.inset)
        setFrameSize(size)
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("快速設定")
        return size
    }
}

final class QuickSettingsPanel {
    static let shared = QuickSettingsPanel()
    /// Reads the current switches; set by the input controller that opens the panel.
    var state: (() -> QuickSettings.State)?
    var onToggle: ((QuickSettings.Item) -> Void)?

    private let content = QuickSettingsView(frame: .zero)
    private lazy var surface = InputSurface(content: content)
    private lazy var panel = InputPanel(content: surface)
    private var caret = NSRect.zero
    private var clickMonitor: Any?

    var isVisible: Bool { panel.isVisible }

    func show(caret: NSRect, windowLevel: NSWindow.Level) {
        self.caret = caret
        _ = panel
        panel.level = windowLevel
        panel.appearance = Preferences.shared.panelAppearance.appearance
        refresh()
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        panel.orderFrontRegardless()
        // A click anywhere else closes it (mouse monitors need no permission; keys come through the controller).
        if clickMonitor == nil {
            clickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
                self?.hide()
            }
        }
    }

    /// Draws the switches as they are now, keeping the panel where it is.
    func refresh() {
        guard let state = state?() else { return }
        content.onToggle = { [weak self] item in
            self?.onToggle?(item)
            self?.refresh()
        }
        content.onLink = { [weak self] link in
            self?.hide()
            QuickSettings.open(link)
        }
        let size = content.update(state)
        surface.fitContent()
        let visible = InputPanelPlacement.screen(at: caret)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        panel.setFrame(InputPanelPlacement.frame(size: size, caret: caret, visible: visible, textInset: 0,
                                                 above: Preferences.shared.candidatePlacement == .above), display: true)
    }

    func hide() {
        if let clickMonitor { NSEvent.removeMonitor(clickMonitor) }
        clickMonitor = nil
        guard panel.isVisible else { return }
        panel.orderOut(nil)
    }

    /// A key typed while the panel is open: 1–4 switch, Esc closes; true when the panel used it.  Any other key closes
    /// the panel and goes on as usual.
    func handle(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool {
        guard isVisible else { return false }
        if modifiers.isDisjoint(with: [.command, .control, .option]) {
            if keyCode == 53 { hide(); return true }
            // The digit keys 1–4 of the US layout.
            if let index = [18, 19, 20, 21].firstIndex(of: keyCode), let item = QuickSettings.Item(rawValue: index) {
                onToggle?(item)
                refresh()
                return true
            }
        }
        hide()
        return false
    }
}
