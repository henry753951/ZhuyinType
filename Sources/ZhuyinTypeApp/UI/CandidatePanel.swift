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
    private var selected = false
    private var hovered = false
    private var tracking: NSTrackingArea?
    private var keyAttributes: [NSAttributedString.Key: Any] = [:]
    private var textAttributes: [NSAttributedString.Key: Any] = [:]
    private var annotationAttributes: [NSAttributedString.Key: Any] = [:]
    var action: (() -> Void)?
    var textInset: CGFloat { 8 + ceil(fontSize * 0.68) + 8 }
    var preferredWidth: CGFloat = 0

    func update(candidate: Candidate, key: String, fontSize: CGFloat, selected: Bool) {
        if self.candidate == candidate, self.key == key, self.fontSize == fontSize, self.selected == selected { return }
        self.candidate = candidate
        self.key = key
        self.fontSize = fontSize
        self.selected = selected
        refreshAttributes()
        let textWidth = (candidate.text as NSString).size(withAttributes: textAttributes).width
        let annotationWidth = candidate.annotation.map { ($0 as NSString).size(withAttributes: annotationAttributes).width + 10 } ?? 0
        preferredWidth = ceil(textInset + textWidth + annotationWidth + 10)
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
    override func draw(_ dirtyRect: NSRect) {
        if selected || hovered {
            (selected ? NSColor.controlAccentColor : NSColor.quaternaryLabelColor).setFill()
            NSBezierPath(roundedRect: bounds.insetBy(dx: 0, dy: 1), xRadius: 8, yRadius: 8).fill()
        }
        let textHeight = (candidate.text as NSString).size(withAttributes: textAttributes).height
        let keyHeight = (key as NSString).size(withAttributes: keyAttributes).height
        (key as NSString).draw(in: NSRect(x: 8, y: (bounds.height - keyHeight) / 2, width: textInset - 12, height: keyHeight), withAttributes: keyAttributes)
        let available = max(0, bounds.width - textInset - 10)
        let annotationWidth = candidate.annotation.map { ($0 as NSString).size(withAttributes: annotationAttributes).width } ?? 0
        let mainWidth = min((candidate.text as NSString).size(withAttributes: textAttributes).width, max(0, available - (annotationWidth > 0 ? annotationWidth + 10 : 0)))
        (candidate.text as NSString).draw(in: NSRect(x: textInset, y: (bounds.height - textHeight) / 2, width: mainWidth, height: textHeight), withAttributes: textAttributes)
        if let annotation = candidate.annotation {
            let height = (annotation as NSString).size(withAttributes: annotationAttributes).height
            (annotation as NSString).draw(in: NSRect(x: textInset + mainWidth + 10, y: (bounds.height - height) / 2,
                                                     width: max(0, available - mainWidth - 10), height: height), withAttributes: annotationAttributes)
        }
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
    private var keycapWidth: CGFloat { max(19, ceil((key as NSString).size(withAttributes: [.font: hintFont]).width) + 9) }
    var preferredWidth: CGFloat { keycapWidth + 9 + (hint as NSString).size(withAttributes: [.font: hintFont]).width }
    override func draw(_ dirtyRect: NSRect) {
        let keycap = NSRect(x: 0.5, y: (bounds.height - 19) / 2, width: keycapWidth, height: 19)
        let border = NSBezierPath(roundedRect: keycap, xRadius: 4, yRadius: 4)
        border.lineWidth = 1
        NSColor.quaternaryLabelColor.setStroke()
        border.stroke()
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

final class CandidateContentView: InputMaterialView {
    override var isFlipped: Bool { true }
    private var rows: [CandidateRow] = []
    private let previewLabel = NSTextField(labelWithString: "")
    private let noticeLabel = NSTextField(labelWithString: "")
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
        addSubview(previewLabel)
        noticeLabel.maximumNumberOfLines = 1
        noticeLabel.lineBreakMode = .byTruncatingTail
        noticeLabel.textColor = .secondaryLabelColor
        addSubview(noticeLabel)
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

    @discardableResult
    func update(list: CandidateList?, preview: String?, keys: [Character], layout: CandidateLayout,
                fontSize: CGFloat, maxWidth: CGFloat = 1200, notice: String? = nil, zhuyinHint: String? = nil) -> NSSize {
        let indices: [Int]
        if let list, list.items.indices.contains(list.selection) { indices = Array(list.pageRange) }
        else { indices = [] }
        while rows.count < indices.count {
            let row = CandidateRow(frame: .zero)
            rows.append(row)
            addSubview(row)
        }
        let rowHeight = ceil(fontSize * 1.55)
        let padding: CGFloat = 6
        let spacing: CGFloat = 10
        let hasPreview = !(preview ?? "").isEmpty
        let hasPages = (list?.pageCount ?? 1) > 1 && !indices.isEmpty
        // The reading being changed heads the list and never sets the width; the footer holds only fixed-width
        // controls, so nothing moves as the reading grows or the page count changes.
        let hasHint = !(zhuyinHint ?? "").isEmpty && !indices.isEmpty
        let hasToggle = list?.tolerant != nil && !indices.isEmpty
        let hasSingle = hasHint
        let hasFooter = hasPages || hasToggle || hasSingle
        let pageWidth: CGFloat = hasFooter ? 90 : 0
        zhuyinButton.hint = zhuyinHint ?? ""
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
        var width: CGFloat = 160
        var y: CGFloat = padding
        noticeLabel.isHidden = notice == nil
        noticeLabel.stringValue = notice ?? ""
        noticeLabel.font = .systemFont(ofSize: fontSize * 0.75)
        if notice != nil {
            width = max(width, noticeLabel.intrinsicContentSize.width + 28)
            noticeLabel.frame = NSRect(x: 14, y: y, width: width - 28, height: ceil(fontSize * 1.2))
            y += ceil(fontSize * 1.2) + 3
        }
        let previewY = y
        previewLabel.isHidden = !hasPreview
        if let preview, hasPreview {
            let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: fontSize),
                .foregroundColor: NSColor.labelColor, .underlineStyle: NSUnderlineStyle.single.rawValue]
            previewLabel.attributedStringValue = NSAttributedString(string: preview, attributes: attributes)
            width = max(width, previewLabel.intrinsicContentSize.width + 28)
            y += rowHeight + (indices.isEmpty ? 0 : 5)
        }
        let hintY = y
        if hasHint { y += 26 }
        for (offset, row) in rows.enumerated() {
            row.isHidden = offset >= indices.count
            guard offset < indices.count, let list else { row.action = nil; continue }
            let index = indices[offset]
            row.update(candidate: list.items[index], key: offset < keys.count ? String(keys[offset]) : "",
                       fontSize: fontSize, selected: index == list.selection)
            row.action = { [weak self] in self?.onSelect?(index) }
        }
        let activeRows = rows.prefix(indices.count)
        let gaps = CGFloat(max(0, indices.count - 1)) * spacing
        let naturalRowWidth = activeRows.reduce(0) { $0 + $1.preferredWidth }
        if layout == .vertical { width = max(width, (activeRows.map(\.preferredWidth).max() ?? 0) + padding * 2) }
        else { width = max(width, naturalRowWidth + gaps + padding * 2 + footerWidth) }
        width = max(width, footerWidth + padding * 2)
        width = min(maxWidth, ceil(width))
        noticeLabel.frame.size.width = max(0, width - 28)
        previewLabel.frame = NSRect(x: 14, y: previewY, width: max(0, width - 28), height: rowHeight)
        var x = padding
        let availableWidth = max(0, width - padding * 2 - gaps - (layout == .horizontal ? footerWidth : 0))
        let scale = min(1, availableWidth / max(1, naturalRowWidth))
        for row in activeRows {
            let rowWidth = layout == .vertical ? width - padding * 2 : floor(row.preferredWidth * scale)
            row.frame = NSRect(x: x, y: y, width: max(0, rowWidth), height: rowHeight)
            if layout == .vertical { y += rowHeight } else { x += rowWidth + spacing }
        }
        let pageY = layout == .horizontal ? y + (rowHeight - 22) / 2 : y + 3
        if layout == .horizontal && !indices.isEmpty { y += rowHeight }
        candidateTextInset = indices.isEmpty ? 14 : padding + (activeRows.first?.textInset ?? 29)
        pageLabel.isHidden = !hasFooter
        previous.isHidden = !hasFooter
        next.isHidden = !hasFooter
        zhuyinButton.isHidden = !hasHint
        correctionButton.isHidden = !hasToggle
        singleButton.isHidden = !hasSingle
        if hasHint {
            zhuyinButton.frame = NSRect(x: padding + 4, y: hintY + 2,
                                        width: min(ceil(zhuyinButton.preferredWidth), width - padding * 2 - 8), height: 22)
        }
        if hasFooter {
            let footerX = layout == .horizontal ? width - footerWidth : padding + 4
            singleButton.frame = NSRect(x: footerX, y: pageY, width: max(0, singleWidth - 8), height: 22)
            correctionButton.frame = NSRect(x: footerX + singleWidth, y: pageY, width: max(0, toggleWidth - 8), height: 22)
        }
        if hasFooter, let list {
            pageLabel.stringValue = "\(list.page + 1) / \(list.pageCount)"
            pageLabel.frame = NSRect(x: width - 65, y: pageY + 3, width: 40, height: 16)
            previous.frame = NSRect(x: width - 89, y: pageY, width: 22, height: 22)
            next.frame = NSRect(x: width - 25, y: pageY, width: 22, height: 22)
            previous.isEnabled = list.page > 0
            next.isEnabled = list.page + 1 < list.pageCount
            if layout == .vertical { y += 25 }
        }
        let size = NSSize(width: width, height: y + padding)
        if frame.size != size { setFrameSize(size) }
        return size
    }
    func resetHover() { rows.forEach { $0.resetHover() } }
}

/// Pure geometry shared by the panel, toast and offscreen checks.
enum InputPanelPlacement {
    static func frame(size: NSSize, caret: NSRect, visible: NSRect, textInset: CGFloat,
                      previousX: CGFloat? = nil) -> NSRect {
        let x = min(max(previousX ?? caret.minX - textInset, visible.minX), max(visible.minX, visible.maxX - size.width))
        var y = caret.minY - 4 - size.height
        if y < visible.minY { y = caret.maxY + 4 }
        y = min(max(y, visible.minY), max(visible.minY, visible.maxY - size.height))
        return NSRect(origin: NSPoint(x: x, y: y), size: size)
    }
    static func screen(at caret: NSRect) -> NSScreen? {
        NSScreen.screens.first { $0.frame.contains(NSPoint(x: caret.midX, y: caret.midY)) } ?? NSScreen.main
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
        content.keyLabels = Preferences.shared.candidateKeyLabels
        let preferences = Preferences.shared
        let size = content.update(list: list, preview: preview, keys: keys, layout: preferences.candidateLayout,
                                  fontSize: preferences.candidateFontSize, maxWidth: visible.width,
                                  notice: notice, zhuyinHint: zhuyinHint)
        let sameLine = panel.isVisible && lastScreen == visible && lastCaret.map { abs($0.minY - caret.minY) < 2 && abs($0.maxY - caret.maxY) < 2 } == true
        let frame = InputPanelPlacement.frame(size: size, caret: caret, visible: visible, textInset: content.candidateTextInset,
                                             previousX: sameLine ? panel.frame.minX : nil)
        panel.level = windowLevel
        surface.fitContent()
        panel.setFrame(frame, display: true)
        lastCaret = caret
        lastScreen = visible
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        panel.orderFrontRegardless()
    }
    func hide() {
        panel.orderOut(nil)
        lastCaret = nil
        content.resetHover()
    }
    var isVisible: Bool { panel.isVisible }
}

final class ModeBadgeView: InputMaterialView {
    let label = NSTextField(labelWithString: "中")
    override init(frame: NSRect) {
        super.init(frame: frame)
        label.font = .systemFont(ofSize: 23, weight: .medium)
        label.alignment = .center
        label.textColor = .labelColor
        addSubview(label)
        setFrameSize(NSSize(width: 52, height: 48))
        label.frame = NSRect(x: 5, y: 9, width: 42, height: 30)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}

final class ModeToast {
    static let shared = ModeToast()
    private let badge = ModeBadgeView(frame: .zero)
    private lazy var panel: InputPanel = {
        let panel = InputPanel(content: InputSurface(content: badge))
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
        let visible = InputPanelPlacement.screen(at: caret)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        panel.level = windowLevel
        panel.setFrame(InputPanelPlacement.frame(size: badge.frame.size, caret: caret, visible: visible, textInset: 0), display: true)
        panel.alphaValue = 1
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        panel.orderFrontRegardless()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.generation == current else { return }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.2
                self.panel.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                guard let self, self.generation == current else { return }
                self.panel.orderOut(nil)
            }
        }
        dismissal = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8, execute: work)
    }
}
