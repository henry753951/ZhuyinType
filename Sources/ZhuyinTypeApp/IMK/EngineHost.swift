import Foundation
import os
import ZhuyinCore
import ZhuyinML

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
                if let chat = Bundle.main.resourceURL?.appendingPathComponent("lm_chat"),
                   FileManager.default.fileExists(atPath: chat.appendingPathComponent("bigram.tsv").path) {
                    Engine.chatModelDirectory = chat
                }
                let loaded = try Engine(lexiconURL: lexiconURL, userDataDirectory: directory)
                logger.info("Engine loaded in \(Date().timeIntervalSince(start), privacy: .public) seconds")
                let neural = loadReranker()
                let characters = loadCharacterModel()
                DispatchQueue.main.async {
                    engine = loaded
                    reranker = neural
                    characterModel = characters
                    SettingsWindowController.shared.engine = loaded
                    NotificationCenter.default.post(name: didLoad, object: nil)
                }
            } catch {
                logger.error("Engine load failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private static func loadReranker() -> NeuralReranker? {
        guard let directory = Bundle.main.resourceURL?.appendingPathComponent("reranker"),
              FileManager.default.fileExists(atPath: directory.appendingPathComponent("vocab.json").path) else { return nil }
        let start = Date()
        do {
            var weight = 0.5
            if let data = try? Data(contentsOf: directory.appendingPathComponent("config.json")),
               let config = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let value = config["weight"] as? Double { weight = value }
            let reranker = try NeuralReranker(directory: directory, weight: weight)
            if let data = try? Data(contentsOf: directory.appendingPathComponent("config.json")),
               let config = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let count = config["count"] as? Int, count > 0 { reranker.candidateCount = count }
            logger.info("Reranker loaded in \(Date().timeIntervalSince(start), privacy: .public) seconds")
            return reranker
        } catch {
            logger.error("Reranker load failed: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    private static func loadCharacterModel() -> SyllableScorer? {
        guard let directory = Bundle.main.resourceURL?.appendingPathComponent("v3"),
              FileManager.default.fileExists(atPath: directory.appendingPathComponent("chars.json").path) else { return nil }
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
}
