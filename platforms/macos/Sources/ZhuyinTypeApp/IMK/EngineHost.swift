import Carbon
import Foundation
import os
import ZhuyinEngine

/// Loads the engine and models, and keeps them current: models and data placed in the models folder are loaded in the
/// background and swapped in between compositions; a new version of the app itself is picked up by restarting the
/// process once nothing is being composed (macOS starts the input method again on the next key).
enum EngineHost {
    static let connectionName = "org.zhuyintype.inputmethod.ZhuyinType_Connection"
    static let didLoad = Notification.Name("ZhuyinTypeEngineDidLoad")
    static let logger = Logger(subsystem: "org.zhuyintype", category: "IMK")
    // Published and read only on the main thread, including the mode shared by this input-method process.
    static private(set) var engine: Engine?
    /// Chooses the displayed conversion among the engine's best; nil when the app ships no model.
    static private(set) var reranker: NeuralReranker?
    /// The character model scoring each syllable's characters in the lattice; nil when the app ships none.
    static private(set) var characterModel: SyllableScorer?
    /// Increases with every load; a controller whose core was built from an older one rebuilds it when idle.
    static private(set) var generation = 0
    static var lastEnglishMode: Bool?
    /// Asks every controller whether it is in the middle of a composition.
    static var isComposing: () -> Bool = { false }

    static let userDirectory = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Application Support/ZhuyinType", isDirectory: true)
    /// Data and models placed here replace the bundled ones: `lexicon.tsv` with `lm/` and `lm_chat/` as one set, and
    /// `reranker/` and `v3/` each on their own.
    static let modelsDirectory = userDirectory.appendingPathComponent("Models", isDirectory: true)

    private static var loading = false
    private static var reloadAgain = false
    private static var loadedFingerprint = ""
    private static var monitor: UpdateMonitor?
    private static var restartPending = false
    private static var idleTimer: Timer?
    private static let launchedVersion = Bundle.main.infoDictionary.map(versionString) ?? ""
    private static let launchedExecutable = executableStamp()
    private static let modeKey = "ZhuyinTypeModeAcrossRestart"

    static var isLoading: Bool { loading }

    /// How long the last load took, in seconds, for the 分析 page: the lexicon and language models, the reranker and
    /// the character model.  Nil until loaded (or when a model is not shipped).
    static private(set) var loadDurations: (engine: TimeInterval?, reranker: TimeInterval?, characters: TimeInterval?) = (nil, nil, nil)

    static func load() {
        // Static lets initialise on first use: record what was launched now, before any update can replace it.
        _ = (launchedVersion, launchedExecutable)
        restoreModeAfterRestart()
        observeInputSourceChanges()
        reload()
        monitor = UpdateMonitor(paths: [Bundle.main.bundleURL.deletingLastPathComponent().path, modelsDirectory.path]) {
            checkForUpdates()
        }
    }

    // MARK: - Loading

    private struct Sources {
        var lexicon: URL
        var chat: URL?
        var reranker: URL?
        var characters: URL?
    }

    private static func sources() -> Sources? {
        let bundled = Bundle.main.resourceURL
        let manager = FileManager.default
        func exists(_ url: URL?, _ file: String) -> URL? {
            guard let url, manager.fileExists(atPath: url.appendingPathComponent(file).path) else { return nil }
            return url
        }
        // lexicon.tsv or the compiled lexicon.bin beside it; the engine prefers the compiled one.
        let dataRoot = exists(modelsDirectory, "lexicon.bin") ?? exists(modelsDirectory, "lexicon.tsv")
            ?? exists(bundled, "lexicon.bin") ?? exists(bundled, "lexicon.tsv")
        guard let dataRoot else { return nil }
        return Sources(
            lexicon: dataRoot.appendingPathComponent("lexicon.tsv"),
            chat: exists(dataRoot.appendingPathComponent("lm_chat"), "bigram.tsv")
                ?? exists(dataRoot.appendingPathComponent("lm_chat"), "lm.bin"),
            reranker: exists(modelsDirectory.appendingPathComponent("reranker"), "vocab.json")
                ?? exists(bundled?.appendingPathComponent("reranker"), "vocab.json"),
            characters: exists(modelsDirectory.appendingPathComponent("v3"), "chars.json")
                ?? exists(bundled?.appendingPathComponent("v3"), "chars.json"))
    }

    /// Loads everything in the background and swaps it in on the main thread.  A request while loading runs once more
    /// afterwards.
    static func reload() {
        guard !loading else { reloadAgain = true; return }
        guard let sources = sources() else {
            logger.error("Engine load failed: missing lexicon")
            return
        }
        loading = true
        let fingerprint = currentFingerprint()
        // The new engine reads the personal data the old one may still be about to write.
        engine?.flushPersonalData()
        DispatchQueue.global(qos: .userInitiated).async {
            let start = Date()
            do {
                try FileManager.default.createDirectory(at: userDirectory, withIntermediateDirectories: true)
                Engine.chatModelDirectory = sources.chat
                let loaded = try Engine(lexiconURL: sources.lexicon, userDataDirectory: userDirectory)
                let engineSeconds = Date().timeIntervalSince(start)
                logger.info("Engine loaded in \(engineSeconds, privacy: .public) seconds")
                let rerankerStart = Date()
                let neural = sources.reranker.flatMap(loadReranker)
                let rerankerSeconds = neural == nil ? nil : Date().timeIntervalSince(rerankerStart)
                let charactersStart = Date()
                let characters = sources.characters.flatMap(loadCharacterModel)
                let charactersSeconds = characters == nil ? nil : Date().timeIntervalSince(charactersStart)
                DispatchQueue.main.async {
                    engine?.flushPersonalData()
                    engine = loaded
                    loadDurations = (engineSeconds, rerankerSeconds, charactersSeconds)
                    reranker = neural
                    characterModel = characters
                    generation += 1
                    loadedFingerprint = fingerprint
                    loading = false
                    SettingsWindowController.shared.engine = loaded
                    NotificationCenter.default.post(name: didLoad, object: nil)
                    if reloadAgain { reloadAgain = false; reload() }
                }
            } catch {
                logger.error("Engine load failed: \(error.localizedDescription, privacy: .public)")
                DispatchQueue.main.async { loading = false }
            }
        }
    }

    private static func loadReranker(_ directory: URL) -> NeuralReranker? {
        let start = Date()
        do {
            let reranker = try NeuralReranker.configured(directory: directory)
            logger.info("Reranker loaded in \(Date().timeIntervalSince(start), privacy: .public) seconds")
            return reranker
        } catch {
            logger.error("Reranker load failed: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    private static func loadCharacterModel(_ directory: URL) -> SyllableScorer? {
        let start = Date()
        do {
            let model = try SyllableScorer(directory: directory)
            logger.info("Character model loaded in \(Date().timeIntervalSince(start), privacy: .public) seconds")
            return model
        } catch {
            logger.error("Character model load failed: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    // MARK: - Updates

    /// Sizes and modification times of the data and model files in use, bundled and in the models folder.
    private static func currentFingerprint() -> String {
        var parts: [String] = []
        let manager = FileManager.default
        for root in [Bundle.main.resourceURL, modelsDirectory].compactMap({ $0 }) {
            for name in ["manifest.json", "lexicon.tsv", "lexicon.bin", "lm", "lm_chat", "reranker", "v3"] {
                let url = root.appendingPathComponent(name)
                guard manager.fileExists(atPath: url.path) else { continue }
                let files = manager.enumerator(at: url, includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey])
                    .map { $0.compactMap { $0 as? URL } } ?? []
                for file in [url] + files {
                    let values = try? file.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey])
                    parts.append("\(file.path)|\(values?.fileSize ?? -1)|\(values?.contentModificationDate?.timeIntervalSince1970 ?? 0)")
                }
            }
        }
        return parts.sorted().joined(separator: "\n")
    }

    private static func versionString(_ info: [String: Any]) -> String {
        "\(info["CFBundleShortVersionString"] ?? "")(\(info["CFBundleVersion"] ?? ""))"
    }

    /// Identity of the executable file on disk: an installer writes a new file.
    private static func executableStamp() -> String {
        guard let path = Bundle.main.executablePath,
              let attributes = try? FileManager.default.attributesOfItem(atPath: path) else { return "" }
        return "\(attributes[.systemFileNumber] ?? "")|\((attributes[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0)"
    }

    private static func checkForUpdates() {
        let plist = Bundle.main.bundleURL.appendingPathComponent("Contents/Info.plist")
        let stamp = executableStamp()
        // During an install the bundle is missing or half written (the installer writes Info.plist before the
        // executable): look again shortly instead of acting on a partial copy.
        guard let data = try? Data(contentsOf: plist),
              let info = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any],
              !stamp.isEmpty else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { checkForUpdates() }
            return
        }
        let version = versionString(info)
        if version != launchedVersion || stamp != launchedExecutable {
            logger.info("New version installed (\(launchedVersion, privacy: .public) → \(version, privacy: .public)); restarting when idle")
            restartWhenIdle()
        } else if currentFingerprint() != loadedFingerprint {
            logger.info("Data or models changed; reloading")
            reload()
        }
    }

    // MARK: - Restart

    /// Restarts as soon as nothing is being composed.  Called again after every key while a restart is pending.
    static func restartWhenIdle() {
        restartPending = true
        if idleTimer == nil {
            idleTimer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { _ in restartIfIdle() }
        }
        restartIfIdle()
    }

    static func restartIfIdle() {
        guard restartPending, !loading, !isComposing() else { return }
        prepareForExit()
        logger.info("Restarting for the new version")
        launchNewBuildAfterExit()
        exit(0)
    }

    /// macOS starts an input method again on the next key only some of the time; after several updates in a row it
    /// may not, and then no key types anything.  So the new build is started by a detached `open` a second after this
    /// process ends (`open` does nothing if the system has already started it).
    private static func launchNewBuildAfterExit() {
        let launcher = Process()
        launcher.executableURL = URL(fileURLWithPath: "/bin/sh")
        launcher.arguments = ["-c", "sleep 1; /usr/bin/open -g \"$0\"", Bundle.main.bundleURL.path]
        launcher.standardInput = FileHandle.nullDevice
        launcher.standardOutput = FileHandle.nullDevice
        launcher.standardError = FileHandle.nullDevice
        do { try launcher.run() } catch {
            logger.error("Could not schedule the new build's launch: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Before the process ends for an update: write personal data and the correction log, and remember Chinese or
    /// English mode.
    static func prepareForExit() {
        engine?.flushPersonalData()
        CorrectionLog.shared.flush()
        UsageStatsStore.shared.flush()
        if let lastEnglishMode {
            UserDefaults.standard.set(["english": lastEnglishMode, "time": Date().timeIntervalSince1970], forKey: modeKey)
        }
    }

    /// Switching to another input source forgets Chinese or English, so coming back starts as the settings say
    /// (Chinese unless 「開始輸入時使用英文」).  Moving between apps and fields with ZhuyinType selected keeps the mode.
    private static func observeInputSourceChanges() {
        DistributedNotificationCenter.default().addObserver(
            forName: NSNotification.Name(kTISNotifySelectedKeyboardInputSourceChanged as String), object: nil,
            queue: .main
        ) { _ in
            inputSourceChanged(toOurs: currentInputSourceIsOurs())
        }
    }

    /// Whether the selected keyboard input source is one of this input method's modes.
    static func currentInputSourceIsOurs() -> Bool {
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
              let pointer = TISGetInputSourceProperty(source, kTISPropertyBundleID) else { return false }
        let bundle = Unmanaged<CFString>.fromOpaque(pointer).takeUnretainedValue() as String
        return bundle == Bundle.main.bundleIdentifier
    }

    static func inputSourceChanged(toOurs: Bool) {
        if !toOurs { lastEnglishMode = nil }
    }

    /// Keeps Chinese or English across a restart for an update (within a minute of it).
    private static func restoreModeAfterRestart() {
        guard let saved = UserDefaults.standard.dictionary(forKey: modeKey) else { return }
        UserDefaults.standard.removeObject(forKey: modeKey)
        if let english = saved["english"] as? Bool, let time = saved["time"] as? Double,
           Date().timeIntervalSince1970 - time < 60 {
            lastEnglishMode = english
        }
    }
}
