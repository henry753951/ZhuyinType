import AppKit
import SwiftUI

/// The setup wizard's window.  It opens when the user asks (介紹, the input menu, `--setup-wizard`) and once on the
/// first opening of the settings after install; it never opens by itself while the user is typing.
final class WizardWindowController: NSObject, NSWindowDelegate {
    static let shared = WizardWindowController()
    private var window: NSWindow?
    private(set) var model: WizardModel?

    func show(_ mode: WizardModel.Mode = .quick) {
        let model = WizardModel(mode: mode)
        model.onClose = { [weak self] in self?.close() }
        self.model = model
        let window = self.window ?? makeWindow()
        window.contentViewController = NSHostingController(rootView: WizardView(model: model))
        window.setContentSize(WizardLayout.defaultSize)
        window.title = mode == .keysOnly ? "你會用到的按鍵" : "設定嚮導"
        if case .newSince = mode { window.title = "新功能導覽" }
        self.window = window
        window.center()
        // Headless runs (self-tests) show it without taking the keyboard.
        guard !Headless.isActive else { window.orderFrontRegardless(); return }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    var isOpen: Bool { window?.isVisible == true }

    /// The first opening of the settings after install also opens the wizard, once.
    func showOnFirstUse() {
        guard WizardProgress.neverSeen, !SettingsWindowController.isPreview else { return }
        WizardProgress.markSeen()
        DispatchQueue.main.async { self.show(.quick) }
    }

    /// Closes the wizard only: the settings window, if it is open, stays where it was and comes back to the front.
    func close() {
        window?.orderOut(nil)
        window?.contentViewController = nil
        model = nil
        returnToSettings()
    }

    private func returnToSettings() {
        if let settings = SettingsWindowController.shared.visibleWindow {
            if !Headless.isActive { settings.makeKeyAndOrderFront(nil) }
        } else if !Headless.isActive {
            NSApp.setActivationPolicy(.accessory)
        }
    }

    private func makeWindow() -> NSWindow {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: WizardLayout.defaultSize),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        // Laid out to fit its smallest without scrolling; never larger than a MacBook screen comfortably shows.
        window.contentMinSize = WizardLayout.minSize
        window.contentMaxSize = WizardLayout.maxSize
        window.isReleasedWhenClosed = false
        window.delegate = self
        return window
    }

    func windowWillClose(_ notification: Notification) {
        model = nil
        DispatchQueue.main.async { self.returnToSettings() }
    }
}
