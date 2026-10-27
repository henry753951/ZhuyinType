import AppKit
import Combine
import SwiftUI
import ZhuyinEngine

/// The settings window: a source-list sidebar and one pane at a time, like System Settings.
final class SettingsWindowController: NSObject, NSWindowDelegate, NSToolbarDelegate, NSSearchFieldDelegate {
    static let shared = SettingsWindowController()
    /// Rendering previews: the window's frame and the last pane are not saved.
    static var isPreview = false

    /// Set once the engine has loaded; the word lists read and edit through it.
    var engine: Engine? {
        didSet { model.engine = engine }
    }

    let model = SettingsModel()
    private var window: NSWindow?
    private var split: SettingsSplitViewController?
    private var pane: SettingsPane = .intro
    private static let searchItem = NSToolbarItem.Identifier("search")
    private var subpageObserver: AnyCancellable?
    private weak var toolbar: NSToolbar?

    /// Shows the window, at `place` when given (otherwise where it was).
    func show(_ place: SettingsPlace? = nil) {
        UsageStatsStore.shared.saveIfNeeded()
        let window = self.window ?? makeWindow()
        if let place { open(place) }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        model.reloadLists()
        WizardWindowController.shared.showOnFirstUse()
    }

    /// The window, built but not shown, open on `pane`; also used by the preview renderer.
    func makeWindow(pane: SettingsPane? = nil) -> NSWindow {
        if let window {
            if let pane { split?.select(pane) }
            return window
        }
        let split = SettingsSplitViewController(model: model)
        let window = Self.isPreview ? HeadlessTitledWindow(contentViewController: split) : NSWindow(contentViewController: split)
        if Self.isPreview { window.ignoresMouseEvents = true }
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView]
        let toolbar = NSToolbar(identifier: "ZhuyinTypeSettings")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        window.toolbar = toolbar
        window.toolbarStyle = .unified
        window.setContentSize(NSSize(width: 720, height: 520))
        window.contentMinSize = NSSize(width: 640, height: 440)
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        if !Self.isPreview { window.setFrameAutosaveName("ZhuyinTypeSettings") }
        self.toolbar = toolbar
        split.onSelect = { [weak self, weak window] pane in
            window?.title = pane.title
            self?.model.subpage = nil
            self?.paneChanged(pane)
        }
        // A sub-page with a list (自訂詞, 記住的字詞) gets the toolbar search.
        subpageObserver = model.$subpage.dropFirst().receive(on: DispatchQueue.main).sink { [weak self] _ in
            guard let self else { return }
            self.paneChanged(self.pane)
        }
        split.select(pane ?? SettingsPane.lastSelected)
        self.window = window
        self.split = split
        return window
    }

    /// The UI previews: forget the window, so the next `makeWindow` builds a new one and the old views go away.
    func discardWindow() {
        window?.orderOut(nil)
        window?.contentViewController = nil
        window?.close()
        window = nil
        split = nil
        subpageObserver = nil
    }

    /// Shows `pane`, as when its row in the sidebar is clicked.
    func select(_ pane: SettingsPane) {
        split?.select(pane)
    }

    /// Whether `pane` is the page on screen.
    func isShowing(_ pane: SettingsPane) -> Bool { self.pane == pane }

    /// Shows a place: a page, a sub-page of it, scrolled to a row.
    func open(_ place: SettingsPlace) {
        if split == nil { _ = makeWindow() }
        select(place.pane)
        model.subpage = place.subpage
        model.scrollTarget = place.row
    }

    /// Shows the 快捷鍵 page scrolled to `section`.
    func showKeys(_ section: KeySection, command: ShortcutCommand? = nil) {
        open(SettingsPlace(.keys, row: RowSections.headerID((command?.section ?? section).title)))
    }

    func windowWillClose(_ notification: Notification) {
        guard !Self.isPreview else { return }
        NSApp.setActivationPolicy(.accessory)
    }

    // MARK: Toolbar search, for the word lists

    /// The toolbar search filters the word lists, and the actions of the 快捷鍵 page.
    private var searches: Bool { pane == .keys }

    /// The window sheets of the list pages attach to.
    var sheetWindow: NSWindow? { window }

    func windowDidResignKey(_ notification: Notification) { HoverState.shared.clear() }

    private func paneChanged(_ pane: SettingsPane) {
        HoverState.shared.clear()
        model.phrases.list.query = ""
        model.learned.list.query = ""
        model.keysQuery = ""
        self.pane = pane
        guard let toolbar else { return }
        let index = toolbar.items.firstIndex { $0.itemIdentifier == Self.searchItem }
        if searches, index == nil {
            toolbar.insertItem(withItemIdentifier: Self.searchItem, at: toolbar.items.count)
        } else if !searches, let index {
            toolbar.removeItem(at: index)
        }
        (toolbar.items.first { $0.itemIdentifier == Self.searchItem } as? NSSearchToolbarItem)?.searchField.stringValue = ""
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.sidebarTrackingSeparator, .flexibleSpace, Self.searchItem]
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.sidebarTrackingSeparator, .flexibleSpace]
    }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier identifier: NSToolbarItem.Identifier,
                 willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        guard identifier == Self.searchItem else { return nil }
        let item = NSSearchToolbarItem(itemIdentifier: identifier)
        item.searchField.placeholderString = "搜尋"
        item.searchField.delegate = self
        item.preferredWidthForSearchField = 200
        return item
    }

    func controlTextDidChange(_ notification: Notification) {
        guard let field = notification.object as? NSSearchField else { return }
        if pane == .keys { model.keysQuery = field.stringValue }
    }
}

/// Shared state for the panes: the engine and the list controllers, which outlive pane switches.
final class SettingsModel: ObservableObject {
    @Published var engine: Engine? {
        didSet { reloadLists() }
    }
    lazy var phrases = PhraseListController(model: self)
    lazy var learned = LearnedListController(model: self)
    lazy var apps = AppListController()
    /// Records in the correction log, for the 進階 page.
    @Published var correctionCount = 0
    /// The toolbar search on the 快捷鍵 page.
    @Published var keysQuery = ""
    /// The sub-page shown in the current page (one level down), or nil for the page itself.
    @Published var subpage: SettingsSubpage? { didSet { HoverState.shared.clear() } }
    /// A row to scroll to, set by the links between pages; cleared once shown.
    @Published var scrollTarget: String?
    /// The row a link just led to, and its section, highlighted for a moment.  A section alone: all its rows.
    @Published var arrivedRow: String?
    @Published var arrivedSection: String?
    private var correctionObserver: NSObjectProtocol?

    init() {
        correctionObserver = NotificationCenter.default.addObserver(forName: CorrectionLog.didChange, object: nil,
                                                                    queue: .main) { [weak self] _ in
            self?.reloadCorrections()
        }
    }

    deinit { correctionObserver.map(NotificationCenter.default.removeObserver) }

    func reloadLists() {
        phrases.reload()
        learned.reload()
        apps.reload()
        reloadCorrections()
    }

    func reloadCorrections() {
        CorrectionLog.shared.count { [weak self] in self?.correctionCount = $0 }
    }
}

/// The sidebar's pages (docs/product/SETTINGS_IA.md).
enum SettingsPane: String, CaseIterable {
    case intro, typing, choosing, candidates, words, analytics, keys, advanced, about

    /// The sidebar's sections, each under a small grey header.
    static let groups: [(title: String, panes: [SettingsPane])] = [
        ("概覽", [.intro, .analytics]), ("輸入", [.typing, .choosing, .words]), ("外觀", [.candidates]),
        ("一般", [.keys, .advanced, .about]),
    ]

    var title: String {
        switch self {
        case .intro: return "介紹"
        case .typing: return "打字"
        case .choosing: return "選字"
        case .candidates: return "視窗與提示"
        case .words: return "字詞"
        case .analytics: return "分析"
        case .keys: return "快捷鍵"
        case .advanced: return "進階"
        case .about: return "關於"
        }
    }

    var symbol: String {
        switch self {
        case .intro: return "sparkles"
        case .typing: return "keyboard.fill"
        case .choosing: return "wand.and.stars"
        case .candidates: return "list.bullet.rectangle"
        case .words: return "character.book.closed.fill"
        case .analytics: return "chart.bar.fill"
        case .keys: return "command"
        case .advanced: return "slider.horizontal.3"
        case .about: return "info"
        }
    }

    var tint: NSColor {
        switch self {
        case .intro: return .systemPink
        case .typing: return .systemIndigo
        case .choosing: return .systemTeal
        case .candidates: return .systemBlue
        case .words: return .systemOrange
        case .analytics: return .systemGreen
        case .keys, .advanced, .about: return .systemGray
        }
    }

    /// Pages of earlier versions, for the remembered last page.
    private static let earlier: [String: SettingsPane] = ["general": .typing, "context": .choosing, "phrases": .words,
                                                           "learned": .words]

    private static let defaultsKey = "settingsLastPane"
    static var lastSelected: SettingsPane {
        get {
            guard let raw = UserDefaults.standard.string(forKey: defaultsKey) else { return .intro }
            return SettingsPane(rawValue: raw) ?? earlier[raw] ?? .intro
        }
        set { if !SettingsWindowController.isPreview { UserDefaults.standard.set(newValue.rawValue, forKey: defaultsKey) } }
    }

    /// A System Settings style icon: a white symbol on a rounded colour tile.  Drawn on demand, so it stays sharp at
    /// any scale and keeps its colour in both appearances.
    var icon: NSImage {
        let symbol = self.symbol
        let tint = self.tint
        let image = NSImage(size: NSSize(width: 22, height: 22), flipped: false) { rect in
            let tile = NSBezierPath(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), xRadius: 6.5, yRadius: 6.5)
            NSGradient(starting: tint.blended(withFraction: 0.2, of: .white) ?? tint, ending: tint)?.draw(in: tile, angle: -90)
            let configuration = NSImage.SymbolConfiguration(pointSize: 12, weight: .semibold)
                .applying(.init(paletteColors: [.white]))
            if let glyph = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
                .withSymbolConfiguration(configuration) {
                let size = glyph.size
                glyph.draw(in: NSRect(x: (rect.width - size.width) / 2, y: (rect.height - size.height) / 2,
                                      width: size.width, height: size.height))
            }
            return true
        }
        image.accessibilityDescription = title
        return image
    }
}

final class SettingsSplitViewController: NSSplitViewController {
    var onSelect: ((SettingsPane) -> Void)?
    private let sidebar = SettingsSidebarViewController()
    private let detail = SettingsDetailViewController()

    init(model: SettingsModel) {
        detail.model = model
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func viewDidLoad() {
        super.viewDidLoad()
        // Offscreen drawing leaves the sidebar material blank, so previews draw the sidebar on a plain background.
        let sidebarItem = SettingsWindowController.isPreview
            ? NSSplitViewItem(viewController: sidebar) : NSSplitViewItem(sidebarWithViewController: sidebar)
        sidebarItem.canCollapse = false
        sidebarItem.minimumThickness = 190
        sidebarItem.maximumThickness = 230
        addSplitViewItem(sidebarItem)
        let detailItem = NSSplitViewItem(viewController: detail)
        detailItem.minimumThickness = 440
        addSplitViewItem(detailItem)
        sidebar.onSelect = { [weak self] pane in self?.show(pane) }
    }

    func select(_ pane: SettingsPane) {
        _ = view
        sidebar.select(pane)
        show(pane)
    }

    private func show(_ pane: SettingsPane) {
        detail.show(pane)
        SettingsPane.lastSelected = pane
        onSelect?(pane)
    }
}

final class SettingsSidebarViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate {
    var onSelect: ((SettingsPane) -> Void)?
    private let table = NSTableView()
    private enum Row: Equatable {
        case header(String)
        case pane(SettingsPane)
        var pane: SettingsPane? { if case .pane(let pane) = self { return pane } else { return nil } }
    }
    /// Section headers and panes.
    private let rows: [Row] = SettingsPane.groups.flatMap { [Row.header($0.title)] + $0.panes.map(Row.pane) }
    private var icons: [SettingsPane: NSImage] = [:]

    override func loadView() {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = false
        let column = NSTableColumn(identifier: .init("pane"))
        table.addTableColumn(column)
        table.headerView = nil
        // Previews draw the sidebar without its material, where the source-list selection would be drawn black.
        table.style = SettingsWindowController.isPreview ? .inset : .sourceList
        table.rowSizeStyle = .custom
        table.rowHeight = 32
        table.backgroundColor = .clear
        table.dataSource = self
        table.delegate = self
        table.setAccessibilityLabel("設定項目")
        scroll.documentView = table
        view = scroll
        SettingsPane.allCases.forEach { icons[$0] = $0.icon }
    }

    func select(_ pane: SettingsPane) {
        _ = view
        guard let row = rows.firstIndex(of: .pane(pane)), table.selectedRow != row else { return }
        table.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
    }

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        rows[row].pane == nil ? (row == 0 ? 22 : 30) : 30
    }

    func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool { rows[row].pane != nil }

    func tableView(_ tableView: NSTableView, isGroupRow row: Int) -> Bool { false }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let pane = rows[row].pane else {
            // A section header: small grey text, as the sidebars of macOS apps.
            let header = NSTableCellView()
            let text = NSTextField(labelWithString: { if case .header(let title) = rows[row] { return title } else { return "" } }())
            text.font = .systemFont(ofSize: NSFont.smallSystemFontSize, weight: .semibold)
            text.textColor = .tertiaryLabelColor
            text.translatesAutoresizingMaskIntoConstraints = false
            header.addSubview(text)
            NSLayoutConstraint.activate([
                text.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: 4),
                text.bottomAnchor.constraint(equalTo: header.bottomAnchor, constant: -4),
            ])
            header.setAccessibilityRole(.staticText)
            return header
        }
        let identifier = NSUserInterfaceItemIdentifier("SidebarCell")
        let cell = tableView.makeView(withIdentifier: identifier, owner: self) as? NSTableCellView ?? {
            let cell = NSTableCellView()
            cell.identifier = identifier
            let image = OpaqueImageView()
            let text = NSTextField(labelWithString: "")
            text.font = .systemFont(ofSize: NSFont.systemFontSize)
            text.lineBreakMode = .byTruncatingTail
            for view in [image, text] as [NSView] {
                view.translatesAutoresizingMaskIntoConstraints = false
                cell.addSubview(view)
            }
            NSLayoutConstraint.activate([
                image.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 2),
                image.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                image.widthAnchor.constraint(equalToConstant: 22),
                image.heightAnchor.constraint(equalToConstant: 22),
                text.leadingAnchor.constraint(equalTo: image.trailingAnchor, constant: 8),
                text.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -4),
                text.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            ])
            cell.imageView = image
            cell.textField = text
            return cell
        }()
        cell.imageView?.image = icons[pane]
        cell.textField?.stringValue = pane.title
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        guard rows.indices.contains(table.selectedRow), let pane = rows[table.selectedRow].pane else { return }
        onSelect?(pane)
    }
}

/// Hosts the selected pane.  Each pane's hosting controller is built once and kept.
final class SettingsDetailViewController: NSViewController {
    var model: SettingsModel!
    private var controllers: [SettingsPane: NSViewController] = [:]
    private var current: NSViewController?

    override func loadView() { view = NSView() }

    func show(_ pane: SettingsPane) {
        _ = view
        let controller = controllers[pane] ?? {
            let controller = NSHostingController(rootView: SettingsPaneView(pane: pane, model: model))
            controller.sizingOptions = []
            controllers[pane] = controller
            return controller
        }()
        guard controller !== current else { return }
        current?.view.removeFromSuperview()
        current?.removeFromParent()
        addChild(controller)
        controller.view.frame = view.bounds
        controller.view.autoresizingMask = [.width, .height]
        view.addSubview(controller.view)
        current = controller
    }
}

/// The sidebar is vibrant, and vibrancy would wash out the coloured icon tiles; draw them as they are.
private final class OpaqueImageView: NSImageView {
    override var allowsVibrancy: Bool { false }
}
