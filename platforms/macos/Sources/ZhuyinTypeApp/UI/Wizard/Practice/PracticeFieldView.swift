import AppKit
import ZhuyinEngine

/// The practice field in AppKit: a one-line text box drawn like a text field (sent text, the composition with the
/// underlines the core sets, the caret), and under it, inside the wizard window, the input method's own candidate
/// window content (`CandidateContentView` on an `InputSurface`) and 中/英 badge, placed by `InputPanelPlacement` as
/// the floating panels are.  Room for a full page of the candidate window is kept below the box (above it when the
/// window opens above the caret), so the wizard's layout does not move while typing.
final class PracticeFieldView: NSView {
    let session: PracticeSession
    let textView: PracticeTextView
    let candidates = CandidateContentView(frame: .zero)
    private(set) lazy var surface = InputSurface(content: candidates)
    /// Holds the window: a window wider than the field (a long horizontal list) is drawn smaller, as a whole, as the
    /// settings preview does, instead of squeezing its rows.
    let panelHost = PanelHost()
    private let badge = InputSurface.modeBadge()
    /// The type size of the field's text.
    static let fontSize: CGFloat = 18
    static let boxHeight: CGFloat = 46
    /// Reduce Motion, read through a closure so the self-test can turn it on.
    var reduceMotion: () -> Bool = { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    private var reserve: CGFloat = 0
    private var badgeWork: DispatchWorkItem?
    private var badgeGeneration = 0
    private var windowObservers: [NSObjectProtocol] = []
    private var monitor: Any?
    /// A key event the monitor gave back to the window: the text view passes it on instead of handling it again.
    fileprivate weak var passedThrough: NSEvent?

    override var isFlipped: Bool { false }

    init(session: PracticeSession) {
        self.session = session
        textView = PracticeTextView(session: session)
        super.init(frame: NSRect(x: 0, y: 0, width: 480, height: 200))
        textView.field = self
        addSubview(textView)
        panelHost.isHidden = true
        panelHost.addSubview(surface)
        addSubview(panelHost)
        badge.isHidden = true
        addSubview(badge)
        candidates.onSelect = { [weak session] in session?.selectCandidate(at: $0) }
        candidates.onPage = { [weak session] in session?.page($0) }
        candidates.onZhuyin = { [weak session] in session?.selectZhuyin() }
        candidates.onCorrections = { [weak session] in session?.toggleCorrections() }
        candidates.onSingleCharacter = { [weak session] in session?.toggleSingleCharacter() }
        session.client.compositionRect = { [weak self] in self?.compositionRect() ?? .zero }
        session.textChanged = { [weak self] in self?.textView.textDidChange() }
        session.panelChanged = { [weak self] in self?.show($0) }
        session.listEdge = { [weak self] in self?.candidates.bounce(direction: $0) }
        session.modeChanged = { [weak self] in self?.showBadge(english: $0) }
        setAccessibilityElement(false)
        settingsChanged()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    deinit {
        windowObservers.forEach(NotificationCenter.default.removeObserver)
        if let monitor { NSEvent.removeMonitor(monitor) }
    }

    private var preferences: Preferences { session.preferences }
    private var opensAbove: Bool { preferences.candidatePlacement == .above }

    /// The draft settings changed: the room kept for the window, the box's place and the window's look.
    func settingsChanged() {
        reserve = Self.reservedHeight(preferences)
        candidates.animatesOverride = preferences.panelAnimation
        invalidateIntrinsicContentSize()
        needsLayout = true
        show(session.panel)
    }

    /// The height of the window with a full page of candidates, the reading and the footer, at these settings.
    static func reservedHeight(_ preferences: Preferences) -> CGFloat {
        let keys = preferences.pageSelectionKeys
        let items = (0..<(keys.count * 2)).map { Candidate(text: $0 == 0 ? "候選字詞" : "字") }
        let list = CandidateList(items: items, selection: 0, pageSize: keys.count, tolerant: false)
        let size = PanelMeasure.measure(list: list, keys: keys, style: preferences.candidateStyle, notice: nil,
                                        zhuyinHint: "ㄏㄡˋ ㄒㄩㄢˇ").size
        return max(size.height, ModeBadgeView.size.height) + InputPanelPlacement.gap
    }

    override var intrinsicContentSize: NSSize {
        // The window starts its entrance `PanelMotion.slide` further from the caret: room for that too.
        NSSize(width: NSView.noIntrinsicMetric, height: Self.boxHeight + reserve - textView.lineInset + PanelMotion.slide)
    }

    override func layout() {
        super.layout()
        let y = opensAbove ? 0 : bounds.height - Self.boxHeight
        textView.frame = NSRect(x: 0, y: y, width: bounds.width, height: Self.boxHeight)
        show(session.panel)
    }

    // MARK: Candidate window

    /// The line of the composition's start, in this view: the rectangle the input method's text client reports.
    func compositionRect() -> NSRect {
        let line = textView.compositionLine()
        return convert(line, from: textView)
    }

    /// Shows the candidate window as the core asked (nil hides it), placed as `CandidatePanelController` places the
    /// floating one, inside this view instead of on the screen.
    func show(_ panel: PracticePanel?) {
        guard let panel, panel.list?.items.isEmpty == false || !(panel.preview ?? "").isEmpty || panel.notice != nil else {
            panelHost.isHidden = true
            candidates.resetHover()
            candidates.reset()
            textView.panelChanged(nil)
            return
        }
        let appearing = panelHost.isHidden
        surface.appearance = preferences.panelAppearance.appearance
        let size = candidates.update(list: panel.list, preview: panel.preview, keys: panel.keys, style: preferences.candidateStyle,
                                     maxWidth: 4000, notice: panel.notice, zhuyinHint: panel.zhuyinHint)
        surface.fitContent()
        surface.setFrameOrigin(.zero)
        let scale = min(1, max(1, bounds.width) / max(1, size.width))
        let shown = NSSize(width: floor(size.width * scale), height: floor(size.height * scale))
        let caret = compositionRect()
        let frame = InputPanelPlacement.frame(size: shown, caret: caret, visible: bounds,
                                             textInset: candidates.candidateTextInset * scale,
                                             previousX: appearing ? nil : panelHost.frame.minX, above: opensAbove)
        panelHost.setFrameSize(shown)
        panelHost.setBoundsSize(size)
        panelHost.isHidden = false
        present(panelHost, origin: frame.origin, caret: caret, appearing: appearing)
        textView.panelChanged(panel)
    }

    /// The window as placed (for the self-tests): its frame in the field, and the scale it is drawn at.
    var panelFrame: NSRect { panelHost.frame }
    var panelShown: Bool { !panelHost.isHidden }
    var panelScale: CGFloat { panelHost.bounds.width > 0 ? panelHost.frame.width / panelHost.bounds.width : 1 }

    /// The window's entrance, as `PanelMotion.present`: with 動畫 on it fades in over 0.12 s and moves 4 pt towards
    /// the caret; under Reduce Motion it only fades.
    private func present(_ view: NSView, origin: NSPoint, caret: NSRect, appearing: Bool) {
        view.layer?.removeAllAnimations()
        guard appearing, preferences.panelAnimation else {
            view.setFrameOrigin(origin)
            view.alphaValue = 1
            return
        }
        let frame = NSRect(origin: origin, size: view.frame.size)
        view.setFrameOrigin(Self.entranceFrame(frame, caret: caret, reduceMotion: reduceMotion()).origin)
        view.alphaValue = 0
        NSAnimationContext.runAnimationGroup { context in
            context.duration = PanelMotion.appear
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            view.animator().alphaValue = 1
            view.animator().setFrameOrigin(origin)
        }
    }

    /// Where the window starts its entrance: `slide` points further from the caret, or in place under Reduce Motion.
    static func entranceFrame(_ frame: NSRect, caret: NSRect, reduceMotion: Bool) -> NSRect {
        guard !reduceMotion else { return frame }
        var start = frame
        start.origin.y += frame.midY < caret.midY ? -PanelMotion.slide : PanelMotion.slide
        return start
    }

    // MARK: 中/英 badge

    /// The badge beside the caret for 0.8 s, then a 0.2 s fade (gone after 1 s with 動畫 off), as `ModeToast`.
    func showBadge(english: Bool) {
        guard preferences.showModeToast else { return }
        badgeWork?.cancel()
        badgeGeneration += 1
        let generation = badgeGeneration
        (badge.content as? ModeBadgeView)?.label.stringValue = english ? "英" : "中"
        (badge.content as? ModeBadgeView)?.setFrameSize(ModeBadgeView.size)
        badge.fitContent()
        badge.appearance = preferences.panelAppearance.appearance
        badge.frame = InputPanelPlacement.frame(size: ModeBadgeView.size, caret: compositionRect(), visible: bounds,
                                                textInset: 0, above: opensAbove)
        badge.layer?.removeAllAnimations()
        badge.alphaValue = 1
        badge.isHidden = false
        badge.setAccessibilityLabel(english ? "英文" : "中文")
        let animates = preferences.panelAnimation
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.badgeGeneration == generation else { return }
            guard animates else { self.badge.isHidden = true; return }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = ModeToast.fade
                self.badge.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                guard let self, self.badgeGeneration == generation else { return }
                self.badge.isHidden = true
            }
        }
        badgeWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + ModeToast.duration + (animates ? 0 : ModeToast.fade), execute: work)
        textView.announce(english ? "英文" : "中文")
    }

    /// The badge is showing (for the self-tests and previews).
    var badgeText: String? { badge.isHidden ? nil : (badge.content as? ModeBadgeView)?.label.stringValue }
    var badgeFrame: NSRect { badge.frame }

    /// Keeps the badge on screen (UI previews): no fade.
    func pinBadge() { badgeWork?.cancel(); badgeWork = nil }

    // MARK: Focus and keys

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        windowObservers.forEach(NotificationCenter.default.removeObserver)
        windowObservers = []
        updateMonitor()
        guard let window else { return }
        let center = NotificationCenter.default
        windowObservers.append(center.addObserver(forName: NSWindow.didBecomeKeyNotification, object: window, queue: .main) {
            [weak self] _ in
            self?.focusIfNothingElse()
            self?.updateMonitor()
            self?.textView.focusChanged()
        })
        windowObservers.append(center.addObserver(forName: NSWindow.didResignKeyNotification, object: window, queue: .main) {
            [weak self] _ in
            // Like switching apps: the composition is sent and the window closes.
            self?.session.endSession()
            self?.updateMonitor()
            self?.textView.focusChanged()
        })
        focusIfNothingElse()
    }

    /// In a window the user is typing in, the field takes the keyboard when nothing else has it.  Windows that are not
    /// key (the headless renders and self-tests) are never touched.
    private func focusIfNothingElse() {
        guard let window, window.isKeyWindow, window.firstResponder === window || window.firstResponder == nil else { return }
        window.makeFirstResponder(textView)
    }

    /// While the field has the keyboard in the key window, every key goes to it first, before key equivalents: Return
    /// and Esc reach the input method instead of the wizard's default and cancel buttons, as in an app's text field.
    func updateMonitor() {
        let wanted = window?.isKeyWindow == true && window?.firstResponder === textView
        if wanted, monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { [weak self] event in
                guard let self, event.window === self.window, self.window?.firstResponder === self.textView else { return event }
                if event.type == .flagsChanged {
                    self.textView.handleFlags(event)
                    self.passedThrough = event
                    return event
                }
                if self.textView.handleKey(event) { return nil }
                self.passedThrough = event
                return event
            }
        } else if !wanted, let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    fileprivate func wasPassedThrough(_ event: NSEvent) -> Bool {
        guard let passed = passedThrough, passed === event else { return false }
        passedThrough = nil
        return true
    }
}

/// The text box: draws the sent text, the composition and the caret, takes the keyboard, and is the element VoiceOver
/// reads.
final class PracticeTextView: NSView {
    let session: PracticeSession
    fileprivate weak var field: PracticeFieldView?
    /// Draw as focused without being first responder (UI previews).
    var pinnedFocus = false
    /// The task is done: a green outline.
    var isDone = false { didSet { if isDone != oldValue { updateBorder(animated: true) } } }
    var placeholder = "在這裡試打" { didSet { needsDisplay = true } }
    private var scrollOffset: CGFloat = 0
    private var caretVisible = true
    private var blink: Timer?
    private static let padding: CGFloat = 14
    private var font: NSFont { .systemFont(ofSize: PracticeFieldView.fontSize) }
    private var lineHeight: CGFloat { ceil(font.ascender - font.descender + font.leading) + 2 }
    /// From the box's bottom edge to the bottom of the line.
    var lineInset: CGFloat { (PracticeFieldView.boxHeight - lineHeight) / 2 }

    init(session: PracticeSession) {
        self.session = session
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 9
        layer?.cornerCurve = .continuous
        setAccessibilityElement(true)
        setAccessibilityRole(.textField)
        updateBorder(animated: false)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { true }
    private var focused: Bool { pinnedFocus || (window?.firstResponder === self && window?.isKeyWindow == true) }

    // The window makes this the first responder only after these return: the monitor and the look follow then.
    override func becomeFirstResponder() -> Bool {
        DispatchQueue.main.async { [weak self] in self?.field?.updateMonitor(); self?.focusChanged() }
        return true
    }

    override func resignFirstResponder() -> Bool {
        session.endSession()
        DispatchQueue.main.async { [weak self] in self?.field?.updateMonitor(); self?.focusChanged() }
        return true
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        if field?.wasPassedThrough(event) == true || !handleKey(event) { super.keyDown(with: event) }
    }

    override func flagsChanged(with event: NSEvent) {
        if field?.wasPassedThrough(event) != true { handleFlags(event) }
        super.flagsChanged(with: event)
    }

    /// A key for the field: the input method's core first, then the field itself.  False: neither used it.
    @discardableResult
    func handleKey(_ event: NSEvent) -> Bool {
        restartBlink()
        return session.keyDown(keyCode: event.keyCode, modifiers: event.modifierFlags, characters: event.characters,
                               time: event.timestamp)
    }

    /// Shift and Caps Lock.  Whether the Shift key itself is down comes from the device bits of the event (left 0x2,
    /// right 0x4), which the window server sets for real presses.
    func handleFlags(_ event: NSEvent) {
        let raw = event.modifierFlags.rawValue
        let down: Bool
        switch event.keyCode {
        case 56: down = raw & 0x02 != 0
        case 60: down = raw & 0x04 != 0
        default: down = event.modifierFlags.contains(.capsLock)
        }
        session.flagsChanged(keyCode: event.keyCode, modifiers: event.modifierFlags, physicallyDown: down, time: event.timestamp)
    }

    // MARK: Drawing

    /// The sent text and the composition as one string, with the composition's own attributes.
    private func content() -> NSAttributedString {
        let base: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.labelColor]
        let result = NSMutableAttributedString(string: session.client.text, attributes: base)
        let marked = NSMutableAttributedString(attributedString: session.client.marked)
        marked.addAttributes([.font: font], range: NSRange(location: 0, length: marked.length))
        marked.enumerateAttribute(.foregroundColor, in: NSRange(location: 0, length: marked.length)) { value, range, _ in
            if value == nil { marked.addAttribute(.foregroundColor, value: NSColor.labelColor, range: range) }
        }
        result.append(marked)
        return result
    }

    /// The caret's offset in the string: the core's cursor inside the composition, or the end.
    private var caretIndex: Int {
        let sent = session.client.text.utf16.count
        guard session.client.marked.length > 0 else { return sent }
        return sent + min(max(0, session.presentation.cursor), session.client.marked.length)
    }

    private func width(_ string: NSAttributedString, upTo index: Int) -> CGFloat {
        guard index > 0 else { return 0 }
        return ceil(string.attributedSubstring(from: NSRange(location: 0, length: min(index, string.length))).size().width)
    }

    private var lineTop: CGFloat { (bounds.height - lineHeight) / 2 }

    /// The line at the composition's start, in this (flipped) view.
    func compositionLine() -> NSRect {
        let x = Self.padding - scrollOffset + width(content(), upTo: session.client.text.utf16.count)
        return NSRect(x: x, y: lineTop, width: 0, height: lineHeight)
    }

    func textDidChange() {
        let string = content()
        let caret = width(string, upTo: caretIndex)
        let room = bounds.width - Self.padding * 2
        if caret - scrollOffset > room { scrollOffset = caret - room }
        if caret < scrollOffset { scrollOffset = max(0, caret - room / 3) }
        if string.size().width <= room { scrollOffset = 0 }
        restartBlink()
        needsDisplay = true
        setAccessibilityValue(session.client.text + session.client.marked.string)
        setAccessibilityNumberOfCharacters(string.length)
        setAccessibilitySelectedTextRange(NSRange(location: caretIndex, length: 0))
        NSAccessibility.post(element: self, notification: .valueChanged)
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.textBackgroundColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 9, yRadius: 9).fill()
        let clip = bounds.insetBy(dx: Self.padding - 2, dy: 2)
        NSGraphicsContext.saveGraphicsState()
        NSBezierPath(rect: clip).addClip()
        let string = content()
        let origin = NSPoint(x: Self.padding - scrollOffset, y: lineTop + 1)
        if string.length == 0, !placeholder.isEmpty {
            NSAttributedString(string: placeholder, attributes: [.font: font, .foregroundColor: NSColor.placeholderTextColor])
                .draw(at: NSPoint(x: Self.padding + 3, y: origin.y))
        }
        string.draw(at: origin)
        if focused, caretVisible {
            let x = origin.x + width(string, upTo: caretIndex)
            NSColor.textInsertionPointColor.setFill()
            NSRect(x: x, y: lineTop + 1, width: 1.5, height: lineHeight - 2).fill()
        }
        NSGraphicsContext.restoreGraphicsState()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateBorder(animated: false)
    }

    func focusChanged() {
        updateBorder(animated: false)
        restartBlink()
        needsDisplay = true
    }

    /// Accent while typing, green once done (0.2 s), a hairline otherwise.
    private func updateBorder(animated: Bool) {
        guard let layer else { return }
        var color = NSColor.separatorColor.cgColor
        let width: CGFloat = isDone ? 2 : focused ? 1.5 : 1
        effectiveAppearance.performAsCurrentDrawingAppearance {
            color = (isDone ? NSColor.systemGreen : focused ? NSColor.controlAccentColor.withAlphaComponent(0.6)
                     : NSColor.separatorColor).cgColor
        }
        if animated {
            for (key, from, to) in [("borderColor", layer.borderColor as Any, color as Any), ("borderWidth", layer.borderWidth as Any, width as Any)] {
                let change = CABasicAnimation(keyPath: key)
                change.fromValue = from
                change.toValue = to
                change.duration = 0.2
                change.timingFunction = CAMediaTimingFunction(name: .easeOut)
                layer.add(change, forKey: key)
            }
        }
        layer.borderColor = color
        layer.borderWidth = width
    }

    /// The caret blinks while the field has the keyboard in the key window; it is steady otherwise.
    private func restartBlink() {
        blink?.invalidate()
        blink = nil
        caretVisible = true
        needsDisplay = true
        guard window?.isKeyWindow == true, window?.firstResponder === self else { return }
        blink = Timer.scheduledTimer(withTimeInterval: 0.53, repeats: true) { [weak self] _ in
            guard let self else { return }
            self.caretVisible.toggle()
            self.needsDisplay = true
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil { blink?.invalidate(); blink = nil }
        focusChanged()
    }

    // MARK: VoiceOver

    /// The candidate window changed: VoiceOver says the highlighted candidate, or the notice.
    func panelChanged(_ panel: PracticePanel?) {
        guard let text = Self.announcement(for: panel), text != lastAnnouncement else {
            if panel == nil { lastAnnouncement = nil }
            return
        }
        lastAnnouncement = text
        announce(text)
    }
    private var lastAnnouncement: String?

    /// What VoiceOver says for the candidate window: 「曉明，第 1 個，共 14 個」 or the notice.
    static func announcement(for panel: PracticePanel?) -> String? {
        guard let panel else { return nil }
        if let notice = panel.notice { return notice }
        guard let list = panel.list, list.items.indices.contains(list.selection) else { return nil }
        var text = "\(list.items[list.selection].text)，第 \(list.selection + 1) 個，共 \(list.items.count) 個"
        if let annotation = list.items[list.selection].annotation { text += "，\(annotation)" }
        return text
    }

    func announce(_ text: String) {
        guard NSWorkspace.shared.isVoiceOverEnabled else { return }
        NSAccessibility.post(element: self, notification: .announcementRequested,
                             userInfo: [.announcement: text, .priority: NSAccessibilityPriorityLevel.high.rawValue])
    }
}

/// The candidate window's holder in the field: its bounds are the window's own size, its frame what fits the field.
final class PanelHost: NSView {
    override var isFlipped: Bool { false }
    override init(frame: NSRect) {
        super.init(frame: frame)
        autoresizesSubviews = false
    }
    convenience init() { self.init(frame: .zero) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}
