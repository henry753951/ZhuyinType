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
    case space, shiftSpace
    case enter, shiftEnter
    case backspace, optionBackspace, forwardDelete
    case escape, tab, shiftTab
    case left, right, up, down, home, end, pageUp, pageDown
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

    public init(items: [Candidate], selection: Int = 0, pageSize: Int = 9) {
        self.items = items
        self.selection = selection
        self.pageSize = max(1, pageSize)
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
    /// Index into `clauses` that the open candidate window replaces; nil when no candidate window is open.
    public var focusedClause: Int?
    /// The open candidate window, or nil.
    public var candidates: CandidateList?

    public init(text: String = "", cursor: Int = 0, clauses: [Range<Int>] = [], focusedClause: Int? = nil,
                candidates: CandidateList? = nil) {
        self.text = text
        self.cursor = cursor
        self.clauses = clauses
        self.focusedClause = focusedClause
        self.candidates = candidates
    }

    public static let empty = Presentation()
    public var isEmpty: Bool { text.isEmpty && candidates == nil }
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

    public init() {}
}
