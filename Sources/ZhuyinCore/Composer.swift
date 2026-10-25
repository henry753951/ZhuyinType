// STUB — replaced by the ZhuyinCore implementation.  The public signatures below are the contract; keep them.
import Foundation

/// Lexicon, language model and the user's learned data.  Loading is slow (hundreds of ms); do it once, off the main
/// thread, and share the instance between composers.
public final class Engine: @unchecked Sendable {
    /// - Parameters:
    ///   - lexiconURL: `lexicon.tsv` (`reading<TAB>word<TAB>logp`, see data/tools/build_lexicon.py).
    ///   - userDataDirectory: where learned data and user phrases are kept; nil keeps nothing on disk.
    public init(lexiconURL: URL, userDataDirectory: URL?) throws {
        fatalError("stub")
    }

    /// User phrases (the 自訂詞 list in Settings).  `reading` is Zhuyin syllables separated by spaces.
    public func userPhrases() -> [(reading: String, word: String)] { fatalError("stub") }
    public func addUserPhrase(word: String, reading: String) throws { fatalError("stub") }
    public func removeUserPhrase(word: String, reading: String) { fatalError("stub") }
    /// The readings a word can have, for filling in the reading when the user adds a phrase.  Empty if unknown.
    public func readings(of word: String) -> [String] { fatalError("stub") }

    /// Learned entries (the 學到的詞 list in Settings).
    public func learnedEntries() -> [(reading: String, word: String, count: Double)] { fatalError("stub") }
    public func forgetLearned(word: String, reading: String) { fatalError("stub") }
    public func forgetAllLearned() { fatalError("stub") }
}

public final class Composer {
    public var options: ComposerOptions

    public init(engine: Engine, options: ComposerOptions = ComposerOptions()) {
        fatalError("stub")
    }

    public private(set) var presentation: Presentation = .empty
    /// True while there is marked text or an open candidate window.
    public var isComposing: Bool { !presentation.isEmpty }

    public func handle(_ key: KeyInput) -> KeyResult { fatalError("stub") }
    /// A click on candidate `index` (into `presentation.candidates.items`).
    public func selectCandidate(at index: Int) -> KeyResult { fatalError("stub") }
    /// Commit everything (focus lost, mode switched, a shortcut pressed): returns the text to insert, and resets.
    public func commitAll() -> String { fatalError("stub") }
    /// Drop everything without committing.
    public func reset() { fatalError("stub") }
}
