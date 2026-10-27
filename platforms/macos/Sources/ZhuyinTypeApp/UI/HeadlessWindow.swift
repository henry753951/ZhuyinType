import AppKit

/// The headless modes (--render-ui, --self-test, --status and the install commands) run while the user works in other
/// apps: they must never take the keyboard.  The app is `.prohibited` (it cannot become active), and every window
/// these modes make is one of these: it can never become key or main, takes no mouse events, and sits off-screen and
/// transparent when it has to be ordered in for drawing.
enum Headless {
    static let flags: Set<String> = ["--render-ui", "--render-ui-preview", "--self-test", "--install", "--register",
                                     "--enable", "--select", "--disable", "--unregister", "--status"]

    /// This process runs a headless mode: it must not touch the user's settings either.
    private(set) static var isActive = false

    /// Makes this process unable to become active, before any window or view exists.
    static func begin() {
        isActive = true
        NSApplication.shared.setActivationPolicy(.prohibited)
    }

    /// Watches for this process becoming active or one of its windows becoming key, which the headless modes must
    /// never cause.
    final class FocusGuard {
        private(set) var violations: [String] = []
        private var observers: [NSObjectProtocol] = []
        private let frontmost = NSWorkspace.shared.frontmostApplication

        init() {
            let center = NotificationCenter.default
            observers.append(center.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil,
                                                queue: .main) { [weak self] _ in self?.violations.append("ZhuyinType became active") })
            observers.append(center.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil, queue: .main) {
                [weak self] note in
                self?.violations.append("a window became key: \((note.object as? NSWindow).map { String(describing: type(of: $0)) } ?? "?")")
            })
        }

        /// Problems seen so far, and whether ZhuyinType is (or became) the frontmost app.
        func check() -> [String] {
            var problems = violations
            if NSApp.isActive { problems.append("ZhuyinType is active") }
            if let key = NSApp.keyWindow { problems.append("ZhuyinType has a key window: \(type(of: key))") }
            let now = NSWorkspace.shared.frontmostApplication
            if now?.processIdentifier == ProcessInfo.processInfo.processIdentifier,
               frontmost?.processIdentifier != now?.processIdentifier {
                problems.append("ZhuyinType became the frontmost app")
            }
            return problems
        }

        deinit { observers.forEach(NotificationCenter.default.removeObserver) }
    }
}

/// A window for drawing offscreen: borderless, never key or main, no mouse, transparent and far off-screen.
final class HeadlessWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }

    init(size: NSSize) {
        super.init(contentRect: NSRect(origin: NSPoint(x: -30000, y: -30000), size: size), styleMask: [.borderless],
                   backing: .buffered, defer: false)
        isReleasedWhenClosed = false
        ignoresMouseEvents = true
        alphaValue = 0
        hidesOnDeactivate = false
    }
}

/// The settings window as the UI previews build it: titled like the real one, but it can never become key or main.
final class HeadlessTitledWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
