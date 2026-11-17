import AppKit

// The 符號與 Emoji picker's window: a floating input panel like the candidate window (never key or main, never
// activating ZhuyinType), by the caret.  From the top: the search line, the category tabs, a grid of rows of nine with
// the highlighted row numbered 1–9, and a footer with the highlighted symbol's name and the keys.

/// The picker as drawn; `SymbolPicker` holds the state, the controller routes keys to it.
final class SymbolPickerView: InputMaterialView {
    static let cell: CGFloat = 40
    static let padding: CGFloat = 8
    static let searchHeight: CGFloat = 32
    static let tabHeight: CGFloat = 30
    static let footerHeight: CGFloat = 26

    var onPick: ((Int) -> Void)?
    var onTab: ((Int) -> Void)?
    var onScroll: ((Int) -> Void)?
    /// False for illustrations: no hover.
    var hoverEnabled = true

    private var tabs: [SymbolCategory] = []
    private var tab = 0
    private var items: [SymbolItem] = []
    private var highlight = 0
    private var topRow = 0
    private var rowCount = 0
    private var query = ""
    private var searching = false
    private var hovered: Int?
    private var hoveredTab: Int?
    private var scrollRemainder: CGFloat = 0
    private var tracking: NSTrackingArea?

    static var size: NSSize {
        NSSize(width: CGFloat(SymbolPicker.columns) * cell + padding * 2,
               height: padding + searchHeight + tabHeight + 6 + CGFloat(SymbolPicker.visibleRows) * cell + footerHeight + padding / 2)
    }

    override init(frame: NSRect) {
        super.init(frame: frame)
        setFrameSize(Self.size)
        setAccessibilityElement(false)
        setAccessibilityRole(.group)
        setAccessibilityLabel("符號與 Emoji")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    /// Shows `picker` and returns the view's size (fixed, so the window never jumps as the picker changes).
    @discardableResult
    func update(_ picker: SymbolPicker) -> NSSize {
        tabs = picker.tabs
        tab = picker.tab
        items = picker.items
        highlight = picker.highlight
        topRow = picker.topRow
        rowCount = picker.rowCount
        query = picker.shownQuery
        searching = picker.isSearching
        if frame.size != Self.size { setFrameSize(Self.size) }
        rebuildAccessibility()
        needsDisplay = true
        return Self.size
    }

    // MARK: Geometry

    private var searchRect: NSRect { NSRect(x: Self.padding, y: Self.padding, width: bounds.width - Self.padding * 2, height: Self.searchHeight) }
    private var tabsRect: NSRect { NSRect(x: Self.padding, y: searchRect.maxY, width: searchRect.width, height: Self.tabHeight) }
    private var gridRect: NSRect {
        NSRect(x: Self.padding, y: tabsRect.maxY + 6, width: CGFloat(SymbolPicker.columns) * Self.cell,
               height: CGFloat(SymbolPicker.visibleRows) * Self.cell)
    }
    private var footerRect: NSRect { NSRect(x: Self.padding, y: gridRect.maxY, width: searchRect.width, height: Self.footerHeight) }

    private func tabRect(_ index: Int) -> NSRect {
        let width = tabsRect.width / CGFloat(max(1, tabs.count))
        return NSRect(x: tabsRect.minX + CGFloat(index) * width, y: tabsRect.minY + 2, width: width, height: Self.tabHeight - 4)
    }

    /// The rectangle of item `index` when it is in view.
    private func cellRect(_ index: Int) -> NSRect? {
        let row = index / SymbolPicker.columns - topRow
        guard row >= 0, row < SymbolPicker.visibleRows else { return nil }
        let column = index % SymbolPicker.columns
        return NSRect(x: gridRect.minX + CGFloat(column) * Self.cell, y: gridRect.minY + CGFloat(row) * Self.cell,
                      width: Self.cell, height: Self.cell)
    }

    private var visibleIndices: Range<Int> {
        let start = min(items.count, topRow * SymbolPicker.columns)
        return start..<min(items.count, start + SymbolPicker.visibleRows * SymbolPicker.columns)
    }

    private func item(at point: NSPoint) -> Int? { visibleIndices.first { cellRect($0)?.contains(point) == true } }
    private func tab(at point: NSPoint) -> Int? { tabs.indices.first { tabRect($0).contains(point) } }

    // MARK: Drawing

    /// The search line's well: a light tint of the text colour, as a search field on glass.
    private static let fieldColor = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? NSColor(white: 1, alpha: 0.1) : NSColor(white: 0, alpha: 0.06)
    }

    override func draw(_ dirtyRect: NSRect) {
        drawSearch()
        drawTabs()
        drawGrid()
        drawFooter()
    }

    private func drawSearch() {
        let rect = searchRect
        Self.fieldColor.setFill()
        NSBezierPath(roundedRect: rect.insetBy(dx: 0, dy: 3), xRadius: 7, yRadius: 7).fill()
        if let glass = NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 12, weight: .medium).applying(.init(paletteColors: [.secondaryLabelColor]))) {
            glass.draw(in: NSRect(x: rect.minX + 8, y: rect.midY - glass.size.height / 2, width: glass.size.width, height: glass.size.height))
        }
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let text = searching ? query : "打注音或英文搜尋"
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 14), .paragraphStyle: paragraph,
            .foregroundColor: searching ? NSColor.labelColor : NSColor.tertiaryLabelColor,
        ]
        let trailing = searching ? (items.isEmpty ? "沒有符合的符號" : "\(items.count) 個") : (tabs.indices.contains(tab) ? tabs[tab].title : "")
        let trailingAttributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.secondaryLabelColor]
        let trailingWidth = ceil((trailing as NSString).size(withAttributes: trailingAttributes).width)
        let height = (text as NSString).size(withAttributes: attributes).height
        (text as NSString).draw(in: NSRect(x: rect.minX + 28, y: rect.midY - height / 2, width: max(0, rect.width - 44 - trailingWidth),
                                           height: height), withAttributes: attributes)
        let trailingHeight = (trailing as NSString).size(withAttributes: trailingAttributes).height
        (trailing as NSString).draw(at: NSPoint(x: rect.maxX - 9 - trailingWidth, y: rect.midY - trailingHeight / 2),
                                    withAttributes: trailingAttributes)
    }

    private func drawTabs() {
        for index in tabs.indices {
            let rect = tabRect(index).insetBy(dx: 1, dy: 0)
            let selected = !searching && index == tab
            if selected {
                NSColor.controlAccentColor.withAlphaComponent(0.22).setFill()
                NSBezierPath(roundedRect: rect, xRadius: 6, yRadius: 6).fill()
            } else if hoveredTab == index {
                NSColor.quaternaryLabelColor.setFill()
                NSBezierPath(roundedRect: rect, xRadius: 6, yRadius: 6).fill()
            }
            let color: NSColor = selected ? .labelColor : searching ? .tertiaryLabelColor : .secondaryLabelColor
            if tabs[index].id == "recent",
               let clock = NSImage(systemSymbolName: "clock", accessibilityDescription: nil)?
                .withSymbolConfiguration(.init(pointSize: 13, weight: .regular).applying(.init(paletteColors: [color]))) {
                clock.draw(in: NSRect(x: rect.midX - clock.size.width / 2, y: rect.midY - clock.size.height / 2,
                                      width: clock.size.width, height: clock.size.height))
                continue
            }
            let glyph = Self.glyph(tabs[index])
            let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 14), .foregroundColor: color]
            let size = (glyph as NSString).size(withAttributes: attributes)
            // Emoji tabs are dimmed like the others when not selected.
            NSGraphicsContext.current?.saveGraphicsState()
            if !selected, glyph.unicodeScalars.contains(where: { $0.properties.isEmojiPresentation }) {
                NSGraphicsContext.current?.cgContext.setAlpha(searching ? 0.4 : 0.75)
            }
            (glyph as NSString).draw(at: NSPoint(x: rect.midX - size.width / 2, y: rect.midY - size.height / 2), withAttributes: attributes)
            NSGraphicsContext.current?.restoreGraphicsState()
        }
        NSColor.separatorColor.setFill()
        NSRect(x: tabsRect.minX, y: tabsRect.maxY + 2.5, width: tabsRect.width, height: 1).fill()
    }

    /// The tab's mark: one typical symbol of the category.
    static func glyph(_ category: SymbolCategory) -> String {
        let marks = ["punctuation": "，", "brackets": "「」", "math": "±", "arrows": "→", "units": "℃", "bopomofo": "ㄅ",
                     "greek": "α", "box": "┼", "smileys": "😀", "people": "👋", "animals": "🐶", "food": "🍔",
                     "travel": "🚗", "activities": "⚽", "objects": "💡", "symbols": "❤️", "flags": "🏁"]
        return marks[category.id] ?? category.symbols.first?.text ?? "?"
    }

    private func drawGrid() {
        if items.isEmpty {
            let text = searching ? "沒有符合「\(query)」的符號" : ""
            let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 13), .foregroundColor: NSColor.secondaryLabelColor]
            let size = (text as NSString).size(withAttributes: attributes)
            (text as NSString).draw(at: NSPoint(x: gridRect.midX - size.width / 2, y: gridRect.midY - size.height / 2), withAttributes: attributes)
            return
        }
        let highlightedRow = highlight / SymbolPicker.columns
        let symbolFont = NSFont.systemFont(ofSize: 21)
        let numberFont = NSFont.monospacedDigitSystemFont(ofSize: 9, weight: .medium)
        for index in visibleIndices {
            guard let rect = cellRect(index) else { continue }
            let selected = index == highlight
            if selected || hovered == index {
                (selected ? NSColor.controlAccentColor : NSColor.quaternaryLabelColor).setFill()
                NSBezierPath(roundedRect: rect.insetBy(dx: 1.5, dy: 1.5), xRadius: 7, yRadius: 7).fill()
            }
            let text = items[index].text
            let attributes: [NSAttributedString.Key: Any] = [.font: symbolFont, .foregroundColor: selected ? NSColor.white : NSColor.labelColor]
            let size = (text as NSString).size(withAttributes: attributes)
            let drawn = NSRect(x: rect.midX - min(size.width, rect.width - 6) / 2, y: rect.midY - size.height / 2 + 1,
                               width: min(size.width, rect.width - 6), height: size.height)
            (text as NSString).draw(with: drawn, options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine], attributes: attributes)
            if index / SymbolPicker.columns == highlightedRow {
                let number = "\(index % SymbolPicker.columns + 1)"
                let numberAttributes: [NSAttributedString.Key: Any] = [
                    .font: numberFont, .foregroundColor: selected ? NSColor.white.withAlphaComponent(0.85) : NSColor.secondaryLabelColor,
                ]
                (number as NSString).draw(at: NSPoint(x: rect.minX + 5, y: rect.minY + 3), withAttributes: numberAttributes)
            }
        }
        // Where the rows in view are among all of them.
        if rowCount > SymbolPicker.visibleRows {
            let track = NSRect(x: bounds.maxX - 5, y: gridRect.minY + 4, width: 3, height: gridRect.height - 8)
            let length = max(14, track.height * CGFloat(SymbolPicker.visibleRows) / CGFloat(rowCount))
            let offset = (track.height - length) * CGFloat(topRow) / CGFloat(max(1, rowCount - SymbolPicker.visibleRows))
            NSColor.tertiaryLabelColor.setFill()
            NSBezierPath(roundedRect: NSRect(x: track.minX, y: track.minY + offset, width: 3, height: length), xRadius: 1.5, yRadius: 1.5).fill()
        }
    }

    private func drawFooter() {
        let rect = footerRect
        let shown = hovered.flatMap { items.indices.contains($0) ? items[$0] : nil } ?? (items.indices.contains(highlight) ? items[highlight] : nil)
        let hint = "1–9 選取　Tab 類別　Esc 關閉"
        let hintAttributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 11), .foregroundColor: NSColor.tertiaryLabelColor]
        let hintSize = (hint as NSString).size(withAttributes: hintAttributes)
        (hint as NSString).draw(at: NSPoint(x: rect.maxX - hintSize.width - 2, y: rect.midY - hintSize.height / 2), withAttributes: hintAttributes)
        guard let shown, !shown.name.isEmpty else { return }
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.secondaryLabelColor,
                                                         .paragraphStyle: paragraph]
        let text = shown.text + "  " + shown.name
        let height = (text as NSString).size(withAttributes: attributes).height
        (text as NSString).draw(in: NSRect(x: rect.minX + 4, y: rect.midY - height / 2, width: max(0, rect.width - hintSize.width - 16),
                                           height: height), withAttributes: attributes)
    }

    // MARK: Mouse

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        if let index = item(at: point) { onPick?(index) } else if let index = tab(at: point) { onTab?(index) }
    }

    override func scrollWheel(with event: NSEvent) {
        scrollRemainder += event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / Self.cell : event.scrollingDeltaY
        let rows = Int(scrollRemainder.rounded(.towardZero))
        guard rows != 0 else { return }
        scrollRemainder -= CGFloat(rows)
        onScroll?(-rows)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        guard hoverEnabled else { tracking = nil; return }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        tracking = area
    }
    override func mouseMoved(with event: NSEvent) { hover(at: convert(event.locationInWindow, from: nil)) }
    override func mouseEntered(with event: NSEvent) { hover(at: convert(event.locationInWindow, from: nil)) }
    override func mouseExited(with event: NSEvent) { hover(at: nil) }

    func hover(at point: NSPoint?) {
        let item = point.flatMap(item(at:))
        let tab = point.flatMap(tab(at:))
        guard item != hovered || tab != hoveredTab else { return }
        hovered = item
        hoveredTab = tab
        needsDisplay = true
    }

    func resetHover() {
        hovered = nil
        hoveredTab = nil
        scrollRemainder = 0
    }

    // MARK: Accessibility and help tags

    private var elements: [NSAccessibilityElement] = []

    private func rebuildAccessibility() {
        removeAllToolTips()
        var list: [NSAccessibilityElement] = []
        let search = NSAccessibilityElement.element(withRole: .staticText, frame: .zero, label: "搜尋", parent: self) as? NSAccessibilityElement
        if let search {
            search.setAccessibilityValue(searching ? query : "")
            search.setAccessibilityFrameInParentSpace(flippedToParent(searchRect))
            list.append(search)
        }
        for index in tabs.indices {
            guard let element = NSAccessibilityElement.element(withRole: .radioButton, frame: .zero, label: tabs[index].title,
                                                               parent: self) as? NSAccessibilityElement else { continue }
            element.setAccessibilityValue(!searching && index == tab ? 1 : 0)
            element.setAccessibilityFrameInParentSpace(flippedToParent(tabRect(index)))
            addToolTip(tabRect(index), owner: tabs[index].title as NSString, userData: nil)
            list.append(element)
        }
        let highlightedRow = highlight / SymbolPicker.columns
        for index in visibleIndices {
            guard let rect = cellRect(index) else { continue }
            let symbol = items[index]
            let label = symbol.name.isEmpty ? symbol.text : symbol.name
            guard let element = NSAccessibilityElement.element(withRole: .button, frame: .zero, label: label,
                                                               parent: self) as? NSAccessibilityElement else { continue }
            if index / SymbolPicker.columns == highlightedRow { element.setAccessibilityHelp("按 \(index % SymbolPicker.columns + 1) 選取") }
            element.setAccessibilitySelected(index == highlight)
            element.setAccessibilityFrameInParentSpace(flippedToParent(rect))
            addToolTip(rect, owner: (symbol.name.isEmpty ? symbol.text : symbol.text + " " + symbol.name) as NSString, userData: nil)
            list.append(element)
        }
        elements = list
        setAccessibilityChildren(list)
    }

    /// Accessibility frames are in the parent's unflipped coordinates.
    private func flippedToParent(_ rect: NSRect) -> NSRect {
        NSRect(x: rect.minX, y: bounds.height - rect.maxY, width: rect.width, height: rect.height)
    }
}

/// The picker's floating window.
final class SymbolPickerPanelController {
    static let shared = SymbolPickerPanelController()
    var onPick: ((Int) -> Void)?
    var onTab: ((Int) -> Void)?
    var onScroll: ((Int) -> Void)?
    private let content = SymbolPickerView(frame: .zero)
    private lazy var surface = InputSurface(content: content)
    private lazy var panel = InputPanel(content: surface)

    init() {
        content.onPick = { [weak self] in self?.onPick?($0) }
        content.onTab = { [weak self] in self?.onTab?($0) }
        content.onScroll = { [weak self] in self?.onScroll?($0) }
    }

    var isVisible: Bool { panel.isVisible }

    func show(_ picker: SymbolPicker, caret: NSRect, windowLevel: NSWindow.Level) {
        let visible = InputPanelPlacement.screen(at: caret)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let preferences = Preferences.shared
        panel.appearance = preferences.panelAppearance.appearance
        _ = panel
        let size = content.update(picker)
        surface.fitContent()
        panel.level = windowLevel
        let frame = InputPanelPlacement.frame(size: size, caret: caret, visible: visible, textInset: SymbolPickerView.padding,
                                             previousX: panel.isVisible ? panel.frame.minX : nil,
                                             above: preferences.candidatePlacement == .above)
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        if panel.isVisible || PanelMotion.reduceMotion {
            // Reduce Motion: the picker appears at once, with no fade or slide.
            panel.alphaValue = 1
            panel.setFrame(frame, display: true)
            panel.orderFrontRegardless()
        } else {
            PanelMotion.present(panel, frame: frame, caret: caret)
        }
    }

    func hide() {
        panel.orderOut(nil)
        panel.alphaValue = 1
        content.resetHover()
    }
}
