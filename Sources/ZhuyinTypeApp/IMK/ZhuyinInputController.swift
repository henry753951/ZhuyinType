import AppKit
import InputMethodKit

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

    func caretRect(cursorIndex: Int) -> NSRect {
        var rect = NSRect.zero
        _ = input.attributes(forCharacterIndex: max(0, cursorIndex), lineHeightRectangle: &rect)
        rect = rect.standardized
        if !valid(rect) { rect = input.firstRect(forCharacterRange: selectedRange, actualRange: nil).standardized }
        if valid(rect) { lastCaret = rect }
        return lastCaret
    }

    private func valid(_ rect: NSRect) -> Bool {
        guard rect.origin.x.isFinite, rect.origin.y.isFinite,
              rect.width.isFinite, rect.height.isFinite, rect.width > 0, rect.height > 0 else { return false }
        return NSScreen.screens.contains { $0.frame.intersects(rect) }
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
        core?.commit()
        if Self.activeController === self { CandidatePanelController.shared.hide() }
    }

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
            return core.keyDown(keyCode: event.keyCode, modifiers: event.modifierFlags)
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
                                  isEnglish: EngineHost.lastEnglishMode ?? preferences.startInEnglish)
        self.core = core
        EngineHost.lastEnglishMode = core.isEnglish
        loggedNotReady = false
        core.showPanel = { [weak self] list, preview, caret in
            guard let self, self.active, Self.activeController === self, let client = self.textClient else { return }
            CandidatePanelController.shared.show(list: list, preview: preview, caret: caret,
                                                keys: self.core?.composer.options.selectionKeys ?? [],
                                                windowLevel: client.windowLevel)
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
