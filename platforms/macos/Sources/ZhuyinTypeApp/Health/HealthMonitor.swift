import AppKit
import Combine

/// The checks' one owner: runs them off the main thread when the settings window becomes key (and after a button),
/// and keeps the latest for the 關於 row, its sidebar badge and the 檢查輸入法 page.
final class HealthMonitor: ObservableObject {
    static let shared = HealthMonitor()

    @Published private(set) var checks: [HealthCheck] = []
    @Published private(set) var checkedAt: Date?
    /// Facts used instead of the system's (UI previews, self-tests): nothing is read.
    var pinned: HealthFacts? {
        didSet { if let pinned { apply(pinned) } }
    }

    private var running = false
    private var again = false
    private var observer: NSObjectProtocol?

    private init() {
        observer = NotificationCenter.default.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil,
                                                          queue: .main) { [weak self] note in
            guard let window = note.object as? NSWindow, window === SettingsWindowController.shared.sheetWindow else { return }
            self?.refresh()
        }
    }

    deinit { observer.map(NotificationCenter.default.removeObserver) }

    var needsAttention: Bool { checks.contains { $0.status.needsAttention } }

    /// The 關於 page's row value.
    var summary: String { checks.isEmpty ? "正在檢查…" : HealthChecks.summary(checks) }

    /// Runs the checks once now, unless they have run before.
    func refreshIfNeeded() {
        if checkedAt == nil, !running { refresh() }
    }

    /// Runs the checks again; one more run follows a request made while running.
    func refresh() {
        precondition(Thread.isMainThread)
        if let pinned { apply(pinned); return }
        guard !running else { again = true; return }
        running = true
        let main = HealthFactsReader.readOnMain()
        DispatchQueue.global(qos: .userInitiated).async {
            let facts = HealthFactsReader.read(completing: main)
            DispatchQueue.main.async { [self] in
                running = false
                apply(facts)
                if again { again = false; refresh() }
            }
        }
    }

    /// 回報問題: fresh checks and our recent errors, as the report's text, delivered on the main thread.
    func makeReport(_ completion: @escaping (String) -> Void) {
        precondition(Thread.isMainThread)
        let pinned = self.pinned
        let main = pinned == nil ? HealthFactsReader.readOnMain() : nil
        DispatchQueue.global(qos: .userInitiated).async {
            let facts = pinned ?? HealthFactsReader.read(completing: main!)
            let errors = pinned == nil ? HealthFactsReader.recentErrors() : []
            let text = HealthReport.text(facts: facts, checks: HealthChecks.evaluate(facts), errors: errors)
            DispatchQueue.main.async {
                self.apply(facts)
                completion(text)
            }
        }
    }

    private func apply(_ facts: HealthFacts) {
        let checks = HealthChecks.evaluate(facts)
        if checks != self.checks { self.checks = checks }
        checkedAt = facts.now
    }
}

extension HealthFacts {
    /// Made-up facts for the UI previews and self-tests: an installed, enabled copy, everything in place, the update
    /// checked an hour before `now`.  Tests change one fact at a time.
    static func sample(now: Date = Date(timeIntervalSince1970: 1_795_500_000)) -> HealthFacts {
        let home = "/Users/sample"
        let app = URL(fileURLWithPath: home + "/Library/Input Methods/ZhuyinType.app")
        return HealthFacts(
            home: home, inputSource: .registered(enabled: true, selectable: true), running: app,
            installLocations: [app, URL(fileURLWithPath: "/Library/Input Methods/ZhuyinType.app")], copies: [app],
            secureInput: .off, perDocumentInputSource: false,
            lexicon: .readable(app.appendingPathComponent("Contents/Resources/lexicon.bin")), missingModels: [],
            settings: .readable, version: "0.6.1", build: "123",
            updates: .available(automaticChecks: true, lastCheck: now.addingTimeInterval(-3600), pending: nil),
            systemVersion: "26.1", systemBuild: "25B78", architecture: "arm64", now: now)
    }
}
