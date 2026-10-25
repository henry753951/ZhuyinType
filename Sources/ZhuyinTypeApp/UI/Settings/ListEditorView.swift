import AppKit
import SwiftUI

/// One row of a `ListEditorView`.  `sortKeys` parallel `cells`; numbers sort numerically.
struct ListRow {
    enum SortKey: Comparable {
        case text(String)
        case number(Double)
        static func < (lhs: SortKey, rhs: SortKey) -> Bool {
            switch (lhs, rhs) {
            case let (.number(a), .number(b)): return a < b
            case let (.text(a), .text(b)): return a.localizedStandardCompare(b) == .orderedAscending
            case (.number, .text): return true
            case (.text, .number): return false
            }
        }
    }

    let id: String
    let cells: [String]
    let sortKeys: [SortKey]
    var icon: NSImage?
    /// Lower-cased text the search matches against.
    let searchText: String

    init(id: String, cells: [String], sortKeys: [SortKey]? = nil, icon: NSImage? = nil) {
        self.id = id
        self.cells = cells
        self.sortKeys = sortKeys ?? cells.map { .text($0) }
        self.icon = icon
        searchText = cells.joined(separator: " ").lowercased()
    }
}

/// A list laid out like the lists in System Settings: one line per row, the first column as the title on the leading
/// side, the other columns as secondary values on the trailing side, and an add/remove bar at the bottom.  Built for
/// long lists: rows are reused, and filtering and sorting work on plain arrays.
final class ListEditorView: NSView, NSTableViewDataSource, NSTableViewDelegate {
    var onAdd: ((NSView) -> Void)?
    var onRemove: (([ListRow]) -> Void)?
    var onExtra: (() -> Void)?

    private let columnCount: Int
    private let table = ListTableView()
    private let scroll = NSScrollView()
    private let empty = NSTextField(labelWithString: "")
    private let emptyText: String
    private let add = NSButton()
    private let remove = NSButton()
    private let extra = NSButton()
    private var allRows: [ListRow] = []
    private var rows: [ListRow] = []
    private var query = ""
    private let sortColumn: Int
    private let ascending: Bool

    init(columns: Int, emptyText: String, extraTitle: String? = nil, sortColumn: Int = 0, ascending: Bool = true) {
        columnCount = columns
        self.emptyText = emptyText
        self.sortColumn = sortColumn
        self.ascending = ascending
        super.init(frame: NSRect(x: 0, y: 0, width: 480, height: 320))

        let column = NSTableColumn(identifier: .init("row"))
        column.resizingMask = .autoresizingMask
        table.addTableColumn(column)
        table.headerView = nil
        table.style = .plain
        table.backgroundColor = .clear
        table.intercellSpacing = .zero
        table.rowHeight = 36
        table.allowsMultipleSelection = true
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.dataSource = self
        table.delegate = self
        table.onDelete = { [weak self] in self?.removeSelected() }
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false

        empty.textColor = .secondaryLabelColor
        empty.alignment = .center

        for (button, symbol, label) in [(add, "plus", "加入"), (remove, "minus", "移除")] {
            button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
            button.bezelStyle = .accessoryBarAction
            button.isBordered = false
            button.imagePosition = .imageOnly
            button.target = self
            button.toolTip = label
        }
        add.action = #selector(addPressed(_:))
        remove.action = #selector(removePressed)

        extra.title = extraTitle ?? ""
        extra.isHidden = extraTitle == nil
        extra.bezelStyle = .push
        extra.controlSize = .small
        extra.target = self
        extra.action = #selector(extraPressed)

        let separator = NSBox()
        separator.boxType = .separator
        let bar = NSStackView(views: [add, remove, NSView(), extra])
        bar.orientation = .horizontal
        bar.spacing = 2
        bar.edgeInsets = NSEdgeInsets(top: 0, left: 6, bottom: 0, right: 8)

        for view in [scroll, empty, separator, bar] as [NSView] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: separator.topAnchor),
            empty.centerXAnchor.constraint(equalTo: scroll.centerXAnchor),
            empty.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
            separator.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            separator.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            separator.bottomAnchor.constraint(equalTo: bar.topAnchor),
            bar.leadingAnchor.constraint(equalTo: leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: trailingAnchor),
            bar.bottomAnchor.constraint(equalTo: bottomAnchor),
            bar.heightAnchor.constraint(equalToConstant: 34),
            add.widthAnchor.constraint(equalToConstant: 26),
            remove.widthAnchor.constraint(equalToConstant: 26),
        ])
        updateChrome()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    var showsAddButton: Bool {
        get { !add.isHidden }
        set { add.isHidden = !newValue }
    }

    /// Replace the rows, keeping the search and (where the ids survive) the selection.
    func setRows(_ newRows: [ListRow], select ids: Set<String>? = nil) {
        let keep = ids ?? Set(selectedRows.map(\.id))
        allRows = newRows
        apply()
        let indexes = IndexSet(rows.indices.filter { keep.contains(rows[$0].id) })
        table.selectRowIndexes(indexes, byExtendingSelection: false)
        if let first = indexes.first, ids != nil { table.scrollRowToVisible(first) }
        updateChrome()
    }

    func setQuery(_ text: String) {
        let next = text.trimmingCharacters(in: .whitespaces).lowercased()
        guard next != query else { return }
        query = next
        apply()
        updateChrome()
    }

    var selectedRows: [ListRow] { table.selectedRowIndexes.compactMap { rows.indices.contains($0) ? rows[$0] : nil } }

    private func apply() {
        var result = query.isEmpty ? allRows : allRows.filter { $0.searchText.contains(query) }
        let column = sortColumn, ascending = self.ascending
        result.sort { a, b in
            let x = a.sortKeys[column], y = b.sortKeys[column]
            if x == y { return a.id < b.id }
            return ascending ? x < y : y < x
        }
        rows = result
        table.reloadData()
    }

    private func updateChrome() {
        empty.isHidden = !rows.isEmpty
        empty.stringValue = query.isEmpty ? emptyText : "沒有符合「\(query)」的項目"
        remove.isEnabled = !table.selectedRowIndexes.isEmpty
        extra.isEnabled = !allRows.isEmpty
    }

    @objc private func addPressed(_ sender: NSButton) { onAdd?(sender) }
    @objc private func removePressed() { removeSelected() }
    @objc private func extraPressed() { onExtra?() }
    private func removeSelected() {
        let selected = selectedRows
        guard !selected.isEmpty else { return }
        onRemove?(selected)
    }

    // MARK: Table

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
        let view = tableView.makeView(withIdentifier: ListRowView.identifier, owner: self) as? ListRowView ?? ListRowView()
        view.drawsSeparator = row < rows.count - 1
        return view
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = tableView.makeView(withIdentifier: ListCellView.identifier, owner: self) as? ListCellView
            ?? ListCellView(valueCount: columnCount - 1)
        cell.show(rows[row])
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) { updateChrome() }
}

/// A row: optional icon, the title, and secondary values on the trailing side.
private final class ListCellView: NSTableCellView {
    static let identifier = NSUserInterfaceItemIdentifier("ListCell")
    private let icon = NSImageView()
    private let title = NSTextField(labelWithString: "")
    private let values: [NSTextField]
    private var iconWidth: NSLayoutConstraint!

    init(valueCount: Int) {
        values = (0..<valueCount).map { _ in NSTextField(labelWithString: "") }
        super.init(frame: .zero)
        identifier = Self.identifier
        title.font = .systemFont(ofSize: NSFont.systemFontSize)
        title.lineBreakMode = .byTruncatingTail
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let stack = NSStackView(views: [icon, title, NSView()] + values)
        stack.orientation = .horizontal
        stack.spacing = 8
        stack.setCustomSpacing(16, after: title)
        for (index, value) in values.enumerated() {
            value.font = index == values.count - 1 && valueCount > 1
                ? .monospacedDigitSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
                : .systemFont(ofSize: NSFont.systemFontSize)
            value.textColor = index == 0 ? .secondaryLabelColor : .tertiaryLabelColor
            value.alignment = .right
            value.setContentHuggingPriority(.required, for: .horizontal)
            if index > 0 { stack.setCustomSpacing(18, after: values[index - 1]) }
        }
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        iconWidth = icon.widthAnchor.constraint(equalToConstant: 0)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconWidth,
            icon.heightAnchor.constraint(equalToConstant: 20),
        ])
        textField = title
        imageView = icon
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func show(_ row: ListRow) {
        title.stringValue = row.cells[0]
        for (index, value) in values.enumerated() { value.stringValue = row.cells[index + 1] }
        icon.image = row.icon
        icon.isHidden = row.icon == nil
        iconWidth.constant = row.icon == nil ? 0 : 20
    }

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet {
            let emphasized = backgroundStyle == .emphasized
            title.textColor = emphasized ? .alternateSelectedControlTextColor : .labelColor
            for (index, value) in values.enumerated() {
                value.textColor = emphasized ? .alternateSelectedControlTextColor
                    : (index == 0 ? .secondaryLabelColor : .tertiaryLabelColor)
            }
        }
    }
}

/// Rounded selection and an inset separator under each row, as in grouped settings lists.
private final class ListRowView: NSTableRowView {
    static let identifier = NSUserInterfaceItemIdentifier("ListRow")
    var drawsSeparator = true { didSet { if drawsSeparator != oldValue { needsDisplay = true } } }

    override init(frame: NSRect) {
        super.init(frame: frame)
        identifier = Self.identifier
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func drawSelection(in dirtyRect: NSRect) {
        let rect = bounds.insetBy(dx: 6, dy: 2)
        (isEmphasized ? NSColor.controlAccentColor : NSColor.unemphasizedSelectedContentBackgroundColor).setFill()
        NSBezierPath(roundedRect: rect, xRadius: 8, yRadius: 8).fill()
    }

    override func drawBackground(in dirtyRect: NSRect) {
        guard drawsSeparator, !isSelected, !isNextRowSelected else { return }
        NSColor.separatorColor.setFill()
        NSRect(x: 12, y: bounds.maxY - 0.5, width: bounds.width - 24, height: 0.5).fill()
    }

    override var isFlipped: Bool { true }
}

private final class ListTableView: NSTableView {
    var onDelete: (() -> Void)?
    override func keyDown(with event: NSEvent) {
        if event.keyCode == 51 || event.keyCode == 117 { onDelete?() } else { super.keyDown(with: event) }
    }
}

/// Puts a `ListEditorView` inside a SwiftUI form section.
struct ListEditor: NSViewRepresentable {
    let view: ListEditorView
    func makeNSView(context: Context) -> ListEditorView { view }
    func updateNSView(_ nsView: ListEditorView, context: Context) {}
}
