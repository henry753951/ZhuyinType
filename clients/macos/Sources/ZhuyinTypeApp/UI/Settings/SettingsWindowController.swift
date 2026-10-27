import AppKit
import SwiftUI
import ZhuyinCore

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
    private var pane: SettingsPane = .general
    private static let searchItem = NSToolbarItem.Identifier("search")

    func show() {
        let window = self.window ?? makeWindow()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        model.reloadLists()
    }

    /// The window, built but not shown, open on `pane`; also used by the preview renderer.
    func makeWindow(pane: SettingsPane? = nil) -> NSWindow {
        if let window {
            if let pane { split?.select(pane) }
            return window
        }
        let split = SettingsSplitViewController(model: model)
        let window = NSWindow(contentViewController: split)
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
        split.onSelect = { [weak self, weak window] pane in
            window?.title = pane.title
            self?.paneChanged(pane, toolbar: toolbar)
        }
        split.select(pane ?? SettingsPane.lastSelected)
        self.window = window
        self.split = split
        return window
    }

    /// Shows `pane`, as when its row in the sidebar is clicked.
    func select(_ pane: SettingsPane) {
        split?.select(pane)
    }

    func windowWillClose(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }

    // MARK: Toolbar search, for the word lists

    private var searchedList: ListEditorView? {
        switch pane {
        case .phrases: return model.phrases.view
        case .learned: return model.learned.view
        default: return nil
        }
    }

    private func paneChanged(_ pane: SettingsPane, toolbar: NSToolbar) {
        searchedList?.setQuery("")
        self.pane = pane
        let index = toolbar.items.firstIndex { $0.itemIdentifier == Self.searchItem }
        if searchedList != nil, index == nil {
            toolbar.insertItem(withItemIdentifier: Self.searchItem, at: toolbar.items.count)
        } else if searchedList == nil, let index {
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
        searchedList?.setQuery(field.stringValue)
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

    func reloadLists() {
        phrases.reload()
        learned.reload()
        apps.reload()
    }
}

enum SettingsPane: String, CaseIterable {
    case general, keys, candidates, context, phrases, learned, advanced, about

    /// The sidebar's groups, separated by a little space as in System Settings.
    static let groups: [[SettingsPane]] = [[.general, .keys, .candidates, .context], [.phrases, .learned], [.advanced, .about]]

    var title: String {
        switch self {
        case .general: return "一般"
        case .keys: return "快捷鍵"
        case .candidates: return "候選字視窗"
        case .context: return "前文"
        case .phrases: return "自訂詞"
        case .learned: return "記住的字詞"
        case .advanced: return "進階"
        case .about: return "關於"
        }
    }

    var symbol: String {
        switch self {
        case .general: return "gearshape.fill"
        case .keys: return "keyboard.fill"
        case .candidates: return "list.bullet.rectangle"
        case .context: return "text.alignleft"
        case .phrases: return "character.book.closed.fill"
        case .learned: return "clock.arrow.circlepath"
        case .advanced: return "slider.horizontal.3"
        case .about: return "info"
        }
    }

    var tint: NSColor {
        switch self {
        case .general: return .systemGray
        case .keys: return .systemIndigo
        case .candidates: return .systemBlue
        case .context: return .systemTeal
        case .phrases: return .systemOrange
        case .learned: return .systemPurple
        case .advanced: return .systemGray
        case .about: return .systemGray
        }
    }

    private static let defaultsKey = "settingsLastPane"
    static var lastSelected: SettingsPane {
        get { UserDefaults.standard.string(forKey: defaultsKey).flatMap(SettingsPane.init(rawValue:)) ?? .general }
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
    /// Panes, with nil for the small gap between groups.
    private let rows: [SettingsPane?] = SettingsPane.groups.enumerated().flatMap { index, group in
        (index == 0 ? [] : [nil]) + group.map { Optional($0) }
    }
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
        guard let row = rows.firstIndex(of: pane), table.selectedRow != row else { return }
        table.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
    }

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat { rows[row] == nil ? 10 : 32 }

    func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool { rows[row] != nil }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let pane = rows[row] else { return NSView() }
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
        guard rows.indices.contains(table.selectedRow), let pane = rows[table.selectedRow] else { return }
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
