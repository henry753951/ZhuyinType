import AppKit
import InputMethodKit
import os

final class IMKTextClient: TextClient {
    let input: IMKTextInput
    private var lastCaret = NSRect.zero

    init(_ input: IMKTextInput) { self.input = input }
    var markedRange: NSRange { input.markedRange() }
    var selectedRange: NSRange { input.selectedRange() }
    var bundleIdentifier: String { input.bundleIdentifier() ?? "" }
    var windowLevel: NSWindow.Level {
        NSWindow.Level(rawValue: max(NSWindow.Level.popUpMenu.rawValue, Int(input.windowLevel()) + 1))
    }

    func insertText(_ text: String, replacementRange: NSRange) {
        input.insertText(text, replacementRange: replacementRange)
    }

    func setMarkedText(_ text: NSAttributedString, selectionRange: NSRange, replacementRange: NSRange) {
        input.setMarkedText(text, selectionRange: selectionRange, replacementRange: replacementRange)
    }

    /// Where the candidate window goes: a zero-width rectangle as tall as the line.  Sources, most reliable first:
    /// the attributes query at the composition start (answered by Cocoa and web-engine clients alike), the composed text's own
    /// rectangle, the selection, the last caret found for this client, and finally the mouse.  Rectangles returned
    /// through the client proxy can arrive garbled (subnormal numbers), so every one is checked before use.
    func caretRect(cursorIndex: Int) -> NSRect {
        let marked = markedRange
        var sources: [(String, NSRect)] = []
        // Index 0 is the start of the composition, the one index every client answers; the candidate window lines
        // up with it.  (Web-engine clients return nothing for an index at the end of the composition.)
        var line = NSRect.zero
        _ = input.attributes(forCharacterIndex: 0, lineHeightRectangle: &line)
        sources.append(("attributes", line.standardized))
        if marked.location != NSNotFound {
            let cursor = min(max(0, cursorIndex), marked.length)
            if cursor > 0 {
                let character = input.firstRect(forCharacterRange: NSRange(location: marked.location + cursor - 1, length: 1),
                                                actualRange: nil).standardized
                sources.append(("marked-before-caret", NSRect(x: character.maxX, y: character.minY, width: 0, height: character.height)))
            }
            let start = input.firstRect(forCharacterRange: NSRange(location: marked.location, length: 0), actualRange: nil)
            sources.append(("marked-start", start.standardized))
        }
        let selection = selectedRange
        if selection.location != NSNotFound {
            sources.append(("selection", input.firstRect(forCharacterRange: NSRange(location: selection.location, length: 0),
                                                          actualRange: nil).standardized))
        }
        let chosen = sources.first { valid($0.1) }
        Self.log.debug("caret \(self.bundleIdentifier, privacy: .public) cursor=\(cursorIndex) marked=\(marked.location),\(marked.length) chosen=\(chosen?.0 ?? "none", privacy: .public) \(sources.map { "\($0.0)=\(NSStringFromRect($0.1))" }.joined(separator: " "), privacy: .public)")
        if let chosen {
            lastCaret = chosen.1
            return chosen.1
        }
        if valid(lastCaret) { return lastCaret }
        let mouse = NSEvent.mouseLocation
        return NSRect(x: mouse.x, y: mouse.y - 10, width: 0, height: 20)
    }

    private static let log = Logger(subsystem: "org.zhuyintype", category: "caret")

    /// A caret is a zero-width rectangle, so only the height must be positive — and plausible: a line between 4 and
    /// 400 points tall, on a screen, not the zero rectangle clients return when they cannot answer.
    private func valid(_ rect: NSRect) -> Bool {
        guard rect.origin.x.isFinite, rect.origin.y.isFinite, rect.width.isFinite, rect.height.isFinite,
              rect.width >= 0, rect.width < 10_000, rect.height >= 4, rect.height < 400,
              abs(rect.origin.x) >= 0.001 || abs(rect.origin.y) >= 0.001 else { return false }
        return NSScreen.screens.contains { $0.frame.insetBy(dx: -1, dy: -1).contains(NSPoint(x: rect.minX, y: rect.midY)) }
    }
}

@objc(ZhuyinInputController)
final class ZhuyinInputController: IMKInputController {
    private static weak var activeController: ZhuyinInputController?
    private var core: ControllerCore?
    private var textClient: IMKTextClient?
    private var clientIdentity: ObjectIdentifier?
    private var observers: [NSObjectProtocol] = []
    private var active = false
    private var showModeToast = true
    private var loggedNotReady = false

    deinit { observers.forEach(NotificationCenter.default.removeObserver) }

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask.keyDown.rawValue | NSEvent.EventTypeMask.flagsChanged.rawValue)
    }

    override func activateServer(_ sender: Any!) {
        guard bind(sender) else { return }
        core?.endSession()
        core = nil
        Self.activeController = self
        active = true
        textClient?.input.overrideKeyboard(withKeyboardNamed: "com.apple.keylayout.ABC")
        installObservers()
        makeCoreIfReady()
        setPanelCallbacks()
        CandidatePanelController.shared.hide()
    }

    override func deactivateServer(_ sender: Any!) {
        core?.endSession()
        active = false
        if Self.activeController === self {
            CandidatePanelController.shared.hide()
            // TODO(UI): dismiss ModeToast here once its contract exposes hide(); it currently uses its own timer.
            Self.activeController = nil
        }
    }

    override func commitComposition(_ sender: Any!) {
        // The client asks for this when the user clicks into the text or it moves the insertion point.
        core?.commit()
        core?.breakContext()
        if Self.activeController === self { CandidatePanelController.shared.hide() }
    }

    private var lastKeyDown: (keyCode: UInt16, timestamp: TimeInterval, handled: Bool)?
    private static let keyLog = Logger(subsystem: "org.zhuyintype", category: "key")

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event, bind(sender) else { return false }
        makeCoreIfReady()
        guard let core else {
            if !loggedNotReady {
                EngineHost.logger.info("Engine not ready; passing session keys through")
                loggedNotReady = true
            }
            return false
        }
        switch event.type {
        case .keyDown:
            // Some web-engine clients hand the same key event to the input method twice; the copy carries the
            // original's timestamp, while a real repeat or a second press never does.
            if let last = lastKeyDown, last.keyCode == event.keyCode, last.timestamp == event.timestamp {
                Self.keyLog.debug("key \(event.keyCode) duplicate ignored")
                return last.handled
            }
            let handled = core.keyDown(keyCode: event.keyCode, modifiers: event.modifierFlags)
            lastKeyDown = (event.keyCode, event.timestamp, handled)
            Self.keyLog.debug("key \(event.keyCode) t=\(event.timestamp) repeat=\(event.isARepeat) handled=\(handled)")
            return handled
        case .flagsChanged:
            return core.flagsChanged(keyCode: event.keyCode, modifiers: event.modifierFlags,
                                     physicallyDown: CGEventSource.keyState(.combinedSessionState, key: event.keyCode),
                                     timestamp: event.timestamp)
        default:
            return false
        }
    }

    override func menu() -> NSMenu! {
        let menu = NSMenu()
        for title in ["設定…", "自訂詞與學到的詞…"] {
            let item = menu.addItem(withTitle: title, action: #selector(openSettings(_:)), keyEquivalent: "")
            item.target = self
        }
        menu.addItem(.separator())
        let about = menu.addItem(withTitle: "關於 ZhuyinType", action: #selector(openSettings(_:)), keyEquivalent: "")
        about.target = self
        return menu
    }

    @objc private func openSettings(_ sender: Any?) { SettingsWindowController.shared.show() }

    private func bind(_ sender: Any?) -> Bool {
        guard let input = (sender as? IMKTextInput) ?? client() else { return false }
        let identity = ObjectIdentifier(input as AnyObject)
        if identity != clientIdentity {
            core?.endSession() // Finish against the old client before replacing its adapter.
            core = nil
            textClient = IMKTextClient(input)
            clientIdentity = identity
        }
        return true
    }

    private func makeCoreIfReady() {
        guard core == nil, let engine = EngineHost.engine, let textClient else { return }
        let preferences = Preferences.shared
        let core = ControllerCore(engine: engine, client: textClient, options: preferences.composerOptions,
                                  isEnglish: EngineHost.lastEnglishMode ?? preferences.startInEnglish,
                                  reranker: EngineHost.reranker)
        self.core = core
        EngineHost.lastEnglishMode = core.isEnglish
        loggedNotReady = false
        core.showPanel = { [weak self] list, preview, caret, notice, zhuyinHint in
            guard let self, self.active, Self.activeController === self, let client = self.textClient else { return }
            CandidatePanelController.shared.show(list: list, preview: preview, caret: caret,
                                                keys: self.core?.composer.options.selectionKeys ?? [],
                                                windowLevel: client.windowLevel, notice: notice, zhuyinHint: zhuyinHint)
        }
        core.hidePanel = { [weak self] in
            if let self, self.active, Self.activeController === self { CandidatePanelController.shared.hide() }
        }
        core.modeChanged = { [weak self] english, caret in
            EngineHost.lastEnglishMode = english
            guard let self, self.active, Self.activeController === self,
                  self.showModeToast, let client = self.textClient else { return }
            ModeToast.shared.show(english ? "英" : "中", caret: caret, windowLevel: client.windowLevel)
        }
        refreshPreferences()
    }

    private func refreshPreferences() {
        let p = Preferences.shared
        showModeToast = p.showModeToast
        core?.update(options: p.composerOptions, shiftToggle: p.shiftToggle, popupCompositionApps: p.popupCompositionApps)
    }

    private func setPanelCallbacks() {
        CandidatePanelController.shared.onZhuyin = { [weak self] in
            guard let self, self.active, Self.activeController === self else { return }
            self.core?.selectZhuyin()
        }
        CandidatePanelController.shared.onSelect = { [weak self] index in
            guard let self, self.active, Self.activeController === self else { return }
            self.core?.selectCandidate(at: index)
        }
        CandidatePanelController.shared.onPage = { [weak self] direction in
            guard let self, self.active, Self.activeController === self else { return }
            _ = self.core?.handle(direction < 0 ? .pageUp : .pageDown)
        }
    }

    private func installObservers() {
        guard observers.isEmpty else { return }
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: Preferences.didChange, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.active else { return }
            self.refreshPreferences()
        })
        observers.append(center.addObserver(forName: EngineHost.didLoad, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.active else { return }
            self.makeCoreIfReady()
        })
    }
}
