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
        window.setContentSize(NSSize(width: 760, height: 600))
        window.title = mode == .keysOnly ? "你會用到的按鍵" : "設定嚮導"
        if case .newSince = mode { window.title = "新功能導覽" }
        self.window = window
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.center()
        window.makeKeyAndOrderFront(nil)
    }

    /// The first opening of the settings after install also opens the wizard, once.
    func showOnFirstUse() {
        guard WizardProgress.neverSeen, !SettingsWindowController.isPreview else { return }
        WizardProgress.markSeen()
        DispatchQueue.main.async { self.show(.quick) }
    }

    func close() {
        window?.orderOut(nil)
        window?.contentViewController = nil
        model = nil
        NSApp.setActivationPolicy(.accessory)
    }

    private func makeWindow() -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 600),
                              styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        window.delegate = self
        return window
    }

    func windowWillClose(_ notification: Notification) {
        model = nil
        NSApp.setActivationPolicy(.accessory)
    }
}
