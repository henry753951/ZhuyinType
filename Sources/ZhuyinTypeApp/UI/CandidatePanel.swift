// STUB — replaced by the UI implementation.  The signatures below are the contract; keep them.
import AppKit
import ZhuyinCore

/// The candidate window: a borderless, non-activating panel next to the caret.
final class CandidatePanelController {
    static let shared = CandidatePanelController()

    /// A click on candidate `index` (into `CandidateList.items`).
    var onSelect: ((Int) -> Void)?

    /// Show or update the window.
    /// - Parameters:
    ///   - list: the candidates; nil shows only `preview` (the per-app popup composition fallback).
    ///   - preview: the composition text, shown above the candidates for apps in `popupCompositionApps`; usually nil.
    ///   - caret: the caret rectangle in screen coordinates (the window goes below it, or above when there is no room).
    ///   - keys: the selection-key labels for the rows of the current page.
    ///   - windowLevel: at least the client window's level + 1.
    func show(list: CandidateList?, preview: String?, caret: NSRect, keys: [Character], windowLevel: NSWindow.Level) {}

    func hide() {}

    var isVisible: Bool { false }
}

/// The 中／英 badge shown briefly at the caret when the mode changes.
final class ModeToast {
    static let shared = ModeToast()
    func show(_ text: String, caret: NSRect, windowLevel: NSWindow.Level) {}
}
