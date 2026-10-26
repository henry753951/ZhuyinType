// The contract between ZhuyinCore and the app.  The app turns hardware keys into `KeyInput`, hands them to a
// `Composer`, inserts `KeyResult.commit` into the client, then draws `Composer.presentation` (marked text and the
// candidate window).  Nothing here touches AppKit, so every behaviour can be checked from the command line.
import Foundation

/// A key as the composer sees it.  The app maps hardware key codes on the US layout to these, so a character is the
/// key's unshifted ASCII character whatever keyboard layout the user has selected.
public enum KeyInput: Equatable, Sendable {
    /// A printable ASCII key with no modifier: "a", "1", ",", "-", "/", "`", "[", ...
    case char(Character)
    /// A printable ASCII key with Shift held, named by its unshifted character: Shift+1 is `.shiftedChar("1")`.
    case shiftedChar(Character)
    /// A digit or symbol key with Option held: the ASCII character printed on it, typed as is (Option+. is ".", Option+Shift+. is
    /// ">"), for the symbols the Zhuyin keys and Chinese punctuation cover.
    case printed(Character)
    case space, shiftSpace
    case enter, shiftEnter
    case backspace, optionBackspace, forwardDelete
    case escape, tab, shiftTab
    case left, right, up, down, home, end, pageUp, pageDown
    case shiftLeft, shiftRight
    /// Candidates for the one character at the cursor, for the keys as typed; in the window, switches lists.
    case shiftDown
}

public struct KeyResult: Equatable, Sendable {
    /// False: the key was not for the input method and goes on to the application (after `commit`, if any).
    public var handled: Bool
    /// Text to insert into the client, before the new presentation is shown.
    public var commit: String?

    public init(handled: Bool, commit: String? = nil) {
        self.handled = handled
        self.commit = commit
    }

    public static let passThrough = KeyResult(handled: false)
    public static let consumed = KeyResult(handled: true)
}

public struct Candidate: Equatable, Sendable {
    public var text: String
    /// Small grey text after the candidate, e.g. the reading of a rare character.  Nil for most candidates.
    public var annotation: String?

    public init(text: String, annotation: String? = nil) {
        self.text = text
        self.annotation = annotation
    }
}

public struct CandidateList: Equatable, Sendable {
    public var items: [Candidate]
    /// Highlighted index into `items`.
    public var selection: Int
    public var pageSize: Int

    /// Whether the list shows typing corrections (true) or only words for the keys as typed (false); nil when the
    /// list cannot have corrections (punctuation, correction off).
    public var tolerant: Bool?

    public init(items: [Candidate], selection: Int = 0, pageSize: Int = 9, tolerant: Bool? = nil) {
        self.items = items
        self.selection = selection
        self.pageSize = max(1, pageSize)
        self.tolerant = tolerant
    }

    public var page: Int { selection / pageSize }
    public var pageCount: Int { max(1, (items.count + pageSize - 1) / pageSize) }
    /// Range of `items` on the current page.
    public var pageRange: Range<Int> { (page * pageSize)..<min(items.count, page * pageSize + pageSize) }
}

public struct Presentation: Equatable, Sendable {
    /// The marked text: converted characters, with the unfinished syllable's Zhuyin symbols at the cursor.
    public var text: String
    /// Caret position in `text`, in UTF-16 units.
    public var cursor: Int
    /// UTF-16 ranges of the words in `text`, for clause underlines.  Covers `text` without gaps.
    public var clauses: [Range<Int>]
    /// Index into `clauses` replaced by the highlighted candidate, or selected for adding a user phrase.
    public var focusedClause: Int?
    /// The open candidate window, or nil.
    public var candidates: CandidateList?
    public var notice: String?
    /// Standard plain Zhuyin for the highlighted span; the backtick action in the window footer.
    public var zhuyinHint: String?

    public init(text: String = "", cursor: Int = 0, clauses: [Range<Int>] = [], focusedClause: Int? = nil,
                candidates: CandidateList? = nil, notice: String? = nil, zhuyinHint: String? = nil) {
        self.text = text
        self.cursor = cursor
        self.clauses = clauses
        self.focusedClause = focusedClause
        self.candidates = candidates
        self.notice = notice
        self.zhuyinHint = zhuyinHint
    }

    public static let empty = Presentation()
    public var isEmpty: Bool { text.isEmpty && candidates == nil }
}

/// Strength of the physical-keyboard error channel. Neutral-tone matching is independent.
public enum TypoTolerance: String, Sendable, CaseIterable {
    case off, standard, aggressive
    var costMultiplier: Double { self == .aggressive ? 0.7 : 1 }
}

/// Sound pairs many speakers merge.  With a pair on, each sound also matches the other one at a small cost, so an
/// exact match ranks first unless the other word is clearly likelier; a syllable can use one initial and one final pair.
public struct FuzzySounds: OptionSet, Hashable, Sendable {
    public let rawValue: Int
    public init(rawValue: Int) { self.rawValue = rawValue }
    /// ㄗ and ㄓ
    public static let zZh = FuzzySounds(rawValue: 1 << 0)
    /// ㄘ and ㄔ
    public static let cCh = FuzzySounds(rawValue: 1 << 1)
    /// ㄙ and ㄕ
    public static let sSh = FuzzySounds(rawValue: 1 << 2)
    /// ㄣ and ㄥ
    public static let enEng = FuzzySounds(rawValue: 1 << 3)
    /// ㄢ and ㄤ
    public static let anAng = FuzzySounds(rawValue: 1 << 4)

    /// Each pair, and whether it is an initial (true) or a final.
    static let pairs: [(set: FuzzySounds, a: Character, b: Character, initial: Bool)] = [
        (.zZh, "ㄗ", "ㄓ", true), (.cCh, "ㄘ", "ㄔ", true), (.sSh, "ㄙ", "ㄕ", true),
        (.enEng, "ㄣ", "ㄥ", false), (.anAng, "ㄢ", "ㄤ", false),
    ]
}

public enum CandidateAnchor: Sendable {
    /// Spans start at the cursor, except at the composition end where they end at the cursor.
    case afterCursor
    /// Spans end at the cursor, except at the composition start where they start at zero.
    case beforeCursor
}

/// Ranges are composition token offsets, not UTF-16 offsets. Literals have an empty reading.
public struct SentenceWord: Equatable, Sendable {
    public var range: Range<Int>
    public var text: String
    public var reading: String
    /// Matched through a typing correction or a fuzzy sound, not the keys as typed.
    public var corrected = false
}

public struct ScoredSentence: Equatable, Sendable {
    public var words: [SentenceWord]
    /// Natural-log decoder score, including sentence boundaries.
    public var score: Double
    public var text: String { words.map(\.text).joined() }
    /// Some word differs from the keys as typed.
    public var corrected: Bool { words.contains(where: \.corrected) }
}

public struct ComposerOptions: Equatable, Sendable {
    /// Keys that pick candidates 1..n on the current page.  Also sets the page size.
    public var selectionKeys: [Character] = Array("123456789")
    /// Space with no unfinished syllable opens the candidate window (true) or commits the composition (false).
    public var spaceOpensCandidates: Bool = true
    /// Escape with no candidate window open clears the whole composition (true) or only the unfinished syllable.
    public var escapeClearsComposition: Bool = true
    /// Learn from commits and candidate picks.
    public var learningEnabled: Bool = true
    /// Longest composition, in syllables; further Zhuyin keys are refused (the key is consumed, nothing changes).
    public var maxSyllables: Int = 60
    /// Keyboard typo correction; aggressive uses 70% of standard channel costs.
    public var typoTolerance: TypoTolerance = .standard
    public var candidateAnchor: CandidateAnchor = .afterCursor

    public init() {}
}
