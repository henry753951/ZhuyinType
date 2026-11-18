import Foundation

// 檢查輸入法 (the settings page of that name): what the system and this copy of ZhuyinType look like, as plain facts
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

/// One fact behind a check or in a report section: what was looked at and what it was, 「已加入：是」.
struct HealthDetail: Equatable, Hashable {
    var label: String
    var value: String

    init(_ label: String, _ value: String) {
        self.label = label
        self.value = value
    }

    /// 是 / 否.
    static func yesNo(_ label: String, _ value: Bool) -> HealthDetail { HealthDetail(label, value ? "是" : "否") }
    /// 開啟 / 關閉.
    static func onOff(_ label: String, _ value: Bool) -> HealthDetail { HealthDetail(label, value ? "開啟" : "關閉") }
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
    /// What was checked and what it was, under 「詳細資料」 on the page and indented under the line in the report.
    var details: [HealthDetail] = []

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

    // The report's sections (HealthReport) and the checks' details.  Defaults: not read.

    /// The account's short name, written 「<使用者>」 in the report.
    var user = ""
    /// Our input mode's ID, `org.zhuyintype.inputmethod.ZhuyinType.Zhuyin`.
    var inputModeID = ""
    /// The selected keyboard input source's ID, and the IDs of the enabled ones in the system's order.
    var currentInputSource: String?
    var enabledInputSources: [String] = []
    /// The folder holding the lexicon the engine loads, and every data file it would load (or 「缺少」).
    var dataFolder: URL?
    var dataItems: [DataItem] = []
    /// The data versions this build bundles (resources/assets.json at build time, in manifest.json).
    var bundledData: [BundledData] = []
    /// How long the last engine load took (EngineHost.loadDurations); nil before the first load.
    var loadTimes: LoadTimes?
    var app = AppBundle()
    /// The settings file and the user data folder (記住的字詞, 自訂詞).
    var preferencesFile: URL?
    var userData: URL?
    /// When this process started.
    var started: Date?
    /// Stored settings that differ from their defaults (`HealthSettings.changes`).
    var changedSettings: [HealthSettings.Change] = []
    var display = Display()

    struct DataItem: Equatable {
        /// 「詞庫」, 「語言模型」…
        var name: String
        /// The file or folder; nil: missing.
        var url: URL?
        var bytes: Int64?
    }

    struct BundledData: Equatable {
        /// assets.json's component: `lexicon`, `lm`…
        var component: String
        /// `data-7`.
        var release: String
        /// The archive's SHA-256, first 12 digits.
        var hash: String
    }

    struct LoadTimes: Equatable {
        var engine: TimeInterval?
        var reranker: TimeInterval?
        var characters: TimeInterval?
    }

    /// The running bundle on disk.
    struct AppBundle: Equatable {
        /// The owner's account name.
        var owner: String?
        /// `drwxr-xr-x`.
        var permissions: String?
        /// It carries `com.apple.quarantine` (downloaded and not yet opened through Gatekeeper).
        var quarantined: Bool?
    }

    struct Display: Equatable {
        /// The main screen's backing scale, 2 for Retina.
        var scale: Double?
        /// The system appearance is dark.
        var dark: Bool?
        /// The floating panels' 外觀 setting.
        var panelAppearance: PanelAppearance?
        var voiceOver: Bool?
        var reduceMotion: Bool?
        var increaseContrast: Bool?
        var reduceTransparency: Bool?
    }
}

enum HealthChecks {
    /// The checks, in the page's order.
    static func evaluate(_ facts: HealthFacts) -> [HealthCheck] {
        [inputSource(facts), installedCopy(facts), secureInput(facts), perDocumentSource(facts), dataFiles(facts),
         settings(facts), version(facts)].map { check in
            var check = check
            check.details = details(check.kind, facts)
            return check
        }
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
        let others = otherCopies(facts)
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

    // MARK: Details

    /// What each check looked at, as short label–value lines; paths with 「~」.
    private static func details(_ kind: HealthCheck.Kind, _ facts: HealthFacts) -> [HealthDetail] {
        func shown(_ url: URL) -> String { abbreviate(url.path, home: facts.home) }
        switch kind {
        case .inputSource:
            var lines: [HealthDetail] = []
            if !facts.inputModeID.isEmpty { lines.append(HealthDetail("輸入方式 ID", facts.inputModeID)) }
            switch facts.inputSource {
            case .notRegistered:
                lines.append(.yesNo("已登記", false))
            case .registered(let enabled, let selectable):
                lines += [.yesNo("已登記", true), .yesNo("已加入", enabled), .yesNo("可選用", selectable)]
            }
            return lines
        case .installedCopy:
            let installed = Set(facts.installLocations.map(\.standardizedFileURL.path))
            let others = otherCopies(facts)
            return [HealthDetail("位置", shown(facts.running)),
                    .yesNo("在輸入法資料夾中", installed.contains(facts.running.standardizedFileURL.path)),
                    HealthDetail("其他副本", others.isEmpty ? "沒有" : others.map(shown).joined(separator: "、"))]
        case .secureInput:
            guard case .on(let owner) = facts.secureInput else { return [.onOff("狀態", false)] }
            return [.onOff("狀態", true), HealthDetail("開啟者", owner ?? "不明")]
        case .perDocumentSource:
            return [.onOff("自動切換成文件的輸入方式", facts.perDocumentInputSource)]
        case .dataFiles:
            var lines: [HealthDetail] = []
            if let folder = facts.dataFolder { lines.append(HealthDetail("資料夾", shown(folder))) }
            switch facts.lexicon {
            case .missing: lines.append(HealthDetail("詞庫", "缺少"))
            case .unreadable(let url): lines.append(HealthDetail("詞庫", "\(url.lastPathComponent)，無法讀取"))
            case .readable(let url): lines.append(HealthDetail("詞庫", url.lastPathComponent))
            }
            if !facts.missingModels.isEmpty {
                lines.append(HealthDetail("缺少", facts.missingModels.map(\.name).joined(separator: "、")))
            }
            return lines
        case .settings:
            var lines: [HealthDetail] = []
            switch facts.settings {
            case .none:
                if let file = facts.preferencesFile { lines.append(HealthDetail("檔案", shown(file))) }
                lines.append(HealthDetail("狀態", "尚未建立"))
            case .readable:
                if let file = facts.preferencesFile { lines.append(HealthDetail("檔案", shown(file))) }
                lines.append(HealthDetail("狀態", "可以讀取"))
            case .unreadable(let url):
                lines += [HealthDetail("檔案", shown(url)), HealthDetail("狀態", "無法讀取")]
            }
            lines.append(HealthDetail("改過的設定", "\(facts.changedSettings.count) 項"))
            return lines
        case .version:
            return updateDetails(facts)
        }
    }

    /// The copies other than the running one, each once, without those in the Trash.
    private static func otherCopies(_ facts: HealthFacts) -> [URL] {
        let running = facts.running.standardizedFileURL
        return facts.copies.map(\.standardizedFileURL)
            .filter { $0.path != running.path && !$0.path.contains("/.Trash/") }
            .reduce(into: [URL]()) { list, url in if !list.contains(url) { list.append(url) } }
    }

    /// 自動檢查, 上次檢查 and 已下載的更新: the version check's details and the report's 更新 section.
    static func updateDetails(_ facts: HealthFacts) -> [HealthDetail] {
        guard case .available(let automatic, let last, let pending) = facts.updates else {
            return [HealthDetail("自動檢查", "不提供（沒有更新來源）")]
        }
        return [.onOff("自動檢查", automatic), HealthDetail("上次檢查", last.map(timestamp) ?? "還沒有檢查過"),
                HealthDetail("已下載的更新", pending ?? "沒有")]
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
