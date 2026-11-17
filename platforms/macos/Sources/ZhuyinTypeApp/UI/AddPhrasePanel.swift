// 「選取的字加入自訂詞」: a small window with the text selected in the app and the engine's best reading, both
// editable, that adds a custom word.  Typing in it needs the keyboard, so the input method's app becomes active while
// it is open; closing it brings back the app the user was in.
import AppKit
import ZhuyinEngine

/// Why a custom word cannot be added, as the form says it; nil when the engine may try.
enum AddPhraseCheck {
    static let maxCharacters = 6

    static func problem(word: String, reading: String) -> String? {
        if word.isEmpty { return "沒有選取文字" }
        let characters = word.count
        if characters > maxCharacters || !word.unicodeScalars.allSatisfy(isHan) { return "自訂詞需為 1 到 \(maxCharacters) 個中文字" }
        let syllables = reading.split(whereSeparator: \.isWhitespace).count
        if syllables == 0 { return "請輸入讀音" }
        if syllables != characters { return "讀音的音節數要和字數相同（\(characters) 個字，\(syllables) 個音節）" }
        return nil
    }

    static func isHan(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar.value {
        case 0x3400...0x4DBF, 0x4E00...0x9FFF, 0xF900...0xFAFF, 0x20000...0x3134F, 0x3007: return true
        default: return false
        }
    }
}

final class AddPhrasePanel: NSObject, NSWindowDelegate, NSComboBoxDelegate {
    static let shared = AddPhrasePanel()

    private lazy var window: NSPanel = makeWindow()
    private let wordField = NSTextField()
    private let readingField = NSComboBox()
    private let message = NSTextField(labelWithString: "")
    private let addButton = NSButton(title: "加入", target: nil, action: nil)
    /// The app that was in front when the form opened, given the focus back on close.
    private var previousApp: NSRunningApplication?

    /// Opens the form with `text` (nil or empty: nothing was selected) and its most likely reading.
    func show(text: String?) {
        let word = text ?? ""
        let front = NSWorkspace.shared.frontmostApplication
        if front?.processIdentifier != ProcessInfo.processInfo.processIdentifier { previousApp = front }
        _ = window
        wordField.stringValue = word
        readingField.stringValue = ""
        updateReadings()
        message.stringValue = word.isEmpty ? "沒有選取文字：請先在 App 裡選取 1 到 6 個字，或在這裡輸入" : ""
        message.textColor = .secondaryLabelColor
        if !word.isEmpty, let problem = AddPhraseCheck.problem(word: word, reading: readingField.stringValue) { show(problem: problem) }
        window.center()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(word.isEmpty ? wordField : readingField)
    }

    private func makeWindow() -> NSPanel {
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 360, height: 150),
                            styleMask: [.titled, .closable], backing: .buffered, defer: true)
        panel.title = "加入自訂詞"
        panel.isReleasedWhenClosed = false
        panel.level = .floating
        panel.delegate = self
        wordField.placeholderString = "例如：王小明"
        wordField.delegate = self
        readingField.placeholderString = "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ"
        readingField.completes = false
        readingField.numberOfVisibleItems = 6
        readingField.delegate = self
        message.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        message.lineBreakMode = .byWordWrapping
        message.maximumNumberOfLines = 2
        addButton.target = self
        addButton.action = #selector(add)
        addButton.keyEquivalent = "\r"
        let cancel = NSButton(title: "取消", target: self, action: #selector(cancel))
        cancel.keyEquivalent = "\u{1b}"
        let grid = NSGridView(views: [
            [NSTextField(labelWithString: "詞："), wordField],
            [NSTextField(labelWithString: "讀音："), readingField],
            [NSGridCell.emptyContentView, message],
        ])
        grid.rowSpacing = 8
        grid.column(at: 0).xPlacement = .trailing
        grid.column(at: 1).width = 250
        let buttons = NSStackView(views: [cancel, addButton])
        buttons.spacing = 8
        let stack = NSStackView(views: [grid, buttons])
        stack.orientation = .vertical
        stack.alignment = .trailing
        stack.spacing = 12
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        panel.contentView = stack
        return panel
    }

    @objc private func add() {
        let word = wordField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let reading = readingField.stringValue.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        if let problem = AddPhraseCheck.problem(word: word, reading: reading) { return show(problem: problem) }
        guard let engine = EngineHost.engine else { return show(problem: "輸入法還在載入，請稍後再試") }
        if engine.userPhrases().contains(where: { $0.word == word && $0.reading == reading }) {
            return show(problem: "「\(word)」已是自訂詞")
        }
        do {
            try engine.addUserPhrase(word: word, reading: reading)
            close()
        } catch {
            show(problem: "讀音需為每個字一個注音，以空白分隔")
        }
    }

    @objc private func cancel() { close() }

    private func show(problem: String) {
        message.stringValue = problem
        message.textColor = .systemRed
        NSAccessibility.post(element: message, notification: .announcementRequested,
                             userInfo: [.announcement: problem, .priority: NSAccessibilityPriorityLevel.high.rawValue])
    }

    private func close() { window.close() }

    func windowWillClose(_ notification: Notification) {
        previousApp?.activate()
        previousApp = nil
    }

    func controlTextDidChange(_ notification: Notification) {
        if (notification.object as? NSTextField) === wordField { updateReadings() }
        message.stringValue = ""
    }

    private func updateReadings() {
        let word = wordField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let readings = word.isEmpty ? [] : (EngineHost.engine?.readings(of: word) ?? [])
        readingField.removeAllItems()
        readingField.addItems(withObjectValues: readings)
        if let first = readings.first, !readings.contains(readingField.stringValue) { readingField.stringValue = first }
    }
}
