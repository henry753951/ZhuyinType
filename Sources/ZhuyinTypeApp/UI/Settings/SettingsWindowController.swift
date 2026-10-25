// STUB — replaced by the Settings implementation.  The signature below is the contract; keep it.
import AppKit
import ZhuyinCore

final class SettingsWindowController {
    static let shared = SettingsWindowController()

    /// Set once the engine has loaded; the 詞彙 page reads and edits user phrases and learned entries through it.
    var engine: Engine?

    /// Open the window (or bring it to the front) and activate the app.
    func show() {}
}
