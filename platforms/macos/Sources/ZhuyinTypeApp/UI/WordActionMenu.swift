// The right-click menu on a candidate: 不要再推薦, 忘記這個選字, 加入自訂詞, each with its shortcut, dimmed when it
// does not apply.  The candidate window never becomes key, so the menu pops up over it without activating the app.
import AppKit
import ZhuyinEngine

enum WordActionMenu {
    private final class Target: NSObject {
        let run: (CandidateAction) -> Void
        init(run: @escaping (CandidateAction) -> Void) { self.run = run }
        @objc func choose(_ item: NSMenuItem) {
            if let action = CandidateAction(rawValue: UInt32(item.tag)) { run(action) }
        }
    }
    /// Kept while the menu is open; the menu holds its target weakly.
    private static var target: Target?

    static func show(for core: ControllerCore, index: Int, event: NSEvent, view: NSView) {
        let available = Set(core.availableWordActions(at: index))
        let target = Target { [weak core] action in core?.wordAction(action, at: index) }
        let menu = NSMenu()
        menu.autoenablesItems = false
        for action in CandidateAction.allCases {
            let item = NSMenuItem(title: action.title, action: #selector(Target.choose(_:)), keyEquivalent: "")
            item.tag = Int(action.rawValue)
            item.target = target
            item.isEnabled = available.contains(action)
            if let key = core.shortcuts.first(where: { $0.value.candidateAction == action })?.key {
                item.toolTip = key.spoken
            }
            menu.addItem(item)
        }
        Self.target = target
        NSMenu.popUpContextMenu(menu, with: event, for: view)
        Self.target = nil
    }
}
