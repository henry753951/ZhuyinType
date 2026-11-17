import Foundation

// 檢查輸入法 (關於 › 檢查輸入法): what the system and this copy of ZhuyinType look like, as plain facts
// (`HealthFacts`, read by `HealthFactsReader`), and a pure function from the facts to the checks the page lists and the
// problem report writes.  No side effects here, so every status can be tested with made-up facts (HealthSelfTest).

/// How a check came out, in increasing order of concern.
enum HealthStatus: Int, Comparable, CaseIterable {
    case ok, info, warning, error

    static func < (lhs: HealthStatus, rhs: HealthStatus) -> Bool { lhs.rawValue < rhs.rawValue }

    /// The word the page's icon stands for and the report writes in brackets.
    var label: String {
        switch self {
        case .ok: return "正常"
        case .info: return "說明"
        case .warning: return "注意"
        case .error: return "錯誤"
        }
    }

    /// Counted in 「有 N 個項目需要處理」 and marked on the 關於 sidebar item.
    var needsAttention: Bool { self >= .warning }
}

/// What a check's button does.  Every one is safe to press: nothing is deleted, restarted or switched.
enum HealthAction: Hashable {
    /// Adds ZhuyinType to the input sources, as a first install does (`InstallCommand.enableInputSource`).
    case enableInputSource
    /// System Settings › 鍵盤.
    case openKeyboardSettings
    /// Shows a file in Finder.
    case reveal(URL)
    /// Sparkle's 「檢查更新…」 window.
    case checkForUpdates

    var title: String {
        switch self {
        case .enableInputSource: return "加入輸入方式"
        case .openKeyboardSettings: return "打開鍵盤設定"
        case .reveal: return "在 Finder 中顯示"
        case .checkForUpdates: return "立即檢查…"
        }
    }
}

struct HealthCheck: Identifiable, Equatable {
    enum Kind: String, CaseIterable {
        case inputSource, installedCopy, secureInput, perDocumentSource, dataFiles, settings, version
    }

    let kind: Kind
    let status: HealthStatus
    let title: String
    /// One sentence for the page.
    let explanation: String
    /// The sentence as the problem report writes it, when it differs: without other apps' names.
    var reportExplanation: String?
    var actions: [HealthAction] = []

    var id: Kind { kind }

    /// The report's line: 「[注意] 安全輸入已開啟：……」.
    var reportLine: String { "[\(status.label)] \(title)：\(reportExplanation ?? explanation)" }
}

/// What the checks are made from.  Read in one place (`HealthFactsReader`); here only values.
struct HealthFacts: Equatable {
    enum InputSource: Equatable {
        /// The system lists no input source with our bundle ID.
        case notRegistered
        case registered(enabled: Bool, selectable: Bool)
    }

    enum SecureInput: Equatable {
        case off
        /// `owner`: the localized name of the app that turned it on, when the system says.
        case on(owner: String?)
    }

    enum DataFile: Equatable {
        case missing
        case unreadable(URL)
        case readable(URL)
    }

    /// The parts beside the lexicon that make conversion better; typing works without them.
    enum Model: String, CaseIterable {
        case languageModel, chatModel, readingChannel, reranker, characterModel

        var name: String {
            switch self {
            case .languageModel: return "語言模型"
            case .chatModel: return "口語語言模型"
            case .readingChannel: return "其他讀法表"
            case .reranker: return "選字模型"
            case .characterModel: return "單字模型"
            }
        }
    }

    enum SettingsFile: Equatable {
        /// Nothing saved yet: every setting is its default.
        case none
        case readable
        case unreadable(URL)
    }

    enum Updates: Equatable {
        /// A build without an update feed (development).
        case unavailable
        case available(automaticChecks: Bool, lastCheck: Date?, pending: String?)
    }

    /// The home directory, written as 「~」 in every path shown.
    var home: String
    var inputSource: InputSource
    /// This running copy.
    var running: URL
    /// Where installers put the app: ~/Library/Input Methods and /Library/Input Methods.
    var installLocations: [URL]
    /// Every copy with our bundle ID the system knows of (the running one too, maybe).
    var copies: [URL]
    var secureInput: SecureInput
    /// 「自動切換成文件的輸入方式」 is on.
    var perDocumentInputSource: Bool
    var lexicon: DataFile
    var missingModels: [Model]
    var settings: SettingsFile
    var version: String?
    var build: String?
    var updates: Updates
    var systemVersion: String
    var systemBuild: String
    var architecture: String
    var now: Date
}

enum HealthChecks {
    /// The checks, in the page's order.
    static func evaluate(_ facts: HealthFacts) -> [HealthCheck] {
        [inputSource(facts), installedCopy(facts), secureInput(facts), perDocumentSource(facts), dataFiles(facts),
         settings(facts), version(facts)]
    }

    /// The 關於 page's row: 「全部正常」 or 「有 2 個項目需要處理」.
    static func summary(_ checks: [HealthCheck]) -> String {
        let count = checks.filter { $0.status.needsAttention }.count
        return count == 0 ? "全部正常" : "有 \(count) 個項目需要處理"
    }

    /// Since this long without a check, 「最近沒有檢查更新」 (checks run daily).
    static let staleUpdateCheck: TimeInterval = 7 * 24 * 3600

    // MARK: The checks

    private static func inputSource(_ facts: HealthFacts) -> HealthCheck {
        switch facts.inputSource {
        case .notRegistered:
            return HealthCheck(kind: .inputSource, status: .error, title: "系統找不到 ZhuyinType",
                               explanation: "請重新安裝 ZhuyinType；如果仍然找不到，登出後再登入一次。",
                               actions: [.openKeyboardSettings])
        case .registered(enabled: false, _):
            return HealthCheck(kind: .inputSource, status: .warning, title: "尚未加入輸入方式",
                               explanation: "加入「ZhuyinType 注音」後，才能從選單列或切換鍵選用。",
                               actions: [.enableInputSource, .openKeyboardSettings])
        case .registered(enabled: true, selectable: false):
            return HealthCheck(kind: .inputSource, status: .warning, title: "輸入方式無法選用",
                               explanation: "「ZhuyinType 注音」已加入，但系統目前不讓它被選用；登出後再登入通常可以恢復。",
                               actions: [.openKeyboardSettings])
        case .registered(enabled: true, selectable: true):
            return HealthCheck(kind: .inputSource, status: .ok, title: "輸入方式已加入",
                               explanation: "可以從選單列或切換鍵選用「ZhuyinType 注音」。")
        }
    }

    private static func installedCopy(_ facts: HealthFacts) -> HealthCheck {
        let installed = Set(facts.installLocations.map(\.standardizedFileURL.path))
        let running = facts.running.standardizedFileURL
        let others = facts.copies.map(\.standardizedFileURL)
            .filter { $0.path != running.path && !$0.path.contains("/.Trash/") }
            .reduce(into: [URL]()) { list, url in if !list.contains(url) { list.append(url) } }
        let otherInstalled = others.filter { installed.contains($0.path) }
        func shown(_ url: URL) -> String { abbreviate(url.path, home: facts.home) }
        if !installed.contains(running.path) {
            return HealthCheck(kind: .installedCopy, status: .warning, title: "執行中的不是安裝的版本",
                               explanation: "目前執行的是 \(shown(running))，不是「輸入法」資料夾中的那一份。",
                               actions: [.reveal(running)])
        }
        if let other = otherInstalled.first {
            return HealthCheck(kind: .installedCopy, status: .warning, title: "有兩份安裝的 ZhuyinType",
                               explanation: "另一份在 \(shown(other))，系統可能載入較舊的那一份；不用的那一份可以移到垃圾桶。",
                               actions: [.reveal(other)])
        }
        if let other = others.first {
            let copies = others.count == 1 ? "另有一份在 \(shown(other))" : "另有 \(others.count) 份，其中一份在 \(shown(other))"
            return HealthCheck(kind: .installedCopy, status: .info, title: "這台 Mac 上另有 ZhuyinType",
                               explanation: "\(copies)；輸入法只使用「輸入法」資料夾中的那一份。",
                               actions: [.reveal(other)])
        }
        return HealthCheck(kind: .installedCopy, status: .ok, title: "使用安裝的版本",
                           explanation: "正在執行 \(shown(running))。")
    }

    private static func secureInput(_ facts: HealthFacts) -> HealthCheck {
        guard case .on(let owner) = facts.secureInput else {
            return HealthCheck(kind: .secureInput, status: .ok, title: "安全輸入未開啟",
                               explanation: "打字時參考游標前的文字，並記住選過的字詞。")
        }
        // ControllerCore: under secure input conversion goes on, but nothing before the caret is read and nothing is
        // learned, recorded or counted.  Password fields themselves take no input method (the system's doing).
        let effect = "照常打字，但不參考游標前的文字，也不記住選過的字詞，直到那個 App 關閉安全輸入。"
        let anonymous = "有 App 開啟了安全輸入（例如密碼欄位）：\(effect)"
        return HealthCheck(kind: .secureInput, status: .warning, title: "安全輸入已開啟",
                           explanation: owner.map { "「\($0)」開啟了安全輸入：\(effect)" } ?? anonymous,
                           reportExplanation: anonymous)
    }

    private static func perDocumentSource(_ facts: HealthFacts) -> HealthCheck {
        guard facts.perDocumentInputSource else {
            return HealthCheck(kind: .perDocumentSource, status: .ok, title: "所有 App 使用同一個輸入方式",
                               explanation: "切換到 ZhuyinType 後，在每個 App 都是 ZhuyinType。")
        }
        return HealthCheck(kind: .perDocumentSource, status: .info, title: "每個視窗各自記住輸入方式",
                           explanation: "系統設為自動切換成文件的輸入方式，所以回到某些視窗時會換回其他輸入方式。",
                           actions: [.openKeyboardSettings])
    }

    private static func dataFiles(_ facts: HealthFacts) -> HealthCheck {
        switch facts.lexicon {
        case .missing:
            return HealthCheck(kind: .dataFiles, status: .error, title: "找不到詞庫",
                               explanation: "沒有詞庫就無法把注音轉成中文，請重新安裝 ZhuyinType。")
        case .unreadable(let url):
            return HealthCheck(kind: .dataFiles, status: .error, title: "無法讀取詞庫",
                               explanation: "\(abbreviate(url.path, home: facts.home)) 無法開啟，請重新安裝 ZhuyinType。",
                               actions: [.reveal(url)])
        case .readable:
            break
        }
        guard facts.missingModels.isEmpty else {
            let names = facts.missingModels.map(\.name).joined(separator: "、")
            return HealthCheck(kind: .dataFiles, status: .warning, title: "缺少部分模型",
                               explanation: "缺少\(names)；仍然可以打字，但選字的準確度較低。")
        }
        return HealthCheck(kind: .dataFiles, status: .ok, title: "詞庫與模型完整",
                           explanation: "詞庫、語言模型與選字模型都可以讀取。")
    }

    private static func settings(_ facts: HealthFacts) -> HealthCheck {
        switch facts.settings {
        case .none:
            return HealthCheck(kind: .settings, status: .ok, title: "使用預設設定", explanation: "還沒有改過任何設定。")
        case .readable:
            return HealthCheck(kind: .settings, status: .ok, title: "設定已讀取", explanation: "使用已儲存的設定。")
        case .unreadable(let url):
            return HealthCheck(kind: .settings, status: .warning, title: "無法讀取設定檔",
                               explanation: "\(abbreviate(url.path, home: facts.home)) 無法讀取，目前使用預設設定。",
                               actions: [.reveal(url)])
        }
    }

    private static func version(_ facts: HealthFacts) -> HealthCheck {
        let title = "版本 \(versionText(facts))"
        switch facts.updates {
        case .unavailable:
            return HealthCheck(kind: .version, status: .info, title: title, explanation: "這個版本不會自動更新。")
        case .available(_, _, let pending?):
            return HealthCheck(kind: .version, status: .info, title: title, explanation: "已下載 \(pending)，組字結束後安裝。")
        case .available(automaticChecks: false, _, _):
            return HealthCheck(kind: .version, status: .info, title: title,
                               explanation: "已關閉自動檢查更新，可以在「關於」立即檢查。", actions: [.checkForUpdates])
        case .available(automaticChecks: true, let last, nil):
            guard let last, facts.now.timeIntervalSince(last) < staleUpdateCheck else {
                let when = last.map { "上次檢查是 \(timestamp($0))" } ?? "還沒有檢查過更新"
                return HealthCheck(kind: .version, status: .info, title: title,
                                   explanation: "\(when)；請確認網路連線，或立即檢查。", actions: [.checkForUpdates])
            }
            return HealthCheck(kind: .version, status: .ok, title: title, explanation: "上次檢查更新：\(timestamp(last))。")
        }
    }

    // MARK: Text

    /// 「0.6.1（建置 123）」, or 「開發版」 without a version.
    static func versionText(_ facts: HealthFacts) -> String {
        guard let version = facts.version else { return "開發版" }
        return facts.build.map { "\(version)（建置 \($0)）" } ?? version
    }

    /// `path` with the home directory written as 「~」.
    static func abbreviate(_ path: String, home: String) -> String {
        guard !home.isEmpty, home != "/" else { return path }
        if path == home { return "~" }
        if path.hasPrefix(home + "/") { return "~" + path.dropFirst(home.count) }
        return path
    }

    /// 「2026-11-18 14:03」, in the Mac's time zone.
    static func timestamp(_ date: Date) -> String { formatter("yyyy-MM-dd HH:mm").string(from: date) }

    static func formatter(_ format: String) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.dateFormat = format
        return formatter
    }
}

/// 回報問題: the text the sheet shows, copies and saves.  The same layout as the Linux and Windows versions.
/// Never typed text, the user's words, correction logs, context or other apps' identifiers or names; paths with 「~」.
enum HealthReport {
    /// Our own recent errors (error and fault level, this process), or nil when the log could not be read in time.
    struct LogLine: Equatable {
        var date: Date
        var message: String
    }

    static func text(facts: HealthFacts, checks: [HealthCheck], errors: [LogLine]?) -> String {
        var lines = [
            "ZhuyinType 問題回報",
            "產生時間：\(HealthChecks.timestamp(facts.now))",
            "版本：\(HealthChecks.versionText(facts))",
            "系統：macOS \(facts.systemVersion)（\(facts.systemBuild)）",
            "架構：\(facts.architecture)",
            "",
            "檢查結果",
        ]
        lines += checks.map(\.reportLine)
        lines += ["", "最近的錯誤"]
        if let errors {
            let time = HealthChecks.formatter("HH:mm:ss")
            lines += errors.isEmpty ? ["（沒有）"] : errors.map { "\(time.string(from: $0.date)) \(scrub($0.message, home: facts.home))" }
        } else {
            lines.append("（無法讀取）")
        }
        return lines.joined(separator: "\n") + "\n"
    }

    /// A log message on one line, with the home directory written as 「~」.
    static func scrub(_ message: String, home: String) -> String {
        var text = message.replacingOccurrences(of: "\n", with: " ")
        if !home.isEmpty, home != "/" { text = text.replacingOccurrences(of: home + "/", with: "~/") }
        return text
    }
}
