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
func composer(_ engine: Engine, _ keys: String = "", options: ComposerOptions = ComposerOptions()) -> Composer {
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
                      .left, .right, .home, .end, .up, .down, .pageUp, .pageDown, .tab, .shiftTab, .escape, .char("?")] {
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
let exactPreferred = try Engine(lexiconText: "ㄅㄚ\t甲\t-5\nㄅㄚ˙\t甲\t0\nㄅㄚ\t乙\t-4\n")
let exactChoice = composer(exactPreferred, "18 ")
check(exactChoice.presentation.text == "乙", "exact same-word entry wins over a higher scoring tolerant entry")
_ = exactChoice.handle(.down)
check(exactChoice.presentation.candidates?.items.map(\.text) == ["乙", "甲"], "same-word exact wins and candidates deduplicate")
let tolerantPick = composer(phraseOnly, "vu,4vu,4")
_ = tolerantPick.handle(.down)
check(tolerantPick.presentation.candidates?.items.first?.text == "謝謝", "tolerant phrase offered at end cursor")
_ = tolerantPick.selectCandidate(at: 0)
_ = tolerantPick.handle(.left)
check(tolerantPick.presentation.text == "謝謝", "tolerant pick fixed through navigation")
check(tolerantPick.commitAll() == "謝謝", "tolerant pick commits")
check(phraseOnly.learnedEntries().contains { $0.reading == "ㄒㄧㄝˋ ㄒㄧㄝˋ" && $0.word == "謝謝" && $0.count == 1.5 },
      "tolerant picks and commits learn under typed reading")
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
check(middle.presentation.clauses == [0..<2], "cursor inside a word retains one converted clause")
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
let plainPunctuation: [(Character, String)] = [("[", "「"), ("]", "」"), ("'", "、"), ("\\", "＼"), ("=", "＝"), ("`", "‵")]
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
    check(result == KeyResult(handled: !shifted, commit: "八趴") && !c.isComposing, "enter/shift-enter completes valid buffer")
    type("18 1u", into: c)
    check(c.commitAll() == "八", "commit drops invalid buffer")
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
var limitedOptions = ComposerOptions(); limitedOptions.maxSyllables = 2
let limited = composer(engine, "18 q8 a8", options: limitedOptions)
check(limited.presentation.text == "八趴", "maxSyllables refuses new symbols")
check(limited.handle(.char("6")) == .consumed && limited.presentation.text == "八趴", "tone at limit empty ignored")
_ = limited.handle(.shiftedChar("a"))
check(limited.presentation.text == "八趴A", "literal allowed at limit")
_ = limited.handle(.backspace); _ = limited.handle(.backspace)
type("a8 ", into: limited)
check(limited.presentation.text == "八媽", "deleting a syllable restores capacity")

// Candidate ordering, selection keys, paging, clamping and normal processing after dismissal.
var twoKeys = ComposerOptions(); twoKeys.selectionKeys = Array("as")
let candidates = composer(engine, "18 q8 ", options: twoKeys)
_ = candidates.handle(.space)
check(candidates.presentation.candidates?.items.prefix(2).map(\.text) == ["八趴", "巴趴"], "current first, longer spans before singles")
check(candidates.presentation.candidates?.items.last?.text == "趴", "short span after long spans")
check(candidates.presentation.focusedClause == 0, "focused converted word")
validPresentation(candidates, "candidate window")
_ = candidates.handle(.char("s"))
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
_ = paging.handle(.left)
check(paging.presentation.candidates?.page == 0, "left previous page")
for key: KeyInput in [.right, .pageDown, .tab] {
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
let pagePick = composer(engine, "18 ", options: threeKeys)
_ = pagePick.handle(.down); _ = pagePick.handle(.right)
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
check(interior.presentation.candidates!.items.first?.text == "八趴", "shown text first when cursor is inside a longer word")
let sixEngine = try Engine(lexiconText: tiny + "\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t八八八八八八\t-0.1\nㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ ㄅㄚ\t巴巴巴巴巴巴巴\t0\n")
let six = composer(sixEngine, String(repeating: "18 ", count: 7))
check(six.presentation.clauses.count == 2 && !six.presentation.text.contains("巴"), "word spans capped at six syllables")
_ = six.handle(.down)
_ = six.selectCandidate(at: six.presentation.candidates!.items.firstIndex { $0.text == "八" }!)
_ = six.handle(.down)
check(six.presentation.candidates!.items.first?.text == "八", "current single word precedes longer candidates")
check(six.presentation.candidates!.items[1].text == "八八八八八八", "six-syllable candidate ordered before shorter spans")

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

// Learning affects later conversions, picks count +1, commits count +0.5, reset does not learn.
let learnedEngine = try Engine(lexiconText: tiny)
let learnedComposer = composer(learnedEngine, "18 ")
_ = learnedComposer.handle(.down)
_ = learnedComposer.selectCandidate(at: learnedComposer.presentation.candidates!.items.firstIndex { $0.text == "巴" }!)
check(learnedEngine.learnedEntries().first?.count == 1, "pick learns +1")
check(learnedComposer.commitAll() == "巴", "picked word committed")
check(learnedEngine.learnedEntries().first?.count == 1.5, "fixed commit learns +0.5")
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
    if loaded.learnedEntries().first?.count == 1.5 && loaded.userPhrases().count == 1 { roundTrip = loaded; break }
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

// Real lexicon regression checks and release-mode latency measurement.
let realURL = URL(fileURLWithPath: "data/build/lexicon.tsv")
if FileManager.default.fileExists(atPath: realURL.path) {
    let (real, initMS) = try timed { try Engine(lexiconURL: realURL, userDataDirectory: nil) }
    print(String(format: "TIMING real lexicon init: %.3f ms (target <400 ms)", initMS))
    check(initMS < 400, "real lexicon init under 400 ms")
    let reverseKeys = Dictionary(uniqueKeysWithValues: keySymbols.map { ($0.1, String($0.0)) })
    let toneKeys: [Character: String] = ["ˊ": "6", "ˇ": "3", "ˋ": "4", "˙": "7"]
    func keysForReading(_ reading: String) -> String {
        reading.split(separator: " ").map { syllable in
            let keys = syllable.map { reverseKeys[$0] ?? toneKeys[$0] ?? "" }.joined()
            return toneKeys[syllable.last!] == nil ? keys + " " : keys
        }.joined()
    }
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
        let actual = composer(real, keys).presentation.text
        print("REAL \(word) → \(actual) [\(keys)]")
        check(actual == word, "real conversion \(word) [\(keys)]")
    }
    let realEnd = composer(real, "cp3cl3")
    _ = realEnd.handle(.down)
    let hasVeryGood = try String(contentsOf: realURL, encoding: .utf8).split(separator: "\n")
        .contains { $0.hasPrefix("ㄏㄣˇ ㄏㄠˇ\t很好\t") }
    if hasVeryGood {
        check(realEnd.presentation.candidates?.items.prefix(3).map(\.text) == ["很好", "好", "郝"], "real end-cursor phrase candidates")
    } else {
        print("WARNING 很好 has no word entry in the real lexicon; conversion uses 很 + 好 (fixture checks phrase-first candidates)")
        check(realEnd.presentation.candidates?.items.prefix(2).map(\.text) == ["好", "郝"], "real end-cursor singles when phrase is absent")
    }
    let realPhraseEnd = composer(real, "vu,4vu,4")
    _ = realPhraseEnd.handle(.down)
    check(realPhraseEnd.presentation.candidates?.items.first?.text == "謝謝", "real tolerant multi-syllable candidate at end cursor")
    for (label, keys, nextKeys) in [
        ("first-tone", String(repeating: "wu0 ", count: 40), "wu0 "),
        ("neutral-tolerant", String(repeating: "vu,4vu,4", count: 20), "vu,4"),
    ] {
        let long = composer(real, keys)
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
        print(String(format: "TIMING %@ handle on 40 syllables: median %.3f ms, p95 %.3f ms, max %.3f ms (target <0.3 ms)", label, ordered[ordered.count / 2], p95, ordered.last!))
        check(ordered.last! < 0.3, "40-syllable \(label) handle under 0.3 ms")
    }
} else { print("WARNING real lexicon absent; skipped real-data checks") }
print("\(checks - failures)/\(checks) checks passed; \(failures) failures")
exit(failures == 0 ? 0 : 1)
