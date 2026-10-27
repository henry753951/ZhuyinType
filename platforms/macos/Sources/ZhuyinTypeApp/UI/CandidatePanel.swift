import AppKit
import ZhuyinCore

/// Input UI must never take focus away from the document being edited.
final class InputPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }

    init(content: NSView) {
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        hidesOnDeactivate = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        isReleasedWhenClosed = false
        contentView = content
    }
}

/// The backdrop of the input UI: Liquid Glass on macOS 26 and later, the popover material before that.  The owner
/// sizes `content`; the surface takes the same size.
final class InputSurface: NSView {
    static let cornerRadius: CGFloat = 14
    let content: NSView
    private let backdrop: NSView

    init(content: NSView, cornerRadius: CGFloat = InputSurface.cornerRadius) {
        self.content = content
        if #available(macOS 26.0, *) {
            let glass = NSGlassEffectView()
            glass.cornerRadius = cornerRadius
            glass.contentView = content
            backdrop = glass
        } else {
            let effect = NSVisualEffectView()
            effect.material = .popover
            effect.blendingMode = .behindWindow
            effect.state = .active
            effect.wantsLayer = true
            effect.layer?.cornerRadius = cornerRadius
            effect.layer?.masksToBounds = true
            content.frame = effect.bounds
            content.autoresizingMask = [.width, .height]
            effect.addSubview(content)
            backdrop = effect
        }
        super.init(frame: NSRect(origin: .zero, size: content.frame.size))
        backdrop.frame = bounds
        backdrop.autoresizingMask = [.width, .height]
        addSubview(backdrop)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    /// Follow the content's size after the owner has laid it out.
    func fitContent() {
        if frame.size != content.frame.size { setFrameSize(content.frame.size) }
    }
}

/// Content drawn on an `InputSurface`; the surface provides the background.
class InputMaterialView: NSView {
    override var isFlipped: Bool { true }
}

private final class CandidateRow: NSView {
    override var isFlipped: Bool { true }
    private var candidate = Candidate(text: "")
    private var key = ""
    private var fontSize: CGFloat = 18
    private var compact = false
    private var selected = false
    private var hovered = false
    private var tracking: NSTrackingArea?
    private var keyAttributes: [NSAttributedString.Key: Any] = [:]
    private var textAttributes: [NSAttributedString.Key: Any] = [:]
    private var annotationAttributes: [NSAttributedString.Key: Any] = [:]
    var action: (() -> Void)?
    private var keyInset: CGFloat { compact ? 6 : 8 }
    var textInset: CGFloat { keyInset + ceil(fontSize * 0.68) + (compact ? 5 : 8) }
    private var trailingPad: CGFloat { compact ? 7 : 10 }
    var isSelectedRow: Bool { selected }
    var preferredWidth: CGFloat = 0

    func update(candidate: Candidate, key: String, fontSize: CGFloat, compact: Bool, selected: Bool) {
        if self.candidate == candidate, self.key == key, self.fontSize == fontSize, self.compact == compact,
           self.selected == selected { return }
        self.candidate = candidate
        self.key = key
        self.fontSize = fontSize
        self.compact = compact
        self.selected = selected
        refreshAttributes()
        let textWidth = (candidate.text as NSString).size(withAttributes: textAttributes).width
        let annotationWidth = candidate.annotation.map { ($0 as NSString).size(withAttributes: annotationAttributes).width + 10 } ?? 0
        preferredWidth = ceil(textInset + textWidth + annotationWidth + trailingPad)
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(([key, candidate.text, candidate.annotation].compactMap { $0 }).joined(separator: "，"))
        setAccessibilityValue(selected ? "已選取" : "")
        needsDisplay = true
    }
    private func refreshAttributes() {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        keyAttributes = [.font: NSFont.monospacedDigitSystemFont(ofSize: fontSize * 0.68, weight: .medium),
                         .foregroundColor: selected ? NSColor.white.withAlphaComponent(0.85) : NSColor.secondaryLabelColor,
                         .paragraphStyle: paragraph]
        textAttributes = [.font: NSFont.systemFont(ofSize: fontSize),
                          .foregroundColor: selected ? NSColor.white : NSColor.labelColor, .paragraphStyle: paragraph]
        annotationAttributes = [.font: NSFont.systemFont(ofSize: fontSize * 0.68),
                                .foregroundColor: selected ? NSColor.white.withAlphaComponent(0.8) : NSColor.tertiaryLabelColor,
                                .paragraphStyle: paragraph]
    }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        refreshAttributes()
        needsDisplay = true
    }
    /// The highlight of the selected row is drawn by the window (`SelectionHighlight`), so it stays put while pages turn.
    var drawsSelection = false
    var cornerRadius: CGFloat { compact ? 6 : 8 }
    override func draw(_ dirtyRect: NSRect) {
        if selected && drawsSelection || hovered && !selected {
            (selected ? NSColor.controlAccentColor : NSColor.quaternaryLabelColor).setFill()
            let radius: CGFloat = compact ? 6 : 8
            NSBezierPath(roundedRect: bounds.insetBy(dx: 0, dy: 1), xRadius: radius, yRadius: radius).fill()
        }
        let textHeight = (candidate.text as NSString).size(withAttributes: textAttributes).height
        let keyHeight = (key as NSString).size(withAttributes: keyAttributes).height
        (key as NSString).draw(in: NSRect(x: keyInset, y: (bounds.height - keyHeight) / 2, width: textInset - keyInset - 4,
                                          height: keyHeight), withAttributes: keyAttributes)
        let (available, mainWidth) = textWidths()
        (candidate.text as NSString).draw(in: NSRect(x: textInset, y: (bounds.height - textHeight) / 2, width: mainWidth, height: textHeight), withAttributes: textAttributes)
        if let annotation = candidate.annotation {
            let height = (annotation as NSString).size(withAttributes: annotationAttributes).height
            (annotation as NSString).draw(in: NSRect(x: textInset + mainWidth + 10, y: (bounds.height - height) / 2,
                                                     width: max(0, available - mainWidth - 10), height: height), withAttributes: annotationAttributes)
        }
    }
    /// The room for text after the key, and the width the candidate's own text gets.
    private func textWidths() -> (available: CGFloat, main: CGFloat) {
        let available = max(0, bounds.width - textInset - trailingPad)
        let annotationWidth = candidate.annotation.map { ($0 as NSString).size(withAttributes: annotationAttributes).width } ?? 0
        let main = min((candidate.text as NSString).size(withAttributes: textAttributes).width,
                       max(0, available - (annotationWidth > 0 ? annotationWidth + 10 : 0)))
        return (available, main)
    }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        tracking = area
    }
    override func mouseEntered(with event: NSEvent) { hovered = true; needsDisplay = true }
    override func mouseExited(with event: NSEvent) { hovered = false; needsDisplay = true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { action?() }
    override func accessibilityPerformPress() -> Bool { action?(); return true }
    func resetHover() { hovered = false; needsDisplay = true }
}

private final class ZhuyinHintButton: NSButton {
    var hint = ""
    /// The key that types the reading as Zhuyin.
    var key = "`"
    private let hintFont = NSFont.systemFont(ofSize: 12)
    /// No keycap when typing the Zhuyin itself is turned off.
    private var keycapWidth: CGFloat {
        key.isEmpty ? -8.5 : max(19, ceil((key as NSString).size(withAttributes: [.font: hintFont]).width) + 9)
    }
    var preferredWidth: CGFloat { keycapWidth + 9 + (hint as NSString).size(withAttributes: [.font: hintFont]).width }
    override func draw(_ dirtyRect: NSRect) {
        let keycap = NSRect(x: 0.5, y: (bounds.height - 19) / 2, width: keycapWidth, height: 19)
        if !key.isEmpty {
            let border = NSBezierPath(roundedRect: keycap, xRadius: 4, yRadius: 4)
            border.lineWidth = 1
            NSColor.quaternaryLabelColor.setStroke()
            border.stroke()
        }
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let attributes: [NSAttributedString.Key: Any] = [.font: hintFont, .foregroundColor: NSColor.secondaryLabelColor,
                                                         .paragraphStyle: paragraph]
        let height = (hint as NSString).size(withAttributes: attributes).height
        let keyWidth = (key as NSString).size(withAttributes: attributes).width
        (key as NSString).draw(at: NSPoint(x: keycap.midX - keyWidth / 2, y: (bounds.height - height) / 2), withAttributes: attributes)
        let textX = keycap.maxX + 8
        (hint as NSString).draw(in: NSRect(x: textX, y: (bounds.height - height) / 2,
                                          width: max(0, bounds.width - textX), height: height), withAttributes: attributes)
    }
}

/// The engine's one-line notice, in the footer's keycap style: "Enter 加入自訂詞：你好" shows a ↩ keycap, the action in
/// secondary text and the phrase in full colour; "已加入自訂詞：你好" leads with a green check instead.
private final class NoticeView: NSView {
    override var isFlipped: Bool { true }
    var message = "" { didSet { if message != oldValue { parse(); needsDisplay = true } } }
    var fontSize: CGFloat = 13 { didSet { if fontSize != oldValue { needsDisplay = true } } }
    private var key: String?
    private var done = false
    private var label = ""
    private var value = ""
    private var font: NSFont { .systemFont(ofSize: fontSize) }
    private var keycapHeight: CGFloat { ceil(fontSize * 1.45) }
    private var keycapWidth: CGFloat { max(keycapHeight, ceil(((key ?? "") as NSString).size(withAttributes: [.font: font]).width) + 9) }
    private var leadWidth: CGFloat { key != nil ? keycapWidth + 7 : done ? fontSize + 6 : 0 }
    private var valueText: String { value.isEmpty ? "" : "：" + value }
    var preferredWidth: CGFloat {
        ceil(leadWidth + (label as NSString).size(withAttributes: [.font: font]).width
             + (valueText as NSString).size(withAttributes: [.font: NSFont.systemFont(ofSize: fontSize, weight: .medium)]).width)
    }
    private func parse() {
        var rest = message
        key = nil
        if rest.hasPrefix("Enter ") { key = "↩"; rest.removeFirst(6) }
        done = rest.hasPrefix("已")
        if let colon = rest.firstIndex(of: "：") {
            label = String(rest[..<colon])
            value = String(rest[rest.index(after: colon)...])
        } else {
            label = rest
            value = ""
        }
        setAccessibilityElement(true)
        setAccessibilityRole(.staticText)
        setAccessibilityLabel(message)
    }
    override func draw(_ dirtyRect: NSRect) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let labelAttributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.secondaryLabelColor,
                                                              .paragraphStyle: paragraph]
        let textHeight = (label as NSString).size(withAttributes: labelAttributes).height
        let textY = (bounds.height - textHeight) / 2
        if let key {
            let keycap = NSRect(x: 0.5, y: ((bounds.height - keycapHeight) / 2).rounded() + 0.5, width: keycapWidth - 1, height: keycapHeight - 1)
            let border = NSBezierPath(roundedRect: keycap, xRadius: 4, yRadius: 4)
            border.lineWidth = 1
            NSColor.tertiaryLabelColor.setStroke()
            border.stroke()
            let keyWidth = (key as NSString).size(withAttributes: labelAttributes).width
            (key as NSString).draw(at: NSPoint(x: keycap.midX - keyWidth / 2, y: textY), withAttributes: labelAttributes)
        } else if done, let check = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: nil)?
                    .withSymbolConfiguration(.init(pointSize: fontSize, weight: .medium).applying(.init(paletteColors: [.white, .systemGreen]))) {
            let size = check.size
            check.draw(in: NSRect(x: 0, y: (bounds.height - size.height) / 2, width: size.width, height: size.height))
        }
        let labelWidth = (label as NSString).size(withAttributes: labelAttributes).width
        let available = max(0, bounds.width - leadWidth)
        (label as NSString).draw(in: NSRect(x: leadWidth, y: textY, width: min(labelWidth, available), height: textHeight),
                                 withAttributes: labelAttributes)
        let valueAttributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: fontSize, weight: .medium),
                                                              .foregroundColor: NSColor.labelColor, .paragraphStyle: paragraph]
        (valueText as NSString).draw(in: NSRect(x: leadWidth + labelWidth, y: textY, width: max(0, available - labelWidth), height: textHeight),
                                     withAttributes: valueAttributes)
    }
}

/// A footer switch drawn as its keyboard key and a short name, accented while on: ⇧↓ 單字 (only the character at the
/// target) and ⇧↑ 修正 (typing corrections and fuzzy matches first).
private final class KeycapToggle: NSButton {
    var on = false { didSet { needsDisplay = true; setAccessibilityValue(on ? "開啟" : "關閉") } }
    /// The key shown on the keycap: the command's current shortcut.
    var key: String { didSet { needsDisplay = true } }
    private let label: String
    private let labelFont = NSFont.systemFont(ofSize: 12)
    init(key: String, label: String, help: String) {
        self.key = key
        self.label = label
        super.init(frame: .zero)
        isBordered = false
        toolTip = help
        setAccessibilityLabel("\(label)，\(help)")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    private var keycapWidth: CGFloat { max(19, ceil((key as NSString).size(withAttributes: [.font: labelFont]).width) + 9) }
    var preferredWidth: CGFloat { keycapWidth + 8 + (label as NSString).size(withAttributes: [.font: labelFont]).width }
    override func draw(_ dirtyRect: NSRect) {
        let keycap = NSRect(x: 0.5, y: (bounds.height - 19) / 2, width: keycapWidth, height: 19)
        let border = NSBezierPath(roundedRect: keycap, xRadius: 4, yRadius: 4)
        if on {
            NSColor.controlAccentColor.setFill()
            border.fill()
        } else {
            border.lineWidth = 1
            NSColor.quaternaryLabelColor.setStroke()
            border.stroke()
        }
        let color: NSColor = on ? .controlAccentColor : .secondaryLabelColor
        let keyAttributes: [NSAttributedString.Key: Any] = [.font: labelFont, .foregroundColor: on ? NSColor.white : color]
        let attributes: [NSAttributedString.Key: Any] = [.font: labelFont, .foregroundColor: color]
        let height = (label as NSString).size(withAttributes: attributes).height
        let keyWidth = (key as NSString).size(withAttributes: keyAttributes).width
        (key as NSString).draw(at: NSPoint(x: keycap.midX - keyWidth / 2, y: (bounds.height - height) / 2), withAttributes: keyAttributes)
        (label as NSString).draw(at: NSPoint(x: keycap.maxX + 7, y: (bounds.height - height) / 2), withAttributes: attributes)
    }
}

/// The settings that shape the candidate window, from the 候選字視窗 page.
struct CandidateStyle: Equatable {
    var layout: CandidateLayout = .vertical
    var fontSize: Double = 18
    var density: CandidateDensity = .standard
    var showsZhuyinHint = true
    var showsAnnotations = true
    var showsListSwitches = true
    var keyLabels = CandidateKeyLabels()
}

/// One page of candidate rows.  The window has two, so a page change can move the old page out while the new one
/// comes in; both sit in a clip the size of the rows' area.
private final class RowPage: NSView {
    override var isFlipped: Bool { true }
    var rows: [CandidateRow] = []
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    func ensure(_ count: Int) {
        while rows.count < count {
            let row = CandidateRow(frame: .zero)
            rows.append(row)
            addSubview(row)
        }
    }
}

/// The selected row's highlight, under the pages: it does not move or fade when the page turns.
private final class SelectionHighlight: NSView {
    var radius: CGFloat = 8
    override func draw(_ dirtyRect: NSRect) {
        NSColor.controlAccentColor.setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 0, dy: 1), xRadius: radius, yRadius: radius).fill()
    }
}

/// What makes two lists the same list, so that a new page of it is a page change (and not a new list).
private struct ListIdentity: Equatable {
    var count: Int, first: String, pageSize: Int, tolerant: Bool?, singleCharacter: Bool
    init(_ list: CandidateList) {
        count = list.items.count
        first = list.items.first?.text ?? ""
        pageSize = list.pageSize
        tolerant = list.tolerant
        singleCharacter = list.singleCharacter
    }
}

/// How a page change and the end of the list move: short, eased out, a few points along the list, never delaying a key.
enum PageMotion {
    static let duration: TimeInterval = 0.14
    /// The old rows are gone after the first 35% and the new ones start at 30%, so two pages of text are never
    /// legible at once (their opacities add up to about 1 at most).
    static let outDuration: TimeInterval = 0.05
    static let inDelay: TimeInterval = 0.042
    /// How far the pages travel.
    static let offset: CGFloat = 10
    /// The nudge at the first or last page.
    static let bounce: CGFloat = 3
    static let bounceOut: TimeInterval = 0.06
    static let bounceBack: TimeInterval = 0.12
}

final class CandidateContentView: InputMaterialView {
    override var isFlipped: Bool { true }
    private let rowClip = InputMaterialView()
    private let pageViews = [RowPage(frame: .zero), RowPage(frame: .zero)]
    private let highlight = SelectionHighlight()
    private var highlightOrigin = NSPoint.zero
    private var front = 0
    private var rows: [CandidateRow] { pageViews[front].rows }
    /// The list shown and its page, to tell a page change of the same list from a new list.
    private var shown: (identity: ListIdentity, page: Int)?
    private var shownStyle: CandidateStyle?
    /// The largest size of the list shown, so turning pages never resizes the window.
    private var steadySize = NSSize.zero
    private var transitionGeneration = 0
    /// For the UI previews: draw a page change (`direction`) or the end bounce at `progress` (0…1) instead of animating.
    var pinnedMotion: (kind: PinnedMotion, direction: Int, progress: CGFloat)?
    enum PinnedMotion { case page, bounce }
    private var layoutIsVertical = true
    private let previewLabel = NSTextField(labelWithString: "")
    private let noticeView = NoticeView()
    private let zhuyinButton = ZhuyinHintButton()
    private let singleButton = KeycapToggle(key: "⇧↓", label: "單字", help: "只列這一個字")
    private let correctionButton = KeycapToggle(key: "⇧↑", label: "修正", help: "列出按錯修正的字")
    private let pageLabel = NSTextField(labelWithString: "")
    private let previous = NSButton()
    private let next = NSButton()
    var onSelect: ((Int) -> Void)?
    var onPage: ((Int) -> Void)?
    var onZhuyin: (() -> Void)?
    var onCorrections: (() -> Void)?
    var onSingleCharacter: (() -> Void)?
    /// The current shortcuts on the keycaps, so the window matches the 快捷鍵 settings.
    var keyLabels = CandidateKeyLabels()
    private(set) var candidateTextInset: CGFloat = 36

    override init(frame: NSRect) {
        super.init(frame: frame)
        rowClip.wantsLayer = true
        rowClip.layer?.masksToBounds = true
        rowClip.addSubview(highlight)
        for page in pageViews { rowClip.addSubview(page) }
        pageViews[1].isHidden = true
        addSubview(rowClip)
        addSubview(previewLabel)
        addSubview(noticeView)
        zhuyinButton.isBordered = false
        zhuyinButton.target = self
        zhuyinButton.action = #selector(chooseZhuyin)
        addSubview(zhuyinButton)
        for (button, action) in [(singleButton, #selector(toggleSingleCharacter)), (correctionButton, #selector(toggleCorrections))] {
            button.target = self
            button.action = action
            addSubview(button)
        }
        previewLabel.lineBreakMode = .byTruncatingTail
        previewLabel.maximumNumberOfLines = 1
        pageLabel.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        pageLabel.textColor = .secondaryLabelColor
        pageLabel.alignment = .center
        pageLabel.wantsLayer = true
        addSubview(pageLabel)
        for (button, symbol, label) in [(previous, "chevron.left", "上一頁"), (next, "chevron.right", "下一頁")] {
            button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
            button.imageScaling = .scaleProportionallyDown
            button.isBordered = false
            button.bezelStyle = .inline
            button.target = self
            button.setAccessibilityLabel(label)
            addSubview(button)
        }
        previous.action = #selector(pageBack)
        next.action = #selector(pageForward)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    @objc private func pageBack() { onPage?(-1) }
    @objc private func pageForward() { onPage?(1) }
    @objc private func chooseZhuyin() { onZhuyin?() }
    @objc private func toggleCorrections() { onCorrections?() }
    @objc private func toggleSingleCharacter() { onSingleCharacter?() }

    /// Lays the window out for `list` and returns its size.  `style` holds the settings of the 候選字視窗 page.
    @discardableResult
    func update(list: CandidateList?, preview: String?, keys: [Character], style: CandidateStyle,
                maxWidth: CGFloat = 1200, notice: String? = nil, zhuyinHint: String? = nil) -> NSSize {
        keyLabels = style.keyLabels
        let layout = style.layout
        let fontSize = CGFloat(style.fontSize)
        let compact = style.density == .compact
        let indices: [Int]
        if let list, list.items.indices.contains(list.selection) { indices = Array(list.pageRange) }
        else { indices = [] }
        // A new page of the same list: the page shown becomes the outgoing one and the other takes the new rows.
        let identity = list.map(ListIdentity.init)
        var direction = 0
        if let list, !indices.isEmpty, let shown, shown.identity == identity, shown.page != list.page {
            direction = shown.page == list.pageCount - 1 && list.page == 0 ? 1 : list.page > shown.page ? 1 : -1
        }
        if identity == nil || identity != shown?.identity || style != shownStyle { steadySize = .zero }
        shownStyle = style
        shown = identity.map { ($0, list?.page ?? 0) }
        if direction != 0 { front = 1 - front }
        pageViews[front].ensure(indices.count)
        let rowHeight = ceil(fontSize * (compact ? 1.32 : 1.55))
        let padding: CGFloat = compact ? 4 : 6
        let spacing: CGFloat = compact ? 4 : 10
        let hintHeight: CGFloat = compact ? 23 : 26
        let footerHeight: CGFloat = compact ? 23 : 25
        let hasPreview = !(preview ?? "").isEmpty
        let listOpen = !indices.isEmpty
        let hasPages = (list?.pageCount ?? 1) > 1 && listOpen
        // The reading being changed heads the list and never sets the width; the footer holds only fixed-width
        // controls, so nothing moves as the reading grows or the page count changes.  With the switches hidden, a
        // switch still shows while it is on, so the list in use is always visible.
        let readable = !(zhuyinHint ?? "").isEmpty && listOpen
        let hasHint = readable && style.showsZhuyinHint
        // A switch whose list is turned off on the 快捷鍵 page has no key and is not shown.
        let hasToggle = list?.tolerant != nil && listOpen && !style.keyLabels.corrections.isEmpty
            && (style.showsListSwitches || list?.tolerant == true)
        let hasSingle = readable && !style.keyLabels.single.isEmpty
            && (style.showsListSwitches || list?.singleCharacter == true)
        let hasFooter = hasPages || hasToggle || hasSingle
        let pageWidth: CGFloat = hasFooter ? 90 : 0
        zhuyinButton.hint = zhuyinHint ?? ""
        zhuyinButton.isEnabled = keyLabels.zhuyinClickable
        zhuyinButton.key = keyLabels.zhuyin
        singleButton.key = keyLabels.single
        correctionButton.key = keyLabels.corrections
        zhuyinButton.setAccessibilityLabel(keyLabels.zhuyin + "，" + (zhuyinHint ?? ""))
        zhuyinButton.needsDisplay = true
        correctionButton.on = list?.tolerant == true
        singleButton.on = list?.singleCharacter == true
        let singleWidth: CGFloat = hasSingle ? ceil(singleButton.preferredWidth) + 8 : 0
        let toggleWidth: CGFloat = hasToggle ? ceil(correctionButton.preferredWidth) + 8 : 0
        let footerWidth = pageWidth + singleWidth + toggleWidth
        // A notice on its own is a compact pill; above candidates it is a row of the window.
        var width: CGFloat = hasPreview || listOpen ? (compact ? 96 : 160) : 0
        var y: CGFloat = padding
        noticeView.isHidden = notice == nil
        noticeView.message = notice ?? ""
        noticeView.fontSize = (fontSize * 0.72).rounded()
        let noticeHeight = ceil(fontSize * 1.25)
        if notice != nil {
            width = max(width, noticeView.preferredWidth + 28)
            y += noticeHeight + (hasPreview || listOpen ? 3 : 0)
        }
        let previewY = y
        previewLabel.isHidden = !hasPreview
        if let preview, hasPreview {
            let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: fontSize),
                .foregroundColor: NSColor.labelColor, .underlineStyle: NSUnderlineStyle.single.rawValue]
            previewLabel.attributedStringValue = NSAttributedString(string: preview, attributes: attributes)
            width = max(width, previewLabel.intrinsicContentSize.width + 28)
            y += rowHeight + (listOpen ? 5 : 0)
        }
        let hintY = y
        if hasHint { y += hintHeight }
        for (offset, row) in rows.enumerated() {
            row.isHidden = offset >= indices.count
            guard offset < indices.count, let list else { row.action = nil; continue }
            let index = indices[offset]
            var candidate = list.items[index]
            if !style.showsAnnotations { candidate.annotation = nil }
            row.update(candidate: candidate, key: offset < keys.count ? String(keys[offset]) : "",
                       fontSize: fontSize, compact: compact, selected: index == list.selection)
            row.action = { [weak self] in self?.onSelect?(index) }
        }
        let activeRows = rows.prefix(indices.count)
        let gaps = CGFloat(max(0, indices.count - 1)) * spacing
        let naturalRowWidth = activeRows.reduce(0) { $0 + $1.preferredWidth }
        if layout == .vertical { width = max(width, (activeRows.map(\.preferredWidth).max() ?? 0) + padding * 2) }
        else { width = max(width, naturalRowWidth + gaps + padding * 2 + footerWidth) }
        width = max(width, footerWidth + padding * 2)
        // Pages of one list keep the widest width seen, so the window does not jump as pages turn.
        width = min(maxWidth, ceil(max(width, steadySize.width)))
        noticeView.frame = NSRect(x: 14, y: padding, width: max(0, width - 28), height: noticeHeight)
        previewLabel.frame = NSRect(x: 14, y: previewY, width: max(0, width - 28), height: rowHeight)
        var x: CGFloat = 0
        let availableWidth = max(0, width - padding * 2 - gaps - (layout == .horizontal ? footerWidth : 0))
        let scale = min(1, availableWidth / max(1, naturalRowWidth))
        // The rows' area: a vertical list with pages keeps room for a full page, so the last page is the same height.
        let slots = layout == .vertical && hasPages ? max(indices.count, list?.pageSize ?? 0) : indices.count
        let rowsTop = y
        let rowsArea = NSRect(x: padding, y: rowsTop, width: max(0, width - padding * 2 - (layout == .horizontal ? footerWidth : 0)),
                              height: listOpen ? (layout == .vertical ? CGFloat(slots) * rowHeight : rowHeight) : 0)
        var rowY: CGFloat = 0
        for row in activeRows {
            let rowWidth = layout == .vertical ? width - padding * 2 : floor(row.preferredWidth * scale)
            row.frame = NSRect(x: x, y: rowY, width: max(0, rowWidth), height: rowHeight)
            if layout == .vertical { rowY += rowHeight } else { x += rowWidth + spacing }
        }
        rowClip.frame = rowsArea
        rowClip.isHidden = !listOpen
        if let selected = activeRows.first(where: { $0.isSelectedRow }) {
            highlight.radius = selected.cornerRadius
            highlight.frame = selected.frame
            highlightOrigin = selected.frame.origin
            highlight.isHidden = false
            highlight.needsDisplay = true
        } else {
            highlight.isHidden = true
        }
        layoutIsVertical = layout == .vertical
        if layout == .vertical { y += rowsArea.height }
        let pageY = layout == .horizontal ? y + (rowHeight - 22) / 2 : y + (compact ? 1 : 3)
        if layout == .horizontal && listOpen { y += rowHeight }
        candidateTextInset = listOpen ? padding + (activeRows.first?.textInset ?? 29) : 14
        pageLabel.isHidden = !hasFooter
        previous.isHidden = !hasFooter
        next.isHidden = !hasFooter
        zhuyinButton.isHidden = !hasHint
        correctionButton.isHidden = !hasToggle
        singleButton.isHidden = !hasSingle
        if hasHint {
            zhuyinButton.frame = NSRect(x: padding + 4, y: hintY + (compact ? 1 : 2),
                                        width: min(ceil(zhuyinButton.preferredWidth), width - padding * 2 - 8), height: 22)
        }
        if hasFooter {
            let footerX = layout == .horizontal ? width - footerWidth : padding + 4
            singleButton.frame = NSRect(x: footerX, y: pageY, width: max(0, singleWidth - 8), height: 22)
            correctionButton.frame = NSRect(x: footerX + singleWidth, y: pageY, width: max(0, toggleWidth - 8), height: 22)
        }
        if hasFooter, let list {
            let pageText = "\(list.page + 1) / \(list.pageCount)"
            if direction != 0, pageText != pageLabel.stringValue, PanelMotion.animates, window?.isVisible == true {
                let fade = CATransition()
                fade.type = .fade
                fade.duration = PageMotion.duration
                fade.timingFunction = CAMediaTimingFunction(name: .easeOut)
                pageLabel.layer?.add(fade, forKey: "page")
            }
            pageLabel.stringValue = pageText
            pageLabel.frame = NSRect(x: width - 65, y: pageY + 3, width: 40, height: 16)
            previous.frame = NSRect(x: width - 89, y: pageY, width: 22, height: 22)
            next.frame = NSRect(x: width - 25, y: pageY, width: 22, height: 22)
            previous.isEnabled = list.page > 0
            next.isEnabled = list.page + 1 < list.pageCount
            if layout == .vertical { y += footerHeight }
        }
        let size = NSSize(width: width, height: max(y + padding, listOpen ? steadySize.height : 0))
        if listOpen { steadySize = size }
        if frame.size != size { setFrameSize(size) }
        placePages(direction: direction)
        return size
    }
    func resetHover() { pageViews.forEach { $0.rows.forEach { $0.resetHover() } } }

    /// Forgets the list shown, when the window closes: the next list starts at its own size.
    func reset() {
        shown = nil
        steadySize = .zero
        stopMotion()
    }

    // MARK: Page motion

    /// The point a page sits at when moved `amount` along the list (down or right for positive amounts).
    private func origin(_ amount: CGFloat) -> NSPoint {
        layoutIsVertical ? NSPoint(x: 0, y: amount) : NSPoint(x: amount, y: 0)
    }

    private func stopMotion() {
        transitionGeneration += 1
        for page in pageViews {
            page.layer?.removeAllAnimations()
            page.setFrameOrigin(.zero)
            page.alphaValue = 1
        }
        pageViews[front].isHidden = false
        pageViews[1 - front].isHidden = true
        highlight.layer?.removeAllAnimations()
        highlight.setFrameOrigin(highlightOrigin)
    }

    /// After a page change (`direction`): the old page moves out against it and fades, the new one comes in from the
    /// other side.  The rows are already the new ones, so keys never wait; a further change cuts this one short.
    private func placePages(direction: Int) {
        let incoming = pageViews[front], outgoing = pageViews[1 - front]
        for page in pageViews { page.frame.size = rowClip.bounds.size }
        if let pinned = pinnedMotion {
            pinMotion(pinned, incoming: incoming, outgoing: outgoing)
            return
        }
        guard direction != 0, PanelMotion.animates, window?.isVisible == true else {
            if direction != 0 || outgoing.isHidden == false { stopMotion() }
            return
        }
        stopMotion()
        outgoing.isHidden = false
        let generation = transitionGeneration
        let travel = PanelMotion.reduceMotion ? 0 : PageMotion.offset * CGFloat(direction)
        incoming.setFrameOrigin(origin(travel))
        NSAnimationContext.runAnimationGroup { context in
            context.duration = PageMotion.duration
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            context.allowsImplicitAnimation = true
            incoming.animator().setFrameOrigin(.zero)
        }
        // The new rows stay clear until the old ones are nearly gone, then fade in.
        incoming.alphaValue = 1
        let fadeIn = CABasicAnimation(keyPath: "opacity")
        fadeIn.fromValue = 0
        fadeIn.toValue = 1
        fadeIn.beginTime = CACurrentMediaTime() + PageMotion.inDelay
        fadeIn.duration = PageMotion.duration - PageMotion.inDelay
        fadeIn.fillMode = .backwards
        fadeIn.timingFunction = CAMediaTimingFunction(name: .easeOut)
        incoming.layer?.add(fadeIn, forKey: "pageIn")
        NSAnimationContext.runAnimationGroup { context in
            context.duration = PageMotion.outDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            context.allowsImplicitAnimation = true
            outgoing.animator().setFrameOrigin(origin(-travel))
            outgoing.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            guard let self, self.transitionGeneration == generation else { return }
            outgoing.isHidden = true
            outgoing.setFrameOrigin(.zero)
            outgoing.alphaValue = 1
        }
    }

    /// Paging stopped at the first or last page: the page nudges a few points that way and settles back (a short dip
    /// in opacity under Reduce Motion).
    func bounce(direction: Int) {
        guard PanelMotion.animates, window?.isVisible == true, !rowClip.isHidden else { return }
        stopMotion()
        let page = pageViews[front]
        let generation = transitionGeneration
        let reduce = PanelMotion.reduceMotion
        NSAnimationContext.runAnimationGroup { context in
            context.duration = PageMotion.bounceOut
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            context.allowsImplicitAnimation = true
            if reduce {
                page.animator().alphaValue = 0.7
            } else {
                // The rows and the highlight nudge together.
                let nudge = origin(PageMotion.bounce * CGFloat(direction))
                page.animator().setFrameOrigin(nudge)
                highlight.animator().setFrameOrigin(NSPoint(x: highlightOrigin.x + nudge.x, y: highlightOrigin.y + nudge.y))
            }
        } completionHandler: { [weak self] in
            guard let self, self.transitionGeneration == generation else { return }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = PageMotion.bounceBack
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                context.allowsImplicitAnimation = true
                page.animator().setFrameOrigin(.zero)
                page.animator().alphaValue = 1
                self.highlight.animator().setFrameOrigin(self.highlightOrigin)
            }
        }
    }

    private func pinMotion(_ pinned: (kind: PinnedMotion, direction: Int, progress: CGFloat), incoming: RowPage, outgoing: RowPage) {
        // The same easing as the animation (ease-out), drawn still for review.
        let t = pinned.progress
        let eased = 1 - (1 - t) * (1 - t)
        incoming.isHidden = false
        switch pinned.kind {
        case .page:
            let travel = PageMotion.offset * CGFloat(pinned.direction)
            incoming.setFrameOrigin(origin(travel * (1 - eased)))
            let fadeIn = max(0, min(1, (t * PageMotion.duration - PageMotion.inDelay) / (PageMotion.duration - PageMotion.inDelay)))
            incoming.alphaValue = 1 - (1 - fadeIn) * (1 - fadeIn)
            let out = min(1, t * PageMotion.duration / PageMotion.outDuration)
            let outEased = 1 - (1 - out) * (1 - out)
            outgoing.isHidden = out >= 1
            outgoing.setFrameOrigin(origin(-travel * outEased))
            outgoing.alphaValue = 1 - outEased
        case .bounce:
            // Out over the first third, back over the rest.
            let amount = t < 1.0 / 3 ? t * 3 : max(0, 1 - (t - 1.0 / 3) * 1.5)
            incoming.setFrameOrigin(origin(PageMotion.bounce * CGFloat(pinned.direction) * amount))
            highlight.setFrameOrigin(NSPoint(x: highlight.frame.minX + incoming.frame.minX, y: highlight.frame.minY + incoming.frame.minY))
            incoming.alphaValue = 1
            outgoing.isHidden = true
        }
    }
}

/// Pure geometry shared by the panel, toast and offscreen checks.
enum InputPanelPlacement {
    /// The gap between the line and the window.
    static let gap: CGFloat = 4

    /// Below the caret's line (or above it, `above`), its text lined up with the composition; on the other side
    /// when the screen has no room, and always inside the visible frame.
    static func frame(size: NSSize, caret: NSRect, visible: NSRect, textInset: CGFloat,
                      previousX: CGFloat? = nil, above: Bool = false) -> NSRect {
        let x = min(max(previousX ?? caret.minX - textInset, visible.minX), max(visible.minX, visible.maxX - size.width))
        let below = caret.minY - gap - size.height
        let over = caret.maxY + gap
        var y = above ? over : below
        if above, over + size.height > visible.maxY, below >= visible.minY { y = below }
        if !above, below < visible.minY { y = over }
        y = min(max(y, visible.minY), max(visible.minY, visible.maxY - size.height))
        return NSRect(origin: NSPoint(x: x, y: y), size: size)
    }
    static func screen(at caret: NSRect) -> NSScreen? {
        NSScreen.screens.first { $0.frame.contains(NSPoint(x: caret.midX, y: caret.midY)) } ?? NSScreen.main
    }
}

/// Motion for the floating panels: the 動畫 setting, with Reduce Motion turning slides into fades.
enum PanelMotion {
    static var animates: Bool { Preferences.shared.panelAnimation }
    static var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    /// How long the candidate window takes to appear.
    static let appear: TimeInterval = 0.12
    /// How far it travels towards its place, without Reduce Motion.
    static let slide: CGFloat = 4

    /// Orders `panel` in at `frame`, fading (and without Reduce Motion, sliding a few points away from `caret`) when
    /// it was hidden and animation is on.
    static func present(_ panel: NSPanel, frame: NSRect, caret: NSRect) {
        let appearing = !panel.isVisible
        guard appearing, animates else {
            if panel.alphaValue < 1 {
                // Still fading in: finish at once at the new place, so the window follows every key.
                NSAnimationContext.runAnimationGroup { context in
                    context.duration = 0
                    panel.animator().alphaValue = 1
                    panel.animator().setFrame(frame, display: true)
                }
            } else {
                panel.setFrame(frame, display: true)
            }
            panel.orderFrontRegardless()
            return
        }
        var start = frame
        if !reduceMotion { start.origin.y += frame.midY < caret.midY ? slide : -slide }
        panel.setFrame(start, display: true)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = appear
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().alphaValue = 1
            if start != frame { panel.animator().setFrame(frame, display: true) }
        }
    }
}

final class CandidatePanelController {
    static let shared = CandidatePanelController()
    var onSelect: ((Int) -> Void)?
    var onPage: ((Int) -> Void)?
    var onZhuyin: (() -> Void)?
    var onCorrections: (() -> Void)?
    var onSingleCharacter: (() -> Void)?
    private let content = CandidateContentView(frame: .zero)
    private lazy var surface = InputSurface(content: content)
    private lazy var panel = InputPanel(content: surface)
    private var lastCaret: NSRect?
    private var lastScreen: NSRect?

    func show(list: CandidateList?, preview: String?, caret: NSRect, keys: [Character], windowLevel: NSWindow.Level,
              notice: String? = nil, zhuyinHint: String? = nil) {
        guard list?.items.isEmpty == false || !(preview ?? "").isEmpty || notice != nil else { hide(); return }
        let visible = InputPanelPlacement.screen(at: caret)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        content.onSelect = { [weak self] in self?.onSelect?($0) }
        content.onPage = { [weak self] in self?.onPage?($0) }
        content.onZhuyin = { [weak self] in self?.onZhuyin?() }
        content.onCorrections = { [weak self] in self?.onCorrections?() }
        content.onSingleCharacter = { [weak self] in self?.onSingleCharacter?() }
        let preferences = Preferences.shared
        panel.appearance = preferences.panelAppearance.appearance
        let size = content.update(list: list, preview: preview, keys: keys, style: preferences.candidateStyle,
                                  maxWidth: visible.width, notice: notice, zhuyinHint: zhuyinHint)
        let sameLine = panel.isVisible && lastScreen == visible && lastCaret.map { abs($0.minY - caret.minY) < 2 && abs($0.maxY - caret.maxY) < 2 } == true
        let frame = InputPanelPlacement.frame(size: size, caret: caret, visible: visible, textInset: content.candidateTextInset,
                                             previousX: sameLine ? panel.frame.minX : nil,
                                             above: preferences.candidatePlacement == .above)
        panel.level = windowLevel
        surface.fitContent()
        lastCaret = caret
        lastScreen = visible
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        PanelMotion.present(panel, frame: frame, caret: caret)
    }
    func hide() {
        panel.orderOut(nil)
        panel.alphaValue = 1
        lastCaret = nil
        content.resetHover()
        content.reset()
    }

    /// Paging stopped at an end of the list (`direction`: 1 past the last page, -1 before the first).
    func bounce(direction: Int) { content.bounce(direction: direction) }
    var isVisible: Bool { panel.isVisible }
}

final class ModeBadgeView: InputMaterialView {
    static let size = NSSize(width: 52, height: 48)
    let label = NSTextField(labelWithString: "中")
    override init(frame: NSRect) {
        super.init(frame: frame)
        label.font = .systemFont(ofSize: 23, weight: .medium)
        label.alignment = .center
        label.textColor = .labelColor
        addSubview(label)
        setFrameSize(Self.size)
        label.frame = NSRect(x: 5, y: 9, width: 42, height: 30)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}

extension InputSurface {
    /// A surface holding a 中/英 badge at its size.
    static func modeBadge() -> InputSurface {
        let badge = ModeBadgeView(frame: .zero)
        let surface = InputSurface(content: badge)
        badge.setFrameSize(ModeBadgeView.size)
        surface.fitContent()
        return surface
    }
}

final class ModeToast {
    static let shared = ModeToast()
    private let badge = ModeBadgeView(frame: .zero)
    private lazy var surface = InputSurface(content: badge)
    private lazy var panel: InputPanel = {
        let panel = InputPanel(content: surface)
        panel.ignoresMouseEvents = true
        return panel
    }()
    private var dismissal: DispatchWorkItem?
    private var generation = 0

    func hide() {
        dismissal?.cancel()
        dismissal = nil
        generation += 1
        panel.orderOut(nil)
        panel.alphaValue = 1
    }

    func show(_ text: String, caret: NSRect, windowLevel: NSWindow.Level) {
        dismissal?.cancel()
        generation += 1
        let current = generation
        badge.label.stringValue = text
        // The glass backdrop (macOS 26 and later) sizes its content to itself when it adopts it, which leaves the badge
        // at zero size: set the size every time, as the candidate window does.
        _ = panel
        badge.setFrameSize(ModeBadgeView.size)
        surface.fitContent()
        let visible = InputPanelPlacement.screen(at: caret)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        panel.level = windowLevel
        panel.appearance = Preferences.shared.panelAppearance.appearance
        panel.setFrame(InputPanelPlacement.frame(size: ModeBadgeView.size, caret: caret, visible: visible, textInset: 0,
                                                 above: Preferences.shared.candidatePlacement == .above), display: true)
        panel.alphaValue = 1
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        panel.orderFrontRegardless()
        // Shown for `Self.duration`, then faded out over `Self.fade` (or removed at once, with animation off).
        let animates = PanelMotion.animates
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.generation == current else { return }
            guard animates else { self.panel.orderOut(nil); return }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = Self.fade
                self.panel.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                guard let self, self.generation == current else { return }
                self.panel.orderOut(nil)
            }
        }
        dismissal = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.duration + (animates ? 0 : Self.fade), execute: work)
    }

    /// How long the badge stays, and how long it takes to fade (docs/product/DESIGN.md §4.2).
    static let duration: TimeInterval = 0.8
    static let fade: TimeInterval = 0.2
}
