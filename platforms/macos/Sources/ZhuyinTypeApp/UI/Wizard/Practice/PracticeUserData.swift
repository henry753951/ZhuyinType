import Foundation
import ZhuyinEngine

/// The practice field's personal data: an engine of its own, loaded from the same lexicon and models as the input
/// method, whose learned words and user phrases go to a folder in the temporary directory, with a correction log and
/// usage statistics there too.  The folder is deleted when the last practice field lets go of it.  The input
/// method's engine, `CorrectionLog.shared`, `UsageStatsStore.shared` and the files in
/// ~/Library/Application Support/ZhuyinType are never used.
///
/// Every field open at once (the steps of one wizard) shares one, so what was learned in one step carries to the next,
/// as it would in the input method.
final class PracticeUserData {
    let directory: URL
    private(set) var engine: Engine?
    let corrections: CorrectionLog
    let stats: UsageStatsStore
    /// The input method's models: read only, they keep no personal data.
    let reranker: NeuralReranker?
    let characterModel: SyllableScorer?

    init(engine: Engine, directory: URL, reranker: NeuralReranker? = nil, characterModel: SyllableScorer? = nil) {
        self.engine = engine
        self.directory = directory
        corrections = CorrectionLog(directory: directory)
        stats = UsageStatsStore(directory: directory)
        self.reranker = reranker
        self.characterModel = characterModel
    }

    deinit {
        // Let every pending write finish first (none may land after the folder is gone and make it again), then
        // remove the folder with everything in it.
        engine?.flushPersonalData()
        engine = nil
        corrections.flush()
        stats.flush()
        try? FileManager.default.removeItem(at: directory)
    }

    /// The user phrases added while practising.
    var userPhrases: Set<String> { Set(engine?.userPhrases().map(\.word) ?? []) }

    /// Writes the engine's personal data, the log and the statistics to the folder now (for the self-tests).
    func flush() {
        engine?.flushPersonalData()
        corrections.flush()
        stats.flush()
    }

    /// A new folder for practice data in the temporary directory, readable by this user only.
    static func makeDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ZhuyinTypePractice-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
                                                attributes: [.posixPermissions: 0o700])
        madeDirectories.append(directory)
        return directory
    }

    /// Every folder made in this process, so the self-test can check that none is left behind.
    private(set) static var madeDirectories: [URL] = []

    /// Loads practice data synchronously: an engine for `lexiconURL` in a new temporary folder.
    static func make(lexiconURL: URL, reranker: NeuralReranker? = EngineHost.reranker,
                     characterModel: SyllableScorer? = EngineHost.characterModel) throws -> PracticeUserData {
        let directory = try makeDirectory()
        do {
            let engine = try Engine(lexiconURL: lexiconURL, userDataDirectory: directory)
            return PracticeUserData(engine: engine, directory: directory, reranker: reranker, characterModel: characterModel)
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }

    // MARK: Shared

    private static weak var current: PracticeUserData?
    private static var waiting: [(PracticeUserData?) -> Void] = []

    /// The practice data in use, or new data loaded in the background (the lexicon takes a moment); `completion` runs
    /// on the main thread, with nil when the lexicon could not be loaded.
    static func shared(lexiconURL: URL? = EngineHost.lexiconURL(), completion: @escaping (PracticeUserData?) -> Void) {
        precondition(Thread.isMainThread)
        if let current { completion(current); return }
        waiting.append(completion)
        guard waiting.count == 1 else { return }
        guard let lexiconURL else { finish(nil); return }
        let reranker = EngineHost.reranker, characterModel = EngineHost.characterModel
        DispatchQueue.global(qos: .userInitiated).async {
            let data: PracticeUserData?
            do {
                data = try make(lexiconURL: lexiconURL, reranker: reranker, characterModel: characterModel)
            } catch {
                EngineHost.logger.error("Practice engine load failed: \(error.localizedDescription, privacy: .public)")
                data = nil
            }
            DispatchQueue.main.async { finish(data) }
        }
    }

    private static func finish(_ data: PracticeUserData?) {
        current = data
        let callbacks = waiting
        waiting = []
        callbacks.forEach { $0(data) }
    }
}
