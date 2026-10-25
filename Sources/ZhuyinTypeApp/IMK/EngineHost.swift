import Foundation
import os
import ZhuyinCore

enum EngineHost {
    static let connectionName = "org.zhuyintype.inputmethod.ZhuyinType_Connection"
    static let didLoad = Notification.Name("ZhuyinTypeEngineDidLoad")
    static let logger = Logger(subsystem: "org.zhuyintype", category: "IMK")
    // Published and read only on the main thread, including the mode shared by this input-method process.
    static private(set) var engine: Engine?
    static var lastEnglishMode: Bool?

    static func load() {
        let lexiconURL = Bundle.main.url(forResource: "lexicon", withExtension: "tsv")
        DispatchQueue.global(qos: .userInitiated).async {
            let start = Date()
            do {
                guard let lexiconURL else {
                    throw NSError(domain: "ZhuyinType", code: 1,
                                  userInfo: [NSLocalizedDescriptionKey: "Missing bundled lexicon.tsv"])
                }
                let directory = FileManager.default.homeDirectoryForCurrentUser
                    .appendingPathComponent("Library/Application Support/ZhuyinType", isDirectory: true)
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let loaded = try Engine(lexiconURL: lexiconURL, userDataDirectory: directory)
                logger.info("Engine loaded in \(Date().timeIntervalSince(start), privacy: .public) seconds")
                DispatchQueue.main.async {
                    engine = loaded
                    SettingsWindowController.shared.engine = loaded
                    NotificationCenter.default.post(name: didLoad, object: nil)
                }
            } catch {
                logger.error("Engine load failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }
}
