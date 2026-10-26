import AppKit
import ZhuyinCore

enum KeyMapping {
    // US ANSI hardware positions; event.characters depends on the client's layout.
    static let printable: [UInt16: Character] = [
        0: "a", 1: "s", 2: "d", 3: "f", 4: "h", 5: "g", 6: "z", 7: "x", 8: "c", 9: "v",
        11: "b", 12: "q", 13: "w", 14: "e", 15: "r", 16: "y", 17: "t",
        18: "1", 19: "2", 20: "3", 21: "4", 22: "6", 23: "5", 24: "=", 25: "9", 26: "7",
        27: "-", 28: "8", 29: "0", 30: "]", 31: "o", 32: "u", 33: "[", 34: "i", 35: "p",
        37: "l", 38: "j", 39: "'", 40: "k", 41: ";", 42: "\\", 43: ",", 44: "/", 45: "n",
        46: "m", 47: ".", 50: "`",
    ]

    /// The upper character printed on each key of the US layout.
    static let shifted: [Character: Character] = Dictionary(uniqueKeysWithValues:
        zip(Array("1234567890-=[]\\;',./`"), Array("!@#$%^&*()_+{}|:\"<>?~")))

    static func input(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> KeyInput? {
        if !modifiers.intersection([.command, .control]).isEmpty { return nil }
        if modifiers.contains(.option) {
            if keyCode == 51 { return .optionBackspace }
            // Digit and symbol keys type what is printed on them; Option-letter stays the app's (shortcuts).
            guard let character = printable[keyCode], !character.isLetter else { return nil }
            return .printed(modifiers.contains(.shift) ? shifted[character] ?? character : character)
        }
        if let character = printable[keyCode] {
            if modifiers.contains(.capsLock), character.isLetter { return nil }
            return modifiers.contains(.shift) ? .shiftedChar(character) : .char(character)
        }
        switch keyCode {
        case 36, 76: return modifiers.contains(.shift) ? .shiftEnter : .enter
        case 49: return modifiers.contains(.shift) ? .shiftSpace : .space
        case 51: return .backspace
        case 117: return .forwardDelete
        case 53: return .escape
        case 48: return modifiers.contains(.shift) ? .shiftTab : .tab
        case 123: return modifiers.contains(.shift) ? .shiftLeft : .left
        case 124: return modifiers.contains(.shift) ? .shiftRight : .right
        case 125: return modifiers.contains(.shift) ? .shiftDown : .down
        case 126: return .up
        case 115: return .home
        case 119: return .end
        case 116: return .pageUp
        case 121: return .pageDown
        default: return nil // Includes keypad digits, so the client receives their original events.
        }
    }
}
