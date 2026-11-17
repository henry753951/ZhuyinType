import AppKit
import Carbon
import OSLog

/// Reads `HealthFacts` from the system: every side effect of 檢查輸入法 is here, and every one only reads.  Nothing
/// here selects an input source, restarts a process or writes a setting.
enum HealthFactsReader {
    /// The part read on the main thread: Text Input Sources, secure input and Sparkle are main-thread APIs.  Fast.
    struct MainFacts {
        var inputSource: HealthFacts.InputSource
        var secureInput: HealthFacts.SecureInput
        var updates: HealthFacts.Updates
    }

    static func readOnMain() -> MainFacts {
        precondition(Thread.isMainThread)
        let mode = InstallCommand.inputModeState()
        var secure = HealthFacts.SecureInput.off
        if IsSecureEventInputEnabled() { secure = .on(owner: secureInputOwner()) }
        let updater = Updater.shared
        let updates: HealthFacts.Updates = updater.isAvailable
            ? .available(automaticChecks: updater.automaticallyChecks, lastCheck: updater.lastCheck, pending: updater.pendingVersion)
            : .unavailable
        return MainFacts(inputSource: mode.map { .registered(enabled: $0.enabled, selectable: $0.selectable) } ?? .notRegistered,
                         secureInput: secure, updates: updates)
    }

    /// The rest, from files and LaunchServices; off the main thread (a few milliseconds).
    static func read(completing main: MainFacts) -> HealthFacts {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let data = EngineHost.dataFiles()
        let info = Bundle.main.infoDictionary
        let os = ProcessInfo.processInfo.operatingSystemVersion
        return HealthFacts(
            home: home.path,
            inputSource: main.inputSource,
            running: Bundle.main.bundleURL,
            installLocations: [home.appendingPathComponent("Library/Input Methods/ZhuyinType.app"),
                               URL(fileURLWithPath: "/Library/Input Methods/ZhuyinType.app")],
            copies: NSWorkspace.shared.urlsForApplications(withBundleIdentifier: bundleID),
            secureInput: main.secureInput,
            perDocumentInputSource: perDocumentInputSource(),
            lexicon: lexicon(in: data.root),
            missingModels: missingModels(data),
            settings: settingsFile(home: home),
            version: info?["CFBundleShortVersionString"] as? String,
            build: info?["CFBundleVersion"] as? String,
            updates: main.updates,
            systemVersion: "\(os.majorVersion).\(os.minorVersion)" + (os.patchVersion > 0 ? ".\(os.patchVersion)" : ""),
            systemBuild: sysctl("kern.osversion") ?? "?",
            architecture: machine(),
            now: Date())
    }

    private static var bundleID: String { Bundle.main.bundleIdentifier ?? "org.zhuyintype.inputmethod.ZhuyinType" }

    /// The app holding secure input, by its localized name, when the window server says which process it is.
    private static func secureInputOwner() -> String? {
        guard let session = CGSessionCopyCurrentDictionary() as? [String: Any],
              let pid = (session["kCGSSessionSecureInputPID"] as? NSNumber)?.int32Value, pid > 0 else { return nil }
        return NSRunningApplication(processIdentifier: pid)?.localizedName
    }

    /// 鍵盤 › 輸入方式 › 「自動切換成文件的輸入方式」.
    private static func perDocumentInputSource() -> Bool {
        let value = CFPreferencesCopyAppValue("AppleGlobalTextInputProperties" as CFString, "com.apple.HIToolbox" as CFString)
        let properties = value as? [String: Any]
        return (properties?["TextInputGlobalPropertyPerContextInput"] as? NSNumber)?.boolValue ?? false
    }

    /// The lexicon the engine would open (the compiled one first), and whether its first bytes can be read.
    private static func lexicon(in root: URL?) -> HealthFacts.DataFile {
        guard let root else { return .missing }
        let manager = FileManager.default
        guard let file = ["lexicon.bin", "lexicon.tsv"].map(root.appendingPathComponent)
            .first(where: { manager.fileExists(atPath: $0.path) }) else { return .missing }
        guard let handle = try? FileHandle(forReadingFrom: file) else { return .unreadable(file) }
        defer { try? handle.close() }
        let bytes = try? handle.read(upToCount: 16)
        return bytes?.isEmpty == false ? .readable(file) : .unreadable(file)
    }

    private static func missingModels(_ data: (root: URL?, chat: Bool, channel: Bool, reranker: Bool, characters: Bool))
        -> [HealthFacts.Model]
    {
        guard let root = data.root else { return [] }
        let manager = FileManager.default
        let lm = ["lm.bin", "bigram.tsv"].contains { manager.fileExists(atPath: root.appendingPathComponent("lm/\($0)").path) }
        let present: [HealthFacts.Model: Bool] = [.languageModel: lm, .chatModel: data.chat, .readingChannel: data.channel,
                                                   .reranker: data.reranker, .characterModel: data.characters]
        return HealthFacts.Model.allCases.filter { present[$0] != true }
    }

    /// The app's settings file: none yet, a property list, or something that is not one.
    private static func settingsFile(home: URL) -> HealthFacts.SettingsFile {
        let file = home.appendingPathComponent("Library/Preferences/\(bundleID).plist")
        guard FileManager.default.fileExists(atPath: file.path) else { return .none }
        guard let data = try? Data(contentsOf: file),
              (try? PropertyListSerialization.propertyList(from: data, format: nil)) is [String: Any] else {
            return .unreadable(file)
        }
        return .readable
    }

    private static func sysctl(_ name: String) -> String? {
        var size = 0
        guard sysctlbyname(name, nil, &size, nil, 0) == 0, size > 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: size)
        guard sysctlbyname(name, &buffer, &size, nil, 0) == 0 else { return nil }
        return String(cString: buffer)
    }

    private static func machine() -> String {
        var name = utsname()
        uname(&name)
        return withUnsafeBytes(of: &name.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
    }

    // MARK: The report's recent errors

    /// Our own error and fault messages from this process in the last hour (the last 20), or nil when the log cannot
    /// be read within `timeout`.  Blocks the calling thread up to `timeout`: call it off the main thread.
    static func recentErrors(timeout: TimeInterval = 0.3) -> [HealthReport.LogLine]? {
        precondition(!Thread.isMainThread)
        final class Box: @unchecked Sendable {
            let lock = NSLock()
            var lines: [HealthReport.LogLine]?
        }
        let box = Box()
        let done = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .utility).async {
            let lines = try? readErrors(since: Date().addingTimeInterval(-3600))
            box.lock.withLock { box.lines = lines }
            done.signal()
        }
        guard done.wait(timeout: .now() + timeout) == .success else { return nil }
        return box.lock.withLock { box.lines }
    }

    private static func readErrors(since start: Date) throws -> [HealthReport.LogLine] {
        let store = try OSLogStore(scope: .currentProcessIdentifier)
        let entries = try store.getEntries(at: store.position(date: start),
                                           matching: NSPredicate(format: "subsystem == %@", "org.zhuyintype"))
        var lines: [HealthReport.LogLine] = []
        for case let entry as OSLogEntryLog in entries where entry.level == .error || entry.level == .fault {
            lines.append(HealthReport.LogLine(date: entry.date, message: entry.composedMessage))
        }
        return Array(lines.suffix(20))
    }
}

/// The checks' buttons.
enum HealthActions {
    static let keyboardSettings = URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension")!

    static func perform(_ action: HealthAction) {
        switch action {
        case .enableInputSource: InstallCommand.enableInputSource()
        case .openKeyboardSettings: NSWorkspace.shared.open(keyboardSettings)
        case .reveal(let url):
            if FileManager.default.fileExists(atPath: url.path) { NSWorkspace.shared.activateFileViewerSelecting([url]) }
            else { NSWorkspace.shared.open(url.deletingLastPathComponent()) }
        case .checkForUpdates: Updater.shared.checkNow()
        }
        HealthMonitor.shared.refresh()
    }
}
