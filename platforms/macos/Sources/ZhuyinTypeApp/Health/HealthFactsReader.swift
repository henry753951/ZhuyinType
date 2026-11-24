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
        var currentInputSource: String?
        var enabledInputSources: [String]
        var loadTimes: HealthFacts.LoadTimes?
        var display: HealthFacts.Display
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
        let current = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue()
        let durations = EngineHost.loadDurations
        let workspace = NSWorkspace.shared
        let appearance = NSApp?.effectiveAppearance ?? NSAppearance.currentDrawing()
        let display = HealthFacts.Display(
            scale: NSScreen.main.map { Double($0.backingScaleFactor) },
            dark: appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua,
            panelAppearance: Preferences.shared.panelAppearance, voiceOver: workspace.isVoiceOverEnabled,
            reduceMotion: workspace.accessibilityDisplayShouldReduceMotion,
            increaseContrast: workspace.accessibilityDisplayShouldIncreaseContrast,
            reduceTransparency: workspace.accessibilityDisplayShouldReduceTransparency)
        return MainFacts(inputSource: mode.map { .registered(enabled: $0.enabled, selectable: $0.selectable) } ?? .notRegistered,
                         secureInput: secure, updates: updates, currentInputSource: current.flatMap(sourceID),
                         enabledInputSources: enabledKeyboardSources(),
                         loadTimes: durations.engine == nil ? nil
                             : HealthFacts.LoadTimes(engine: durations.engine, reranker: durations.reranker,
                                                     characters: durations.characters),
                         display: display)
    }

    /// The rest, from files and LaunchServices; off the main thread (a few milliseconds).
    static func read(completing main: MainFacts) -> HealthFacts {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let data = EngineHost.dataFiles()
        let info = Bundle.main.infoDictionary
        let os = ProcessInfo.processInfo.operatingSystemVersion
        let preferences = home.appendingPathComponent("Library/Preferences/\(bundleID).plist")
        var facts = HealthFacts(
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
            settings: settingsFile(preferences),
            version: info?["CFBundleShortVersionString"] as? String,
            build: info?["CFBundleVersion"] as? String,
            updates: main.updates,
            systemVersion: "\(os.majorVersion).\(os.minorVersion)" + (os.patchVersion > 0 ? ".\(os.patchVersion)" : ""),
            systemBuild: sysctl("kern.osversion") ?? "?",
            architecture: machine(),
            now: Date())
        facts.user = NSUserName()
        facts.inputModeID = bundleID + ".Zhuyin"
        facts.currentInputSource = main.currentInputSource
        facts.enabledInputSources = main.enabledInputSources
        facts.dataFolder = data.root
        facts.dataItems = dataItems(root: data.root)
        facts.bundledData = bundledData()
        facts.loadTimes = main.loadTimes
        facts.app = appBundle(Bundle.main.bundleURL)
        facts.preferencesFile = preferences
        facts.userData = EngineHost.userDirectory
        facts.started = processStart()
        facts.changedSettings = HealthSettings.changes(UserDefaults.standard.persistentDomain(forName: bundleID) ?? [:])
        facts.display = main.display
        return facts
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
    private static func settingsFile(_ file: URL) -> HealthFacts.SettingsFile {
        guard FileManager.default.fileExists(atPath: file.path) else { return .none }
        guard let data = try? Data(contentsOf: file),
              (try? PropertyListSerialization.propertyList(from: data, format: nil)) is [String: Any] else {
            return .unreadable(file)
        }
        return .readable
    }

    static func sysctl(_ name: String) -> String? {
        var size = 0
        guard sysctlbyname(name, nil, &size, nil, 0) == 0, size > 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: size)
        guard sysctlbyname(name, &buffer, &size, nil, 0) == 0 else { return nil }
        return String(cString: buffer)
    }

    static func machine() -> String {
        var name = utsname()
        uname(&name)
        return withUnsafeBytes(of: &name.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
    }

    // MARK: The report's sections

    private static func sourceID(_ source: TISInputSource) -> String? { property(source, kTISPropertyInputSourceID) }

    private static func property(_ source: TISInputSource, _ key: CFString) -> String? {
        guard let value = TISGetInputSourceProperty(source, key) else { return nil }
        return Unmanaged<CFString>.fromOpaque(value).takeUnretainedValue() as String
    }

    /// The enabled keyboard input sources and input methods, by ID (palettes and dictation left out).
    private static func enabledKeyboardSources() -> [String] {
        guard let list = TISCreateInputSourceList(nil, false)?.takeRetainedValue() as? [TISInputSource] else { return [] }
        return list.filter { property($0, kTISPropertyInputSourceCategory) == (kTISCategoryKeyboardInputSource as String) }
            .compactMap(sourceID)
    }

    /// Every file and folder the engine would load, where it would load it from (EngineHost.sources), with its size.
    private static func dataItems(root: URL?) -> [HealthFacts.DataItem] {
        let manager = FileManager.default
        func first(_ candidates: [URL?]) -> URL? {
            candidates.compactMap { $0 }.first { manager.fileExists(atPath: $0.path) }
        }
        func folder(_ name: String, marker: String) -> URL? {
            [EngineHost.modelsDirectory, Bundle.main.resourceURL].compactMap { $0 }
                .map { $0.appendingPathComponent(name, isDirectory: true) }
                .first { manager.fileExists(atPath: $0.appendingPathComponent(marker).path) }
        }
        let items: [(String, URL?)] = [
            ("詞庫", first(["lexicon.bin", "lexicon.tsv"].map { root?.appendingPathComponent($0) })),
            (HealthFacts.Model.languageModel.name, first(["lm/lm.bin", "lm/bigram.tsv"].map { root?.appendingPathComponent($0) })),
            (HealthFacts.Model.chatModel.name,
             first(["lm_chat/lm.bin", "lm_chat/bigram.tsv"].map { root?.appendingPathComponent($0) })),
            (HealthFacts.Model.readingChannel.name, first([root?.appendingPathComponent("channel.bin")])),
            (HealthFacts.Model.reranker.name, folder("reranker", marker: "vocab.json")),
            (HealthFacts.Model.characterModel.name, folder("v3", marker: "chars.json")),
        ]
        return items.map { name, url in HealthFacts.DataItem(name: name, url: url, bytes: url.flatMap(size)) }
    }

    /// A file's size, or the total of the files in a folder.
    private static func size(_ url: URL) -> Int64? {
        let keys: Set<URLResourceKey> = [.fileSizeKey, .isDirectoryKey]
        guard let values = try? url.resourceValues(forKeys: keys) else { return nil }
        guard values.isDirectory == true else { return values.fileSize.map(Int64.init) }
        let files = FileManager.default.enumerator(at: url, includingPropertiesForKeys: Array(keys))
        var total: Int64 = 0
        while let file = files?.nextObject() as? URL {
            total += Int64((try? file.resourceValues(forKeys: [.fileSizeKey]))?.fileSize ?? 0)
        }
        return total
    }

    /// The data versions build-app.sh wrote into manifest.json from resources/assets.json.
    private static func bundledData() -> [HealthFacts.BundledData] {
        guard let url = Bundle.main.resourceURL?.appendingPathComponent("manifest.json"),
              let data = try? Data(contentsOf: url),
              let manifest = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let assets = manifest["assets"] as? [String: [String: Any]] else { return [] }
        return assets.keys.sorted().compactMap { component in
            guard let release = assets[component]?["release"] as? String,
                  let hash = assets[component]?["sha256"] as? String else { return nil }
            return HealthFacts.BundledData(component: component, release: release, hash: String(hash.prefix(12)))
        }
    }

    /// The bundle's owner, its permissions as `ls` writes them, and whether it carries the quarantine attribute.
    private static func appBundle(_ url: URL) -> HealthFacts.AppBundle {
        let attributes = try? FileManager.default.attributesOfItem(atPath: url.path)
        var permissions: String?
        if let mode = (attributes?[.posixPermissions] as? NSNumber)?.intValue {
            let directory = attributes?[.type] as? FileAttributeType == .typeDirectory
            permissions = (directory ? "d" : "-") + (0..<9).map { bit in
                mode & (1 << (8 - bit)) != 0 ? String(Array("rwx")[bit % 3]) : "-"
            }.joined()
        }
        let quarantined = getxattr(url.path, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW) >= 0
        return HealthFacts.AppBundle(owner: attributes?[.ownerAccountName] as? String, permissions: permissions,
                                     quarantined: attributes == nil ? nil : quarantined)
    }

    /// When this process started, from the kernel.
    private static func processStart() -> Date? {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var name: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard Darwin.sysctl(&name, u_int(name.count), &info, &size, nil, 0) == 0, size > 0 else { return nil }
        let start = info.kp_proc.p_un.__p_starttime
        return Date(timeIntervalSince1970: TimeInterval(start.tv_sec) + TimeInterval(start.tv_usec) / 1_000_000)
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
