import Combine
import Foundation
import Sparkle

/// Over-the-air updates through Sparkle.  The feed (SUFeedURL) and its EdDSA key (SUPublicEDKey) are in Info.plist;
/// every update archive and the appcast itself must carry a valid signature.
///
/// An input method almost never quits, so Sparkle's usual "install on quit" would never happen: a downloaded update is
/// installed as soon as no text field is composing, the same moment EngineHost restarts for a manually installed
/// version.  Checks run in the background without windows; only "檢查更新…" shows Sparkle's own window.
final class Updater: NSObject, ObservableObject, SPUUpdaterDelegate, SPUStandardUserDriverDelegate {
    static let shared = Updater()

    private var controller: SPUStandardUpdaterController?
    private var installNow: (() -> Void)?
    private var idleTimer: Timer?
    /// The version Sparkle has downloaded and will install when idle, for the settings window.
    @Published private(set) var pendingVersion: String?
    @Published private(set) var lastCheck: Date?

    var isAvailable: Bool { controller != nil }

    /// Starts the updater when the bundle declares a feed (development builds without one stay offline).
    func start() {
        guard controller == nil, Bundle.main.object(forInfoDictionaryKey: "SUFeedURL") != nil,
              Bundle.main.object(forInfoDictionaryKey: "SUPublicEDKey") != nil else { return }
        controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: self, userDriverDelegate: self)
        lastCheck = controller?.updater.lastUpdateCheckDate
    }

    var automaticallyChecks: Bool {
        get { controller?.updater.automaticallyChecksForUpdates ?? false }
        set { controller?.updater.automaticallyChecksForUpdates = newValue; objectWillChange.send() }
    }

    var automaticallyInstalls: Bool {
        get { controller?.updater.automaticallyDownloadsUpdates ?? false }
        set { controller?.updater.automaticallyDownloadsUpdates = newValue; objectWillChange.send() }
    }

    /// "檢查更新…": Sparkle's window, which also offers an update found in the background.
    func checkNow() { controller?.checkForUpdates(nil) }

    /// Installs a downloaded update if nothing is being composed.  Called after every key and on a timer.
    func installIfIdle() {
        guard let installNow, !EngineHost.isLoading, !EngineHost.isComposing() else { return }
        self.installNow = nil
        idleTimer?.invalidate()
        EngineHost.prepareForExit()
        EngineHost.logger.info("Installing update \(self.pendingVersion ?? "?", privacy: .public)")
        installNow()
    }

    // MARK: SPUUpdaterDelegate

    func updater(_ updater: SPUUpdater, willInstallUpdateOnQuit item: SUAppcastItem,
                 immediateInstallationBlock: @escaping () -> Void) -> Bool {
        installNow = immediateInstallationBlock
        pendingVersion = item.displayVersionString
        if idleTimer == nil {
            idleTimer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in self?.installIfIdle() }
        }
        installIfIdle()
        return true
    }

    func updater(_ updater: SPUUpdater, didFinishUpdateCycleFor updateCheck: SPUUpdateCheck, error: (any Error)?) {
        lastCheck = updater.lastUpdateCheckDate
    }

    func updaterShouldPromptForPermissionToCheck(forUpdates updater: SPUUpdater) -> Bool {
        // Checking is a setting with a default (on), shown in 一般; no permission dialog from a background process.
        false
    }

    // MARK: SPUStandardUserDriverDelegate

    var supportsGentleScheduledUpdateReminders: Bool { true }

    /// Background checks stay silent when updates install automatically; with automatic installation off, Sparkle's
    /// window offers the update.
    func standardUserDriverShouldHandleShowingScheduledUpdate(_ update: SUAppcastItem, andInImmediateFocus immediateFocus: Bool) -> Bool {
        !automaticallyInstalls
    }
}
