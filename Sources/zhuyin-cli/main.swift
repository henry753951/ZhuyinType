import Foundation
import ZhuyinCore

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
var arguments = Array(CommandLine.arguments.dropFirst())
var lexiconPath = "data/build/lexicon.tsv"
if arguments.first == "--lexicon" {
    guard arguments.count >= 3 else { fail("Usage: zhuyin-cli [--lexicon PATH] \"<keys>\"") }
    lexiconPath = arguments[1]
    arguments.removeFirst(2)
}
guard arguments.count == 1 else { fail("Usage: zhuyin-cli [--lexicon PATH] \"<keys>\"") }
let specials: [String: KeyInput] = [
    "space": .space, "bs": .backspace, "obs": .optionBackspace, "del": .forwardDelete,
    "left": .left, "right": .right, "home": .home, "end": .end, "up": .up, "down": .down,
    "enter": .enter, "senter": .shiftEnter, "esc": .escape, "tab": .tab,
    "stab": .shiftTab, "sspace": .shiftSpace, "pgup": .pageUp, "pgdown": .pageDown
]
func parse(_ input: String) throws -> [(String, KeyInput)] {
    var result: [(String, KeyInput)] = []
    var index = input.startIndex
    while index < input.endIndex {
        let character = input[index]
        if character == "<", let end = input[index...].firstIndex(of: ">") {
            let name = String(input[input.index(after: index)..<end])
            if let key = specials[name] { result.append(("<\(name)>", key)) }
            else if name.hasPrefix("S-"), name.dropFirst(2).count == 1, let key = name.last {
                result.append(("<\(name)>", .shiftedChar(key)))
            } else {
                throw NSError(domain: "zhuyin-cli", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unknown key escape <\(name)>"])
            }
            index = input.index(after: end)
        } else {
            result.append((character == " " ? "<space>" : String(character), character == " " ? .space : .char(character)))
            index = input.index(after: index)
        }
    }
    return result
}
assert((try? parse("a<space><bs><obs><del><left><right><home><end><up><down><enter><senter><esc><tab><S-x>"))?.map(\.1)
       == [.char("a"), .space, .backspace, .optionBackspace, .forwardDelete, .left, .right,
           .home, .end, .up, .down, .enter, .shiftEnter, .escape, .tab, .shiftedChar("x")])
assert((try? parse("<unknown>")) == nil)
do {
    let engine = try Engine(lexiconURL: URL(fileURLWithPath: lexiconPath), userDataDirectory: nil)
    let composer = Composer(engine: engine)
    for (label, key) in try parse(arguments[0]) {
        let result = composer.handle(key)
        let p = composer.presentation
        let string = p.text as NSString
        let before = string.substring(to: p.cursor)
        let after = string.substring(from: p.cursor)
        print("\(label) → \(before)|\(after)\(result.handled ? "" : " [pass]")")
        if let list = p.candidates {
            let items = list.pageRange.map { index in
                "\(index == list.selection ? ">" : "")\(index - list.page * list.pageSize + 1):\(list.items[index].text)"
            }.joined(separator: "  ")
            print("  candidates \(list.page + 1)/\(list.pageCount): \(items)")
        }
        if let text = result.commit { print("  committed: \(text)") }
    }
} catch { fail(error.localizedDescription) }
