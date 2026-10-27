// The composer: keys in, commits and the presentation out (Rust, through zhuyin-ffi).  The model hooks receive views
// of the composer's own data (sentences, candidates) that go straight back to the Rust reranker and character model, so
// nothing is copied into Swift on the way.
import CZhuyin
import Foundation

/// One of the engine's best sentences, as the reranker hook shows it.
public struct ScoredSentence {
    public let text: String
    public let score: Double
    /// Some word differs from the keys as typed.
    public let corrected: Bool
}

/// The engine's best sentences, as the reranker hook receives them; valid only during the hook.
public struct ScoredSentences: RandomAccessCollection {
    let pointer: OpaquePointer
    public var startIndex: Int { 0 }
    public var endIndex: Int { zt_sentences_count(pointer) }
    public subscript(index: Int) -> ScoredSentence {
        ScoredSentence(text: take(zt_sentences_text(pointer, index)) ?? "", score: zt_sentences_score(pointer, index),
                     corrected: zt_sentences_corrected(pointer, index))
    }
}

/// The candidates of a span and room for their scores, as the candidate scorer hook receives them; valid only during
/// the hook.
public struct CandidateTexts {
    let pointer: OpaquePointer
    let scores: UnsafeMutablePointer<Double>
    public var count: Int { zt_strings_count(pointer) }
}

/// Each syllable's candidate characters and room for their scores, as the character scorer hook receives them; valid
/// only during the hook.
public struct CandidateRows {
    let pointer: OpaquePointer
    let scores: UnsafeMutablePointer<Double>
    let lengths: UnsafeMutablePointer<Int>
    public var count: Int { zt_candidate_rows_count(pointer) }
}

/// Scores written into the hook's buffers (a scorer returns this, or nil when it could not score).
public struct ScoresWritten {
    init() {}
}

private final class Box<T> {
    let value: T
    init(_ value: T) { self.value = value }
}

private func retained<T>(_ value: T) -> UnsafeMutableRawPointer { Unmanaged.passRetained(Box(value)).toOpaque() }

private typealias RerankerClosure = (ScoredSentences) -> Int
private typealias CandidateClosure = (String, CandidateTexts) -> ScoresWritten?
private typealias CharacterClosure = ([CharacterModelSyllable], CandidateRows) -> ScoresWritten?
private typealias CommitClosure = (String) -> Void

private let releaseReranker: @convention(c) (UnsafeMutableRawPointer?) -> Void = {
    Unmanaged<Box<RerankerClosure>>.fromOpaque($0!).release()
}
private let releaseCandidates: @convention(c) (UnsafeMutableRawPointer?) -> Void = {
    Unmanaged<Box<CandidateClosure>>.fromOpaque($0!).release()
}
private let releaseCharacters: @convention(c) (UnsafeMutableRawPointer?) -> Void = {
    Unmanaged<Box<CharacterClosure>>.fromOpaque($0!).release()
}
private let releaseCommit: @convention(c) (UnsafeMutableRawPointer?) -> Void = {
    Unmanaged<Box<CommitClosure>>.fromOpaque($0!).release()
}

extension KeyInput {
    var zt: ZtKey {
        func key(_ kind: Int32, _ character: Character? = nil) -> ZtKey {
            ZtKey(kind: UInt32(kind), character: character?.unicodeScalars.first?.value ?? 0)
        }
        switch self {
        case .char(let c): return key(ZT_KEY_CHAR, c)
        case .shiftedChar(let c): return key(ZT_KEY_SHIFTED_CHAR, c)
        case .printed(let c): return key(ZT_KEY_PRINTED, c)
        case .controlChar(let c): return key(ZT_KEY_CONTROL_CHAR, c)
        case .space: return key(ZT_KEY_SPACE)
        case .shiftSpace: return key(ZT_KEY_SHIFT_SPACE)
        case .enter: return key(ZT_KEY_ENTER)
        case .shiftEnter: return key(ZT_KEY_SHIFT_ENTER)
        case .backspace: return key(ZT_KEY_BACKSPACE)
        case .optionBackspace: return key(ZT_KEY_OPTION_BACKSPACE)
        case .forwardDelete: return key(ZT_KEY_FORWARD_DELETE)
        case .escape: return key(ZT_KEY_ESCAPE)
        case .tab: return key(ZT_KEY_TAB)
        case .shiftTab: return key(ZT_KEY_SHIFT_TAB)
        case .left: return key(ZT_KEY_LEFT)
        case .right: return key(ZT_KEY_RIGHT)
        case .up: return key(ZT_KEY_UP)
        case .down: return key(ZT_KEY_DOWN)
        case .home: return key(ZT_KEY_HOME)
        case .end: return key(ZT_KEY_END)
        case .pageUp: return key(ZT_KEY_PAGE_UP)
        case .pageDown: return key(ZT_KEY_PAGE_DOWN)
        case .shiftLeft: return key(ZT_KEY_SHIFT_LEFT)
        case .shiftRight: return key(ZT_KEY_SHIFT_RIGHT)
        case .shiftDown: return key(ZT_KEY_SHIFT_DOWN)
        case .shiftUp: return key(ZT_KEY_SHIFT_UP)
        }
    }
}

private func result(_ result: ZtKeyResult) -> KeyResult {
    KeyResult(handled: result.handled, commit: take(result.commit))
}

public final class Composer {
    let handle: OpaquePointer
    private let engine: Engine
    private var cached: Presentation?

    public var options: ComposerOptions {
        didSet {
            guard options != oldValue else { return }
            Self.withOptions(options) { _ = zt_composer_set_options(handle, $0) }
            cached = nil
        }
    }

    public init(engine: Engine, options: ComposerOptions = ComposerOptions()) {
        self.engine = engine
        self.options = options
        handle = Self.withOptions(options) { zt_composer_new(engine.handle, $0) }!
    }

    deinit { zt_composer_free(handle) }

    private static func withOptions<T>(_ options: ComposerOptions, _ body: (UnsafePointer<ZtComposerOptions>) -> T) -> T {
        var o = zt_composer_options_default()
        let spaces: [SpaceAction: Int32] = [.openCandidates: ZT_SPACE_OPEN_CANDIDATES, .commit: ZT_SPACE_COMMIT,
                                            .commitAndSpace: ZT_SPACE_COMMIT_AND_SPACE, .insertSpace: ZT_SPACE_INSERT_SPACE]
        let typos: [TypoTolerance: Int32] = [.off: ZT_TYPO_OFF, .standard: ZT_TYPO_STANDARD, .aggressive: ZT_TYPO_AGGRESSIVE]
        let decoders: [KeyDecoderMethod: Int32] = [.slots: ZT_DECODER_SLOTS, .boundaries: ZT_DECODER_BOUNDARIES,
                                                   .reorder: ZT_DECODER_REORDER, .timed: ZT_DECODER_TIMED]
        o.space_action = UInt32(spaces[options.spaceAction]!)
        o.escape_clears_composition = options.escapeClearsComposition
        o.learning_enabled = options.learningEnabled
        o.max_syllables = max(0, options.maxSyllables)
        o.early_commit_syllables = max(0, options.earlyCommitSyllables)
        o.typo_tolerance = UInt32(typos[options.typoTolerance]!)
        o.candidate_anchor = UInt32(options.candidateAnchor == .afterCursor ? ZT_ANCHOR_AFTER_CURSOR : ZT_ANCHOR_BEFORE_CURSOR)
        o.key_decoder = UInt32(decoders[options.keyDecoder]!)
        o.vertical_list = options.verticalList
        o.page_at_list_edge = options.pageAtListEdge
        o.character_forms = UInt32(options.characterForms == .common ? ZT_FORMS_COMMON : ZT_FORMS_STANDARD)
        o.punctuation_keys = UInt32(options.punctuationKeys == .windows ? ZT_PUNCTUATION_WINDOWS : ZT_PUNCTUATION_MAC)
        return String(options.selectionKeys).withCString { keys in
            o.selection_keys = keys
            return withUnsafePointer(to: &o, body)
        }
    }

    // MARK: - Keys

    @discardableResult
    public func handle(_ key: KeyInput, at time: TimeInterval) -> KeyResult {
        defer { cached = nil }
        return result(zt_composer_handle_at(handle, key.zt, time))
    }

    @discardableResult
    public func handle(_ key: KeyInput) -> KeyResult {
        defer { cached = nil }
        return result(zt_composer_handle(handle, key.zt))
    }

    @discardableResult
    public func selectCandidate(at index: Int) -> KeyResult {
        defer { cached = nil }
        guard index >= 0 else { return .consumed }
        return result(zt_composer_select_candidate(handle, index))
    }

    public func commitAll() -> String {
        defer { cached = nil }
        return take(zt_composer_commit_all(handle)) ?? ""
    }

    public func setPrecedingText(_ text: String) { zt_composer_set_preceding_text(handle, text); cached = nil }
    public func breakContext() { zt_composer_break_context(handle); cached = nil }
    public func reset() { zt_composer_reset(handle); cached = nil }
    public func toggleCorrections() { zt_composer_toggle_corrections(handle); cached = nil }

    public var learningSuspended: Bool {
        get { zt_composer_learning_suspended(handle) }
        set { zt_composer_set_learning_suspended(handle, newValue) }
    }

    public var freezesOnPick = true {
        didSet { zt_composer_set_freezes_on_pick(handle, freezesOnPick) }
    }

    public var isComposing: Bool { !presentation.isEmpty }

    public var presentation: Presentation {
        if let cached { return cached }
        let value = Self.presentation(zt_composer_presentation(handle))
        cached = value
        return value
    }

    private static func presentation(_ pointer: UnsafeMutablePointer<ZtPresentation>?) -> Presentation {
        guard let pointer else { return .empty }
        defer { zt_presentation_free(pointer) }
        let p = pointer.pointee
        let clauses = (0..<p.clause_count).map { p.clauses[$0].start..<p.clauses[$0].end }
        let candidates = p.candidates.map { list -> CandidateList in
            let l = list.pointee
            let items = (0..<l.count).map { i in
                Candidate(text: String(cString: l.items[i].text), annotation: l.items[i].annotation.map { String(cString: $0) })
            }
            return CandidateList(items: items, selection: l.selection, pageSize: l.page_size,
                                 tolerant: l.tolerant < 0 ? nil : l.tolerant == 1, singleCharacter: l.single_character)
        }
        return Presentation(text: String(cString: p.text), cursor: p.cursor, clauses: clauses,
                            focusedClause: p.focused_clause < 0 ? nil : p.focused_clause,
                            targetClause: p.target_clause < 0 ? nil : p.target_clause, candidates: candidates,
                            notice: p.notice.map { String(cString: $0) }, zhuyinHint: p.zhuyin_hint.map { String(cString: $0) })
    }

    // MARK: - Models

    public var rerankingCount: Int = 8 {
        didSet { zt_composer_set_reranking_count(handle, max(0, rerankingCount)) }
    }

    public var characterWeight = 1.0 {
        didSet { zt_composer_set_character_weight(handle, characterWeight) }
    }

    public var characterScoresInList = true {
        didSet { zt_composer_set_character_scores_in_list(handle, characterScoresInList) }
    }

    /// Chooses the displayed sentence among the engine's best (an index into them).
    public var reranker: ((ScoredSentences) -> Int)? {
        didSet {
            guard let reranker else {
                zt_composer_set_reranker(handle, ZtRerankerHook(context: nil, choose: nil, release: nil))
                return
            }
            let hook = ZtRerankerHook(context: retained(reranker as RerankerClosure), choose: { context, sentences in
                let closure = Unmanaged<Box<RerankerClosure>>.fromOpaque(context!).takeUnretainedValue().value
                return closure(ScoredSentences(pointer: sentences!))
            }, release: releaseReranker)
            zt_composer_set_reranker(handle, hook)
        }
    }

    /// Scores a span's candidates for list order: `before` is the composition text before the span.
    public var candidateScorer: ((_ before: String, _ candidates: CandidateTexts) -> ScoresWritten?)? {
        didSet {
            guard let candidateScorer else {
                zt_composer_set_candidate_scorer(handle, ZtCandidateScorerHook(context: nil, score: nil, release: nil))
                return
            }
            let hook = ZtCandidateScorerHook(context: retained(candidateScorer as CandidateClosure), score: { context, before, candidates, scores in
                let closure = Unmanaged<Box<CandidateClosure>>.fromOpaque(context!).takeUnretainedValue().value
                return closure(String(cString: before!), CandidateTexts(pointer: candidates!, scores: scores!)) != nil
            }, release: releaseCandidates)
            zt_composer_set_candidate_scorer(handle, hook)
        }
    }

    /// Told the text a full composition commits by itself in the middle of a key (`ComposerOptions.maxSyllables`),
    /// before the rest is converted again: the models' text before the composition should end with it.  The same
    /// text also comes back in the key's `KeyResult.commit`.  It must not use the composer.
    public var commitListener: ((String) -> Void)? {
        didSet {
            guard let commitListener else {
                zt_composer_set_commit_hook(handle, ZtCommitHook(context: nil, committed: nil, release: nil))
                return
            }
            let hook = ZtCommitHook(context: retained(commitListener as CommitClosure), committed: { context, text in
                let closure = Unmanaged<Box<CommitClosure>>.fromOpaque(context!).takeUnretainedValue().value
                closure(String(cString: text!))
            }, release: releaseCommit)
            zt_composer_set_commit_hook(handle, hook)
        }
    }

    /// Scores each syllable's candidate characters for the lattice.
    public var characterScorer: ((_ syllables: [CharacterModelSyllable], _ candidates: CandidateRows) -> ScoresWritten?)? {
        didSet {
            guard let characterScorer else {
                zt_composer_set_character_scorer(handle, ZtCharacterScorerHook(context: nil, score: nil, release: nil))
                return
            }
            let hook = ZtCharacterScorerHook(context: retained(characterScorer as CharacterClosure), score: { context, syllables, count, candidates, scores, lengths in
                let closure = Unmanaged<Box<CharacterClosure>>.fromOpaque(context!).takeUnretainedValue().value
                let list = (0..<count).map { i -> CharacterModelSyllable in
                    let s = syllables![i]
                    return CharacterModelSyllable(base: s.base.map { String(cString: $0) }, tone: Int(s.tone),
                                                  costBucket: Int(s.cost_bucket), implicit: s.implicit)
                }
                return closure(list, CandidateRows(pointer: candidates!, scores: scores!, lengths: lengths!)) != nil
            }, release: releaseCharacters)
            zt_composer_set_character_scorer(handle, hook)
        }
    }
}
