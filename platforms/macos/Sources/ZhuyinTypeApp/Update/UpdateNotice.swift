// 更新後提示更新內容: after an update, one small panel in the screen's corner with what this version changed
// (docs/product/DESIGN.md §3.6).  It waits for the first typing after the update to pause, and it never takes the focus:
// a non-activating panel that cannot become key.  Not while composing, not under secure input (a password field), not
// over the settings or the wizard, and not while something is full screen.
import AppKit
import Carbon
import SwiftUI

final class UpdateNotice {
    static let shared = UpdateNotice()

    /// How long the typing must have paused, and how long the panel stays when nothing happens.
    static let pause: TimeInterval = 4
    static let duration: TimeInterval = 20

    private var pending: ReleaseNotes.Announcement?
    private var timer: Timer?
    private var dismissal: Timer?
    private var panel: InputPanel?

    private var progress: ReleaseNotesProgress { ReleaseNotesProgress(defaults: UserDefaults.standard) }

    /// Decides whether this launch follows an update.  Call once when the input method starts, before anything else
    /// makes its folder or stores settings.
    func start() {
        guard !Headless.isActive, let current = ReleaseNotes.currentVersion else { return }
        let usedBefore = WizardProgress.seenVersion != nil
            || FileManager.default.fileExists(atPath: EngineHost.userDirectory.path)
        switch progress.start(current: current, usedBefore: usedBefore, wanted: Preferences.shared.showsUpdateNotes) {
        case .nothing: break
        case .remember: progress.remember(current)
        case .announce(let previous):
            pending = ReleaseNotes.announcement(ReleaseNotes.between(ReleaseNotes.all, previous: previous, current: current),
                                                current: current)
        }
    }

    /// A key was typed: look again once the typing has paused.
    func typed() {
        guard pending != nil else { return }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: Self.pause, repeats: true) { [weak self] _ in self?.showIfQuiet() }
    }

    /// This version's notes need no notice any more (「更新內容」 was opened).
    func seen() {
        guard pending != nil, let version = pending?.version else { return }
        pending = nil
        timer?.invalidate()
        timer = nil
        progress.remember(version)
    }

    private func showIfQuiet() {
        guard let announcement = pending else { return }
        guard Preferences.shared.showsUpdateNotes else { return seen() }
        guard !EngineHost.isComposing(), !CandidatePanelController.shared.isVisible, !IsSecureEventInputEnabled(),
              EngineHost.currentInputSourceIsOurs(), SettingsWindowController.shared.visibleWindow == nil,
              !WizardWindowController.shared.isOpen, let screen = NSScreen.main, !Self.fullScreen(on: screen) else { return }
        seen()
        show(announcement, on: screen)
    }

    /// Something covers `screen`: the app in front asked for a full-screen presentation (or hid the menu bar or the
    /// Dock, as games and presentations do), or one of its windows is as large as the screen.
    static func fullScreen(on screen: NSScreen) -> Bool {
        let options = NSApplication.shared.currentSystemPresentationOptions
        if !options.isDisjoint(with: [.fullScreen, .hideMenuBar, .hideDock]) { return true }
        guard let front = NSWorkspace.shared.frontmostApplication?.processIdentifier,
              let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
                as? [[String: Any]] else { return false }
        // Bounds and owners need no screen-recording permission.
        return windows.contains { window in
            guard window[kCGWindowOwnerPID as String] as? pid_t == front,
                  let bounds = window[kCGWindowBounds as String] as? [String: CGFloat],
                  let width = bounds["Width"], let height = bounds["Height"] else { return false }
            return width >= screen.frame.width && height >= screen.frame.height
        }
    }

    private func show(_ announcement: ReleaseNotes.Announcement, on screen: NSScreen) {
        let view = UpdateNoticeView(announcement: announcement) { [weak self] action in self?.perform(action) }
            hover: { [weak self] inside in inside ? self?.dismissal?.invalidate() : self?.scheduleDismissal() }
        let hosting = FirstMouseHostingView(rootView: view)
        hosting.frame.size = hosting.fittingSize
        let surface = InputSurface(content: hosting)
        let panel = InputPanel(content: surface)
        panel.level = .statusBar
        // Not a full-screen app's: it stays on the desktops.
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenNone]
        panel.appearance = Preferences.shared.panelAppearance.appearance
        let visible = screen.visibleFrame
        panel.setFrame(NSRect(x: visible.maxX - hosting.frame.width - 14, y: visible.maxY - hosting.frame.height - 14,
                              width: hosting.frame.width, height: hosting.frame.height), display: true)
        self.panel = panel
        if NSApp.isHidden { NSApp.unhideWithoutActivation() }
        if PanelMotion.animates {
            panel.alphaValue = 0
            panel.orderFrontRegardless()
            panel.animator().alphaValue = 1
        } else {
            panel.orderFrontRegardless()
        }
        ScreenReader.announce(([announcement.title] + announcement.lines).joined(separator: "，"))
        scheduleDismissal()
    }

    private func scheduleDismissal() {
        dismissal?.invalidate()
        dismissal = Timer.scheduledTimer(withTimeInterval: Self.duration, repeats: false) { [weak self] _ in self?.hide() }
    }

    private func hide() {
        dismissal?.invalidate()
        dismissal = nil
        panel?.orderOut(nil)
        panel = nil
    }

    private func perform(_ action: UpdateNoticeView.Action) {
        hide()
        switch action {
        case .close: break
        case .notes: SettingsWindowController.shared.show(SettingsPlace(.about, subpage: .releaseNotes))
        case .feedback: SettingsWindowController.shared.show(Feedback.place)
        }
    }
}

/// The panel never becomes key, so a click must work at once.
final class FirstMouseHostingView<Content: View>: NSHostingView<Content> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

struct UpdateNoticeView: View {
    enum Action { case notes, feedback, close }

    let announcement: ReleaseNotes.Announcement
    let perform: (Action) -> Void
    var hover: (Bool) -> Void = { _ in }

    static let width: CGFloat = 340

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 36, height: 36)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline) {
                    Text(announcement.title).font(.headline)
                    Spacer(minLength: 8)
                    Button { perform(.close) } label: {
                        Image(systemName: "xmark").font(.system(size: 10, weight: .semibold)).foregroundStyle(.secondary)
                            .frame(width: 18, height: 18)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("關閉")
                }
                ForEach(Array(announcement.lines.enumerated()), id: \.offset) { _, line in
                    Text(line)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                HStack(spacing: 8) {
                    Button("查看更新內容") { perform(.notes) }
                    Button("回報問題") { perform(.feedback) }
                }
                .controlSize(.small)
                .padding(.top, 4)
            }
        }
        .padding(14)
        .frame(width: Self.width, alignment: .leading)
        .onHover(perform: hover)
    }
}
