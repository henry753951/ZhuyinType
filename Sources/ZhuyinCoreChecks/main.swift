import Foundation
import ZhuyinCore

var failures = 0
var checks = 0
func check(_ condition: @autoclosure () -> Bool, _ message: String, file: StaticString = #filePath, line: UInt = #line) {
    checks += 1
    if !condition() { failures += 1; print("FAIL \(file):\(line): \(message)") }
}
func timed<T>(_ body: () throws -> T) rethrows -> (T, Double) {
    let start = DispatchTime.now().uptimeNanoseconds
    let result = try body()
    return (result, Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000)
}
let tiny = """
# Tiny, deliberately ambiguous lexicon
ㄅㄚ	八	-2
ㄅㄚ	巴	-2.3
ㄅㄚ	吧	-2.4
ㄅㄚ	芭	-2.5
ㄅㄚ	疤	-2.6
ㄅㄚ	笆	-2.7
ㄅㄚ	捌	-2.8
ㄅㄚ	粑	-2.9
ㄅㄚ	叭	-3.0
ㄅㄚ	豝	-3.1
ㄅㄚ	仈	-3.2
ㄅㄚˊ	拔	-2
ㄅㄚˇ	把	-2
ㄅㄚˋ	爸	-2
ㄅㄚ˙	罷	-2
ㄆㄚ	趴	-2
ㄇㄚ	媽	-2
ㄇㄚˊ	麻	-2
ㄧ	一	-2
ㄨ	烏	-2
ㄩ	迂	-2
ㄅㄧㄢ	邊	-2
ㄅㄚ ㄆㄚ	八趴	-1
ㄅㄚ ㄆㄚ	巴趴	-1.2
ㄅㄚ ㄆㄚ ㄇㄚ	八趴媽	-0.5
ㄅㄚ ㄅㄚ	八八	-5
ㄋㄧˇ	你	-2
ㄏㄠˇ	好	-2
ㄋㄧˇ ㄏㄠˇ	你好	-1
ㄒㄧㄝˋ	謝	-2
ㄐㄧㄣ	今	-2
ㄊㄧㄢ	天	-2
ㄑㄧˋ	氣	-2
ㄏㄣˇ	很	-2
ㄐㄧㄣ ㄊㄧㄢ	今天	-1
ㄊㄧㄢ ㄑㄧˋ	天氣	-1
ㄏㄣˇ ㄏㄠˇ	很好	-1
"""
func type(_ keys: String, into composer: Composer) {
    for character in keys { _ = composer.handle(character == " " ? .space : .char(character)) }
}
var legacyOptions = ComposerOptions(); legacyOptions.typoTolerance = .off
func composer(_ engine: Engine, _ keys: String = "", options: ComposerOptions = legacyOptions) -> Composer {
    let result = Composer(engine: engine, options: options)
    type(keys, into: result)
    return result
}
func validPresentation(_ c: Composer, _ label: String) {
    let p = c.presentation
    check(p.cursor >= 0 && p.cursor <= p.text.utf16.count, "\(label): caret in bounds")
    var end = 0
    for range in p.clauses {
        check(range.lowerBound == end && !range.isEmpty, "\(label): contiguous nonempty clauses")
        end = range.upperBound
    }
    check(end == p.text.utf16.count, "\(label): clauses cover text")
    if let index = p.focusedClause { check(p.clauses.indices.contains(index), "\(label): valid focused clause") }
}
let engine = try Engine(lexiconText: tiny)

// Empty state and pass-through contract.
let empty = composer(engine)
for key: KeyInput in [.space, .shiftSpace, .enter, .shiftEnter, .backspace, .optionBackspace, .forwardDelete,
                      .left, .right, .shiftLeft, .shiftRight, .home, .end, .up, .down, .pageUp, .pageDown, .tab, .shiftTab, .escape, .char("?")] {
    check(empty.handle(key) == .passThrough, "empty pass-through \(key)")
}
for key in "6347" { check(empty.handle(.char(key)) == .consumed && !empty.isComposing, "empty tone ignored") }

// All keyboard symbols, all tone keys, slot replacement, canonical buffer order and tone rejection.
let keySymbols = Array(zip(Array("1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-"),
                           Array("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ")))
for (key, symbol) in keySymbols {
    let c = composer(engine, String(key))
    check(c.presentation.text == String(symbol), "keyboard \(key) = \(symbol)")
}
let replace = composer(engine, "1q8iujm")
check(replace.presentation.text == "ㄆㄩㄛ", "replace all three slots")
let ordered = composer(engine, "0u1")
check(ordered.presentation.text == "ㄅㄧㄢ", "slot order independent of key order")
_ = ordered.handle(.space)
check(ordered.presentation.text == "邊", "first tone completes")
for (tone, word) in [("6", "拔"), ("3", "把"), ("4", "爸"), ("7", "罷")] {
    let c = composer(engine, "18" + tone)
    check(c.presentation.text == word, "tone \(tone)")
}
let rejected = composer(engine, "q86")
check(rejected.presentation.text == "ㄆㄚ", "invalid tone retains buffer without tone")
_ = rejected.handle(.space)
check(rejected.presentation.text == "趴", "rejected tone can be corrected")
let invalid = composer(engine, "1u3")
check(invalid.presentation.text == "ㄅㄧ", "unknown syllable rejected")
_ = invalid.handle(.space)
check(invalid.presentation.text == "ㄅㄧ", "invalid first tone rejected")

// Syllables in any phrase position are valid, including neutral-tone variants with no single-character entry.
let phraseOnly = try Engine(lexiconText: "ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\t-2\n")
check(composer(phraseOnly, "vu,4vu,7").presentation.text == "謝謝", "phrase-only syllables complete")
check(composer(phraseOnly, "vu,4vu,4").presentation.text == "謝謝", "typed fourth tone matches neutral phrase ending")
let nonNeutral = try Engine(lexiconText: "ㄒㄧㄝˋ ㄒㄧㄝˋ\t謝謝\t-2\n")
check(composer(nonNeutral, "vu,7vu,7").presentation.text == "謝謝", "neutral syllables valid through non-neutral phrase only")
let neutralOnly = try Engine(lexiconText: "ㄅㄚ˙\t吧\t-2\n")
for keys in ["18 ", "186", "183", "184", "187"] {
    check(composer(neutralOnly, keys).presentation.text == "吧", "neutral lexicon accepts any typed tone: \(keys)")
}
let fourthOnly = try Engine(lexiconText: "ㄅㄚˋ\t爸\t-2\n")
check(composer(fourthOnly, "187").presentation.text == "爸", "typed neutral accepts another tone")
check(composer(fourthOnly, "183").presentation.text == "ㄅㄚ", "different non-neutral tones never match")

// Bracket each penalty, including its sum over multiple syllables, using competitors 0.01 on either side.
for (typed, source, keys, penalty) in [
    ("ㄅㄚ", "ㄅㄚ˙", "18 ", 0.4),
    ("ㄅㄚˊ", "ㄅㄚ˙", "186", 0.4),
    ("ㄅㄚˇ", "ㄅㄚ˙", "183", 0.4),
    ("ㄅㄚˋ", "ㄅㄚ˙", "184", 0.4),
    ("ㄅㄚ˙", "ㄅㄚ", "187", 1.2),
    ("ㄅㄚ˙", "ㄅㄚˊ", "187", 1.2),
    ("ㄅㄚ˙", "ㄅㄚˇ", "187", 1.2),
    ("ㄅㄚ˙", "ㄅㄚˋ", "187", 1.2),
    ("ㄅㄚ ㄆㄚ", "ㄅㄚ˙ ㄆㄚ˙", "18 q8 ", 0.8),
    ("ㄅㄚ˙ ㄆㄚ˙", "ㄅㄚ ㄆㄚ", "187q87", 2.4),
    ("ㄅㄚ ㄆㄚ˙", "ㄅㄚ˙ ㄆㄚˋ", "18 q87", 1.6),
] {
    let word = typed.contains(" ") ? "甲乙" : "甲"
    let rival = typed.contains(" ") ? "丙丁" : "丙"
    for delta in [-0.01, 0.01] {
        let e = try Engine(lexiconText: "\(source)\t\(word)\t-2\n\(typed)\t\(rival)\t\(-2 - penalty + delta)\n")
        let c = composer(e, keys)
        let expected = delta < 0 ? word : rival
        check(c.presentation.text == expected, "\(penalty) penalty: \(typed) vs \(source), delta \(delta)")
        _ = c.handle(.down)
        check(c.presentation.candidates?.items.prefix(2).map(\.text) == [expected, delta < 0 ? rival : word],
              "candidate ranking includes adjusted tolerant score")
    }
}
// A word keeps its likeliest reading: 甲 is nearly always ㄅㄚ˙, so typed as ㄅㄚ it scores as ㄅㄚ˙ less the
// neutral-tone tolerance, not by its rare first-tone reading.
let exactPreferred = try Engine(lexiconText: "ㄅㄚ\t甲\t-5\nㄅㄚ˙\t甲\t0\nㄅㄚ\t乙\t-4\n")
let exactChoice = composer(exactPreferred, "18 ")
check(exactChoice.presentation.text == "甲", "a word's likelier tolerant reading beats its rare exact one")
_ = exactChoice.handle(.down)
check(exactChoice.presentation.candidates?.items.map(\.text) == ["甲", "乙"], "candidates keep one entry per word")
let sameReading = try Engine(lexiconText: "ㄅㄚ\t甲\t-1\nㄅㄚ˙\t甲\t-1\nㄅㄚ\t乙\t-1.2\n")
check(composer(sameReading, "18 ").presentation.text == "甲", "equally likely readings: the exact one, without tolerance cost")
let tolerantPick = composer(phraseOnly, "vu,4vu,4")
_ = tolerantPick.handle(.down)
check(tolerantPick.presentation.candidates?.items.first?.text == "謝謝", "tolerant phrase offered at end cursor")
_ = tolerantPick.selectCandidate(at: 0)
_ = tolerantPick.handle(.left)
check(tolerantPick.presentation.text == "謝謝", "tolerant pick fixed through navigation")
check(tolerantPick.commitAll() == "謝謝", "tolerant pick commits")
check(phraseOnly.learnedEntries().contains { $0.reading == "ㄒㄧㄝˋ ㄒㄧㄝˋ" && $0.word == "謝謝" },
      "a committed tolerant pick is recalled under its typed reading")
check(!phraseOnly.learnedEntries().contains { $0.reading.contains("˙") }, "learning does not leak into lexicon reading")

let endWords = try Engine(lexiconText: "ㄏㄣˇ\t很\t-2\nㄏㄠˇ\t好\t-2\nㄏㄠˇ\t郝\t-3\nㄏㄣˇ ㄏㄠˇ\t很好\t-1\n")
let endCandidates = composer(endWords, "cp3cl3")
_ = endCandidates.handle(.down)
check(endCandidates.presentation.candidates?.items.map(\.text) == ["很好", "好", "郝"], "end cursor offers phrase before final syllable")

// Backspace edits one symbol/token; option-delete edits a converted word, including from its interior.
let deletion = composer(engine, "1u0")
for expected in ["ㄅㄧ", "ㄅ", ""] {
    _ = deletion.handle(.backspace)
    check(deletion.presentation.text == expected, "backspace one buffer symbol")
}
type("18 q8 a8 ", into: deletion)
check(deletion.presentation.text == "八趴媽", "Viterbi picks three-syllable word")
_ = deletion.handle(.backspace)
check(deletion.presentation.text == "八趴", "backspace one token, never whole word")
_ = deletion.handle(.backspace)
check(deletion.presentation.text == "八", "backspace retains earlier token")
let wordDelete = composer(engine, "18 q8 a8 ")
_ = wordDelete.handle(.left)
_ = wordDelete.handle(.optionBackspace)
check(!wordDelete.isComposing, "option-backspace removes clause containing token before cursor")
let bufferDelete = composer(engine, "18 q8")
_ = bufferDelete.handle(.optionBackspace)
check(bufferDelete.presentation.text == "八", "option-backspace only clears buffer")

// Navigation and insertion/deletion in the middle; UTF-16 clauses around an unfinished buffer.
let middle = composer(engine, "18 q8 ")
_ = middle.handle(.left)
check(middle.presentation.cursor == 1, "left one token inside word")
check(middle.presentation.targetClause.map { middle.presentation.clauses[$0] } == 1..<2,
      "with the cursor inside the composition, the character ↓ would change is its own clause")
type("a8", into: middle)
check(middle.presentation.text == "八ㄇㄚ趴" && middle.presentation.cursor == 3, "buffer at token cursor")
validPresentation(middle, "buffer inside a word")
_ = middle.handle(.left)
_ = middle.handle(.right)
check(middle.presentation.cursor == 3, "left/right ignored with buffer")
_ = middle.handle(.forwardDelete)
check(middle.presentation.text == "八ㄇㄚ趴", "forward-delete ignored with buffer")
_ = middle.handle(.space)
check(middle.presentation.text == "八媽趴" && middle.presentation.cursor == 2, "insert completed token in middle")
_ = middle.handle(.forwardDelete)
check(middle.presentation.text == "八媽", "forward-delete removes next token")
_ = middle.handle(.home)
check(middle.presentation.cursor == 0, "home")
check(middle.handle(.backspace) == .consumed && middle.presentation.text == "八媽", "backspace at start consumed")
_ = middle.handle(.left)
check(middle.presentation.cursor == 0, "left clamps")
_ = middle.handle(.end)
_ = middle.handle(.right)
check(middle.presentation.cursor == 2, "end/right clamp")
check(middle.handle(.forwardDelete) == .consumed, "forward-delete at end consumed")
let moveBuffer = composer(engine, "18 q8")
_ = moveBuffer.handle(.home)
check(moveBuffer.presentation.text == "ㄆㄚ八" && moveBuffer.presentation.cursor == 2, "home relocates unfinished buffer")
_ = moveBuffer.handle(.end)
check(moveBuffer.presentation.text == "八ㄆㄚ", "end relocates unfinished buffer")
for key: KeyInput in [.up, .pageUp, .pageDown, .tab, .shiftTab] {
    check(middle.handle(key) == .consumed, "composition consumes \(key)")
}

// Literal mappings begin composition, flush valid first tone, and discard invalid first tone.
let plainPunctuation: [(Character, String)] = [("[", "「"), ("]", "」"), ("'", "、"), ("\\", "＼"), ("=", "＝")]
let shiftedPunctuation: [(Character, String)] = [(",", "，"), (".", "。"), ("/", "？"), (";", "："), ("'", "；"),
    ("1", "！"), ("2", "＠"), ("3", "＃"), ("4", "＄"), ("5", "％"), ("6", "…"), ("7", "＆"),
    ("8", "＊"), ("9", "（"), ("0", "）"), ("-", "——"), ("=", "＋"), ("[", "『"), ("]", "』"), ("\\", "｜"), ("`", "～")]
for (shifted, table) in [(false, plainPunctuation), (true, shiftedPunctuation)] {
    for (key, text) in table {
        let c = composer(engine)
        check(c.handle(shifted ? .shiftedChar(key) : .char(key)) == .consumed && c.presentation.text == text, "literal mapping \(key), shift=\(shifted)")
        validPresentation(c, "literal")
    }
}
for key in "abcdefghijklmnopqrstuvwxyz" {
    let c = composer(engine)
    _ = c.handle(.shiftedChar(key))
    check(c.presentation.text == String(key).uppercased(), "uppercase literal \(key)")
}
let punctuation = composer(engine, "18")
_ = punctuation.handle(.shiftedChar(","))
check(punctuation.presentation.text == "八，", "punctuation completes buffer as first tone")
type("1u", into: punctuation)
_ = punctuation.handle(.shiftedChar("a"))
check(punctuation.presentation.text == "八，A", "uppercase drops invalid buffer")
type("q8 ", into: punctuation)
check(punctuation.presentation.text == "八，A趴", "words never cross literals")
_ = punctuation.handle(.shiftSpace)
check(punctuation.presentation.text == "八，A趴 ", "shift-space literal")
let literalDelete = composer(engine)
_ = literalDelete.handle(.shiftedChar("-"))
_ = literalDelete.handle(.backspace)
check(!literalDelete.isComposing, "multi-character literal is one token")

// Commit modes, escape options, reset without learning, max syllables.
for shifted in [false, true] {
    let c = composer(engine, "18 q8")
    let result = c.handle(shifted ? .shiftEnter : .enter)
    check(result == KeyResult(handled: !shifted, commit: "八ㄆㄚ") && !c.isComposing, "enter/shift-enter sends the unfinished syllable as typed")
    type("18 1u", into: c)
    check(c.commitAll() == "八ㄅㄧ", "commit sends the unfinished symbols, never a guess")
}
var commitSpace = ComposerOptions(); commitSpace.spaceOpensCandidates = false
let spaced = composer(engine, "18 ", options: commitSpace)
check(spaced.handle(.space).commit == "八" && !spaced.isComposing, "space option commits")
let escape = composer(engine, "18 q8")
_ = escape.handle(.escape)
check(escape.presentation.text == "八", "escape clears only buffer first")
_ = escape.handle(.escape)
check(!escape.isComposing, "escape clears composition")
var noEscape = ComposerOptions(); noEscape.escapeClearsComposition = false
let kept = composer(engine, "18 ", options: noEscape)
_ = kept.handle(.escape)
check(kept.presentation.text == "八", "escape option retains composition")
var limitedOptions = legacyOptions; limitedOptions.maxSyllables = 2
let limited = composer(engine, "18 q8 a8", options: limitedOptions)
check(limited.presentation.text == "八趴", "maxSyllables refuses new symbols")
check(limited.handle(.char("6")) == .consumed && limited.presentation.text == "八趴", "tone at limit empty ignored")
_ = limited.handle(.shiftedChar("a"))
check(limited.presentation.text == "八趴A", "literal allowed at limit")
_ = limited.handle(.backspace); _ = limited.handle(.backspace)
type("a8 ", into: limited)
check(limited.presentation.text == "八媽", "deleting a syllable restores capacity")

// Candidate ordering, selection keys, paging, clamping and normal processing after dismissal.
var twoKeys = legacyOptions; twoKeys.selectionKeys = Array("as")
let candidates = composer(engine, "18 q8 ", options: twoKeys)
_ = candidates.handle(.space)
check(candidates.presentation.candidates?.items.filter { $0.text != "ㄆㄚ" }.prefix(2).map(\.text) == ["八趴", "巴趴"], "current first, longer spans before singles")
check(candidates.presentation.candidates?.items.last?.text == "趴", "short span after long spans")
check(candidates.presentation.focusedClause == 0, "focused converted word")
validPresentation(candidates, "candidate window")
_ = candidates.selectCandidate(at: candidates.presentation.candidates!.items.firstIndex { $0.text == "巴趴" }!)
check(candidates.presentation.text == "巴趴" && candidates.presentation.candidates == nil && candidates.presentation.cursor == 2, "selection key picks and fixes, cursor unchanged")
_ = candidates.handle(.space)
check(candidates.presentation.candidates?.items.first?.text == "巴趴", "fixed current first despite lower score")
_ = candidates.handle(.escape)
check(candidates.presentation.text == "巴趴", "escape closes without changing")
_ = candidates.handle(.space); _ = candidates.handle(.backspace)
check(candidates.presentation.text == "巴趴" && candidates.presentation.candidates == nil, "backspace only closes window")
let paging = composer(engine, "18 ", options: twoKeys)
_ = paging.handle(.down)
check(paging.presentation.candidates?.pageSize == 2, "page size from selection keys")
_ = paging.handle(.up)
check(paging.presentation.candidates?.selection == 0, "up clamps at start")
_ = paging.handle(.down); _ = paging.handle(.down)
check(paging.presentation.candidates?.selection == 2 && paging.presentation.candidates?.page == 1, "down crosses page")
_ = paging.handle(.pageUp)
check(paging.presentation.candidates?.page == 0, "page up previous page")
for key: KeyInput in [.pageDown, .tab] {
    let previous = paging.presentation.candidates!.page
    _ = paging.handle(key)
    check(paging.presentation.candidates!.page == previous + 1, "next page \(key)")
}
for key: KeyInput in [.pageUp, .shiftTab] {
    let previous = paging.presentation.candidates!.page
    _ = paging.handle(key)
    check(paging.presentation.candidates!.page == previous - 1, "previous page \(key)")
}
for _ in 0..<30 { _ = paging.handle(.down) }
check(paging.presentation.candidates!.selection == paging.presentation.candidates!.items.count - 1, "down clamps at final item")
_ = paging.handle(.space)
check(paging.presentation.candidates!.page == 0, "space wraps to first page")
_ = paging.handle(.right)
let highlighted = paging.presentation.candidates!.items[paging.presentation.candidates!.selection].text
_ = paging.handle(.enter)
check(paging.presentation.text == highlighted && paging.presentation.candidates == nil, "enter picks highlighted")
engine.forgetAllLearned()
let dismiss = composer(engine, "18 ", options: twoKeys)
_ = dismiss.handle(.space); _ = dismiss.handle(.char("q"))
check(dismiss.presentation.text == "八ㄆ" && dismiss.presentation.candidates == nil, "symbol dismisses then processes")
_ = dismiss.handle(.escape); _ = dismiss.handle(.space); _ = dismiss.handle(.char("6"))
check(dismiss.presentation.text == "八" && dismiss.presentation.candidates == nil, "non-selection tone dismisses then processes")
_ = dismiss.handle(.space); _ = dismiss.handle(.shiftedChar(","))
check(dismiss.presentation.text == "八，" && dismiss.presentation.candidates == nil, "punctuation dismisses then processes")
let downBuffer = composer(engine, "18")
_ = downBuffer.handle(.down)
check(downBuffer.presentation.text == "八" && downBuffer.presentation.candidates != nil, "down completes valid buffer")
let atStart = composer(engine, "18 q8 a8 ")
_ = atStart.handle(.home); _ = atStart.handle(.down)
check(atStart.presentation.candidates!.items.first?.text == "八趴媽", "cursor zero candidates start at zero")
check(atStart.presentation.candidates!.items.contains { $0.text == "八趴" }, "cursor zero offers shorter spans")
let punctCandidates = composer(engine)
_ = punctCandidates.handle(.shiftedChar(",")); _ = punctCandidates.handle(.down)
check(punctCandidates.presentation.candidates!.items.prefix(3).map(\.text) == ["，", ",", "、"], "punctuation alternatives")
_ = punctCandidates.selectCandidate(at: 1)
check(punctCandidates.presentation.text == "," && punctCandidates.presentation.cursor == 1, "literal candidate pick")
check(punctCandidates.selectCandidate(at: -1) == .consumed, "invalid click safe")
let bracket = composer(engine, "[")
_ = bracket.handle(.down)
check(bracket.presentation.candidates!.items.map(\.text) == ["「", "『", "（", "〈", "《", "【"], "opening bracket alternatives")
var threeKeys = ComposerOptions(); threeKeys.selectionKeys = Array("123"); threeKeys.learningEnabled = false
// A vertical list: ←→ move to the neighbouring character, the list kept open in the same mode.
let moving = composer(engine, "18 q8 ", options: threeKeys)
_ = moving.handle(.home); _ = moving.handle(.shiftDown)
check(moving.presentation.candidates?.singleCharacter == true && focusedRange(moving) == 0..<1, "single list at the first character")
_ = moving.handle(.right)
check(focusedRange(moving) == 1..<2 && moving.presentation.candidates?.singleCharacter == true, "→ moves the list to the next character")
_ = moving.handle(.right)
check(focusedRange(moving) == 1..<2, "→ stops at the last character")
_ = moving.handle(.left)
check(focusedRange(moving) == 0..<1, "← moves the list to the previous character")
// A horizontal list: ←→ move the highlight, ↑↓ turn pages.
var horizontal = threeKeys; horizontal.verticalList = false
let sideways = composer(engine, "18 ", options: horizontal)
_ = sideways.handle(.down)
_ = sideways.handle(.right)
check(sideways.presentation.candidates?.selection == 1, "horizontal → moves the highlight")
_ = sideways.handle(.down)
check(sideways.presentation.candidates?.page == 1, "horizontal ↓ turns the page")
_ = sideways.handle(.up)
check(sideways.presentation.candidates?.page == 0, "horizontal ↑ turns back")
let pagePick = composer(engine, "18 ", options: threeKeys)
_ = pagePick.handle(.down); _ = pagePick.handle(.tab)
let pageWord = pagePick.presentation.candidates!.items[4].text
_ = pagePick.handle(.char("2"))
check(pagePick.presentation.text == pageWord, "selection key is relative to current page")
let numericTone = composer(engine, "18 ", options: threeKeys)
_ = numericTone.handle(.down)
let thirdWord = numericTone.presentation.candidates!.items[2].text
_ = numericTone.handle(.char("3"))
check(numericTone.presentation.text == thirdWord && numericTone.presentation.candidates == nil, "selection key takes precedence over tone")
let dedupeEngine = try Engine(lexiconText: tiny + "\nㄅㄚ\t八\t-2\n")
let deduped = composer(dedupeEngine, "18 ")
_ = deduped.handle(.down)
check(deduped.presentation.candidates!.items.filter { $0.text == "八" }.count == 1, "candidate text deduplicated")
let interiorEngine = try Engine(lexiconText: tiny + "\nㄅㄚ ㄆㄚ\t巴趴\t-0.1\n")
let interior = composer(interiorEngine, "18 q8 a8 ")
_ = interior.handle(.left); _ = interior.handle(.down)
check(interior.presentation.candidates!.items.first?.text == "媽", "after-cursor candidates start inside a longer word")
interior.options.candidateAnchor = .beforeCursor
_ = interior.handle(.down)
check(interior.presentation.candidates!.items.first?.text == "八趴", "before-cursor shown text first inside a longer word")
let sixEngine = try Engine(lexiconText: tiny + "\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t八八八八八八\t-0.1\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t巴巴巴巴巴巴巴\t0\n")
let six = composer(sixEngine, String(repeating: "18 ", count: 7))
check(six.presentation.clauses.count == 2 && !six.presentation.text.contains("巴"), "word spans capped at six syllables")
_ = six.handle(.down)
_ = six.selectCandidate(at: six.presentation.candidates!.items.firstIndex { $0.text == "八" }!)
_ = six.handle(.down)
check(six.presentation.candidates!.items.first?.text == "八八八八八八", "longest span precedes current single word")
check(six.presentation.candidates!.items.last?.text.count == 1, "single-syllable spans come after all longer spans")

// Hard spans survive edits before/after them and are discarded on overlapping edits.
func fixedPair(_ e: Engine) -> Composer {
    let c = composer(e, "18 q8 ")
    _ = c.handle(.down)
    let index = c.presentation.candidates!.items.firstIndex { $0.text == "巴趴" }!
    _ = c.selectCandidate(at: index)
    return c
}
let fixed = fixedPair(engine)
type("a8 ", into: fixed)
check(fixed.presentation.text == "巴趴媽", "hard span beats higher-scoring longer word")
_ = fixed.handle(.backspace)
check(fixed.presentation.text == "巴趴", "delete after fixed span preserves it")
_ = fixed.handle(.home); type("a8 ", into: fixed)
check(fixed.presentation.text == "媽巴趴", "insert before fixed span shifts it")
_ = fixed.handle(.backspace)
check(fixed.presentation.text == "巴趴", "delete before fixed span shifts it")
_ = fixed.handle(.right); type("a8 ", into: fixed)
check(fixed.presentation.text == "八媽趴", "insert inside fixed span drops constraint")
let overlap = fixedPair(engine)
_ = overlap.handle(.backspace)
check(overlap.presentation.text == "八", "deleting fixed token removes constraint")

// Learning happens on commit; an identical composition recalls the correction; reset does not learn.
let learnedEngine = try Engine(lexiconText: tiny)
let learnedComposer = composer(learnedEngine, "18 ")
_ = learnedComposer.handle(.down)
_ = learnedComposer.selectCandidate(at: learnedComposer.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
check(learnedEngine.learnedEntries().isEmpty, "a pick alone does not learn")
check(learnedComposer.commitAll() == "巴", "picked word committed")
check(learnedEngine.learnedEntries().contains { $0.reading == "ㄅㄚ" && $0.word == "巴" }, "commit learns the correction")
check(composer(learnedEngine, "18 ").presentation.text == "巴", "learning boosts next conversion")
let beforeReset = learnedEngine.learnedEntries().first!.count
let resetter = composer(learnedEngine, "18 ")
resetter.reset()
check(learnedEngine.learnedEntries().first!.count == beforeReset && !resetter.isComposing, "reset never learns")
learnedEngine.forgetLearned(word: "巴", reading: "ㄅㄚ")
check(learnedEngine.learnedEntries().isEmpty && composer(learnedEngine, "18 ").presentation.text == "八", "forget removes boost")
var noLearning = ComposerOptions(); noLearning.learningEnabled = false
let disabled = composer(learnedEngine, "18 ", options: noLearning)
_ = disabled.handle(.down); _ = disabled.selectCandidate(at: 1); _ = disabled.commitAll()
check(learnedEngine.learnedEntries().isEmpty, "learning option respected for picks and commits")
_ = fixedPair(learnedEngine)
learnedEngine.forgetAllLearned()
check(learnedEngine.learnedEntries().isEmpty, "forget all")

// User phrases, fallback readings, malformed persistence and asynchronous atomic persistence round-trip.
let personal = try Engine(lexiconText: tiny)
check(personal.readings(of: "八") == ["ㄅㄚ"], "known reading")
check(personal.readings(of: "媽八") == ["ㄇㄚ ㄅㄚ"], "unknown phrase combines most likely character readings")
check(personal.readings(of: "龘").isEmpty && personal.readings(of: "").isEmpty, "unknown or empty word")
try personal.addUserPhrase(word: "新趴", reading: "ㄅㄚ ㄆㄚ")
check(personal.userPhrases().count == 1 && personal.readings(of: "新趴") == ["ㄅㄚ ㄆㄚ"], "add personal phrase")
// The phrase floor is -6; use an independent low-probability lexicon to test its promised advantage.
let phraseEngine = try Engine(lexiconText: "ㄅㄚ\t八\t-9\nㄅㄚ\t巴\t-10\n")
try phraseEngine.addUserPhrase(word: "巴", reading: "ㄅㄚ")
check(composer(phraseEngine, "18 ").presentation.text == "巴", "personal phrase floor and +1")
personal.removeUserPhrase(word: "新趴", reading: "ㄅㄚ ㄆㄚ")
check(personal.userPhrases().isEmpty, "remove personal phrase")
let phraseUpdates = try Engine(lexiconText: "ㄅㄚ\t八\t-9\n")
check(composer(phraseUpdates, "18 ").presentation.text == "八", "warm lookup before phrase insertion")
try phraseUpdates.addUserPhrase(word: "吧", reading: "ㄅㄚ˙")
check(composer(phraseUpdates, "18 ").presentation.text == "吧", "new neutral phrase invalidates tolerant lookup cache")
phraseUpdates.removeUserPhrase(word: "吧", reading: "ㄅㄚ˙")
check(composer(phraseUpdates, "18 ").presentation.text == "八", "removed neutral phrase invalidates tolerant lookup cache")
let newSyllables = try Engine(lexiconText: "")
try newSyllables.addUserPhrase(word: "謝謝", reading: "ㄒㄧㄝˋ ㄒㄧㄝ˙")
check(composer(newSyllables, "vu,4vu,4").presentation.text == "謝謝", "new personal phrase supplies all syllables and tolerant index")
newSyllables.removeUserPhrase(word: "謝謝", reading: "ㄒㄧㄝˋ ㄒㄧㄝ˙")
check(composer(newSyllables, "vu,4").presentation.text == "ㄒㄧㄝ", "removed personal-only syllable no longer accepted")
for (word, reading) in [("", "ㄅㄚ"), ("八", ""), ("八", "foo"), ("八趴", "ㄅㄚ"), ("八\n", "ㄅㄚ"), ("八", "ㄅㄚˊˋ"), ("八", "ㄅㄆㄚ"), ("八", "ㄚㄅ")] {
    do { try personal.addUserPhrase(word: word, reading: reading); check(false, "reject invalid phrase \(word)/\(reading)") }
    catch { check(true, "invalid phrase rejected") }
}
let temp = FileManager.default.temporaryDirectory.appendingPathComponent("ZhuyinCoreChecks-\(UUID().uuidString)")
try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temp) }
try Data("bad\nㄅㄚ\t八\tNaN\t0\nㄅㄚ\t巴\t1\tNaN\n".utf8).write(to: temp.appendingPathComponent("learned.tsv"))
try Data("bad\nnot-reading\t八\n".utf8).write(to: temp.appendingPathComponent("user_phrases.tsv"))
let persisted = try Engine(lexiconText: tiny, userDataDirectory: temp)
check(persisted.learnedEntries().isEmpty && persisted.userPhrases().isEmpty, "bad disk rows ignored")
let neutralDirectory = temp.appendingPathComponent("neutral")
try FileManager.default.createDirectory(at: neutralDirectory, withIntermediateDirectories: true)
try Data("ㄒㄧㄝˋ ㄒㄧㄝ˙\t謝謝\n".utf8).write(to: neutralDirectory.appendingPathComponent("user_phrases.tsv"))
let loadedNeutral = try Engine(lexiconText: "", userDataDirectory: neutralDirectory)
check(composer(loadedNeutral, "vu,4vu,7").presentation.text == "謝謝", "loaded phrase supplies internal syllables")
check(composer(loadedNeutral, "vu,4vu,4").presentation.text == "謝謝", "loaded phrase supplies tolerant reading index")
try persisted.addUserPhrase(word: "媽八", reading: "ㄇㄚ ㄅㄚ")
let saving = composer(persisted, "18 ")
_ = saving.handle(.down)
_ = saving.selectCandidate(at: saving.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
_ = saving.commitAll()
var roundTrip: Engine?
let deadline = Date().addingTimeInterval(3)
while Date() < deadline {
    let loaded = try Engine(lexiconText: tiny, userDataDirectory: temp)
    if loaded.learnedEntries().contains(where: { $0.word == "巴" }) && loaded.userPhrases().count == 1 { roundTrip = loaded; break }
    Thread.sleep(forTimeInterval: 0.02)
}
check(roundTrip != nil, "debounced persistence round-trip")
check(roundTrip.map { composer($0, "18 ").presentation.text == "巴" } ?? false, "persisted boost used")
persisted.forgetAllLearned()
persisted.removeUserPhrase(word: "媽八", reading: "ㄇㄚ ㄅㄚ")
let forgetDeadline = Date().addingTimeInterval(3)
var forgotOnDisk = false
while Date() < forgetDeadline {
    let loaded = try Engine(lexiconText: tiny, userDataDirectory: temp)
    if loaded.learnedEntries().isEmpty && loaded.userPhrases().isEmpty { forgotOnDisk = true; break }
    Thread.sleep(forTimeInterval: 0.02)
}
check(forgotOnDisk, "forget and remove persist")
let removedPhraseEngine = try Engine(lexiconText: tiny)
try removedPhraseEngine.addUserPhrase(word: "餌", reading: "ㄦ")
let removedPhrase = composer(removedPhraseEngine, "- ")
_ = removedPhrase.handle(.down); _ = removedPhrase.selectCandidate(at: 0)
removedPhraseEngine.removeUserPhrase(word: "餌", reading: "ㄦ")
_ = removedPhrase.handle(.left)
check(removedPhrase.presentation.text == "餌", "fixed word survives phrase removal")
check(removedPhrase.commitAll() == "餌", "removed fixed phrase remains committable")

let decayDirectory = temp.appendingPathComponent("decay")
try FileManager.default.createDirectory(at: decayDirectory, withIntermediateDirectories: true)
let now = Date().timeIntervalSince1970
let decayLexicon = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-3\n"
try Data("ㄅㄚ\t巴\t1\t\(now - 90 * 86400)\n".utf8).write(to: decayDirectory.appendingPathComponent("learned.tsv"))
let oldLearning = try Engine(lexiconText: decayLexicon, userDataDirectory: decayDirectory)
check(composer(oldLearning, "18 ").presentation.text == "八", "90-day half-life reduces boost below 1.0")
try Data("ㄅㄚ\t巴\t1\t\(now)\n".utf8).write(to: decayDirectory.appendingPathComponent("learned.tsv"))
let freshLearning = try Engine(lexiconText: decayLexicon, userDataDirectory: decayDirectory)
check(composer(freshLearning, "18 ").presentation.text == "巴", "fresh count 1 gives 2 ln(2) boost")

let capacityDirectory = temp.appendingPathComponent("capacity")
try FileManager.default.createDirectory(at: capacityDirectory, withIntermediateDirectories: true)
let capacityRows = (0..<20002).map { "ㄅㄚ\t\(UnicodeScalar(0x4e00 + $0)!)\t\($0 + 1)\t\(now)\n" }.joined()
try Data(capacityRows.utf8).write(to: capacityDirectory.appendingPathComponent("learned.tsv"))
let capacity = try Engine(lexiconText: tiny, userDataDirectory: capacityDirectory)
check(capacity.learnedEntries().count == 20000, "learning capped at 20000")
check(!capacity.learnedEntries().contains { $0.word == "一" || $0.word == "丁" }, "cap drops lowest boosts")

// Bigram state must retain different previous words, use backoff, and account for sentence boundaries.
let lmDirectory = temp.appendingPathComponent("lm")
try FileManager.default.createDirectory(at: lmDirectory, withIntermediateDirectories: true)
try Data("八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n".utf8).write(to: lmDirectory.appendingPathComponent("unigram.tsv"))
try Data("<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n趴\t</s>\t-0.1\n".utf8).write(to: lmDirectory.appendingPathComponent("bigram.tsv"))
try Data("八\t-0.4\n".utf8).write(to: lmDirectory.appendingPathComponent("backoff.tsv"))
let lm = try BigramLanguageModel(directory: lmDirectory)
check(abs(lm.score(word: "巴", after: "八", unigram: -9) + 2.7) < 0.00001, "bigram backoff = log alpha + unigram")
check(lm.score(word: "趴", after: "巴", unigram: -9) == -0.1, "seen bigram")
let lmLexicon = "ㄅㄚ\t八\t-2\nㄅㄚ\t巴\t-2.3\nㄆㄚ\t趴\t-2\n"
let lmEngine = try Engine(lexiconText: lmLexicon, languageModelDirectory: lmDirectory)
check(composer(lmEngine, "18 q8 ").presentation.text == "巴趴", "Viterbi keeps last-word state")
let neutralLM = try Engine(lexiconText: "ㄅㄚ\t八\t-2\nㄅㄚ˙\t巴\t-9\n", languageModelDirectory: lmDirectory)
let neutralLMChoice = composer(neutralLM, "187")
check(neutralLMChoice.presentation.text == "巴", "neutral penalty is subtracted after bigram scoring")
_ = neutralLMChoice.handle(.down)
check(neutralLMChoice.presentation.candidates?.items.map(\.text) == ["巴", "八"], "candidate scoring retains penalty with bigram model")
let lexiconURL = temp.appendingPathComponent("lexicon.tsv")
try Data(lmLexicon.utf8).write(to: lexiconURL)
let discoveredLM = try Engine(lexiconURL: lexiconURL, userDataDirectory: nil)
let disabledLM = try Engine(lexiconURL: lexiconURL, userDataDirectory: nil, languageModelDirectory: nil)
check(composer(discoveredLM, "18 q8 ").presentation.text == "巴趴", "adjacent lm discovered")
check(composer(disabledLM, "18 q8 ").presentation.text == "八趴", "explicit nil disables LM")
try Data("<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t</s>\t-10\n巴\t</s>\t-0.1\n".utf8).write(to: lmDirectory.appendingPathComponent("bigram.tsv"))
let boundaryLM = try Engine(lexiconText: lmLexicon, languageModelDirectory: lmDirectory)
check(composer(boundaryLM, "18 ").presentation.text == "巴", "end boundary participates in best path")
try FileManager.default.removeItem(at: lmDirectory.appendingPathComponent("backoff.tsv"))
let incompleteLM = try Engine(lexiconText: lmLexicon, languageModelDirectory: lmDirectory)
check(composer(incompleteLM, "18 ").presentation.text == "八", "incomplete optional LM safely falls back to unigram")

// A byte-for-byte fixture, independently written in Swift, exercises every FORMAT.md backoff branch.
let binaryDirectory = temp.appendingPathComponent("binary")
try FileManager.default.createDirectory(at: binaryDirectory, withIntermediateDirectories: true)
let binaryURL = binaryDirectory.appendingPathComponent("lm.bin")
let fixtureUnigrams: [String: Float] = ["</s>": -1, "<unk>": -8, "八": -2, "巴": -2.3,
                                       "趴": -2, "媽": -3, "麻": -3, "𠮷": -4]
let fixtureBigrams: [LMBigram] = [
    .init(first: "<s>", second: "八", logp: -0.1), .init(first: "<s>", second: "巴", logp: -0.5),
    .init(first: "八", second: "趴", logp: -0.2, backoff: -0.7),
    .init(first: "巴", second: "趴", logp: -0.2, backoff: -0.8),
    .init(first: "趴", second: "媽", logp: -0.1), .init(first: "趴", second: "麻", logp: -1),
    .init(first: "媽", second: "</s>", logp: -0.1), .init(first: "麻", second: "</s>", logp: -0.1),
    .init(first: "<unk>", second: "趴", logp: -0.05),
]
let binaryFixture = lmFixture(order: 3, unigrams: fixtureUnigrams, backoffs: ["八": -0.4, "趴": -0.2],
    bigrams: fixtureBigrams, trigrams: [
        .init(first: "八", second: "趴", third: "媽", logp: -4),
        .init(first: "八", second: "趴", third: "麻", logp: -0.05),
        .init(first: "巴", second: "趴", third: "媽", logp: -0.01),
    ])
try binaryFixture.write(to: binaryURL)
let binary = try NGramLanguageModel(directory: binaryDirectory)
func binaryScore(_ word: String, _ older: String?, _ previous: String) -> Double {
    binary.score(wordID: binary.wordID(for: word),
                 history: .init(older: older.flatMap { binary.wordID(for: $0) }, previous: binary.wordID(for: previous)!),
                 unigram: -9)
}
for (word, older, previous, expected) in [
    ("媽", "八", "趴", -4.0), ("麻", "巴", "趴", -1.8), ("巴", "八", "趴", -3.2),
    ("媽", "媽", "趴", -0.1), ("巴", "媽", "八", -2.7),
] {
    check(abs(binaryScore(word, older, previous) - expected) < 0.00001, "binary backoff \(older) \(previous) → \(word)")
}
check(abs(binaryScore("八", nil, "<s>") + 0.1) < 0.00001, "binary sentence start without older history")
check(abs(binaryScore("巴", nil, "八") + 2.7) < 0.00001, "binary one-word backoff")
check(abs(binaryScore("</s>", "八", "趴") + 1.9) < 0.00001, "binary end uses both backoffs")
check(binary.wordID(for: "</s>") == 0 && binary.wordID(for: "<s>") == 1 && binary.wordID(for: "<unk>") == 2,
      "binary reserved word IDs")
check(binary.wordID(for: "𠮷") != nil && binary.wordID(for: "不存在") == nil && binary.wordID(for: "") == nil,
      "binary UTF-8 vocabulary search and missing words")
check(binary.score(wordID: 2, history: .init(), unigram: -9) == -8, "explicit unk uses model unigram")
check(binary.score(wordID: nil, history: .init(), unigram: -9) == -9, "missing metadata defaults unknown shift to zero")
try Data("{\"unknown_word_shift\":-2.3}".utf8).write(to: binaryDirectory.appendingPathComponent("lm.json"))
let shiftedBinary = try NGramLanguageModel(directory: binaryDirectory)
check(abs(shiftedBinary.score(wordID: nil, history: .init(), unigram: -9) + 11.3) < 0.00001,
      "unknown lexicon probability plus metadata shift")
check(shiftedBinary.score(wordID: UInt32.max, history: .init(previous: UInt32.max), unigram: -9).isFinite,
      "invalid external IDs cannot access outside the mapping")
check(abs(shiftedBinary.score(wordID: shiftedBinary.wordID(for: "趴"), history: .init(previous: 2), unigram: -9) + 0.05) < 0.00001,
      "unknown history scores as the explicit unk context")
let shiftedCompetition = "ㄅㄚ\t八\t-2\nㄅㄚ\t新\t-0.05\n"
let shiftedChoice = try Engine(lexiconText: shiftedCompetition, languageModelDirectory: binaryDirectory)
check(composer(shiftedChoice, "18 ").presentation.text == "八", "unknown calibration participates in conversion")
try FileManager.default.removeItem(at: binaryDirectory.appendingPathComponent("lm.json"))
let unshiftedChoice = try Engine(lexiconText: shiftedCompetition, languageModelDirectory: binaryDirectory)
check(composer(unshiftedChoice, "18 ").presentation.text == "新", "missing calibration keeps lexicon probability")
try Data("{\"unknown_word_shift\":-2.3}".utf8).write(to: binaryDirectory.appendingPathComponent("lm.json"))
let binaryLexicon = lmLexicon + "ㄇㄚ\t媽\t-3\nㄇㄚ\t麻\t-3\n"
let trigramEngine = try Engine(lexiconText: binaryLexicon, languageModelDirectory: binaryDirectory)
check(composer(trigramEngine, "18 q8 a8 ").presentation.text == "八趴麻", "trigram changes best conversion")
var fixedOptions = ComposerOptions(); fixedOptions.learningEnabled = false
let fixedHistory = composer(trigramEngine, "18 ", options: fixedOptions)
_ = fixedHistory.handle(.down)
_ = fixedHistory.selectCandidate(at: fixedHistory.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
type("q8 a8 ", into: fixedHistory)
check(fixedHistory.presentation.text == "巴趴媽", "user-picked span retains its word in trigram history")
for literal: KeyInput in [.shiftedChar(","), .shiftedChar("a")] {
    let resetHistory = composer(trigramEngine, "18 q8 ")
    _ = resetHistory.handle(literal)
    type("a8 ", into: resetHistory)
    check(resetHistory.presentation.text.hasSuffix("媽"), "literal resets both history words to sentence start")
}
let oovEngine = try Engine(lexiconText: "ㄅㄚ\t新\t-0.2\nㄆㄚ\t趴\t-2\n", languageModelDirectory: binaryDirectory)
check(composer(oovEngine, "18 q8 ").presentation.text == "新趴", "OOV word becomes unk context for following word")
try oovEngine.addUserPhrase(word: "新", reading: "ㄅㄚ")
check(composer(oovEngine, "18 q8 ").presentation.text == "新趴", "new user phrase resolves its ID before decoding")
let binaryPersonal = temp.appendingPathComponent("binary-personal")
try FileManager.default.createDirectory(at: binaryPersonal, withIntermediateDirectories: true)
try Data("ㄅㄚ\t巴\n".utf8).write(to: binaryPersonal.appendingPathComponent("user_phrases.tsv"))
let loadedBinaryPhrase = try Engine(lexiconText: "ㄆㄚ\t趴\t-2\nㄇㄚ\t媽\t-3\nㄇㄚ\t麻\t-3\n",
                                   userDataDirectory: binaryPersonal, languageModelDirectory: binaryDirectory)
check(composer(loadedBinaryPhrase, "18 q8 a8 ").presentation.text == "巴趴媽", "persisted phrase has its model ID")
let orderTwoFixture = lmFixture(order: 2, unigrams: fixtureUnigrams, backoffs: ["八": -0.4, "趴": -0.2], bigrams: fixtureBigrams)
try orderTwoFixture.write(to: binaryURL, options: .atomic)
let orderTwo = try NGramLanguageModel(directory: binaryDirectory)
check(abs(orderTwo.score(wordID: orderTwo.wordID(for: "媽"),
    history: .init(older: orderTwo.wordID(for: "八"), previous: orderTwo.wordID(for: "趴")!), unigram: -9) + 0.1) < 0.00001,
      "order two ignores older history")
let binaryBigramEngine = try Engine(lexiconText: binaryLexicon, languageModelDirectory: binaryDirectory)
check(composer(binaryBigramEngine, "18 q8 a8 ").presentation.text == "八趴媽", "bigram alone prefers the other conversion")
try lmFixture(order: 3, unigrams: [:], bigrams: []).write(to: binaryURL, options: .atomic)
check((try? NGramLanguageModel(directory: binaryDirectory)) != nil, "empty bigram/trigram sections are valid")

// Every malformed header/section must throw, then Engine must discover TSV or unigram instead.
func replacing32(_ data: Data, _ offset: Int, _ value: UInt32) -> Data {
    var result = data, little = value.littleEndian
    withUnsafeBytes(of: &little) { result.replaceSubrange(offset..<(offset + 4), with: $0) }
    return result
}
let vocabOffset = binaryFixture.withUnsafeBytes { Int(UInt64(littleEndian: $0.loadUnaligned(fromByteOffset: 24, as: UInt64.self))) }
let biOffset = binaryFixture.withUnsafeBytes { Int(UInt64(littleEndian: $0.loadUnaligned(fromByteOffset: 40, as: UInt64.self))) }
let triOffset = binaryFixture.withUnsafeBytes { Int(UInt64(littleEndian: $0.loadUnaligned(fromByteOffset: 48, as: UInt64.self))) }
var corruptFixtures = [Data(), Data(binaryFixture.prefix(63)), Data(binaryFixture.dropLast()),
    replacing32(binaryFixture, 0, 0), replacing32(binaryFixture, 4, 2), replacing32(binaryFixture, 8, 4),
    replacing32(binaryFixture, 12, UInt32.max), replacing32(binaryFixture, 16, UInt32.max),
    replacing32(binaryFixture, 20, UInt32.max), replacing32(binaryFixture, 24, 65),
    replacing32(binaryFixture, 32, 64), replacing32(binaryFixture, 40, UInt32.max),
    replacing32(binaryFixture, 48, 0), replacing32(binaryFixture, 56, 64),
    replacing32(binaryFixture, vocabOffset, 1), replacing32(binaryFixture, vocabOffset + 8, UInt32.max),
    replacing32(binaryFixture, biOffset, 1), replacing32(binaryFixture, biOffset + 8, UInt32.max),
    replacing32(binaryFixture, triOffset, 1), replacing32(binaryFixture, triOffset + 8, UInt32.max),
    replacing32(orderTwoFixture, 20, 1), replacing32(orderTwoFixture, 48, 64)]
for (index, fixture) in corruptFixtures.enumerated() {
    try fixture.write(to: binaryURL, options: .atomic)
    check((try? NGramLanguageModel(directory: binaryDirectory)) == nil, "reject corrupt/truncated binary fixture \(index)")
    let fallback = try Engine(lexiconText: lmLexicon, languageModelDirectory: binaryDirectory)
    check(composer(fallback, "18 q8 ").presentation.text == "八趴", "corrupt binary falls back without crash \(index)")
}
for size in 0..<binaryFixture.count {
    try binaryFixture.prefix(size).write(to: binaryURL, options: .atomic)
    check((try? NGramLanguageModel(directory: binaryDirectory)) == nil, "reject every truncated binary prefix \(size)")
}
// Supply a TSV alongside the binary to prove discovery preference and fallback independently.
try Data("八\t-2\n巴\t-2.3\n趴\t-2\n</s>\t-1\n".utf8).write(to: binaryDirectory.appendingPathComponent("unigram.tsv"))
try Data("<s>\t八\t-0.1\n<s>\t巴\t-0.5\n八\t趴\t-10\n巴\t趴\t-0.1\n趴\t</s>\t-0.1\n".utf8)
    .write(to: binaryDirectory.appendingPathComponent("bigram.tsv"))
try Data().write(to: binaryDirectory.appendingPathComponent("backoff.tsv"))
let binaryFallback = try Engine(lexiconText: lmLexicon, languageModelDirectory: binaryDirectory)
check(composer(binaryFallback, "18 q8 ").presentation.text == "巴趴", "corrupt binary falls back to complete TSV model")
try binaryFixture.write(to: binaryURL, options: .atomic)
let binaryPreferred = try Engine(lexiconText: binaryLexicon, languageModelDirectory: binaryDirectory)
check(composer(binaryPreferred, "18 q8 a8 ").presentation.text == "八趴麻", "binary model takes precedence over TSV")

// Compare the new beam against the original last-word recurrence, including multi-syllable paths and ties.
let equivalenceLexicon = tiny + "\nㄇㄚ\t麻\t-2\n"
// Keep using the TSV model's exact Double probabilities for this comparison.
let equivalenceDirectory = temp.appendingPathComponent("equivalence")
try FileManager.default.createDirectory(at: equivalenceDirectory, withIntermediateDirectories: true)
for name in ["unigram.tsv", "bigram.tsv", "backoff.tsv"] {
    try FileManager.default.copyItem(at: binaryDirectory.appendingPathComponent(name), to: equivalenceDirectory.appendingPathComponent(name))
}
let equivalenceModel = try BigramLanguageModel(directory: equivalenceDirectory)
let equivalenceBigramEngine = try Engine(lexiconText: equivalenceLexicon, languageModelDirectory: equivalenceDirectory)
let syllableScripts = [("ㄅㄚ", "18 "), ("ㄆㄚ", "q8 "), ("ㄇㄚ", "a8 ")]
for length in 1...6 {
    let combinations = Int(pow(3.0, Double(length)))
    for code in 0..<combinations {
        var value = code, readings: [String] = [], keys = ""
        for _ in 0..<length {
            let syllable = syllableScripts[value % 3]; value /= 3
            readings.append(syllable.0); keys += syllable.1
        }
        let old = legacyBigramConversion(readings, lexicon: equivalenceLexicon, model: equivalenceModel)
        let new = composer(equivalenceBigramEngine, keys).presentation
        check(new.text == old.0 && new.clauses == old.1, "old/new bigram equivalence \(code)/\(length)")
    }
}

// Presentation-only refreshes reuse the lattice, but personal-data edits invalidate a live composer.
let cacheEngine = try Engine(lexiconText: "ㄅㄚ\t八\t-9\nㄅㄚ\t巴\t-10\n")
let liveCache = composer(cacheEngine, "18 ")
try cacheEngine.addUserPhrase(word: "巴", reading: "ㄅㄚ")
_ = liveCache.handle(.left)
check(liveCache.presentation.text == "巴", "phrase insertion invalidates live conversion cache")
cacheEngine.removeUserPhrase(word: "巴", reading: "ㄅㄚ")
_ = liveCache.handle(.right)
check(liveCache.presentation.text == "八", "phrase removal invalidates live conversion cache")
let learner = composer(cacheEngine, "18 ")
_ = learner.handle(.down)
_ = learner.selectCandidate(at: learner.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
_ = learner.commitAll()
_ = liveCache.handle(.left)
check(liveCache.presentation.text == "巴", "learning invalidates another composer's conversion cache")
cacheEngine.forgetAllLearned()
_ = liveCache.handle(.right)
check(liveCache.presentation.text == "八", "forgetting invalidates live conversion cache")

// UTF-16 offsets and defensive options.
let unicodeEngine = try Engine(lexiconText: "ㄅㄚ\t𠮷\t-1\nㄆㄚ\t趴\t-2\nㄅㄚ ㄆㄚ\t𠮷趴\t-1\n")
let unicode = composer(unicodeEngine, "18 q8 ")
_ = unicode.handle(.left)
check(unicode.presentation.cursor == 2, "supplementary Han caret uses UTF-16")
type("a8", into: unicode)
check(unicode.presentation.cursor == 4, "buffer after supplementary character")
validPresentation(unicode, "supplementary Han + buffer")
var zeroKeys = ComposerOptions(); zeroKeys.selectionKeys = []
let zero = composer(engine, "18 ", options: zeroKeys)
_ = zero.handle(.down)
check(zero.presentation.candidates?.pageSize == 1, "empty selection keys safe")
_ = zero.handle(.space); _ = zero.handle(.enter)
check(zero.presentation.candidates == nil, "empty selection keys navigation still works")

// Deterministic mixed editing stress: all rendered clauses must remain contiguous and in bounds.
let stress = composer(engine)
let mixed: [KeyInput] = [.char("1"), .char("8"), .space, .char("q"), .char("8"), .space, .down,
    .right, .enter, .left, .char("a"), .char("8"), .space, .home, .forwardDelete,
    .shiftedChar("-"), .end, .backspace, .optionBackspace, .escape, .shiftSpace]
for _ in 0..<4 { for key in mixed { _ = stress.handle(key); validPresentation(stress, "mixed editing") } }

// Keyboard channel: physical geometry, bounded edits, dictionary validity and cached results.
check(ComposerOptions().typoTolerance == .standard, "standard typo tolerance is the public default")
for row in ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"] {
    for key in row {
        for neighbour in KeyChannel.neighbours(of: key) {
            check(KeyChannel.neighbours(of: neighbour).contains(key), "symmetric neighbours \(key)/\(neighbour)")
        }
    }
}
check(Set("2qea s".filter { $0 != " " }).isSubset(of: Set(KeyChannel.neighbours(of: "w"))), "staggered w neighbours")
check(Set("23as").isSubset(of: Set(KeyChannel.neighbours(of: "w"))), "w straddles 2/3 above and a/s below")
check(KeyChannel.neighbours(of: "3").contains("4") && KeyChannel.neighbours(of: "6").contains("7"), "adjacent tone pairs")
check(!KeyChannel.neighbours(of: "4").contains("6"), "4 and 6 are separated by 5")
check(KeyChannel.neighbours(of: "e").contains("w") && KeyChannel.neighbours(of: "a").contains("z"), "ordinary and staggered neighbours")
let channelFixture = try Engine(lexiconText: "ㄊㄜˋ\t特\t-2\nㄊㄜˇ\t忑\t-2\nㄅㄧˇ\t筆\t-2\nㄊㄧㄢˇ\t舔\t-2\n")
let toneAlternatives = channelFixture.typoAlternatives(for: "wk3")
check(toneAlternatives.contains { $0.reading == "ㄊㄜˋ" }, "3/4 tone alternative")
check(toneAlternatives.count <= 8 && zip(toneAlternatives, toneAlternatives.dropFirst()).allSatisfy { $0.cost <= $1.cost }, "eight cheapest sorted alternatives")
check(channelFixture.typoAlternatives(for: "13u").contains { $0.reading == "ㄅㄧˇ" }, "adjacent transposition reparsed")
check(channelFixture.typoAlternatives(for: "1u03").contains { $0.reading == "ㄅㄧˇ" }, "extra typed key deleted")
check(channelFixture.typoAlternatives(for: "u3").contains { $0.reading == "ㄅㄧˇ" }, "missing key inserted")
check(channelFixture.typoAlternatives(for: "su04").contains { $0.reading == "ㄊㄧㄢˇ" }, "four keys allow two edits")
check(!channelFixture.typoAlternatives(for: "sk4").contains { $0.reading == "ㄊㄜˇ" }, "three keys forbid two edits")
let invalidTypoEngine = try Engine(lexiconText: "ㄊㄜˋ\t特\t-2\n")
let acceptedTypo = composer(invalidTypoEngine, "ek4", options: ComposerOptions())
check(acceptedTypo.presentation.text == "特", "invalid exact syllable accepted via neighbour alternative")
_ = acceptedTypo.handle(.down)
check(acceptedTypo.presentation.candidates?.items.first { $0.text == "特" }?.annotation == "ㄊㄜˋ", "corrected candidate annotates actual reading")
_ = acceptedTypo.selectCandidate(at: acceptedTypo.presentation.candidates!.items.firstIndex { $0.text == "特" }!)
check(acceptedTypo.commitAll() == "特", "picked correction fixed and committed")
check(invalidTypoEngine.learnedEntries().contains { $0.reading == "ㄍㄜˋ" && $0.word == "特" }, "correction learns typed reading")
let recalledTypo = composer(invalidTypoEngine, "ek4", options: ComposerOptions())
check(recalledTypo.presentation.text == "特", "repeat typed error recalls learned word directly")
_ = recalledTypo.handle(.down)
check(recalledTypo.presentation.candidates?.items.first { $0.text == "特" }?.annotation == nil, "learned typed reading no longer needs channel correction")
invalidTypoEngine.forgetAllLearned()
check(composer(invalidTypoEngine, "ek4").presentation.text == "ㄍㄜ", "off disables invalid acceptance")
let tieTypo = try Engine(lexiconText: "ㄊㄜˇ\t甲\t-2\nㄊㄜˋ\t乙\t-2\n")
check(composer(tieTypo, "wk3", options: ComposerOptions()).presentation.text == "甲", "exact beats corrected when LM scores tie")
let toneCost = toneAlternatives.first { $0.reading == "ㄊㄜˋ" }!.cost
let strengthFixture = try Engine(lexiconText: "ㄊㄜˇ\t甲\t\(-toneCost * 0.85)\nㄊㄜˋ\t乙\t0\n")
var scaledOptions = ComposerOptions(); scaledOptions.typoTolerance = .aggressive
check(composer(strengthFixture, "wk3", options: ComposerOptions()).presentation.text == "甲", "standard channel cost resists weaker evidence")
check(composer(strengthFixture, "wk3", options: scaledOptions).presentation.text == "乙", "aggressive applies the 0.7 cost multiplier")
let provenanceFixture = try Engine(lexiconText: "ㄊㄢˇ\t坦\t-2\n")
check(composer(provenanceFixture, "we04", options: ComposerOptions()).presentation.text == "坦", "token retains overwritten raw keys and its tone for two-edit recovery")
let contextDirectory = temp.appendingPathComponent("typo-context")
try FileManager.default.createDirectory(at: contextDirectory, withIntermediateDirectories: true)
try Data("八\t-2\n各\t-2\n特\t-2\n</s>\t0\n".utf8).write(to: contextDirectory.appendingPathComponent("unigram.tsv"))
try Data("<s>\t八\t0\n<s>\t各\t0\n<s>\t特\t0\n八\t各\t-1000\n八\t特\t0\n".utf8).write(to: contextDirectory.appendingPathComponent("bigram.tsv"))
try Data().write(to: contextDirectory.appendingPathComponent("backoff.tsv"))
let contextTypo = try Engine(lexiconText: "ㄅㄚ\t八\t-2\nㄍㄜˋ\t各\t-2\nㄊㄜˋ\t特\t-2\n", languageModelDirectory: contextDirectory)
check(composer(contextTypo, "ek4", options: ComposerOptions()).presentation.text == "各", "equal context retains exact reading")
check(composer(contextTypo, "18 ek4", options: ComposerOptions()).presentation.text == "八特", "far likelier context overcomes neighbour cost")
check(composer(contextTypo, "18 ek4").presentation.text == "八各", "off disables corrected conversion")
var aggressiveOptions = ComposerOptions(); aggressiveOptions.typoTolerance = .aggressive
check(composer(contextTypo, "18 ek4", options: aggressiveOptions).presentation.text == "八特", "aggressive uses correction path")
let liveTypoOption = composer(contextTypo, "18 ek4", options: ComposerOptions())
liveTypoOption.options.typoTolerance = .off
_ = liveTypoOption.handle(.left)
check(liveTypoOption.presentation.text == "八各", "changing typo option invalidates live conversion")

// Explicit correction recall beats even a much more frequent exact word, and survives disk reload.
let recallDirectory = temp.appendingPathComponent("typo-recall")
let recallLexicon = "ㄊㄜˇ\t甲\t0\nㄊㄜˋ\t乙\t-100\n"
let recallEngine = try Engine(lexiconText: recallLexicon, userDataDirectory: recallDirectory)
let recallPick = composer(recallEngine, "wk3", options: ComposerOptions())
_ = recallPick.handle(.down)
check(!recallPick.presentation.candidates!.items.contains { $0.text == "乙" }, "the list starts with words for the keys as typed")
recallPick.toggleCorrections()
check(recallPick.presentation.candidates?.tolerant == true, "toggling lists corrections")
_ = recallPick.selectCandidate(at: recallPick.presentation.candidates!.items.firstIndex { $0.text == "乙" }!)
_ = recallPick.commitAll()
check(composer(recallEngine, "wk3", options: ComposerOptions()).presentation.text == "乙", "explicit typo recall outranks a much more frequent exact word")
var reloadedRecall: Engine?
let recallDeadline = Date().addingTimeInterval(3)
while Date() < recallDeadline {
    let loaded = try Engine(lexiconText: recallLexicon, userDataDirectory: recallDirectory)
    if !loaded.learnedEntries().isEmpty { reloadedRecall = loaded; break }
    Thread.sleep(forTimeInterval: 0.02)
}
check(reloadedRecall.map { composer($0, "wk3", options: ComposerOptions()).presentation.text == "乙" } ?? false,
      "corrected typed-reading recall persists")
recallEngine.forgetAllLearned()
check(composer(recallEngine, "wk3", options: ComposerOptions()).presentation.text == "甲", "forget invalidates corrected-reading recall")

// At most one corrected token for short words; two for longer words.
for (length, allowed) in [(3, false), (4, true), (6, true)] {
    let reading = Array(repeating: "ㄊㄜˋ", count: length).joined(separator: " ")
    let word = String(repeating: "特", count: length)
    let e = try Engine(lexiconText: "ㄍㄜˋ\t各\t-100\nㄊㄜˋ\t特\t-100\n\(reading)\t\(word)\t1000\n")
    let c = composer(e, "ek4ek4" + String(repeating: "wk4", count: length - 2), options: ComposerOptions())
    _ = c.handle(.down)
    check((c.presentation.candidates?.items.contains { $0.text == word && $0.annotation != nil } ?? false) == allowed,
          "\(length)-token span correction budget")
}

// Backtick emits plain symbols, preserving standard tone placement and editing bounds.
let plainEmpty = composer(engine)
check(plainEmpty.handle(.char("`")) == .passThrough, "empty backtick passes through")
let plainBuffer = composer(engine, "su")
check(plainBuffer.handle(.char("`")) == .consumed && plainBuffer.presentation.text == "ㄋㄧ", "backtick literalises unfinished buffer")
_ = plainBuffer.handle(.backspace)
check(!plainBuffer.isComposing, "plain buffer is a single literal token")
for (keys, plain) in [("18 ", "ㄅㄚ"), ("186", "ㄅㄚˊ"), ("183", "ㄅㄚˇ"), ("184", "ㄅㄚˋ"), ("187", "˙ㄅㄚ")] {
    let c = composer(engine, keys)
    _ = c.handle(.char("`"))
    check(c.presentation.text == plain, "backtick standard plain formatting \(keys)")
    validPresentation(c, "plain previous syllable")
}
let plainMiddle = composer(engine, "18 q8 ")
_ = plainMiddle.handle(.left); _ = plainMiddle.handle(.char("`"))
check(plainMiddle.presentation.text == "ㄅㄚ趴" && plainMiddle.presentation.cursor == 2, "backtick replaces token before interior cursor")
for size in [0, 1, 2, 9, 30] {
    var opts = legacyOptions; opts.selectionKeys = Array("123456789abcdefghijklmnopqrstu".prefix(size))
    let c = composer(engine, "18 ", options: opts)
    _ = c.handle(.down)
    let list = c.presentation.candidates!
    check(!list.items.contains { $0.text == "ㄅㄚ" }, "plain Zhuyin absent from list of size \(size)")
    check(c.presentation.zhuyinHint == "ㄅㄚ", "plain hint independent of page size \(size)")
    _ = c.handle(.char("`"))
    check(c.presentation.text == "ㄅㄚ" && c.presentation.zhuyinHint == nil, "footer action fixes literal")
}

// Compare every incremental edit with a forced full decode, including EOS/history and word-boundary edits.
let incrementalScript = "18 q8 a8 18 q8 a8 18 q8 a8 ".map { $0 == " " ? KeyInput.space : .char($0) }
    + [.left, .left, .backspace, .char("1"), .char("8"), .space, .home, .forwardDelete,
       .end, .backspace, .shiftedChar(","), .char("1"), .char("8"), .space, .char("e"), .char("k"), .char("4"), .char("`"), .backspace]
for model in [sixEngine, contextTypo, trigramEngine] {
    var opts = ComposerOptions(); opts.learningEnabled = false
    let c = composer(model, options: opts)
    for key in incrementalScript {
        _ = c.handle(key)
        let incremental = c.presentation
        model.forgetLearned(word: "__full_decode_check__", reading: "ㄦ")
        _ = c.handle(.up)
        check(c.presentation == incremental, "incremental/full decode equivalence after \(key)")
    }
}

// Anchors, generated multi-word phrases and the exact highlighted replacement range.
func focusedRange(_ c: Composer) -> Range<Int>? {
    c.presentation.focusedClause.map { c.presentation.clauses[$0] }
}
let groupLexicon = tiny + "\nㄚ\t啊\t-2\nㄏㄠˇ ㄚ\t好啊\t-5\n"
let groupEngine = try Engine(lexiconText: groupLexicon)
check(ComposerOptions().candidateAnchor == .afterCursor, "after-cursor anchor is default")
for anchor: CandidateAnchor in [.afterCursor, .beforeCursor] {
    for position in 0...3 {
        var opts = legacyOptions; opts.candidateAnchor = anchor; opts.learningEnabled = false
        let c = composer(groupEngine, "su3cl38 ", options: opts)
        _ = c.handle(.home)
        for _ in 0..<position { _ = c.handle(.right) }
        _ = c.handle(.down)
        let forward = position == 0 || (anchor == .afterCursor && position < 3)
        let expected = forward ? position..<3 : 0..<position
        check(focusedRange(c) == expected, "anchor \(anchor) at \(position) focuses \(expected)")
        check(c.presentation.candidates?.items.first?.text == String("你好啊".dropFirst(expected.lowerBound).prefix(expected.count)),
              "anchor current text at \(position)")
        var previousLength = 7
        let count = c.presentation.candidates!.items.count
        for index in 0..<count {
            let range = focusedRange(c)!
            check(range.count <= previousLength, "candidate lengths descend")
            check(forward ? range.lowerBound == position : range.upperBound == position, "candidate follows anchor direction")
            check(c.presentation.zhuyinHint?.split(separator: " ").count == range.count, "hint tracks highlighted span length")
            validPresentation(c, "candidate length \(range.count)")
            previousLength = range.count
            if index + 1 < count { _ = c.handle(.down) }
        }
    }
}
let grouped = composer(groupEngine, "su3cl38 ")
check(grouped.presentation.clauses == [0..<2, 2..<3], "three-syllable fixture decodes as two words")
_ = grouped.handle(.down)
check(grouped.presentation.candidates!.items.map(\.text) == ["你好啊", "好啊", "啊"], "generated phrase then two-syllable word then single")
check(focusedRange(grouped) == 0..<3, "multi-word phrase is one focused clause")
_ = grouped.selectCandidate(at: 0)
check(grouped.presentation.clauses == [0..<3], "picked phrase is one fixed span")
type("18 ", into: grouped)
check(grouped.presentation.text == "你好啊八", "fixed phrase survives insertion after it")
_ = grouped.handle(.home); type("q8 ", into: grouped)
check(grouped.presentation.text == "趴你好啊八", "fixed phrase survives insertion before it")
_ = grouped.handle(.backspace)
check(grouped.presentation.text == "你好啊八", "fixed phrase survives deletion elsewhere")
_ = grouped.commitAll()
check(groupEngine.learnedEntries().contains { $0.reading == "ㄋㄧˇ ㄏㄠˇ ㄚ" && $0.word == "你好啊" }, "whole phrase learned under typed reading")
let recalledGroup = composer(groupEngine, "su3cl38 ")
check(recalledGroup.presentation.text == "你好啊" && recalledGroup.presentation.clauses == [0..<3], "learned phrase recalled as a direct entry")
_ = recalledGroup.handle(.down)
check(recalledGroup.presentation.candidates?.items.first?.text == "你好啊", "learned phrase appears first")

let contextualGroup = composer(trigramEngine, "18 q8 a8 ", options: fixedOptions)
contextualGroup.options.candidateAnchor = .beforeCursor
_ = contextualGroup.handle(.left); _ = contextualGroup.handle(.down)
_ = contextualGroup.selectCandidate(at: 0)
check(contextualGroup.presentation.text == "八趴麻", "fixed multi-word phrase scores its internal words and last-word history")
check(contextualGroup.nBest().allSatisfy { $0.text.hasPrefix("八趴") }, "N-best keeps fixed multi-word phrase verbatim")
let historyCandidatesEngine = try Engine(lexiconText: binaryLexicon + "ㄇㄚ\t碼\t1\n", languageModelDirectory: binaryDirectory)
let historyCandidates = composer(historyCandidatesEngine, "18 q8 a8 ", options: fixedOptions)
historyCandidates.options.candidateAnchor = .beforeCursor
_ = historyCandidates.handle(.left); _ = historyCandidates.handle(.down)
_ = historyCandidates.selectCandidate(at: 0)
historyCandidates.options.candidateAnchor = .afterCursor
_ = historyCandidates.handle(.down)
check(historyCandidates.presentation.candidates?.items.prefix(3).map(\.text) == ["麻", "碼", "媽"],
      "candidate scores after a fixed phrase retain both words of trigram history")

let constrained = composer(interiorEngine, "18 q8 a8 ")
constrained.options.candidateAnchor = .beforeCursor
_ = constrained.handle(.left); _ = constrained.handle(.down)
check(constrained.presentation.candidates!.items.prefix(2).map(\.text) == ["八趴", "巴趴"], "current span precedes best constrained conversion")
let brokenGroup = composer(groupEngine, "su3cl3")
_ = brokenGroup.handle(.shiftedChar(",")); type("8 ", into: brokenGroup)
_ = brokenGroup.handle(.down)
check(focusedRange(brokenGroup) == 3..<4 && brokenGroup.presentation.zhuyinHint == "ㄚ", "candidates never cross a literal")

// Shift selection, permanent phrase persistence, cancellation, and no accidental commits.
let selectionDirectory = temp.appendingPathComponent("quick-phrase")
let selectionEngine = try Engine(lexiconText: groupLexicon, userDataDirectory: selectionDirectory)
let selected = composer(selectionEngine, "su3cl38 ")
_ = selected.handle(.shiftLeft); _ = selected.handle(.shiftLeft)
check(focusedRange(selected) == 1..<3 && selected.presentation.notice == "Enter 加入自訂詞：好啊", "backward selection notice")
check(selected.presentation.candidates == nil && selected.presentation.zhuyinHint == nil, "selection closes candidate window")
_ = selected.handle(.shiftRight); _ = selected.handle(.shiftRight)
check(selected.presentation.focusedClause == nil && selected.presentation.notice == nil, "returning to selection anchor clears visible selection")
_ = selected.handle(.shiftLeft); _ = selected.handle(.shiftLeft); _ = selected.handle(.shiftLeft)
let savedSelection = selected.handle(.enter)
check(savedSelection == .consumed && selected.presentation.text == "你好啊", "Enter saves without commit")
check(selected.presentation.notice == "已加入自訂詞：你好啊" && focusedRange(selected) == nil, "saved notice clears selection")
_ = selected.handle(.end)
check(selected.presentation.clauses == [0..<3], "quick phrase fixes whole span")
check(selectionEngine.userPhrases().contains { $0.word == "你好啊" && $0.reading == "ㄋㄧˇ ㄏㄠˇ ㄚ" }, "quick phrase permanent entry")
_ = selected.handle(.end); type("18 ", into: selected)
check(selected.presentation.text == "你好啊八" && selected.presentation.notice == nil, "saved notice lasts until next key; fixed phrase survives edit")
var loadedSelection: Engine?
let selectionDeadline = Date().addingTimeInterval(3)
while Date() < selectionDeadline {
    let loaded = try Engine(lexiconText: groupLexicon, userDataDirectory: selectionDirectory)
    if loaded.userPhrases().contains(where: { $0.word == "你好啊" }) { loadedSelection = loaded; break }
    Thread.sleep(forTimeInterval: 0.02)
}
check(loadedSelection != nil, "quick phrase persisted")
if let loadedSelection {
    let c = composer(loadedSelection, "su3cl38 ")
    check(c.presentation.clauses == [0..<3], "permanent phrase recalled directly after reload")
    _ = c.handle(.down)
    check(c.presentation.candidates?.items.first?.text == "你好啊", "saved phrase offered first next time")
}
let crossed = composer(groupEngine, "su3cl38 ")
_ = crossed.handle(.left); _ = crossed.handle(.shiftLeft)
check(focusedRange(crossed) == 1..<2, "selection left from middle")
_ = crossed.handle(.shiftRight); _ = crossed.handle(.shiftRight)
check(focusedRange(crossed) == 2..<3 && crossed.presentation.notice == "Enter 加入自訂詞：啊", "selection crosses anchor to right")
_ = crossed.handle(.escape)
check(crossed.presentation.text == "你好啊" && crossed.presentation.focusedClause == nil && crossed.presentation.notice == nil, "Escape clears selection only")
_ = crossed.handle(.home); _ = crossed.handle(.shiftRight); _ = crossed.handle(.shiftRight)
check(focusedRange(crossed) == 0..<2, "forward selection")
for key: KeyInput in [.left, .right, .down, .char("1"), .shiftEnter] {
    let c = composer(groupEngine, "su3cl38 ")
    _ = c.handle(.shiftLeft)
    let result = c.handle(key)
    check(c.presentation.notice == nil, "other key clears selection notice \(key)")
    if key == .shiftEnter { check(result.commit == "你好啊" && !result.handled, "Shift+Enter retains commit/pass-through semantics") }
    if key == .left || key == .right { check(c.presentation.focusedClause == nil, "plain arrow clears selection") }
}
for withLiteral in [false, true] {
    let c = composer(groupEngine, withLiteral ? "su3cl3" : String(repeating: "18 ", count: 7))
    if withLiteral { _ = c.handle(.shiftedChar(",")) }
    let original = c.presentation.text
    for _ in 0..<(withLiteral ? 3 : 7) { _ = c.handle(.shiftLeft) }
    check(c.handle(.enter) == .consumed && c.presentation.text == original, "invalid selection does not commit")
    check(c.presentation.notice == "自訂詞需為 1 到 6 個字" && focusedRange(c) != nil, "invalid selection remains with error notice")
    _ = c.handle(.escape)
    check(c.presentation.notice == nil && c.presentation.text == original, "invalid selection escape")
}

// Footer hint formats every tone, and replaces the highlighted span rather than the cursor's preceding token.
for (keys, hint) in [("18 ", "ㄅㄚ"), ("186", "ㄅㄚˊ"), ("183", "ㄅㄚˇ"), ("184", "ㄅㄚˋ"), ("187", "˙ㄅㄚ")] {
    let c = composer(engine, keys)
    _ = c.handle(.down)
    check(c.presentation.zhuyinHint == hint, "footer tone notation \(hint)")
}
let hintGroup = composer(groupEngine, "su3cl38 ")
_ = hintGroup.handle(.home); _ = hintGroup.handle(.right); _ = hintGroup.handle(.down)
check(hintGroup.presentation.zhuyinHint == "ㄏㄠˇ ㄚ", "multi-syllable hint uses spaces and unmarked first tone")
check(hintGroup.handle(.char("`")) == .consumed && hintGroup.presentation.text == "你ㄏㄠˇ ㄚ", "window backtick replaces exact after-cursor span")
check(hintGroup.presentation.candidates == nil && hintGroup.presentation.zhuyinHint == nil, "backtick closes window without commit")
validPresentation(hintGroup, "multi-syllable literal")
let neutralHint = composer(phraseOnly, "vu,4vu,7")
_ = neutralHint.handle(.down)
check(neutralHint.presentation.zhuyinHint == "ㄒㄧㄝˋ ˙ㄒㄧㄝ", "multi-syllable neutral hint")

// N-best stays internal, distinct by full text, score ordered, segmented, and rerankable.
let nbestEngine = try Engine(lexiconText: "ㄅㄚ\t八\t-1\nㄅㄚ\t巴\t-2\nㄆㄚ\t趴\t-1\nㄆㄚ\t啪\t-3\nㄅㄚ ㄆㄚ\t八趴\t-1.5\n")
let nbestComposer = composer(nbestEngine, "18 q8 ")
let pendingReranker = composer(nbestEngine)
var rerankerCalls = 0
pendingReranker.reranker = { _ in rerankerCalls += 1; return 0 }
type("18", into: pendingReranker)
check(rerankerCalls == 0, "reranker never receives an empty completed sentence list")
_ = pendingReranker.handle(.space)
check(rerankerCalls == 1, "reranker runs when a syllable completes")
let sentences = nbestComposer.nBest()
check(sentences.map(\.text) == ["八趴", "巴趴", "八啪", "巴啪"], "distinct N-best includes alternate histories and removes duplicate segmentations")
check(sentences.map(\.score) == [-1.5, -3, -4, -5], "N-best scores ordered")
check(sentences.first?.words.map(\.range) == [0..<2] && sentences[1].words.map(\.range) == [0..<1, 1..<2], "N-best carries segmentation and token spans")
check(nbestComposer.nBest(2).count == 2 && nbestComposer.nBest(0).isEmpty && nbestComposer.nBest(-1).isEmpty, "N-best K bounds")
check(nbestComposer.presentation.text == sentences.first?.text, "default display stays top decoder result")
nbestComposer.reranker = { $0.count - 1 }
check(nbestComposer.presentation.text == "巴啪", "reranker chooses displayed sentence")
_ = nbestComposer.handle(.down)
check(nbestComposer.presentation.candidates?.items.first?.text == "巴啪", "candidate list starts with current span under reranking")
check(nbestComposer.nBest().map(\.score) == [-1.5, -3, -4, -5], "reranker leaves statistical scores ordered")
nbestComposer.reranker = { _ in Int.max }
check(nbestComposer.presentation.text == "八趴", "invalid reranker choice falls back")
nbestComposer.reranker = nil
let nbestFixed = composer(nbestEngine, "18 ")
_ = nbestFixed.handle(.down)
_ = nbestFixed.selectCandidate(at: nbestFixed.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
type("q8 ", into: nbestFixed)
check(nbestFixed.nBest().count == 2 && nbestFixed.nBest().allSatisfy { $0.text.hasPrefix("巴") }, "N-best respects fixed spans")
_ = nbestFixed.handle(.shiftedChar(",")); type("18 ", into: nbestFixed)
check(nbestFixed.nBest().allSatisfy { $0.words.contains { $0.text == "，" && $0.reading.isEmpty && $0.range == 2..<3 } }, "N-best preserves literals and spans")
let wideDirectory = temp.appendingPathComponent("wide-beam")
try FileManager.default.createDirectory(at: wideDirectory, withIntermediateDirectories: true)
let wideWords = (0..<80).map { String(UnicodeScalar(0x4e00 + $0)!) }
try Data((wideWords.map { "\($0)\t-2\n" }.joined() + "</s>\t0\n").utf8)
    .write(to: wideDirectory.appendingPathComponent("unigram.tsv"))
try Data((wideWords.map { "<s>\t\($0)\t0\n" }.joined() + "一\t</s>\t-100\n").utf8)
    .write(to: wideDirectory.appendingPathComponent("bigram.tsv"))
try Data().write(to: wideDirectory.appendingPathComponent("backoff.tsv"))
let wideEngine = try Engine(lexiconText: wideWords.map { "ㄅㄚ\t\($0)\t-2\n" }.joined(), languageModelDirectory: wideDirectory)
let wide = composer(wideEngine, "18 ")
check(wide.presentation.text == "丁", "wide beam ranks EOS before pruning and resolves score ties")
check(wide.nBest(20).count == 20 && wide.nBest(20).first?.text == "丁", "N-best K widens beyond the default beam")
_ = wide.handle(.down)
check(wide.presentation.candidates?.items.count == 60, "candidate list cap at 60 even for wide lexicon")
_ = wide.handle(.escape); type("18 ", into: wide)
check(wide.presentation.text == "一丁", "cached final-position scores never leak into an interior position")
let wideSixWords = wideWords.map { String(repeating: "八", count: 5) + $0 }
try Data((wideSixWords.map { "\($0)\t-2\n" }.joined() + "</s>\t0\n").utf8)
    .write(to: wideDirectory.appendingPathComponent("unigram.tsv"))
try Data((wideSixWords.map { "<s>\t\($0)\t0\n" }.joined() + "\(wideSixWords[0])\t</s>\t-100\n").utf8)
    .write(to: wideDirectory.appendingPathComponent("bigram.tsv"))
let sixReading = Array(repeating: "ㄅㄚ", count: 6).joined(separator: " ")
let wideSixEngine = try Engine(lexiconText: "ㄅㄚ\t八\t-100\n" + wideSixWords.map { "\(sixReading)\t\($0)\t-2\n" }.joined(),
                               languageModelDirectory: wideDirectory)
let wideSix = composer(wideSixEngine, String(repeating: "18 ", count: 6))
check(wideSix.presentation.text == wideSixWords[1], "six-syllable edge prunes with EOS")
type("18 ", into: wideSix)
check(wideSix.presentation.text == wideSixWords[0] + "八", "six-syllable edge at old EOS is rebuilt before prefix reuse")
let invalidWindowBuffer = composer(try Engine(lexiconText: tiny), "18 1u")
_ = invalidWindowBuffer.handle(.down)
check(invalidWindowBuffer.presentation.candidates == nil && invalidWindowBuffer.presentation.text == "八ㄅㄧ", "invalid unfinished buffer cannot enter a candidate replacement span")

// Real lexicon regression checks and release-mode latency measurement.
let realURL = URL(fileURLWithPath: "data/build/lexicon.tsv")
if FileManager.default.fileExists(atPath: realURL.path) {
    let (real, initMS) = try timed { try Engine(lexiconURL: realURL, userDataDirectory: nil) }
    let (_, lexiconMS) = try timed { try Engine(lexiconURL: realURL, userDataDirectory: nil, languageModelDirectory: nil) }
    print(String(format: "TIMING real lexicon + discovered LM init: %.3f ms; lexicon alone %.3f ms (target <400 ms)", initMS, lexiconMS))
    check(lexiconMS < 400, "real lexicon init under 400 ms")
    let reverseKeys = Dictionary(uniqueKeysWithValues: keySymbols.map { ($0.1, String($0.0)) })
    let toneKeys: [Character: String] = ["ˊ": "6", "ˇ": "3", "ˋ": "4", "˙": "7"]
    func keysForReading(_ reading: String) -> String {
        reading.split(separator: " ").map { syllable in
            let keys = syllable.map { reverseKeys[$0] ?? toneKeys[$0] ?? "" }.joined()
            return toneKeys[syllable.last!] == nil ? keys + " " : keys
        }.joined()
    }
    // A composition continues the words committed before it, until a key passes to the app or the host breaks it.
    var unlearned = legacyOptions; unlearned.learningEnabled = false
    let fresh = composer(real, "g4", options: unlearned)
    let continuing = composer(real, "ji3ap7", options: unlearned)
    check(continuing.commitAll() == "我們", "context commit")
    type("g4", into: continuing)
    check(continuing.presentation.text == "是", "ㄕˋ after committed 我們 reads 是")
    check(fresh.presentation.text != "是", "the continuation check needs a reading the sentence start converts differently")
    continuing.reset()
    check(continuing.handle(.left).handled == false, "left passes through with nothing composed")
    type("g4", into: continuing)
    check(continuing.presentation.text == fresh.presentation.text, "a passed-through key restarts the sentence")
    let restarted = composer(real, "ji3ap7", options: unlearned)
    _ = restarted.commitAll()
    restarted.breakContext()
    type("g4", into: restarted)
    check(restarted.presentation.text == fresh.presentation.text, "breakContext restarts the sentence")
    // Backspace passed to the app deletes the last commit from its end; the context is what is left of it.
    let deleting = composer(real, "ji3ap7g4", options: unlearned)
    check(deleting.commitAll() == "我們是", "deletion context commit")
    check(deleting.handle(.backspace).handled == false, "backspace with nothing composed passes through")
    type("g4", into: deleting)
    check(deleting.presentation.text == "是", "after deleting 是, ㄕˋ continues from 我們")
    deleting.reset()
    for _ in 0..<2 { _ = deleting.handle(.backspace) }
    type("g4", into: deleting)
    check(deleting.presentation.text == fresh.presentation.text, "deleting the whole commit leaves no context")
    // Deleting a commit and retyping its reading as another word is learned like a pick.
    let correcting = try Engine(lexiconURL: realURL, userDataDirectory: nil)
    let corrector = composer(correcting, "ek4")
    let first = corrector.commitAll()
    _ = corrector.handle(.backspace)
    type("ek4", into: corrector)
    _ = corrector.handle(.down)
    let other = corrector.presentation.candidates!.items.firstIndex { $0.text != first && $0.text.count == 1 }!
    let picked = corrector.presentation.candidates!.items[other].text
    _ = corrector.selectCandidate(at: other)
    _ = corrector.commitAll()
    check(composer(correcting, "ek4").presentation.text == picked, "a deleted and retyped word is remembered")
    // Words for the keys as typed come before corrections; Shift-↓ lists only the character at the cursor.
    let listing = composer(real, "ji3ap7", options: unlearned)
    _ = listing.handle(.home)
    _ = listing.handle(.down)
    let full = listing.presentation.candidates!.items
    let firstCorrection = full.firstIndex { $0.annotation != nil } ?? full.count
    check(full[firstCorrection...].allSatisfy { $0.annotation != nil } || full.prefix(1).contains { $0.annotation != nil },
          "corrections follow every word for the keys as typed")
    check(full.contains { $0.text == "我們" } && full.contains { $0.text == "我" }, "the full list has phrases and characters")
    _ = listing.handle(.shiftDown)
    let single = listing.presentation.candidates!.items
    check(single.allSatisfy { $0.text.count == 1 && $0.annotation == nil } && single.first?.text == "我",
          "Shift-↓ in the window lists only the character, for the keys as typed")
    _ = listing.handle(.shiftDown)
    check(listing.presentation.candidates!.items == full, "Shift-↓ again returns to the full list")
    listing.reset()
    type("su3cl3", into: listing)
    _ = listing.handle(.shiftDown)
    check(listing.presentation.candidates?.items.map(\.text) == ["好", "郝"], "Shift-↓ opens the single-character list")
    // Export and import carry what was learned to another engine; excluded words are never learned again.
    let receiving = try Engine(lexiconURL: realURL, userDataDirectory: nil)
    let rows = try receiving.importPersonalData(correcting.exportPersonalData())
    check(rows > 0 && composer(receiving, "ek4").presentation.text == picked, "imported memory converts the same")
    check((try? receiving.importPersonalData(Data("not an export".utf8))) == nil, "a foreign file is refused")
    receiving.excludeFromLearning(word: picked, reading: "ㄍㄜˋ")
    check(composer(receiving, "ek4").presentation.text != picked, "excluding forgets the word")
    let excludedPick = composer(receiving, "ek4")
    _ = excludedPick.handle(.down)
    if let index = excludedPick.presentation.candidates?.items.firstIndex(where: { $0.text == picked }) {
        _ = excludedPick.selectCandidate(at: index)
    }
    _ = excludedPick.commitAll()
    check(composer(receiving, "ek4").presentation.text != picked, "an excluded word is not learned from a pick")
    check(receiving.excludedEntries().map(\.word) == [picked], "exclusions are listed")
    let reimported = try Engine(lexiconURL: realURL, userDataDirectory: nil)
    try reimported.importPersonalData(receiving.exportPersonalData())
    check(reimported.excludedEntries().map(\.word) == [picked], "exclusions travel with an export")
    var conversions = [("謝謝", "vu,4vu,7"), ("謝謝", "vu,4vu,4"),
                       ("我們", "ji3ap7"), ("我們", "ji3ap "),
                       ("東西", "2j/ vu "), ("東西", "2j/ vu7"),
                       ("今天天氣很好", "rup wu0 wu0 fu4cp3cl3"), ("你好", "su3cl3")]
    for word in ["什麼", "喜歡", "朋友"] {
        let reading = real.readings(of: word).first
        check(reading != nil, "real lexicon has a reading for \(word)")
        print("READING \(word): \(reading ?? "missing")")
        conversions.append((word, reading.map(keysForReading) ?? ""))
    }
    for (word, keys) in conversions {
        let actual = composer(real, keys, options: ComposerOptions()).presentation.text
        print("REAL \(word) → \(actual) [\(keys)]")
        check(actual == word, "real conversion \(word) [\(keys)]")
    }
    for (label, keys) in [("你好啊", "su3cl38 "), ("今天天氣很好", "rup wu0 wu0 fu4cp3cl3")] {
        let c = composer(real, keys, options: ComposerOptions())
        _ = c.handle(.down)
        let list = c.presentation.candidates!
        print("CANDIDATES \(label) → \(c.presentation.text): \(list.items.map(\.text).joined(separator: " / "))")
        check(list.items.count <= 60, "real candidate list capped at 60")
    }
    let candidateLong = composer(real, String(repeating: "rup wu0 wu0 fu4cp3cl3", count: 6) + "su3cl3su3cl3", options: ComposerOptions())
    var openSamples: [Double] = []
    var highlightSamples: [Double] = []
    for _ in 0..<30 {
        openSamples.append(timed { _ = candidateLong.handle(.down) }.1)
        for _ in 0..<12 { highlightSamples.append(timed { _ = candidateLong.handle(.down) }.1) }
        _ = candidateLong.handle(.escape)
    }
    print(String(format: "TIMING candidates on 40 syllables: open median %.3f ms max %.3f ms; highlight median %.3f ms max %.3f ms",
                 openSamples.sorted()[openSamples.count / 2], openSamples.max()!,
                 highlightSamples.sorted()[highlightSamples.count / 2], highlightSamples.max()!))
    check(openSamples.max()! < 5 && highlightSamples.max()! < 5, "40-syllable candidate opening and highlight under 5 ms")
    for position in [0, 20] {
        _ = candidateLong.handle(.home)
        for _ in 0..<position { _ = candidateLong.handle(.right) }
        let sample = timed { _ = candidateLong.handle(.down) }.1
        print(String(format: "TIMING candidates on 40 syllables at %d: open %.3f ms", position, sample))
        check(sample < 5, "40-syllable candidate opening at \(position) under 5 ms")
        _ = candidateLong.handle(.escape)
    }
    let nbestLong = composer(real, String(repeating: "rup wu0 wu0 fu4cp3cl3", count: 3) + "su3cl3", options: ComposerOptions())
    let nbestSamples = (0..<30).map { _ in timed { nbestLong.nBest(8) }.1 }
    print(String(format: "TIMING 8-best on 20 syllables: median %.3f ms max %.3f ms", nbestSamples.sorted()[nbestSamples.count / 2], nbestSamples.max()!))
    check(nbestLong.nBest(8).count == 8 && nbestSamples.max()! < 10, "20-syllable 8-best under 10 ms")
    let realEnd = composer(real, "cp3cl3")
    _ = realEnd.handle(.down)
    let hasVeryGood = try String(contentsOf: realURL, encoding: .utf8).split(separator: "\n")
        .contains { $0.hasPrefix("ㄏㄣˇ ㄏㄠˇ\t很好\t") }
    if hasVeryGood {
        check(realEnd.presentation.candidates?.items.prefix(3).map(\.text) == ["很好", "好", "郝"], "real end-cursor phrase candidates")
    } else {
        print("WARNING 很好 has no word entry in the real lexicon; conversion uses 很 + 好 (fixture checks phrase-first candidates)")
        check(realEnd.presentation.candidates?.items.prefix(3).map(\.text) == ["很好", "好", "郝"], "real generated phrase precedes singles without lexicon entry")
    }
    let realPhraseEnd = composer(real, "vu,4vu,4")
    _ = realPhraseEnd.handle(.down)
    check(realPhraseEnd.presentation.candidates?.items.first?.text == "謝謝", "real tolerant multi-syllable candidate at end cursor")
    // Previously unseen syllables at the 40-token limit exercise cold channel/span caches too.
    let coldReadings = Array(Set(try String(contentsOf: realURL, encoding: .utf8).split(separator: "\n").compactMap { line -> String? in
        let fields = line.split(separator: "\t")
        return fields.count == 3 && !fields[0].contains(" ") && !line.hasPrefix("#") ? String(fields[0]) : nil
    })).sorted()
    let cold = composer(real, String(repeating: "wu0 ", count: 40), options: ComposerOptions())
    var coldSamples: [Double] = []
    for reading in coldReadings.enumerated().filter({ $0.offset % 37 == 0 }).prefix(80).map(\.element) {
        for character in keysForReading(reading) {
            let ms = timed { _ = cold.handle(character == " " ? .space : .char(character)) }.1
            coldSamples.append(ms)
        }
        _ = cold.handle(.backspace)
    }
    let coldOrdered = coldSamples.sorted()
    print(String(format: "TIMING cold channel on 40 syllables: median %.3f ms, p95 %.3f ms, max %.3f ms", coldOrdered[coldOrdered.count / 2], coldOrdered[Int(Double(coldOrdered.count - 1) * 0.95)], coldOrdered.last!))
    check(coldOrdered.last! < 3, "cold channel at 40 syllables under 3 ms")
    func measureLatency(_ model: Engine, name: String, typo: TypoTolerance = .standard) {
        for (label, keys, nextKeys) in [
            ("first-tone", String(repeating: "wu0 ", count: 40), "wu0 "),
            ("neutral-tolerant", String(repeating: "vu,4vu,4", count: 20), "vu,4"),
        ] {
            var options = ComposerOptions(); options.typoTolerance = typo
            let long = composer(model, keys, options: options)
            var samples: [Double] = []
            for _ in 0..<100 {
                samples.append(timed { _ = long.handle(.left) }.1)
                _ = long.handle(.right)
                for character in nextKeys {
                    samples.append(timed { _ = long.handle(character == " " ? .space : .char(character)) }.1)
                }
                _ = long.handle(.backspace)
            }
            let ordered = samples.sorted()
            let p95 = ordered[Int(Double(ordered.count - 1) * 0.95)]
            print(String(format: "TIMING %@ %@ handle on 40 syllables: median %.3f ms, p95 %.3f ms, max %.3f ms (target <3 ms)", name, label, ordered[ordered.count / 2], p95, ordered.last!))
            check(ordered.last! < 3, "40-syllable \(name) \(label) handle under 3 ms")
        }
    }
    measureLatency(real, name: "discovered LM")
    measureLatency(real, name: "discovered LM aggressive", typo: .aggressive)
    let benchmarkDirectory = temp.appendingPathComponent("150MB")
    try FileManager.default.createDirectory(at: benchmarkDirectory, withIntermediateDirectories: true)
    let realText = try String(contentsOf: realURL, encoding: .utf8)
    let vocabulary = Set(realText.split(separator: "\n").compactMap { line -> String? in
        let fields = line.split(separator: "\t")
        return fields.count == 3 && !line.hasPrefix("#") ? String(fields[1]) : nil
    })
    try largeLMFixture(words: vocabulary).write(to: benchmarkDirectory.appendingPathComponent("lm.bin"))
    let (resolved, binaryMS) = try timed {
        let model = try NGramLanguageModel(directory: benchmarkDirectory)
        return vocabulary.reduce(0) { $0 + (model.wordID(for: $1) == nil ? 0 : 1) }
    }
    check(resolved == vocabulary.count, "150 MB model maps every real lexicon word")
    print(String(format: "TIMING 150 MB synthetic LM + %d unique lexicon IDs: %.3f ms (target <80 ms)", resolved, binaryMS))
    check(binaryMS < 80, "150 MB model load and lexicon ID mapping under 80 ms")
    let (largeEngine, fullBinaryMS) = try timed {
        try Engine(lexiconURL: realURL, userDataDirectory: nil, languageModelDirectory: benchmarkDirectory)
    }
    print(String(format: "TIMING 150 MB synthetic LM complete Engine init: %.3f ms", fullBinaryMS))
    measureLatency(largeEngine, name: "150 MB synthetic trigram")
    measureLatency(largeEngine, name: "150 MB synthetic trigram aggressive", typo: .aggressive)
    let candidateDirectory = URL(fileURLWithPath: "data/build/lm2")
    if FileManager.default.fileExists(atPath: candidateDirectory.appendingPathComponent("lm.bin").path) {
        let candidateLexicon = candidateDirectory.appendingPathComponent("lexicon.tsv")
        let url = FileManager.default.fileExists(atPath: candidateLexicon.path) ? candidateLexicon : realURL
        let (candidate, candidateMS) = try timed {
            try Engine(lexiconURL: url, userDataDirectory: nil, languageModelDirectory: candidateDirectory)
        }
        print(String(format: "TIMING lm2 complete Engine init: %.3f ms", candidateMS))
        measureLatency(candidate, name: "lm2")
    }
} else { print("WARNING real lexicon absent; skipped real-data checks") }

// Personal language model: one correction settles its own context and stays out of others.
do {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("zhuyin-personal-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try "肏\t-9\n操\t-10\n你\t-5\n我\t-5\n嗎\t-7\n媽\t-8\n</s>\t-3\n".write(
        to: directory.appendingPathComponent("unigram.tsv"), atomically: true, encoding: .utf8)
    try "你\t嗎\t-0.5\n你\t媽\t-7\n".write(to: directory.appendingPathComponent("bigram.tsv"), atomically: true, encoding: .utf8)
    try "你\t-1\n".write(to: directory.appendingPathComponent("backoff.tsv"), atomically: true, encoding: .utf8)
    let lexicon = "ㄘㄠˋ\t肏\t-9\nㄘㄠˋ\t操\t-10\nㄋㄧˇ\t你\t-5\nㄨㄛˇ\t我\t-5\nㄇㄚ\t嗎\t-7\nㄇㄚ\t媽\t-8\n"
    let personal = try Engine(lexiconText: lexicon, languageModelDirectory: directory)
    var clock = Date().timeIntervalSince1970
    personal.clock = { clock }
    var plain = ComposerOptions(); plain.typoTolerance = .off
    check(composer(personal, "hl4su3a8 ", options: plain).presentation.text == "肏你嗎", "base model prefers 嗎 after 你")
    let correcting = composer(personal, "hl4su3a8 ", options: plain)
    _ = correcting.handle(.down)
    _ = correcting.selectCandidate(at: correcting.presentation.candidates!.items.firstIndex { $0.text == "媽" }!)
    _ = correcting.handle(.home); _ = correcting.handle(.down)
    _ = correcting.selectCandidate(at: correcting.presentation.candidates!.items.firstIndex { $0.text == "操" }!)
    check(correcting.commitAll() == "操你媽", "corrections committed")
    check(composer(personal, "hl4su3a8 ", options: plain).presentation.text == "操你媽", "one correction settles the same input")
    check(composer(personal, "ji3hl4su3a8 ", options: plain).presentation.text.hasSuffix("操你媽"), "and the same words inside a longer input")
    check(composer(personal, "su3a8 ", options: plain).presentation.text == "你嗎", "but not 你ㄇㄚ elsewhere")
    check(composer(personal, "ji3su3a8 ", options: plain).presentation.text == "我你嗎", "nor after another word")
    clock += 7 * 86400
    check(composer(personal, "hl4su3a8 ", options: plain).presentation.text == "操你媽", "still remembered a week later")
    let back = composer(personal, "hl4su3a8 ", options: plain)
    _ = back.handle(.down)
    _ = back.selectCandidate(at: back.presentation.candidates!.items.firstIndex { $0.text == "嗎" }!)
    _ = back.commitAll()
    check(composer(personal, "hl4su3a8 ", options: plain).presentation.text.hasSuffix("你嗎"), "the newest pick wins")
    let fading = try Engine(lexiconText: lexicon, languageModelDirectory: directory)
    var fadingClock = Date().timeIntervalSince1970
    fading.clock = { fadingClock }
    let once = composer(fading, "su3a8 ", options: plain)
    _ = once.handle(.down)
    _ = once.selectCandidate(at: once.presentation.candidates!.items.firstIndex { $0.text == "媽" }!)
    _ = once.commitAll()
    check(composer(fading, "su3a8 ", options: plain).presentation.text == "你媽", "a correction of a whole input is recalled")
    fadingClock += 120 * 86400
    check(composer(fading, "su3a8 ", options: plain).presentation.text == "你嗎", "an unused single correction fades in months")
    let fresh = try Engine(lexiconText: lexicon, languageModelDirectory: directory)
    check(composer(fresh, "hl4su3a8 ", options: plain).presentation.text == "肏你嗎", "empty memory leaves conversions unchanged")
} catch { check(false, "personal model fixture: \(error)") }

print("\(checks - failures)/\(checks) checks passed; \(failures) failures")
exit(failures == 0 ? 0 : 1)
