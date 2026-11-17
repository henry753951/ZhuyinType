import AppKit
import ZhuyinEngine

/// What VoiceOver says about the candidate window and the input mode.  The window never takes focus (it is a
/// non-activating panel), so VoiceOver does not follow it on its own: the window asks for an announcement when a list
/// opens, its page turns or the highlight moves.  Nothing here runs unless VoiceOver is on.
enum CandidateAnnouncer {
    /// The part of a list an announcement depends on, to say only what changed.
    struct Shown: Equatable {
        var count: Int, first: String, pageSize: Int, tolerant: Bool?, singleCharacter: Bool
        var page: Int, selection: Int

        init(_ list: CandidateList) {
            count = list.items.count
            first = list.items.first?.text ?? ""
            pageSize = list.pageSize
            tolerant = list.tolerant
            singleCharacter = list.singleCharacter
            page = list.page
            selection = list.selection
        }

        func sameList(as other: Shown) -> Bool {
            count == other.count && first == other.first && pageSize == other.pageSize && tolerant == other.tolerant
                && singleCharacter == other.singleCharacter
        }
    }

    /// One candidate as a row reads it: its key, its text and the corrected reading after it ("3，在").
    static func item(_ candidate: Candidate, key: String) -> String {
        [key, candidate.text, candidate.annotation].compactMap { $0?.isEmpty == false ? $0 : nil }.joined(separator: "，")
    }

    /// What to say for `list` after `previous` (nil: no list was open), or nil when nothing changed.  `keys` are the
    /// selection keys of a page; `reading` is the reading of the part being changed.
    ///
    /// - A new list: 「ㄗㄞˋ，候選字清單，共 24 個，3 頁，候選字 1，在」 (修正的字 or 單字 for those lists);
    /// - another page: 「第 2 頁，共 3 頁，候選字 1，在」;
    /// - the highlight moved: 「候選字 3，在」.
    static func announcement(for list: CandidateList, keys: [Character], reading: String?, previous: Shown?) -> String? {
        guard list.items.indices.contains(list.selection) else { return nil }
        let now = Shown(list)
        if now == previous { return nil }
        let offset = list.selection - list.pageRange.lowerBound
        let key = offset < keys.count ? String(keys[offset]) : String(offset + 1)
        let current = "候選字 " + item(list.items[list.selection], key: key)
        guard let previous, previous.sameList(as: now) else {
            let name = list.tolerant == true ? "修正的字" : list.singleCharacter ? "單字" : "候選字清單"
            var parts = [String]()
            if let reading, !reading.isEmpty { parts.append(reading) }
            parts.append(name)
            parts.append("共 \(list.items.count) 個")
            if list.pageCount > 1 { parts.append("\(list.pageCount) 頁") }
            parts.append(current)
            return parts.joined(separator: "，")
        }
        if previous.page != now.page { return "第 \(list.page + 1) 頁，共 \(list.pageCount) 頁，" + current }
        return current
    }

    /// 中/英 (and 全形/半形 punctuation) after the mode changed.
    static func mode(english: Bool) -> String { english ? "英文" : "中文" }
    static func punctuationWidth(half: Bool) -> String { half ? "半形標點" : "全形標點" }
}

/// Posts announcements to VoiceOver, only while it is on.  Whether it is on is kept from a key-value observation, so
/// with VoiceOver off the candidate window does one Bool check per update and nothing else.
enum ScreenReader {
    private(set) static var isRunning = false
    private static var observation: NSKeyValueObservation?

    /// Starts following VoiceOver; call once at launch (the input method), never from the headless modes.
    static func start() {
        guard observation == nil else { return }
        observation = NSWorkspace.shared.observe(\.isVoiceOverEnabled, options: [.initial, .new]) { workspace, _ in
            let running = workspace.isVoiceOverEnabled
            DispatchQueue.main.async { isRunning = running }
        }
    }

    /// Asks VoiceOver to say `text` at once, interrupting what it was saying about the previous candidate.
    static func announce(_ text: String, from element: Any? = nil) {
        guard isRunning, !text.isEmpty else { return }
        NSAccessibility.post(element: element ?? NSApp as Any, notification: .announcementRequested,
                             userInfo: [.announcement: text, .priority: NSAccessibilityPriorityLevel.high.rawValue])
    }
}
