// The contract between ZhuyinCore and the app.  The app turns hardware keys into `KeyInput`, hands them to a
// `Composer`, inserts `KeyResult.commit` into the client, then draws `Composer.presentation` (marked text and the
// candidate window).  Nothing here touches AppKit, so every behaviour can be checked from the command line.
import Foundation

/// A key as the composer sees it.  The app maps hardware key codes on the US layout to these, so a character is the
/// key's unshifted ASCII character whatever keyboard layout the user has selected.
public enum KeyInput: Hashable, Sendable {
    /// A printable ASCII key with no modifier: "a", "1", ",", "-", "/", "`", "[", ...
    case char(Character)
    /// A printable ASCII key with Shift held, named by its unshifted character: Shift+1 is `.shiftedChar("1")`.
    case shiftedChar(Character)
    /// A digit or symbol key with Option held: the ASCII character printed on it, typed as is (Option+. is ".", Option+Shift+. is
    /// ">"), for the symbols the Zhuyin keys and Chinese punctuation cover.
    case printed(Character)
    /// A symbol key with Control held, named by the character printed on it (Control+Shift+/ is "?"): full-width
    /// Chinese punctuation, as Windows' Microsoft Bopomofo types it.
    case controlChar(Character)
    case space, shiftSpace
    case enter, shiftEnter
    case backspace, optionBackspace, forwardDelete
    case escape, tab, shiftTab
    case left, right, up, down, home, end, pageUp, pageDown
    case shiftLeft, shiftRight
    /// Candidates for the one character at the target; in the window, switches the single-character list.
    case shiftDown
    /// Typing corrections and fuzzy matches first; in the window, switches the corrections list.
    case shiftUp
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
    /// Only the one character at the target (Shift-↓), not every span from it.
    public var singleCharacter: Bool

    public init(items: [Candidate], selection: Int = 0, pageSize: Int = 9, tolerant: Bool? = nil,
                singleCharacter: Bool = false) {
        self.items = items
        self.selection = selection
        self.pageSize = max(1, pageSize)
        self.tolerant = tolerant
        self.singleCharacter = singleCharacter
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
    /// Index into `clauses` of the character ↓ would change, while the cursor is inside the composition and no list
    /// is open: shows which side of the cursor the list is for.
    public var targetClause: Int?
    /// The open candidate window, or nil.
    public var candidates: CandidateList?
    public var notice: String?
    /// Standard plain Zhuyin for the highlighted span; the backtick action in the window footer.
    public var zhuyinHint: String?

    public init(text: String = "", cursor: Int = 0, clauses: [Range<Int>] = [], focusedClause: Int? = nil,
                targetClause: Int? = nil, candidates: CandidateList? = nil, notice: String? = nil,
                zhuyinHint: String? = nil) {
        self.text = text
        self.cursor = cursor
        self.clauses = clauses
        self.focusedClause = focusedClause
        self.targetClause = targetClause
        self.candidates = candidates
        self.notice = notice
        self.zhuyinHint = zhuyinHint
    }

    public static let empty = Presentation()
    public var isEmpty: Bool { text.isEmpty && candidates == nil }
}

/// Strength of the physical-keyboard error channel. Neutral-tone matching is independent.
/// What Shift with a symbol key types in Chinese mode.
public enum PunctuationKeys: String, Sendable, CaseIterable {
    /// The symbol printed on the key, half-width (Shift+, is "<", Shift+- is "_"); Control with the key types Chinese
    /// punctuation (Control+, is "，").  As Windows' Microsoft Bopomofo.
    case windows
    /// Chinese punctuation (Shift+, is "，", Shift+- is "——"), as the Zhuyin input of macOS.
    case mac
}

/// Space in a composition with no unfinished syllable.
public enum SpaceAction: String, Sendable, CaseIterable {
    /// Opens the candidate window; in the window, Space turns the page.
    case openCandidates
    /// Commits the composition; the app does not receive the Space.
    case commit
    /// Commits the composition, then the app receives the Space, so a space follows the text.
    case commitAndSpace
    /// Puts a space into the composition and keeps composing (what ⇧Space always does); nothing is sent yet.
    case insertSpace
}

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

/// One syllable as a character model reads it: toneless reading (nil for a literal), tone 0 none, 1 first, 2 ˊ, 3 ˇ,
/// 4 ˋ, 5 ˙, and how far the reading is from the keys typed (0 exact … 3 large correction).
public struct CharacterModelSyllable: Equatable, Sendable {
    public var base: String?
    public var tone: Int
    public var costBucket: Int
    /// The syllable ended without a tone key or Space (a first tone found by the key decoder).
    public var implicit: Bool
    public init(base: String?, tone: Int, costBucket: Int, implicit: Bool = false) {
        self.base = base; self.tone = tone; self.costBucket = costBucket; self.implicit = implicit
    }
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
    /// What Space does when no syllable is being typed (with one, Space is the first tone).  The engine default
    /// opens the candidate window; the app's own default is set in its preferences.
    public var spaceAction: SpaceAction = .openCandidates
    /// Space opens the candidate window (true) or commits the composition (false).
    public var spaceOpensCandidates: Bool {
        get { spaceAction == .openCandidates }
        set { spaceAction = newValue ? .openCandidates : .commit }
    }
    /// Escape with no candidate window open clears the whole composition (true) or only the unfinished syllable.
    public var escapeClearsComposition: Bool = true
    /// Learn from commits and candidate picks.
    public var learningEnabled: Bool = true
    /// Longest composition, in syllables; further Zhuyin keys are refused (the key is consumed, nothing changes).
    public var maxSyllables: Int = 60
    /// Keyboard typo correction; aggressive uses 70% of standard channel costs.
    public var typoTolerance: TypoTolerance = .standard
    public var candidateAnchor: CandidateAnchor = .afterCursor
    /// How keys typed at the end of the composition become syllables (see `KeyDecoderMethod`).
    public var keyDecoder: KeyDecoderMethod = .timed
    /// The candidate window lists top to bottom: ↑↓ move the highlight and ←→ move to the neighbouring character.
    /// Left to right, ←→ move the highlight and ↑↓ turn pages.
    public var verticalList: Bool = true
    /// Which of two interchangeable character forms to write (台 or 臺).  Conversion and learning use the lexicon's
    /// own forms; this only changes the text shown and committed.
    public var characterForms: CharacterForms = .common
    /// What Shift and Control with a symbol key type.
    public var punctuationKeys: PunctuationKeys = .windows
    /// ↓ on a page's last candidate goes to the next page's first (↑ on the first goes back), stopping at the end of
    /// the list without wrapping.  The Swift composer always moves across pages and ignores this; the Rust engine
    /// implements both (`page_at_list_edge`).
    public var pageAtListEdge: Bool = true

    public init() {}
}

/// 台／臺 and similar pairs that Taiwan text writes both ways.  The models learnt the forms of their training text
/// (mostly 臺, from Wikipedia), while everyday writing mostly uses 台.
public enum CharacterForms: String, Sendable, CaseIterable {
    /// The everyday forms: 台.
    case common
    /// The forms of the Ministry of Education's standard: 臺.
    case standard

    static let commonForms: [Character: Character] = ["臺": "台"]

    func apply(_ text: String) -> String {
        guard self == .common, text.contains(where: { Self.commonForms[$0] != nil }) else { return text }
        return String(text.map { Self.commonForms[$0] ?? $0 })
    }
}
